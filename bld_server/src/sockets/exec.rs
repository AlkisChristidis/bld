use crate::extractors::User;
use crate::state::PipelinePool;
use actix::prelude::*;
use actix_web::{error::ErrorUnauthorized, web, Error, HttpRequest, HttpResponse};
use actix_web_actors::ws;
use anyhow::anyhow;
use bld_config::{path, BldConfig};
use bld_core::database::pipeline_runs;
use bld_core::execution::PipelineExecWrapper;
use bld_core::logger::FileLogger;
use bld_core::proxies::{PipelineFileSystemProxy, ServerPipelineProxy};
use bld_core::scanner::{FileScanner, Scanner};
use bld_runner::messages::ExecInfo;
use bld_runner::{Pipeline, Runner, RunnerBuilder};
use bld_utils::fs::IsYaml;
use diesel::r2d2::{ConnectionManager, Pool};
use diesel::sqlite::SqliteConnection;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use tokio::runtime::Runtime;
use tracing::error;
use uuid::Uuid;

type AtomicEx = Arc<Mutex<PipelineExecWrapper>>;
type AtomicFs = Arc<Mutex<FileLogger>>;
type AtomicRecv = Arc<Mutex<Receiver<bool>>>;
type AtomicProxy = Arc<ServerPipelineProxy>;

struct PipelineInfo {
    cfg: Arc<BldConfig>,
    pool: web::Data<PipelinePool>,
    id: String,
    start_time: String,
    name: String,
    ex: AtomicEx,
    lg: AtomicFs,
    prx: AtomicProxy,
    cm: Option<AtomicRecv>,
    env: Arc<HashMap<String, String>>,
    vars: Arc<HashMap<String, String>>,
}

impl PipelineInfo {
    async fn build_runner(&self) -> anyhow::Result<Runner> {
        RunnerBuilder::default()
            .run_id(&self.id)
            .run_start_time(&self.start_time)
            .config(self.cfg.clone())
            .proxy(self.prx.clone())
            .pipeline(&self.name)
            .execution(self.ex.clone())
            .logger(self.lg.clone())
            .receiver(self.cm.clone())
            .environment(self.env.clone())
            .variables(self.vars.clone())
            .build()
            .await
    }

    pub fn spawn(self) {
        thread::spawn(move || {
            let rt = match Runtime::new() {
                Ok(instance) => instance,
                Err(e) => {
                    error!("runtime error occured. {e}");
                    return;
                }
            };
            rt.block_on(async move {
                let runner = match self.build_runner().await {
                    Ok(instance) => instance,
                    Err(e) => {
                        error!("runner build error occured. {e}");
                        return;
                    }
                };
                if let Err(e) = runner.run().await.await {
                    error!("runner returned error: {}", e);
                }
                {
                    let mut pool = self.pool.senders.lock().unwrap();
                    pool.remove(&self.id);
                }
            });
        });
    }
}

pub struct ExecutePipelineSocket {
    hb: Instant,
    pip_pool: web::Data<PipelinePool>,
    db_pool: web::Data<Pool<ConnectionManager<SqliteConnection>>>,
    cfg: web::Data<BldConfig>,
    prx: web::Data<ServerPipelineProxy>,
    user: User,
    exec: Option<AtomicEx>,
    sc: Option<FileScanner>,
}

impl ExecutePipelineSocket {
    pub fn new(
        user: User,
        pip_pool: web::Data<PipelinePool>,
        db_pool: web::Data<Pool<ConnectionManager<SqliteConnection>>>,
        cfg: web::Data<BldConfig>,
        prx: web::Data<ServerPipelineProxy>,
    ) -> Self {
        Self {
            hb: Instant::now(),
            pip_pool,
            db_pool,
            cfg,
            prx,
            user,
            exec: None,
            sc: None,
        }
    }

    fn heartbeat(act: &Self, ctx: &mut <Self as Actor>::Context) {
        if Instant::now().duration_since(act.hb) > Duration::from_secs(10) {
            println!("Websocket heartbeat failed, disconnecting!");
            ctx.stop();
            return;
        }
        ctx.ping(b"");
    }

