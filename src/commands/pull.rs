use std::path;

use tokio::sync::mpsc;

use super::shared;
use crate::library::{config, git, github::live, output};

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
pub enum Error {
    #[error(transparent)]
    #[diagnostic(transparent)]
    Config(#[from] config::Error),
    #[error(transparent)]
    #[diagnostic(transparent)]
    Git(#[from] git::Error),
    #[error(transparent)]
    #[diagnostic(transparent)]
    Live(#[from] live::Error),
    #[error(transparent)]
    #[diagnostic(transparent)]
    Output(#[from] output::Error),
    #[error(transparent)]
    #[diagnostic(transparent)]
    Shared(#[from] shared::Error),
}

#[tracing::instrument(skip_all)]
pub async fn run(force: bool, sender: &mpsc::UnboundedSender<String>) -> Result<(), Error> {
    let file = path::Path::new("github-settings.json");
    let repository = git::repository().await?;
    let client = shared::client()?;
    config::check_destination(file, force).await?;
    let declaration = live::read(&client, &repository, None).await?;
    config::write(file, &declaration, force).await?;
    let event = output::Event::Pulled(file);
    output::emit(sender, event)?;
    Ok(())
}
