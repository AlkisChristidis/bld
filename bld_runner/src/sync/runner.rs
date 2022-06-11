use crate::CheckStopSignal;
use crate::{BuildStep, Container, Machine, Pipeline, RunsOn};
use anyhow::anyhow;
use bld_config::definitions::{
    ENV_TOKEN, GET, PUSH, RUN_PROPS_ID, RUN_PROPS_START_TIME, VAR_TOKEN,
};
use bld_config::BldConfig;
use bld_core::execution::{EmptyExec, Execution};
use bld_core::logger::Logger;
use bld_core::proxies::PipelineFileSystemProxy;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};

type RecursiveFuture = Pin<Box<dyn Future<Output = anyhow::Result<()>>>>;
type AtomicExec = Arc<Mutex<dyn Execution>>;
type AtomicLog = Arc<Mutex<dyn Logger>>;
type AtomicRecv = Arc<Mutex<Receiver<bool>>>;
type AtomicVars = Arc<HashMap<String, String>>;
type AtomicProxy = Arc<dyn PipelineFileSystemProxy>;

pub enum TargetPlatform {
    Machine(Box<Machine>),
    Container(Box<Container>),
}

#[derive(Default)]
pub struct RunnerBuilder {
    run_id: Option<String>,
    run_start_time: Option<String>,
    cfg: Option<Arc<BldConfig>>,
    ex: Option<AtomicExec>,
    lg: Option<AtomicLog>,
    prx: Option<AtomicProxy>,
    pip: Option<String>,
    cm: Option<AtomicRecv>,
    env: Option<AtomicVars>,
    vars: Option<AtomicVars>,
}

impl RunnerBuilder {
    pub fn run_id(mut self, id: &str) -> Self {
        self.run_id = Some(String::from(id));
        self
    }

    pub fn run_start_time(mut self, time: &str) -> Self {
        self.run_start_time = Some(String::from(time));
        self
    }

    pub fn config(mut self, cfg: Arc<BldConfig>) -> Self {
        self.cfg = Some(cfg);
        self
    }

    pub fn execution(mut self, ex: AtomicExec) -> Self {
        self.ex = Some(ex);
        self
    }

    pub fn logger(mut self, lg: AtomicLog) -> Self {
        self.lg = Some(lg);
        self
    }

    pub fn pipeline(mut self, name: &str) -> Self {
        self.pip = Some(name.to_string());
        self
    }

    pub fn proxy(mut self, prx: AtomicProxy) -> Self {
        self.prx = Some(prx);
        self
    }

    pub fn receiver(mut self, cm: Option<AtomicRecv>) -> Self {
        self.cm = cm;
        self
    }

    pub fn environment(mut self, env: AtomicVars) -> Self {
        self.env = Some(env);
        self
    }

    pub fn variables(mut self, vars: AtomicVars) -> Self {
        self.vars = Some(vars);
        self
    }