    fn scan(act: &mut Self, ctx: &mut <Self as Actor>::Context) {
        if let Some(scanner) = act.sc.as_mut() {
            let content = scanner.fetch();
            for line in content.iter() {
                ctx.text(line.to_string());
            }
        }
    }

    fn exec(act: &mut Self, ctx: &mut <Self as Actor>::Context) {
        if let Some(exec) = act.exec.as_mut() {
            let exec = exec.lock().unwrap();
            if !exec.pipeline_run.running {
                ctx.stop();
            }
        }
    }

    fn get_info(&mut self, data: &str) -> anyhow::Result<PipelineInfo> {
        let info = serde_json::from_str::<ExecInfo>(data)?;
        let path = self.prx.path(&info.name)?;
        if !path.is_yaml() {
            let message = String::from("pipeline file not found");
            return Err(anyhow!(message));
        }

        let id = Uuid::new_v4().to_string();
        let config = self.cfg.get_ref();
        let logs = path![&config.local.logs, &id].display().to_string();

        let connection = self.db_pool.get()?;
        let pipeline = pipeline_runs::insert(&connection, &id, &info.name, &self.user.name)?;
        let start_time = String::from(&pipeline.start_date_time);
        let ex = Arc::new(Mutex::new(PipelineExecWrapper::new(
            Arc::clone(&self.db_pool),
            pipeline,
        )?));
        let (tx, rx) = mpsc::channel::<bool>();
        let rx = Arc::new(Mutex::new(rx));
        {
            let mut pool = self.pip_pool.senders.lock().unwrap();
            pool.insert(id.clone(), tx);
        }

        let info = PipelineInfo {
            cfg: Arc::clone(&self.cfg),
            pool: self.pip_pool.clone(),
            id,
            start_time,
            name: info.name,
            ex: Arc::clone(&ex),
            lg: Arc::new(Mutex::new(FileLogger::new(&logs)?)),
            prx: Arc::clone(&self.prx),
            cm: Some(rx),
            env: match info.environment {
                Some(env) => Arc::new(env),
                None => Arc::new(HashMap::<String, String>::new()),
            },
            vars: match info.variables {
                Some(vars) => Arc::new(vars),
                None => Arc::new(HashMap::<String, String>::new()),
            },
        };

        self.exec = Some(ex);
        self.sc = Some(FileScanner::new(&logs)?);

        Ok(info)
    }
}

impl Actor for ExecutePipelineSocket {
    type Context = ws::WebsocketContext<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        ctx.run_interval(Duration::from_millis(500), |act, ctx| {
            ExecutePipelineSocket::heartbeat(act, ctx);
            ExecutePipelineSocket::scan(act, ctx);
        });
        ctx.run_interval(Duration::from_secs(10), |act, ctx| {
            ExecutePipelineSocket::scan(act, ctx);
            ExecutePipelineSocket::exec(act, ctx);
        });
    }
}

impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for ExecutePipelineSocket {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        match msg {
            Ok(ws::Message::Text(txt)) => {
                match self.get_info(&txt) {
                    Ok(info) => {
                        info.spawn();
                    }
                    Err(e) => {
                        error!("{}", e.to_string());
                        ctx.text("Unable to run pipeline");
                        ctx.stop();
                    }
                };
            }
            Ok(ws::Message::Ping(msg)) => {
                self.hb = Instant::now();
                ctx.pong(&msg);
            }
            Ok(ws::Message::Pong(_)) => {
                self.hb = Instant::now();
            }
            Ok(ws::Message::Close(reason)) => {
                ctx.close(reason);
                ctx.stop();
            }
            _ => ctx.stop(),
        }
    }
}

pub async fn ws_exec(
    user: Option<User>,
    req: HttpRequest,
    stream: web::Payload,
    pip_pool: web::Data<PipelinePool>,
    db_pool: web::Data<Pool<ConnectionManager<SqliteConnection>>>,
    config: web::Data<BldConfig>,
    proxy: web::Data<ServerPipelineProxy>,
) -> Result<HttpResponse, Error> {
    let user = user.ok_or_else(|| ErrorUnauthorized(""))?;
    println!("{req:?}");
    let res = ws::start(
        ExecutePipelineSocket::new(user, pip_pool, db_pool, config, proxy),
        &req,
        stream,
    );
    println!("{res:?}");
    res
}
