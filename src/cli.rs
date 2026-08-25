use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "scopefolio",
    version = env!("CARGO_PKG_VERSION"),
    about = "ScopeFolio v0.2.1 — Deterministic scoped file reading"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Read the local scope around a target line.
    Read {
        /// Path to the target file.
        #[arg(long)]
        file: String,

        /// Target line number (1-based).
        #[arg(long)]
        line: usize,

        /// Target number of lines per leaf partition (default: 400).
        #[arg(long, default_value_t = scopefolio::DEFAULT_PARTITION_LINES)]
        partition_lines: usize,

        /// Contextual offset ratio around the selected partition (default: 0).
        #[arg(long, default_value_t = scopefolio::DEFAULT_OFFSET_RATIO)]
        offset_ratio: f64,
    },
}
