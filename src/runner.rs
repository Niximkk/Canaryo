use std::process::{Command, ExitCode};

use crate::{
    analyzer::{self, Compatibility},
    cli::RuntimeMode,
    runtime,
};

pub fn run(
    path: &str,
    arguments: &[String],
    runtime_mode: RuntimeMode,
) -> Result<ExitCode, String> {
    let report = analyzer::analyze_file(path)
        .map_err(|error| format!("não foi possível ler {path}: {error}"))?;
    report.print(path);

    if report.compatibility() == Compatibility::Incompatible {
        return Err("a análise encontrou funcionalidades incompatíveis".into());
    }

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
