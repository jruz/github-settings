use std::path;

use tokio::sync::mpsc;

use crate::core::{output, pull};

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
pub enum Error {
    #[error(transparent)]
    #[diagnostic(transparent)]
    Output(#[from] output::Error),
    #[error(transparent)]
    #[diagnostic(transparent)]
    Pull(#[from] pull::Error),
}

#[tracing::instrument(skip_all)]
pub async fn run(force: bool, sender: &mpsc::UnboundedSender<String>) -> Result<(), Error> {
    let file = path::Path::new("github-settings.json");
    pull::write_declaration(file, force).await?;
    let event = output::Event::Pulled(file);
    output::emit(sender, event)?;
    Ok(())
}
