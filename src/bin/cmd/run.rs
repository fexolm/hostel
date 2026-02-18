use clap::Args;
use hostel::loader::{Loader, Result as LoaderResult};

#[derive(Args)]
pub struct Cmd {
    #[arg(short, long)]
    pub filepath: String,
}

impl Cmd {
    pub fn execute(&self) -> LoaderResult<()> {
        let mut loader = Loader::new()?;
        let module = loader.load(&self.filepath);
        Ok(())
    }
}
