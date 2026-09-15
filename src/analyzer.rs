use std::{
    collections::HashSet,
    fs, io,
    path::{Path, PathBuf},
};

use crate::modules;

const MAX_ANALYZED_FILES: usize = 10_000;
const NODE_BUILTINS: &[&str] = &[
    "_stream_duplex",
    "_stream_passthrough",
    "_stream_readable",
    "_stream_transform",
    "_stream_writable",
    "assert",
    "assert/strict",
    "async_hooks",
    "buffer",
    "child_process",
    "console",
    "constants",
    "crypto",
    "diagnostics_channel",
    "dns",
    "dns/promises",
    "events",
    "fs",
    "fs/promises",
    "http",
    "http2",
    "https",
    "module",
    "net",
    "os",
    "path",
    "path/posix",
    "path/win32",
    "perf_hooks",
    "process",
    "querystring",
    "stream",
    "stream/consumers",
    "stream/promises",
    "stream/web",
    "string_decoder",
    "sys",
    "timers",
    "timers/promises",
    "tty",
    "url",
    "util",
    "util/types",
    "worker_threads",
    "zlib",
];

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

struct Rule {
    patterns: &'static [&'static str],
    compatibility: Compatibility,
    message: &'static str,
}

struct Finding {
    file: PathBuf,
    compatibility: Compatibility,
    message: String,
}

pub struct Report {
    compatibility: Compatibility,
    findings: Vec<Finding>,
    analyzed_files: usize,
}

impl Report {
    pub fn compatibility(&self) -> Compatibility {
        self.compatibility
    }

    pub fn print(&self, path: &str) {
        println!("Relatório Canaryo: {}", self.compatibility.label());
        println!("  {} arquivo(s) analisado(s)", self.analyzed_files);

        if self.findings.is_empty() {
            println!(
                "  Nenhuma incompatibilidade detectada a partir de {}.",
                Path::new(path).display()
            );
            return;
        }

        for finding in &self.findings {
            println!(
                "  [{}] {}: {}",
                finding.compatibility.label(),
                display_path(&finding.file),
                finding.message
            );
        }
    }

    fn add(&mut self, finding: Finding) {
        if finding.compatibility.severity() > self.compatibility.severity() {
            self.compatibility = finding.compatibility;
        }
        self.findings.push(finding);
    }
}

fn display_path(path: &Path) -> String {
    let relative = std::env::current_dir()
        .ok()
        .and_then(|directory| directory.canonicalize().ok())
        .and_then(|directory| path.strip_prefix(directory).ok())
        .unwrap_or(path);
    relative.to_string_lossy().into_owned()
}

pub fn analyze_file(path: &str) -> io::Result<Report> {
    let entry = Path::new(path).canonicalize()?;
    let mut report = Report {
        compatibility: Compatibility::Compatible,
        findings: Vec::new(),
        analyzed_files: 0,
    };
    let mut visited = HashSet::new();

    visit(&entry, &mut report, &mut visited)?;
    Ok(report)
}

fn visit(path: &Path, report: &mut Report, visited: &mut HashSet<PathBuf>) -> io::Result<()> {
    let path = path.canonicalize()?;
    if !visited.insert(path.clone()) {
        return Ok(());
    }
    if visited.len() > MAX_ANALYZED_FILES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "projeto excede o limite de 10.000 arquivos analisados",
        ));
    }

    let source = fs::read_to_string(&path)?;
    let analyzed_source = strip_js_comments(&source);
    report.analyzed_files += 1;

    for finding in scan_source(&analyzed_source, &path) {
        report.add(finding);
    }

    if path.extension().and_then(|extension| extension.to_str()) == Some("json") {
        return Ok(());
    }

    for specifier in extract_specifiers(&analyzed_source) {
        if let Some(message) = unsupported_node_builtin(&specifier) {
            report.add(Finding {
                file: path.clone(),
                compatibility: Compatibility::Incompatible,
                message,
            });
            continue;
        }

        if is_node_builtin(&specifier) {
            continue;
        }

        if specifier.ends_with(".node") {
            report.add(Finding {
                file: path.clone(),
                compatibility: Compatibility::Incompatible,
                message: format!("referencia o módulo nativo '{specifier}'"),
            });
            continue;
        }

        match modules::resolve(path.to_string_lossy().as_ref(), &specifier) {
            Ok(dependency) => visit(&dependency, report, visited)?,
            Err(error) => report.add(Finding {
                file: path.clone(),
                compatibility: Compatibility::Limited,
                message: format!(
                    "dependência dinâmica ou opcional não resolvida '{specifier}': {error}"
                ),
            }),
        }
    }

    Ok(())
}