    pub async fn build(self) -> anyhow::Result<Runner> {
        let id = self.run_id.ok_or_else(|| anyhow!("no run id provided"))?;
        let cfg = self
            .cfg
            .ok_or_else(|| anyhow!("no bld config instance provided"))?;
        let lg = self
            .lg
            .ok_or_else(|| anyhow!("no logger instance provided"))?;
        let prx = self
            .prx
            .ok_or_else(|| anyhow!("no pipeline file system proxy provided"))?;
        let pip_name = self.pip.ok_or_else(|| anyhow!("no pipeline provided"))?;
        let pipeline = Pipeline::parse(&prx.read(&pip_name)?)?;
        let env = self
            .env
            .ok_or_else(|| anyhow!("no environment instance provided"))?;
        let env: Arc<HashMap<String, String>> = Arc::new(
            pipeline
                .environment
                .iter()
                .map(|e| {
                    (
                        e.name.to_string(),
                        env.get(&e.name).unwrap_or(&e.default_value).to_string(),
                    )
                })
                .collect(),
        );
        let vars = self
            .vars
            .ok_or_else(|| anyhow!("no variables instance provided"))?;
        let vars: Arc<HashMap<String, String>> = Arc::new(
            pipeline
                .variables
                .iter()
                .map(|v| {
                    (
                        v.name.to_string(),
                        vars.get(&v.name).unwrap_or(&v.default_value).to_string(),
                    )
                })
                .collect(),
        );
        let platform = match &pipeline.runs_on {
            RunsOn::Machine => {
                let machine = Machine::new(&id, env.clone(), lg.clone())?;
                TargetPlatform::Machine(Box::new(machine))
            }
            RunsOn::Docker(img) => {
                let container = Container::new(img, cfg.clone(), env.clone(), lg.clone()).await?;
                TargetPlatform::Container(Box::new(container))
            }
        };
        Ok(Runner {
            run_id: id,
            run_start_time: self
                .run_start_time
                .ok_or_else(|| anyhow!("no run start time provided"))?,
            cfg,
            ex: self
                .ex
                .ok_or_else(|| anyhow!("no executor instance provided"))?,
            lg,
            prx,
            pip: pipeline,
            cm: self.cm,
            env,
            vars,
            platform,
        })
    }
}

pub struct Runner {
    run_id: String,
    run_start_time: String,
    cfg: Arc<BldConfig>,
    ex: AtomicExec,
    lg: AtomicLog,
    prx: AtomicProxy,
    pip: Pipeline,
    cm: Option<AtomicRecv>,
    env: AtomicVars,
    vars: AtomicVars,
    platform: TargetPlatform,
}

impl Runner {
    fn dumpln(&self, message: &str) {
        let mut lg = self.lg.lock().unwrap();
        lg.dumpln(message);
    }

    fn persist_start(&mut self) {
        let mut exec = self.ex.lock().unwrap();
        let _ = exec.update_running(true);
    }

    fn persist_end(&mut self) {
        let mut exec = self.ex.lock().unwrap();
        let _ = exec.update_running(false);
    }

    fn info(&self) {
        let mut logger = self.lg.lock().unwrap();
        if let Some(name) = &self.pip.name {
            logger.dumpln(&format!("[bld] Pipeline: {name}"));
        }
        logger.dumpln(&format!("[bld] Runs on: {}", self.pip.runs_on));
    }

    fn apply_run_properties(&self, txt: &str) -> String {
        let mut txt_with_props = String::from(txt);
        txt_with_props = txt_with_props.replace(RUN_PROPS_ID, &self.run_id);
        txt_with_props = txt_with_props.replace(RUN_PROPS_START_TIME, &self.run_start_time);
        txt_with_props
    }

    fn apply_environment(&self, txt: &str) -> String {
        let mut txt_with_env = String::from(txt);
        for (key, value) in self.env.iter() {
            let full_name = format!("{ENV_TOKEN}{key}");
            txt_with_env = txt_with_env.replace(&full_name, value);
        }
        for env in self.pip.environment.iter() {
            let full_name = format!("{ENV_TOKEN}{}", &env.name);
            txt_with_env = txt_with_env.replace(&full_name, &env.default_value);
        }
        txt_with_env
    }

    fn apply_variables(&self, txt: &str) -> String {
        let mut txt_with_vars = String::from(txt);
        for (key, value) in self.vars.iter() {
            let full_name = format!("{VAR_TOKEN}{key}");
            txt_with_vars = txt_with_vars.replace(&full_name, value);
        }
        for variable in self.pip.variables.iter() {
            let full_name = format!("{VAR_TOKEN}{}", &variable.name);
            txt_with_vars = txt_with_vars.replace(&full_name, &variable.default_value);
        }
        txt_with_vars
    }

    fn apply_context(&self, txt: &str) -> String {
        let txt = self.apply_run_properties(txt);
        let txt = self.apply_environment(&txt);
        self.apply_variables(&txt)
    }

