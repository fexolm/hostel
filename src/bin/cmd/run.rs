use clap::Args;
use hostel::vm::{Result as VmResult, Vm};

#[derive(Args)]
pub struct Cmd {
    #[arg(short, long)]
    pub filepath: String,
}

impl Cmd {
    pub fn execute(&self) -> VmResult<()> {
        let mut vm = Vm::new()?;
        let data = std::fs::read(&self.filepath)?;
        vm.load_elf(&data)?;
        vm.run()?;
        println!("guest finished execution");
        Ok(())
    }
}
