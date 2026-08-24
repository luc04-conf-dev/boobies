mod cli;
mod commands;
mod database;
mod installer;
mod models;
mod package;
mod repo;

use anyhow::Result;
use clap::Parser;

use crate::cli::Cli;

fn main() -> Result<()> {
    let cli = Cli::parse();
    commands::dispatch(cli)
}
