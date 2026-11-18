mod app;
mod cli;
mod commands;
mod core;
mod libs;
mod utils;

use clap::Parser as _;
use tokio::sync::mpsc;

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
enum Error {
    #[error(transparent)]
    #[diagnostic(transparent)]
    Application(#[from] app::Error),
    #[error(transparent)]
    #[diagnostic(transparent)]
    Log(#[from] utils::error_log::Error),
    #[error(transparent)]
    Task(#[from] tokio::task::JoinError),
}

#[tokio::main]
async fn main() -> std::process::ExitCode {
    let arguments = cli::Arguments::parse();
    if let Err(error) = utils::error_log::start() {
        let error = Error::Log(error);
        let report = report(&error);
        eprintln!("{report}");
        return std::process::ExitCode::FAILURE;
    }
    let (sender, mut receiver) = mpsc::unbounded_channel();
    let process = std::process::id();
    let span = tracing::error_span!("command", process);
    let run = async move { app::run(arguments, &sender).await };
    let traced = tracing::Instrument::instrument(run, span);
    let task = tokio::spawn(traced);
    while let Some(message) = receiver.recv().await {
        println!("{message}");
    }
    match finish(task).await {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            record(&error);
            let report = report(&error);
            eprintln!("{report}");
            std::process::ExitCode::FAILURE
        }
    }
}

async fn finish(task: tokio::task::JoinHandle<Result<(), app::Error>>) -> Result<(), Error> {
    task.await??;
    Ok(())
}

fn record(error: &Error) {
    let arguments = std::env::args().collect::<Vec<_>>();
    let directory = std::env::current_dir();
    let process = std::process::id();
    tracing::error!(
        process,
        version = env!("CARGO_PKG_VERSION"),
        revision = env!("REVISION"),
        ?arguments,
        ?directory,
        %error,
        details = ?error,
        "command failed"
    );
}

fn report(error: &Error) -> String {
    let handler = miette::GraphicalReportHandler::new().with_wrap_lines(false);
    let mut rendered = String::new();
    match handler.render_report(&mut rendered, error) {
        Ok(()) => rendered,
        Err(render_error) => format!("{error}\nfailed to render error report: {render_error}"),
    }
}
