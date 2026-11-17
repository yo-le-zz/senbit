mod cli;
mod filter;
mod log;
mod output;
mod parser;
mod source;

use anyhow::Result;
use clap::Parser;

use cli::Cli;

fn main() {
    if let Err(error) = run() {
        eprintln!("log: {}", error);
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();

    log::run(cli)
}