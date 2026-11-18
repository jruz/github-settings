use tokio::sync::mpsc;

use crate::library::output;

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
pub enum Error {
    #[error(transparent)]
    #[diagnostic(transparent)]
    Output(#[from] output::Error),
}

pub fn run(sender: &mpsc::UnboundedSender<String>) -> Result<(), Error> {
    output::emit(sender, output::Event::Schema)?;
    Ok(())
}
