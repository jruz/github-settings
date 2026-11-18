use tokio::sync::mpsc;

use crate::{cli, commands};

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
pub enum Error {
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
    }
    Ok(())
}