fn scan_source(source: &str, path: &Path) -> Vec<Finding> {
    let rules = [
        Rule {
            patterns: &["node:http", "require('http')", "require(\"http\")"],
            compatibility: Compatibility::Compatible,
            message: "usa a API http",
        },
        Rule {
            patterns: &["node:https", "require('https')", "require(\"https\")"],
            compatibility: Compatibility::Limited,
            message: "usa https; clientes e servidores TLS básicos estão disponíveis, mas opções avançadas ainda não",
        },
        Rule {
            patterns: &["node:fs", "require('fs')", "require(\"fs\")"],
            compatibility: Compatibility::Limited,
            message: "usa fs; o suporte a filesystem ainda é limitado",
        },
        Rule {
            patterns: &["node:net", "require('net')", "require(\"net\")"],
            compatibility: Compatibility::Limited,
            message: "usa net; TCP está disponível, mas IPC e opções avançadas de socket ainda não",
        },
        Rule {
            patterns: &[
                "node:buffer",
                "node:events",
                "node:path",
                "node:stream",
                "node:url",
                "node:util",
            ],
            compatibility: Compatibility::Limited,
            message: "usa uma API Node que ainda possui suporte parcial",
        },
        Rule {
            patterns: &[
                "node:worker_threads",
                "require('worker_threads')",
                "require(\"worker_threads\")",
            ],
            compatibility: Compatibility::Limited,
            message: "usa worker_threads; o shim da thread principal está disponível, mas workers isolados ainda não",
        },
        Rule {
            patterns: &["node:module", "require('module')", "require(\"module\")"],
            compatibility: Compatibility::Limited,
            message: "usa node:module; os auxiliares principais estão disponíveis",
        },
        Rule {
            patterns: &["node:crypto", "require('crypto')", "require(\"crypto\")"],
            compatibility: Compatibility::Limited,
            message: "usa crypto; hashes, HMAC e geração aleatória estão disponíveis, mas criptografia avançada ainda não",
        },
        Rule {
            patterns: &["node:zlib", "require('zlib')", "require(\"zlib\")"],
            compatibility: Compatibility::Limited,
            message: "usa zlib; gzip, deflate e Brotli estão disponíveis, mas opções avançadas ainda não",
        },
    ];
    let mut findings = Vec::new();

    for rule in rules {
        if rule.patterns.iter().any(|pattern| source.contains(pattern)) {
            findings.push(Finding {
                file: path.to_path_buf(),
                compatibility: rule.compatibility,
                message: rule.message.to_string(),
            });
        }
    }

    if source.lines().any(|line| {
        let line = line.trim_start();
        line.starts_with("import ") || line.starts_with("export ")
    }) {
        findings.push(Finding {
            file: path.to_path_buf(),
            compatibility: Compatibility::Limited,
            message: "usa sintaxe ESM; o suporte nativo ainda é parcial".into(),
        });
    }

    findings
}

fn extract_specifiers(source: &str) -> Vec<String> {
    let mut specifiers = Vec::new();

    for marker in ["require(", "from ", "import "] {
        let mut remainder = source;
        while let Some(index) = remainder.find(marker) {
            remainder = &remainder[index + marker.len()..];
            let candidate = remainder.trim_start();
            let Some(quote) = candidate
                .chars()
                .next()
                .filter(|char| *char == '\'' || *char == '"')
            else {
                continue;
            };
            let after_quote = &candidate[quote.len_utf8()..];
            let Some(end) = after_quote.find(quote) else {
                continue;
            };
            let specifier = after_quote[..end].to_string();
            if !specifiers.contains(&specifier) {
                specifiers.push(specifier);
            }
        }
    }

    specifiers
}

