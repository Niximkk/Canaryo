use std::process::{Command, ExitCode};

use crate::analyzer::{self, Compatibility};

pub fn run(path: &str, arguments: &[String]) -> Result<ExitCode, String> {
    let report = analyzer::analyze_file(path)
        .map_err(|error| format!("não foi possível ler {path}: {error}"))?;
    report.print(path);

    if report.compatibility() == Compatibility::Incompatible {
        return Err("a análise encontrou funcionalidades incompatíveis".into());
    }

    eprintln!("Runtime nativo ainda não disponível; executando com Node.js.");
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
