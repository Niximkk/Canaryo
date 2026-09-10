use std::process::{Command, ExitCode};

use crate::{cli::RuntimeMode, runtime};

pub fn run(
    path: &str,
    arguments: &[String],
    runtime_mode: RuntimeMode,
) -> Result<ExitCode, String> {
    match runtime_mode {
        RuntimeMode::Native => {
            runtime::execute(path, arguments)?;
            Ok(ExitCode::SUCCESS)
        }
        RuntimeMode::Node => run_with_node(path, arguments),
    }
}

fn run_with_node(path: &str, arguments: &[String]) -> Result<ExitCode, String> {
    let status = Command::new("node")
        .arg(path)
        .args(arguments)
        .status()
        .map_err(|error| format!("não foi possível iniciar o Node.js: {error}"))?;

    Ok(if status.success() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}
