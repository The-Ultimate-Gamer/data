//! ug-admin: add, edit and publish The Ultimate Gamer tournament data from one binary.

// Without the desktop window (CI builds), the code only it uses is unused.
#![cfg_attr(not(feature = "gui"), allow(dead_code))]

mod cli;
mod config;
mod db;
mod export;
mod github;
#[cfg(feature = "gui")]
mod gui;
mod model;
mod repo;
mod rules;
mod seed;
mod sync;
#[cfg(test)]
mod tests;
mod validate;
mod workspace;

use std::process::ExitCode;

use clap::Parser;

fn main() -> ExitCode {
    let args = cli::Cli::parse();
    let result = match args.command {
        Some(command) => cli::run(command),
        None => open_app(args.repo),
    };
    match result {
        Ok(code) => code,
        Err(err) => {
            eprintln!("error: {err:#}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(feature = "gui")]
fn open_app(folder: Option<std::path::PathBuf>) -> anyhow::Result<ExitCode> {
    gui::run(folder)?;
    Ok(ExitCode::SUCCESS)
}

#[cfg(not(feature = "gui"))]
fn open_app(_folder: Option<std::path::PathBuf>) -> anyhow::Result<ExitCode> {
    eprintln!("This build has no desktop window (built without the `gui` feature). See `ug-admin --help`.");
    Ok(ExitCode::FAILURE)
}
