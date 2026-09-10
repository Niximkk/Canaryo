use std::{fs, io, path::Path};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Compatibility {
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

// Uma regra representa uma API do Node e todas as grafias que reconhecemos para
// importá-la. A fatia `&[&str]` funciona como uma lista emprestada de textos.
struct Rule {
    patterns: &'static [&'static str],
    compatibility: Compatibility,
    message: &'static str,
}

pub struct Report {
    compatibility: Compatibility,
    findings: Vec<Finding>,
}

impl Report {
    pub fn compatibility(&self) -> Compatibility {
        self.compatibility
    }

    pub fn print(&self, path: &str) {
        println!("Relatório Canaryo: {}", self.compatibility.label());

        if self.findings.is_empty() {
            println!(
                "  Nenhuma API incompatível foi detectada em {}.",
                Path::new(path).display()
            );
            return;
        }

        for finding in &self.findings {
            println!("  [{}] {}", finding.compatibility.label(), finding.message);
        }
    }
}

fn scan(source: &str) -> Report {
    let checks = [
        Rule {
            patterns: &[
                "node:http",
                "require('http')",
                "require(\"http\")",
                "from 'http'",
                "from \"http\"",
            ],
            compatibility: Compatibility::Compatible,
            message: "usa a API http",
        },
        Rule {
            patterns: &[
                "node:fs",
                "require('fs')",
                "require(\"fs\")",
                "from 'fs'",
                "from \"fs\"",
            ],
            compatibility: Compatibility::Limited,
            message: "usa fs; o suporte a filesystem ainda é limitado",
        },
        Rule {
            patterns: &[
                "node:net",
                "require('net')",
                "require(\"net\")",
                "from 'net'",
                "from \"net\"",
            ],
            compatibility: Compatibility::Limited,
            message: "usa net; sockets TCP ainda não são suportados",
        },
        Rule {
            patterns: &[
                "node:child_process",
                "require('child_process')",
                "require(\"child_process\")",
                "from 'child_process'",
                "from \"child_process\"",
            ],
            compatibility: Compatibility::Incompatible,
            message: "usa child_process, que ainda não é suportado",
        },
        Rule {
            patterns: &[".node"],
            compatibility: Compatibility::Incompatible,
            message: "referencia um módulo nativo .node",
        },
    ];

    // `mut` permite alterar estas variáveis depois de criá-las. Sem ele, as
    // variáveis Rust são imutáveis por padrão.
    let mut findings = Vec::new();
    let mut overall = Compatibility::Compatible;

    for rule in checks {
        let mut matched = false;

        // Uma única regra pode reconhecer CommonJS, ESM e o prefixo `node:`.
        // Quando um padrão combina, `break` encerra apenas este laço interno.
        for pattern in rule.patterns {
            if source.contains(pattern) {
                matched = true;
                break;
            }
        }

        // `continue` ignora o restante desta volta e passa à próxima API.
        if !matched {
            continue;
        }

        // O relatório geral sempre preserva o problema mais grave encontrado.
        if rule.compatibility.severity() > overall.severity() {
            overall = rule.compatibility;
        }

        // `push` adiciona um novo Finding ao final do vetor.
        findings.push(Finding {
            compatibility: rule.compatibility,
            message: rule.message,
        });
    }

    Report {
        compatibility: overall,
        findings,
    }
}

pub fn analyze_file(path: &str) -> io::Result<Report> {
    let source = fs::read_to_string(path)?;
    Ok(scan(&source))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_a_basic_http_server() {
        let report = scan("const http = require('http');");

        assert_eq!(report.findings.len(), 1);
        assert_eq!(report.compatibility, Compatibility::Compatible);
    }

    #[test]
    fn recognizes_esm_without_the_node_prefix() {
        let report = scan("import { readFile } from 'fs';");

        assert_eq!(report.findings.len(), 1);
        assert_eq!(report.compatibility, Compatibility::Limited);
    }

    #[test]
    fn reports_an_api_only_once_when_multiple_patterns_match() {
        let report = scan("import http from 'node:http'; const other = require('http');");

        assert_eq!(report.findings.len(), 1);
        assert_eq!(report.compatibility, Compatibility::Compatible);
    }

    #[test]
    fn keeps_the_highest_severity() {
        let report = scan("import fs from 'node:fs'; require('./binding.node');");

        assert_eq!(report.findings.len(), 2);
        assert_eq!(report.compatibility, Compatibility::Incompatible);
    }

    #[test]
    fn returns_no_findings_for_plain_javascript() {
        let report = scan("console.log('olá');");

        assert!(report.findings.is_empty());
        assert_eq!(report.compatibility, Compatibility::Compatible);
    }
}