    async fn artifacts(&self, name: &Option<String>) -> anyhow::Result<()> {
        for artifact in self.pip.artifacts.iter().filter(|a| &a.after == name) {
            let can_continue = (artifact.method == Some(PUSH.to_string())
                || artifact.method == Some(GET.to_string()))
                && artifact.from.is_some()
                && artifact.to.is_some();
            if can_continue {
                let method = self.apply_context(artifact.method.as_ref().unwrap());
                let from = self.apply_context(artifact.from.as_ref().unwrap());
                let to = self.apply_context(artifact.to.as_ref().unwrap());
                {
                    let mut logger = self.lg.lock().unwrap();
                    logger.dumpln(&format!(
                        "[bld] Copying artifacts from: {from} into container to: {to}",
                    ));
                }
                let result = match (&self.platform, &method[..]) {
                    (TargetPlatform::Container(container), PUSH) => {
                        container.copy_into(&from, &to).await
                    }
                    (TargetPlatform::Container(container), GET) => {
                        container.copy_from(&from, &to).await
                    }
                    (TargetPlatform::Machine(machine), PUSH) => machine.copy_into(&from, &to),
                    (TargetPlatform::Machine(machine), GET) => machine.copy_from(&from, &to),
                    _ => unreachable!(),
                };
                if !artifact.ignore_errors {
                    result?;
                }
            }
        }
        Ok(())
    }

    async fn steps(&mut self) -> anyhow::Result<()> {
        for step in self.pip.steps.iter() {
            self.step(step).await?;
            self.artifacts(&step.name).await?;
            self.cm.check_stop_signal()?;
        }
        Ok(())
    }

    async fn step(&self, step: &BuildStep) -> anyhow::Result<()> {
        if let Some(name) = &step.name {
            let mut logger = self.lg.lock().unwrap();
            logger.info(&format!("[bld] Step: {name}"));
        }
        self.call(step).await?;
        self.sh(step).await?;
        Ok(())
    }

    async fn call(&self, step: &BuildStep) -> anyhow::Result<()> {
        for call in &step.call {
            let runner = RunnerBuilder::default()
                .run_id(&self.run_id)
                .run_start_time(&self.run_start_time)
                .config(self.cfg.clone())
                .proxy(self.prx.clone())
                .pipeline(call)
                .execution(EmptyExec::atom())
                .logger(self.lg.clone())
                .receiver(self.cm.as_ref().cloned())
                .environment(self.env.clone())
                .variables(self.vars.clone())
                .build()
                .await?;
            runner.run().await.await?;
            self.cm.check_stop_signal()?;
        }
        Ok(())
    }

    async fn sh(&self, step: &BuildStep) -> anyhow::Result<()> {
        for command in step.commands.iter() {
            let working_dir = step.working_dir.as_ref().map(|wd| self.apply_context(wd));
            let command = self.apply_context(command);
            match &self.platform {
                TargetPlatform::Container(container) => {
                    container.sh(&working_dir, &command, &self.cm).await?
                }
                TargetPlatform::Machine(machine) => machine.sh(&working_dir, &command)?,
            }
            self.cm.check_stop_signal()?;
        }
        Ok(())
    }

    async fn dispose(&self) -> anyhow::Result<()> {
        if self.pip.dispose {
            match &self.platform {
                TargetPlatform::Machine(machine) => machine.dispose()?,
                TargetPlatform::Container(container) => container.dispose().await?,
            }
        }
        Ok(())
    }

    pub async fn run(mut self) -> RecursiveFuture {
        Box::pin(async move {
            self.persist_start();
            self.info();
            match self.artifacts(&None).await {
                Ok(_) => {
                    if let Err(e) = self.steps().await {
                        self.dumpln(&e.to_string());
                    }
                }
                Err(e) => self.dumpln(&e.to_string()),
            }
            self.persist_end();
            self.dispose().await
        })
    }
}