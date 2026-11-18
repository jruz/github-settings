use std::path;

use super::session;
use crate::libs::{config, git, github::live};

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
    Session(#[from] session::Error),
}

#[tracing::instrument]
pub async fn write_declaration(file: &path::Path, force: bool) -> Result<(), Error> {
    let repository = git::repository().await?;
    let client = session::client()?;
    config::check_destination(file, force).await?;
    let declaration = live::read(&client, &repository, None).await?;
    config::write(file, &declaration, force).await?;
    Ok(())
}
