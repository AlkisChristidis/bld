use crate::definitions;
use crate::term::print_info;
use clap::ArgMatches;
use std::fs;
use std::io::{self, Error, ErrorKind};
use std::path::Component::Normal;
use std::path::{Path, PathBuf};

fn build_dir_exists() -> io::Result<bool> {
    let curr_dir = std::env::current_dir()?;
    for entry in fs::read_dir(&curr_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            let component = path.components().last();
            if let Some(Normal(name)) = component {
                if name == definitions::TOOL_DIR {
                    return Ok(true);
                }
            }
        }
    }
    Ok(false)
}

fn create_build_dir() -> io::Result<()> {
    let path = Path::new(definitions::TOOL_DIR);
    fs::create_dir(path)?;
    let message = format!("{} directory created", definitions::TOOL_DIR);
    print_info(&message)?;
    Ok(())
}

fn create_logs_dir(is_server: bool) -> io::Result<()> {
    if is_server {
        let path = Path::new(definitions::LOCAL_LOGS);
        fs::create_dir(path)?;
    }
    Ok(())
}

fn create_db_dir(is_server: bool) -> io::Result<()> {
    if is_server {
        let path = Path::new(definitions::LOCAL_DB);
        fs::create_dir(path)?;
    }
    Ok(())
}

fn create_default_yaml() -> io::Result<()> {
    let mut path = PathBuf::new();
    path.push(definitions::TOOL_DIR);
    path.push(format!("{}.yaml", definitions::TOOL_DEFAULT_PIPELINE));
    fs::write(path, definitions::DEFAULT_PIPELINE_CONTENT)?;
    let message = format!("{} yaml file created", definitions::TOOL_DEFAULT_PIPELINE);
    print_info(&message)?;
    Ok(())
}

fn create_config_yaml(is_server: bool) -> io::Result<()> {
    let mut path = PathBuf::new();
    path.push(definitions::TOOL_DIR);
    path.push(format!("{}.yaml", definitions::TOOL_DEFAULT_CONFIG));
    let content = match is_server {
        true => definitions::default_server_config(),
        false => definitions::default_client_config(),
    };
    fs::write(path, &content)?;
    print_info("config file created")?;
    Ok(())
}

pub fn exec(matches: &ArgMatches<'_>) -> io::Result<()> {
    let build_dir_exists = build_dir_exists()?;
    if !build_dir_exists {
        let is_server = matches.is_present("server");
        return create_build_dir()
            .and_then(|_| create_logs_dir(is_server))
            .and_then(|_| create_db_dir(is_server))
            .and_then(|_| create_default_yaml())
            .and_then(|_| create_config_yaml(is_server));
    }
    let message = format!(
        "{} dir already exists in the current directory",
        definitions::TOOL_DIR
    );
    Err(Error::new(ErrorKind::Other, message))
}
