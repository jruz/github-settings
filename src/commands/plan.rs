use tokio::sync::mpsc;

use crate::core::{output, session};

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
pub enum Error {
    #[error(transparent)]
    #[diagnostic(transparent)]
    Output(#[from] output::Error),
    #[error(transparent)]
    #[diagnostic(transparent)]
    Session(#[from] session::Error),
}

#[tracing::instrument(skip_all)]
pub async fn run(sender: &mpsc::UnboundedSender<String>) -> Result<(), Error> {
    let client = session::client()?;
    let comparison = session::read_changes(&client).await?;
    let event = output::Event::Planned(&comparison.changes);
    output::emit(sender, event)?;
    Ok(())
}
