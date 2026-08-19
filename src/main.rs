mod cli;
mod commands;

use clap::Parser;
use cli::{Cli, Command};
use std::process;

fn main() {
    let cli = Cli::parse();

    let exit_code = match cli.command {
        Command::Read {
            file,
            line,
            partition_lines,
            offset_ratio,
        } => commands::read::execute(&file, line, partition_lines, offset_ratio),
    };

    process::exit(exit_code);
}
