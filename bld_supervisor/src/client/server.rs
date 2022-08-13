use crate::base::{
    Queue, UnixSocketConnectionState, UnixSocketHandle, UnixSocketMessage, UnixSocketRead,
    UnixSocketState,
};
use bld_core::workers::PipelineWorker;
use std::{
    env::current_exe,
    process::Command,
    sync::{Arc, Mutex},
};
use tokio::net::UnixStream;
use tracing::{debug, error};
use uuid::Uuid;

pub struct UnixSocketServerReader {
    _id: Uuid,
    stream: UnixStream,
    state: UnixSocketConnectionState,
}

impl UnixSocketServerReader {
    pub fn new(stream: UnixStream) -> Self {
        Self {
            _id: Uuid::new_v4(),
            stream,
            state: UnixSocketConnectionState::Active,
        }
    }
}

impl UnixSocketRead for UnixSocketServerReader {
    fn get_stream(&self) -> &UnixStream {
        &self.stream
    }
}

impl UnixSocketHandle for UnixSocketServerReader {
    fn handle<Q>(&mut self, queue: Arc<Mutex<Q>>, messages: Vec<UnixSocketMessage>)
    where
        Q: Queue<Arc<Mutex<PipelineWorker>>>,
    {
        for message in messages.iter() {
            if let UnixSocketMessage::ServerEnqueue {
                pipeline,
                run_id,
                variables,
                environment,
            } = message
            {
                debug!("received new server enqueue message for pipeline: {pipeline}");
                let exe = match current_exe() {
                    Ok(exe) => exe,
                    Err(e) => {
                        error!("could not get the current executable. {e}");
                        continue;
                    }
                };
                let mut command = Command::new(exe);
                command.arg("worker");
                command.arg("--pipeline");
                command.arg(pipeline);
                command.arg("--run-id");
                command.arg(run_id);
                if let Some(variables) = variables {
                    command.arg("--variables");
                    command.arg(variables);
                }
                if let Some(environment) = environment {
                    command.arg("--environment");
                    command.arg(environment);
                }
                let worker = PipelineWorker::new(command);
                let mut queue = queue.lock().unwrap();
                queue.enqueue(Arc::new(Mutex::new(worker)));
            }
        }
    }
}

impl UnixSocketState for UnixSocketServerReader {
    fn set_state(&mut self, state: UnixSocketConnectionState) {
        self.state = state;
    }

    fn get_state(&self) -> &UnixSocketConnectionState {
        &self.state
    }

    fn has_stopped(&self) -> bool {
        match self.state {
            UnixSocketConnectionState::Stopped => true,
            _ => false,
        }
    }
}
