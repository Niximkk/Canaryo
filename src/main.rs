use std::{
    env, fs,
    path::Path,
    process::{Command, ExitCode},
};

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Compatibility {
    Compatible,
    Limited,
    Incompatible,
}

impl Compatibility {
    fn label(self) -> &'static str {
        match self {
            Self::Compatible => "compatível",
            Self::Limited => "compatível com limitações",
            Self::Incompatible => "incompatível",
        }
    }

    fn severity(self) -> u8 {
        match self {
            Self::Compatible => 0,
            Self::Limited => 1,
            Self::Incompatible => 2,
        }
    }
}

struct Finding {
    compatibility: Compatibility,
    message: &'static str,
}

fn print_usage() {
    println!(
        "Canaryo {VERSION}\n\n\
         Uso:\n  \
         canaryo check <arquivo.js>\n  \
         canaryo run <arquivo.js> [argumentos...]\n  \
         canaryo <arquivo.js> [argumentos...]"
    );
}

fn scan(source: &str) -> Vec<Finding> {
    let checks = [
        (
            "node:http",
            Compatibility::Compatible,
            "usa a API node:http",
        ),
        (
            "require('http')",
            Compatibility::Compatible,
            "usa a API http via CommonJS",
        ),
        (
            "require(\"http\")",
            Compatibility::Compatible,
            "usa a API http via CommonJS",
        ),
        (
            "node:fs",
            Compatibility::Limited,
            "usa node:fs; o suporte a filesystem ainda é limitado",
        ),
        (
            "node:net",
            Compatibility::Limited,
            "usa node:net; sockets TCP ainda não são suportados",
        ),
        (
            "node:child_process",
            Compatibility::Incompatible,
            "usa node:child_process, que ainda não é suportado",
        ),
        (
            ".node",
            Compatibility::Incompatible,
            "referencia um módulo nativo .node",
        ),
    ];

    checks
        .into_iter()
        .filter_map(|(needle, compatibility, message)| {
            source.contains(needle).then_some(Finding {
                compatibility,
                message,
            })
        })
        .collect()
}

fn check_file(path: &str) -> std::io::Result<Compatibility> {
    let source = fs::read_to_string(path)?;
    let findings = scan(&source);

    let overall = findings
        .iter()
        .map(|finding| finding.compatibility)
        .max_by_key(|compatibility| compatibility.severity())
        .unwrap_or(Compatibility::Compatible);

    println!("Relatório Canaryo: {}", overall.label());
    if findings.is_empty() {
        println!(
            "  Nenhuma API incompatível foi detectada em {}.",
            Path::new(path).display()
        );
    } else {
        for finding in findings {
            println!("  [{}] {}", finding.compatibility.label(), finding.message);
        }
    }

    Ok(overall)
}

fn run_file(path: &str, arguments: &[String]) -> Result<ExitCode, String> {
    let compatibility =
        check_file(path).map_err(|error| format!("não foi possível ler {path}: {error}"))?;

    if compatibility == Compatibility::Incompatible {
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

fn main() -> ExitCode {
    let arguments: Vec<String> = env::args().skip(1).collect();

    if arguments.is_empty() {
        print_usage();
        return ExitCode::FAILURE;
    }

    let result = match arguments[0].as_str() {
        "--help" | "-h" => {
            print_usage();
            return ExitCode::SUCCESS;
        }
        "--version" | "-V" => {
            println!("canaryo {VERSION}");
            return ExitCode::SUCCESS;
        }
        "check" => match arguments.get(1) {
            Some(path) => check_file(path)
                .map(|_| ExitCode::SUCCESS)
                .map_err(|error| format!("não foi possível ler {path}: {error}")),
            None => Err("informe o arquivo que deve ser analisado".into()),
        },
        "run" => match arguments.get(1) {
            Some(path) => run_file(path, &arguments[2..]),
            None => Err("informe o arquivo que deve ser executado".into()),
        },
        path => run_file(path, &arguments[1..]),
    };

    match result {
        Ok(exit_code) => exit_code,
        Err(error) => {
            eprintln!("canaryo: {error}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_a_basic_http_server() {
        let findings = scan("const http = require('http');");

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].compatibility, Compatibility::Compatible);
    }

    #[test]
    fn marks_native_addons_as_incompatible() {
        let findings = scan("const binding = require('./binding.node');");

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].compatibility, Compatibility::Incompatible);
    }

    #[test]
    fn returns_no_findings_for_plain_javascript() {
        assert!(scan("console.log('olá');").is_empty());
    }
}