fn strip_js_comments(source: &str) -> String {
    let mut output = String::with_capacity(source.len());
    let mut characters = source.chars().peekable();
    let mut quote = None;

    while let Some(character) = characters.next() {
        if let Some(delimiter) = quote {
            output.push(character);
            if character == '\\' {
                if let Some(escaped) = characters.next() {
                    output.push(escaped);
                }
            } else if character == delimiter {
                quote = None;
            }
            continue;
        }

        if matches!(character, '\'' | '"' | '`') {
            quote = Some(character);
            output.push(character);
            continue;
        }

        if character == '/' && characters.peek() == Some(&'/') {
            characters.next();
            for comment_character in characters.by_ref() {
                if comment_character == '\n' {
                    output.push('\n');
                    break;
                }
            }
            continue;
        }

        if character == '/' && characters.peek() == Some(&'*') {
            characters.next();
            let mut previous = '\0';
            for comment_character in characters.by_ref() {
                if comment_character == '\n' {
                    output.push('\n');
                }
                if previous == '*' && comment_character == '/' {
                    break;
                }
                previous = comment_character;
            }
            continue;
        }

        output.push(character);
    }

    output
}

fn is_node_builtin(specifier: &str) -> bool {
    let name = specifier.strip_prefix("node:").unwrap_or(specifier);
    NODE_BUILTINS.contains(&name)
}

fn unsupported_node_builtin(specifier: &str) -> Option<String> {
    let name = specifier.strip_prefix("node:").unwrap_or(specifier);
    matches!(name, "child_process" | "http2")
        .then(|| format!("usa {name}, que ainda não é suportado"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

    fn fixture() -> PathBuf {
        let id = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let sequence = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "canaryo-analyzer-{}-{id}-{sequence}",
            std::process::id()
        ));
        fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn extracts_commonjs_and_esm_specifiers() {
        let source = "const a = require('./a'); import b from \"./b.js\";";

        assert_eq!(extract_specifiers(source), ["./a", "./b.js"]);
    }

    #[test]
    fn ignores_import_examples_inside_comments() {
        let source = r#"
            // require('commented-package')
            /* import example from "documentation-package" */
            const actual = require('./actual');
            const url = "https://example.com/path";
        "#;

        assert_eq!(extract_specifiers(&strip_js_comments(source)), ["./actual"]);
    }

    #[test]
    fn analyzes_local_dependencies_recursively() {
        let root = fixture();
        fs::write(root.join("main.js"), "require('./nested')").unwrap();
        fs::write(root.join("nested.js"), "require('fs')").unwrap();

        let report = analyze_file(root.join("main.js").to_str().unwrap()).unwrap();

        assert_eq!(report.analyzed_files, 2);
        assert_eq!(report.compatibility, Compatibility::Limited);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn reports_missing_dependencies() {
        let root = fixture();
        fs::write(root.join("main.js"), "require('missing-package')").unwrap();

        let report = analyze_file(root.join("main.js").to_str().unwrap()).unwrap();

        assert_eq!(report.compatibility, Compatibility::Limited);
        assert!(report.findings[0].message.contains("missing-package"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_unsupported_builtins_in_commonjs_and_esm() {
        let root = fixture();
        fs::write(
            root.join("main.js"),
            "import http2 from 'node:http2'; require('child_process');",
        )
        .unwrap();

        let report = analyze_file(root.join("main.js").to_str().unwrap()).unwrap();

        assert_eq!(report.compatibility, Compatibility::Incompatible);
        assert!(
            report
                .findings
                .iter()
                .any(|finding| finding.message.contains("http2"))
        );
        assert!(
            report
                .findings
                .iter()
                .any(|finding| finding.message.contains("child_process"))
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn handles_circular_dependencies_once() {
        let root = fixture();
        fs::write(root.join("a.js"), "require('./b')").unwrap();
        fs::write(root.join("b.js"), "require('./a')").unwrap();

        let report = analyze_file(root.join("a.js").to_str().unwrap()).unwrap();

        assert_eq!(report.analyzed_files, 2);
        fs::remove_dir_all(root).unwrap();
    }
}
