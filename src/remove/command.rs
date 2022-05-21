use crate::cli::BldCommand;
use crate::config::{definitions::VERSION, BldConfig};
use crate::helpers::errors::auth_for_server_invalid;
use crate::helpers::request;
use actix_web::rt::System;
use anyhow::anyhow;
use clap::{App, Arg, ArgMatches, SubCommand};
use tracing::debug;

const REMOVE: &str = "rm";
const SERVER: &str = "server";
const PIPELINE: &str = "pipeline";

pub struct RemoveCommand;

impl RemoveCommand {
    pub fn boxed() -> Box<dyn BldCommand> {
        Box::new(Self)
    }
}

impl BldCommand for RemoveCommand {
    fn id(&self) -> &'static str {
        REMOVE
    }

    fn interface(&self) -> App<'static, 'static> {
        let server = Arg::with_name(SERVER)
            .short("s")
            .long(SERVER)
            .takes_value(true)
            .help("The name of the bld server");
        let pipeline = Arg::with_name(PIPELINE)
            .short("p")
            .long(PIPELINE)
            .takes_value(true)
            .help("The name of the pipeline");
        SubCommand::with_name(REMOVE)
            .about("Removes a pipeline from a bld server")
            .version(VERSION)
            .args(&vec![server, pipeline])
    }

    fn exec(&self, matches: &ArgMatches<'_>) -> anyhow::Result<()> {
        System::new().block_on(async move { do_remove(matches).await })
    }
}

async fn do_remove(matches: &ArgMatches<'_>) -> anyhow::Result<()> {
    let config = BldConfig::load()?;
    let srv = config.remote.server_or_first(matches.value_of(SERVER))?;
    let pipeline = matches
        .value_of(PIPELINE)
        .ok_or_else(|| anyhow!("invalid pipeline"))?
        .to_string();
    debug!(
        "running {} subcommand with --server: {} and --pipeline: {pipeline}",
        REMOVE, srv.name
    );
    let (name, auth) = match &srv.same_auth_as {
        Some(name) => match config.remote.servers.iter().find(|s| &s.name == name) {
            Some(srv) => (&srv.name, &srv.auth),
            None => return auth_for_server_invalid(),
        },
        None => (&srv.name, &srv.auth),
    };
    let url = format!("http://{}:{}/remove", srv.host, srv.port);
    let headers = request::headers(name, auth)?;
    debug!("sending http request to {url}");
    request::post(url, headers, pipeline).await.map(|r| {
        println!("{r}");
    })
}