use tokio::sync::mpsc;

use super::shared;
use crate::library::output;

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
pub enum Error {
    #[error(transparent)]
    #[diagnostic(transparent)]
    Output(#[from] output::Error),
    #[error(transparent)]
    #[diagnostic(transparent)]
    Shared(#[from] shared::Error),
}

#[tracing::instrument(skip_all)]
pub async fn run(sender: &mpsc::UnboundedSender<String>) -> Result<(), Error> {
    let client = shared::client()?;
    let changes = shared::read_changes(&client).await?;
    let event = output::Event::Planned(&changes);
    output::emit(sender, event)?;
    Ok(())
}
