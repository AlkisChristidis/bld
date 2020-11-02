mod config;
mod definitions;
mod init;
mod list;
mod monit;
mod os;
mod persist;
mod run;
mod server;
mod term;

use crate::definitions::VERSION;
use clap::App;
use term::print_error;

fn main() {
    let matches = App::new("Bld")
        .version(VERSION)
        .about("A distributed CI/CD")
        .subcommands(vec![
            init::command(),
            run::command(),
            server::command(),
            monit::command(),
            list::command(),
        ])
        .get_matches();

    let result = match matches.subcommand() {
        ("init", Some(matches)) => init::exec(matches),
        ("run", Some(matches)) => run::exec(matches),
        ("server", Some(matches)) => server::exec(matches),
        ("monit", Some(matches)) => monit::exec(matches),
        ("ls", Some(matches)) => list::exec(matches),
        _ => Ok(()),
    };

    if let Err(e) = result {
        if let Err(e) = print_error(&e.to_string()) {
            eprintln!("{}", e.to_string());
        }
    }
}
