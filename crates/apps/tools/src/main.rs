mod atlas;
mod cli;
mod commands;

use clap::Parser;

fn main() -> anyhow::Result<()> {
    commands::dispatch(cli::Cli::parse().command)
}
