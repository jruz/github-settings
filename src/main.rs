mod app;
mod cli;
mod commands;
#[path = "lib/mod.rs"]
mod library;

use clap::Parser as _;
use tokio::sync::mpsc;

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
enum Error {
    #[error(transparent)]
    #[diagnostic(transparent)]
    Application(#[from] app::Error),
    #[error(transparent)]
    Task(#[from] tokio::task::JoinError),
}

#[tokio::main]
async fn main() -> std::process::ExitCode {
    let arguments = cli::Arguments::parse();
    let (sender, mut receiver) = mpsc::unbounded_channel();
    let task = tokio::spawn(async move { app::run(arguments, &sender).await });
    while let Some(message) = receiver.recv().await {
        println!("{message}");
    }
    match finish(task).await {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
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

fn report(error: &Error) -> String {
    let handler = miette::GraphicalReportHandler::new().with_wrap_lines(false);
    let mut rendered = String::new();
    match handler.render_report(&mut rendered, error) {
        Ok(()) => rendered,
        Err(render_error) => format!("{error}\nfailed to render error report: {render_error}"),
    }
}
