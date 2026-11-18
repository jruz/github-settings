use std::process;

use tokio::sync::mpsc;

use crate::{cli, commands};

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
pub enum Error {
    #[error(transparent)]
    Channel(#[from] mpsc::error::SendError<String>),
    #[error(transparent)]
    #[diagnostic(transparent)]
    Plan(#[from] commands::plan::Error),
    #[error(transparent)]
    #[diagnostic(transparent)]
    Pull(#[from] commands::pull::Error),
    #[error(transparent)]
    #[diagnostic(transparent)]
    Push(#[from] commands::push::Error),
    #[error(transparent)]
    #[diagnostic(transparent)]
    Schema(#[from] commands::schema::Error),
}

#[tracing::instrument(skip_all)]
pub async fn run(
    arguments: cli::Arguments,
    sender: &mpsc::UnboundedSender<String>,
) -> Result<process::ExitCode, Error> {
    let code = match arguments.command {
        cli::Command::Pull { force } => {
            commands::pull::run(force, sender).await?;
            process::ExitCode::SUCCESS
        }
        cli::Command::Plan => {
            commands::plan::run(sender).await?;
            process::ExitCode::SUCCESS
        }
        cli::Command::Push => commands::push::run(sender).await?,
        cli::Command::Schema => {
            commands::schema::run(sender)?;
            process::ExitCode::SUCCESS
        }
        cli::Command::Version => version(sender)?,
    };
    Ok(code)
}

fn version(sender: &mpsc::UnboundedSender<String>) -> Result<process::ExitCode, Error> {
    let output = commands::version::run();
    let text = output.text.trim_end().to_owned();
    sender.send(text)?;
    let code = if output.success {
        process::ExitCode::SUCCESS
    } else {
        process::ExitCode::FAILURE
    };
    Ok(code)
}
