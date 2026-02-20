mod cmd;

use clap::{Parser, Subcommand};

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
        Commands::Run(cmd) => {
            if let Err(e) = cmd.execute() {
                eprintln!("error: {}", e);
                std::process::exit(1);
            }
        }
    }
}
