use std::{env, fs, io, path, sync};

use tracing_subscriber::util::SubscriberInitExt as _;

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
pub enum Error {
    #[error("cannot open the error log in {0}")]
    Open(String, #[source] io::Error),
    #[error("cannot start the error log")]
    Start(#[from] tracing_subscriber::util::TryInitError),
    #[error("cannot locate the error log: neither XDG_STATE_HOME nor HOME is set")]
    Unlocated,
}

pub fn directory() -> Result<path::PathBuf, Error> {
    let state = match (env::var_os("XDG_STATE_HOME"), env::var_os("HOME")) {
        (Some(state), _) if !state.is_empty() => path::PathBuf::from(state),
        (_, Some(home)) => path::Path::new(&home).join(".local").join("state"),
        (_, None) => return Err(Error::Unlocated),
    };
    let name = env!("CARGO_PKG_NAME");
    let directory = state.join(name);
    Ok(directory)
}

pub fn start() -> Result<(), Error> {
    let directory = directory()?;
    let file = open(&directory);
    let file = match file {
        Ok(file) => file,
        Err(source) => {
            let location = directory.display().to_string();
            let error = Error::Open(location, source);
            return Err(error);
        }
    };
    let writer = sync::Mutex::new(file);
    tracing_subscriber::fmt()
        .json()
        .with_current_span(true)
        .with_span_list(true)
        .with_max_level(tracing::Level::ERROR)
        .with_writer(writer)
        .finish()
        .try_init()?;
    Ok(())
}

fn open(directory: &path::Path) -> Result<fs::File, io::Error> {
    fs::create_dir_all(directory)?;
    let file = directory.join("errors.jsonl");
    fs::OpenOptions::new().create(true).append(true).open(file)
}
