mod cmd;

use clap::{Command, Parser, Subcommand};

#[derive(Parser)]
#[command(name = "hostel")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Run(cmd::run::Cmd),
}

fn main() {
    let cli = Cli::parse();

    match &cli.command {
        Commands::Run(cmd) => cmd.execute(),
    }
}
