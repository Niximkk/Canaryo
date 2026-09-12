mod analyzer;
mod cli;
mod esm;
mod http;
mod modules;
mod net;
mod runner;
mod runtime;

use std::{env, process::ExitCode};

use analyzer::Compatibility;
use cli::CliCommand;

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn print_usage() {
    println!(
        "Canaryo {VERSION}\n\n\
         Uso:\n  \
         canaryo check <arquivo.js>\n  \
         canaryo run <arquivo.js> [argumentos...]\n  \
         canaryo run --node <arquivo.js> [argumentos...]\n  \
         canaryo <arquivo.js> [argumentos...]"
    );
}

fn execute(command: CliCommand) -> Result<ExitCode, String> {
    match command {
        CliCommand::Help => {
            print_usage();
            Ok(ExitCode::SUCCESS)
        }
        CliCommand::Version => {
            println!("canaryo {VERSION}");
            Ok(ExitCode::SUCCESS)
        }
        CliCommand::Check { path } => {
            let report = analyzer::analyze_file(&path)
                .map_err(|error| format!("não foi possível ler {path}: {error}"))?;
            report.print(&path);

            Ok(if report.compatibility() == Compatibility::Incompatible {
                ExitCode::FAILURE
            } else {
                ExitCode::SUCCESS
            })
        }
        CliCommand::Run {
            path,
            arguments,
            runtime,
        } => runner::run(&path, &arguments, runtime),
    }
}

fn main() -> ExitCode {
    let arguments: Vec<String> = env::args().skip(1).collect();

    let command = match cli::parse(&arguments) {
        Ok(command) => command,
        Err(error) => {
            eprintln!("canaryo: {error}");
            return ExitCode::FAILURE;
        }
    };

    match execute(command) {
        Ok(exit_code) => exit_code,
        Err(error) => {
            eprintln!("canaryo: {error}");
            ExitCode::FAILURE
        }
    }
}
