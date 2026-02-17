use clap::Args;
use hostel::loader::Loader;

#[derive(Args)]
pub struct Cmd {
    #[arg(short, long)]
    pub filepath: String,
}

impl Cmd {
    pub fn execute(&self) {
        let loader = Loader::new();
        let exe = loader.load(&self.filepath);
        exe.run();
    }
}
