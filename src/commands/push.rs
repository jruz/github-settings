use std::process;

use tokio::sync::mpsc;

use crate::core::{output, push, session};

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
pub enum Error {
    #[error(transparent)]
    #[diagnostic(transparent)]
    Output(#[from] output::Error),
    #[error(transparent)]
    #[diagnostic(transparent)]
    Push(#[from] push::Error),
    #[error(transparent)]
    #[diagnostic(transparent)]
    Session(#[from] session::Error),
}

#[tracing::instrument(skip_all)]
pub async fn run(sender: &mpsc::UnboundedSender<String>) -> Result<process::ExitCode, Error> {
    let client = session::client()?;
    let changes = session::read_changes(&client).await?;
    let event = output::Event::Planned(&changes);
    output::emit(sender, event)?;
    if changes.is_empty() {
        return Ok(process::ExitCode::SUCCESS);
    }
    let outcome = push::apply(&client, &changes, &push::POLLING, sender).await?;
    let code = match outcome {
        push::Outcome::Applied => process::ExitCode::SUCCESS,
        push::Outcome::Failed => process::ExitCode::FAILURE,
    };
    Ok(code)
}
