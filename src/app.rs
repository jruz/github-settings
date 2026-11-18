use tokio::sync::mpsc;

use crate::{cli, commands};

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
pub enum Error {
    #[error(transparent)]
    Channel(#[from] mpsc::error::SendError<String>),
    #[error("command failed")]
    Failed,
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
) -> Result<(), Error> {
    match arguments.command {
        cli::Command::Pull { force } => commands::pull::run(force, sender).await?,
        cli::Command::Plan => commands::plan::run(sender).await?,
        cli::Command::Push => commands::push::run(sender).await?,
        cli::Command::Schema => commands::schema::run(sender)?,
        cli::Command::Version => {
            let output = commands::version::run();
            let text = output.text.trim_end().to_owned();
            sender.send(text)?;
            if !output.success {
                return Err(Error::Failed);
            }
        }
    }
    Ok(())
}
