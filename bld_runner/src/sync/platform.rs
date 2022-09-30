use crate::context::{Container, Machine};
use bld_core::execution::Execution;
use std::sync::{Arc, Mutex};

pub enum TargetPlatform {
    Machine(Box<Machine>),
    Container(Box<Container>),
}

impl TargetPlatform {
    pub async fn push(&self, from: &str, to: &str) -> anyhow::Result<()> {
        match self {
            Self::Machine(machine) => machine.copy_into(from, to),
            Self::Container(container) => container.copy_into(from, to).await,
        }
    }

    pub async fn get(&self, from: &str, to: &str) -> anyhow::Result<()> {
        match self {
            Self::Machine(machine) => machine.copy_from(from, to),
            Self::Container(container) => container.copy_from(from, to).await,
        }
    }

    pub async fn shell(
        &self,
        working_dir: &Option<String>,
        command: &str,
        exec: Arc<Mutex<Execution>>,
    ) -> anyhow::Result<()> {
        match self {
            Self::Machine(machine) => machine.sh(working_dir, command).await,
            Self::Container(container) => container.sh(working_dir, command, exec).await,
        }
    }

    pub async fn dispose(&self, in_child_runner: bool) -> anyhow::Result<()> {
        match self {
            // checking if the runner is a child in order to not cleanup the temp dir for the whole run
            Self::Machine(machine) if !in_child_runner => machine.dispose(),
            Self::Machine(_) => Ok(()),
            Self::Container(container) => container.dispose().await,
        }
    }
}