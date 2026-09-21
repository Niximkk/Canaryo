use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::io::Read;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

const RESULTS_PATH: &str = "compat/results/latest.json";
const REPORT_PATH: &str = "COMPATIBILITY_REPORT.md";

#[derive(Debug)]
struct Manifest {
    probe: PathBuf,
    modules: Vec<String>,
    cases: Vec<CompatCase>,
}

#[derive(Debug)]
struct CompatCase {
    id: String,
    module: String,
    fixture: PathBuf,
    timeout_ms: u64,
    source: String,
    expected: String,
}

#[derive(Clone, Debug)]
struct Runtime {
    name: &'static str,
    executable: PathBuf,
}

#[derive(Debug)]
struct Capture {
    exit_code: i32,
    timed_out: bool,
    stdout: String,
    stderr: String,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("xtask: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let args: Vec<String> = env::args().skip(1).collect();

    match args.as_slice() {
        [group, command] if group == "compat" && command == "probe" => {
            generate_results(&root, None)
        }
        [group, command] if group == "compat" && command == "run" => generate_results(&root, None),
        [group, command, pattern] if group == "compat" && command == "run" => {
            generate_results(&root, Some(pattern))
        }
        [group, command] if group == "compat" && command == "report" => regenerate_report(&root),
        _ => Err(usage()),
    }
}

fn usage() -> String {
    [
        "usage:",
        "  cargo xtask compat probe",
        "  cargo xtask compat run [pattern]",
        "  cargo xtask compat report",
    ]
    .join("\n")
}

fn generate_results(root: &Path, pattern: Option<&str>) -> Result<(), String> {
    let manifest = load_manifest(root)?;
    let canaryo = Runtime {
        name: "Canaryo",
        executable: build_canaryo(root)?,
    };
    let node = Runtime {
        name: "Node.js",
        executable: PathBuf::from("node"),
    };
    let bun = command_available("bun").then(|| Runtime {
        name: "Bun",
        executable: PathBuf::from("bun"),
    });

    if !command_available("node") {
        return Err("Node.js 22 or newer is required as the compatibility oracle".into());
    }

    let modules_json = serde_json::to_string(&manifest.modules)
        .map_err(|error| format!("cannot encode module list: {error}"))?;
    let probe_timeout = Duration::from_secs(15);
    let probe_path = root.join(&manifest.probe);
    let probe_env = [("CANARYO_COMPAT_MODULES", modules_json.as_str())];

    println!("compat: probing {} builtin modules", manifest.modules.len());
    let node_probe = execute(&node, &probe_path, root, probe_timeout, &probe_env)?;
    let canaryo_probe = execute(&canaryo, &probe_path, root, probe_timeout, &probe_env)?;
    let bun_probe = bun
        .as_ref()
        .map(|runtime| execute(runtime, &probe_path, root, probe_timeout, &probe_env))
        .transpose()?;

    let node_surface = parse_json_capture(&node_probe, node.name)?;
    let canaryo_surface = parse_json_capture(&canaryo_probe, canaryo.name)?;
    let bun_surface = bun_probe
        .as_ref()
        .map(|capture| parse_json_capture(capture, "Bun"))
        .transpose()?;

    let mut surface_rows = Vec::new();
    let mut matched_exports = 0usize;
    let mut node_exports = 0usize;

    for module in &manifest.modules {
        let reference = exported_names(&node_surface, module);
        let actual = exported_names(&canaryo_surface, module);
        let matched: Vec<_> = reference.intersection(&actual).cloned().collect();
        let missing: Vec<_> = reference.difference(&actual).cloned().collect();
        let extra: Vec<_> = actual.difference(&reference).cloned().collect();
        let total = reference.len();

        matched_exports += matched.len();
        node_exports += total;

        surface_rows.push(json!({
            "module": module,
            "nodeLoaded": module_loaded(&node_surface, module),
            "canaryoLoaded": module_loaded(&canaryo_surface, module),
            "bunLoaded": bun_surface.as_ref().map(|value| module_loaded(value, module)),
            "matched": matched.len(),
            "total": total,
            "percent": percent(matched.len(), total),
            "missing": missing,
            "extra": extra,
        }));
    }

    let selected: Vec<_> = manifest
        .cases
        .iter()
        .filter(|case| case_matches(case, pattern))
        .collect();
    if selected.is_empty() {
        return Err(format!(
            "no compatibility case matches {:?}",
            pattern.unwrap_or("")
        ));
    }

    println!("compat: running {} behavioral cases", selected.len());
    let mut case_rows = Vec::new();
    let mut passed = 0usize;
    let mut unexpected_failures = 0usize;

    for case in selected {
        let fixture = root.join(&case.fixture);
        let timeout = Duration::from_millis(case.timeout_ms);
        let node_capture = execute_case(&node, &fixture, root, timeout)?;
        let canaryo_capture = execute_case(&canaryo, &fixture, root, timeout)?;
        let bun_capture = bun
            .as_ref()
            .map(|runtime| execute_case(runtime, &fixture, root, timeout))
            .transpose()?;
        let canaryo_matches = captures_match(&node_capture, &canaryo_capture);
        let bun_matches = bun_capture
            .as_ref()
            .map(|capture| captures_match(&node_capture, capture));

        let status = match (case.expected.as_str(), canaryo_matches) {
            ("pass", true) => {
                passed += 1;
                "pass"
            }
            ("pass", false) => {
                unexpected_failures += 1;
                "fail"
            }
            (_, true) => "unexpected-pass",
            _ => "expected-failure",
        };
        println!("  {status:>16}  {}", case.id);
        if status == "fail" {
            println!("    Node.js: {}", node_capture.stdout);
            println!("    Canaryo: {}", canaryo_capture.stdout);
            if node_capture.exit_code != canaryo_capture.exit_code {
                println!(
                    "    exit codes: Node.js={}, Canaryo={}",
                    node_capture.exit_code, canaryo_capture.exit_code
                );
            }
        }

        case_rows.push(json!({
            "id": case.id,
            "module": case.module,
            "fixture": path_for_json(&case.fixture),
            "source": case.source,
            "expected": case.expected,
            "status": status,
            "canaryoMatchesNode": canaryo_matches,
            "bunMatchesNode": bun_matches,
            "node": capture_json(&node_capture),
            "canaryo": capture_json(&canaryo_capture),
            "bun": bun_capture.as_ref().map(capture_json),
        }));
    }

    let result = json!({
        "schemaVersion": 1,
        "generatedBy": "cargo xtask compat",
        "baseline": {
            "node": runtime_version("node"),
            "bun": bun.as_ref().map(|_| runtime_version("bun")),
            "canaryo": env!("CARGO_PKG_VERSION"),
        },
        "summary": {
            "surface": {
                "matched": matched_exports,
                "total": node_exports,
                "percent": percent(matched_exports, node_exports),
            },
            "behavior": {
                "passed": passed,
                "total": case_rows.len(),
                "unexpectedFailures": unexpected_failures,
            }
        },
        "surface": surface_rows,
        "cases": case_rows,
    });

    write_results(root, &result)?;
    write_report(root, &result)?;
    println!(
        "compat: {matched_exports}/{node_exports} exports ({:.1}%), {passed}/{} cases matched Node.js",
        percent(matched_exports, node_exports),
        result["cases"].as_array().map_or(0, Vec::len)
    );
    println!("compat: wrote {RESULTS_PATH} and {REPORT_PATH}");

    if unexpected_failures > 0 {
        Err(format!(
            "{unexpected_failures} required compatibility case(s) failed"
        ))
    } else {
        Ok(())
    }
}

fn regenerate_report(root: &Path) -> Result<(), String> {
    let path = root.join(RESULTS_PATH);
    let source = fs::read_to_string(&path)
        .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    let result: Value = serde_json::from_str(&source)
        .map_err(|error| format!("cannot parse {}: {error}", path.display()))?;
    write_report(root, &result)?;
    println!("compat: wrote {REPORT_PATH}");
    Ok(())
}

fn load_manifest(root: &Path) -> Result<Manifest, String> {
    let path = root.join("compat/manifest.json");
    let source = fs::read_to_string(&path)
        .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    let value: Value = serde_json::from_str(&source)
        .map_err(|error| format!("cannot parse {}: {error}", path.display()))?;

    let probe = required_string(&value, "probe")?.into();
    let modules = value["modules"]
        .as_array()
        .ok_or_else(|| "manifest field modules must be an array".to_string())?
        .iter()
        .map(|item| {
            item.as_str()
                .map(str::to_owned)
                .ok_or_else(|| "manifest module names must be strings".to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;
    let cases = value["cases"]
        .as_array()
        .ok_or_else(|| "manifest field cases must be an array".to_string())?
        .iter()
        .map(|case| {
            Ok(CompatCase {
                id: required_string(case, "id")?,
                module: required_string(case, "module")?,
                fixture: required_string(case, "fixture")?.into(),
                timeout_ms: case["timeoutMs"]
                    .as_u64()
                    .ok_or_else(|| "case timeoutMs must be an unsigned integer".to_string())?,
                source: required_string(case, "source")?,
                expected: required_string(case, "expected")?,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;

    Ok(Manifest {
        probe,
        modules,
        cases,
    })
}

fn required_string(value: &Value, field: &str) -> Result<String, String> {
    value[field]
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| format!("manifest field {field} must be a string"))
}

fn build_canaryo(root: &Path) -> Result<PathBuf, String> {
    println!("compat: building Canaryo");
    let cargo = env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let status = Command::new(cargo)
        .current_dir(root)
        .args(["build", "--quiet", "--bin", "canaryo"])
        .status()
        .map_err(|error| format!("cannot start cargo build: {error}"))?;
    if !status.success() {
        return Err("cargo build --bin canaryo failed".into());
    }

    let target = env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("target"));
    let target = if target.is_absolute() {
        target
    } else {
        root.join(target)
    };
    let executable = if cfg!(windows) {
        "canaryo.exe"
    } else {
        "canaryo"
    };
    Ok(target.join("debug").join(executable))
}

fn command_available(command: &str) -> bool {
    Command::new(command)
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

fn runtime_version(command: &str) -> Option<String> {
    Command::new(command)
        .arg("--version")
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

fn execute(
    runtime: &Runtime,
    fixture: &Path,
    root: &Path,
    timeout: Duration,
    environment: &[(&str, &str)],
) -> Result<Capture, String> {
    let mut command = Command::new(&runtime.executable);
    command
        .arg(fixture)
        .current_dir(root)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (key, value) in environment {
        command.env(key, value);
    }

    let mut child = command.spawn().map_err(|error| {
        format!(
            "cannot start {} ({}): {error}",
            runtime.name,
            runtime.executable.display()
        )
    })?;
    let mut stdout = child
        .stdout
        .take()
        .ok_or_else(|| format!("cannot capture {} stdout", runtime.name))?;
    let mut stderr = child
        .stderr
        .take()
        .ok_or_else(|| format!("cannot capture {} stderr", runtime.name))?;
    let stdout_reader = thread::spawn(move || {
        let mut bytes = Vec::new();
        stdout.read_to_end(&mut bytes).map(|_| bytes)
    });
    let stderr_reader = thread::spawn(move || {
        let mut bytes = Vec::new();
        stderr.read_to_end(&mut bytes).map(|_| bytes)
    });

    let started = Instant::now();
    let (status, timed_out) = loop {
        if let Some(status) = child
            .try_wait()
            .map_err(|error| format!("cannot poll {}: {error}", runtime.name))?
        {
            break (status, false);
        }
        if started.elapsed() >= timeout {
            child
                .kill()
                .map_err(|error| format!("cannot stop timed-out {}: {error}", runtime.name))?;
            let status = child
                .wait()
                .map_err(|error| format!("cannot reap timed-out {}: {error}", runtime.name))?;
            break (status, true);
        }
        thread::sleep(Duration::from_millis(5));
    };

    let stdout = stdout_reader
        .join()
        .map_err(|_| format!("{} stdout reader panicked", runtime.name))?
        .map_err(|error| format!("cannot read {} stdout: {error}", runtime.name))?;
    let stderr = stderr_reader
        .join()
        .map_err(|_| format!("{} stderr reader panicked", runtime.name))?
        .map_err(|error| format!("cannot read {} stderr: {error}", runtime.name))?;

    Ok(Capture {
        exit_code: status.code().unwrap_or(-1),
        timed_out,
        stdout: normalize_output(&String::from_utf8_lossy(&stdout), root),
        stderr: normalize_output(&String::from_utf8_lossy(&stderr), root),
    })
}

fn execute_case(
    runtime: &Runtime,
    fixture: &Path,
    root: &Path,
    timeout: Duration,
) -> Result<Capture, String> {
    let listener = TcpListener::bind(("127.0.0.1", 0))
        .map_err(|error| format!("cannot reserve a compatibility port: {error}"))?;
    let port = listener
        .local_addr()
        .map_err(|error| format!("cannot read the compatibility port: {error}"))?
        .port()
        .to_string();
    drop(listener);

    execute(
        runtime,
        fixture,
        root,
        timeout,
        &[("CANARYO_COMPAT_PORT", port.as_str())],
    )
}

fn normalize_output(output: &str, root: &Path) -> String {
    let normalized = output.replace("\r\n", "\n");
    let root_native = root.to_string_lossy();
    let root_forward = root_native.replace('\\', "/");
    let normalized = normalized
        .replace(root_native.as_ref(), "<ROOT>")
        .replace(&root_forward, "<ROOT>");
    normalize_node_process_ids(&normalized)
        .trim_end()
        .to_owned()
}

fn normalize_node_process_ids(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut remaining = input;

    while let Some(marker) = remaining.find("(node:") {
        let digits_start = marker + "(node:".len();
        let suffix = &remaining[digits_start..];
        let digits = suffix.bytes().take_while(u8::is_ascii_digit).count();
        if digits == 0 || suffix.as_bytes().get(digits) != Some(&b')') {
            output.push_str(&remaining[..digits_start]);
            remaining = suffix;
            continue;
        }
        output.push_str(&remaining[..digits_start]);
        output.push_str("<PID>)");
        remaining = &suffix[digits + 1..];
    }
    output.push_str(remaining);
    output
}

fn parse_json_capture(capture: &Capture, runtime: &str) -> Result<Value, String> {
    if capture.timed_out {
        return Err(format!("{runtime} probe timed out"));
    }
    if capture.exit_code != 0 {
        return Err(format!(
            "{runtime} probe exited with {}: {}",
            capture.exit_code, capture.stderr
        ));
    }
    let line = capture
        .stdout
        .lines()
        .rev()
        .find(|line| !line.trim().is_empty())
        .ok_or_else(|| format!("{runtime} probe produced no output"))?;
    serde_json::from_str(line).map_err(|error| format!("cannot parse {runtime} probe: {error}"))
}

fn exported_names(surface: &Value, module: &str) -> BTreeSet<String> {
    surface[module]["exports"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect()
}

fn module_loaded(surface: &Value, module: &str) -> bool {
    surface[module]["loaded"].as_bool().unwrap_or(false)
}

fn captures_match(reference: &Capture, actual: &Capture) -> bool {
    reference.exit_code == 0
        && actual.exit_code == 0
        && !reference.timed_out
        && !actual.timed_out
        && outputs_match(&reference.stdout, &actual.stdout)
}

fn outputs_match(reference: &str, actual: &str) -> bool {
    if reference == actual {
        return true;
    }
    match (
        serde_json::from_str::<Value>(reference),
        serde_json::from_str::<Value>(actual),
    ) {
        (Ok(reference), Ok(actual)) => reference == actual,
        _ => false,
    }
}

fn case_matches(case: &CompatCase, pattern: Option<&str>) -> bool {
    pattern.is_none_or(|pattern| case.id.contains(pattern) || case.module.contains(pattern))
}

fn percent(part: usize, total: usize) -> f64 {
    if total == 0 {
        100.0
    } else {
        part as f64 * 100.0 / total as f64
    }
}

fn capture_json(capture: &Capture) -> Value {
    json!({
        "exitCode": capture.exit_code,
        "timedOut": capture.timed_out,
        "stdout": capture.stdout,
        "stderr": capture.stderr,
    })
}

fn path_for_json(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn write_results(root: &Path, result: &Value) -> Result<(), String> {
    let path = root.join(RESULTS_PATH);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("cannot create {}: {error}", parent.display()))?;
    }
    let mut encoded = serde_json::to_string_pretty(result)
        .map_err(|error| format!("cannot encode compatibility results: {error}"))?;
    encoded.push('\n');
    fs::write(&path, encoded).map_err(|error| format!("cannot write {}: {error}", path.display()))
}

fn write_report(root: &Path, result: &Value) -> Result<(), String> {
    let surface = &result["summary"]["surface"];
    let behavior = &result["summary"]["behavior"];
    let mut report = String::new();
    report.push_str("# Compatibility Report\n\n");
    report
        .push_str("Generated by `cargo xtask compat probe`. Node.js is the behavioral oracle.\n\n");
    report.push_str("## Baseline\n\n");
    report.push_str(&format!(
        "- Node.js: {}\n- Bun: {}\n- Canaryo: {}\n\n",
        display_value(&result["baseline"]["node"]),
        display_value(&result["baseline"]["bun"]),
        display_value(&result["baseline"]["canaryo"]),
    ));
    report.push_str("## Current score\n\n");
    report.push_str(&format!(
        "- Exported API surface: **{}/{} ({:.1}%)**\n- Required behavioral cases: **{}/{} passed**\n\n",
        surface["matched"].as_u64().unwrap_or(0),
        surface["total"].as_u64().unwrap_or(0),
        surface["percent"].as_f64().unwrap_or(0.0),
        behavior["passed"].as_u64().unwrap_or(0),
        behavior["total"].as_u64().unwrap_or(0),
    ));
    report.push_str("The export score measures discoverable names, not complete semantics. Behavioral cases provide the semantic evidence.\n\n");
    report.push_str("## Builtin surface\n\n");
    report.push_str("| Module | Loads | Matched exports | Coverage | Missing exports |\n");
    report.push_str("|---|---:|---:|---:|---|\n");
    for row in result["surface"].as_array().into_iter().flatten() {
        let loads = if row["canaryoLoaded"].as_bool().unwrap_or(false) {
            "Yes"
        } else {
            "No"
        };
        report.push_str(&format!(
            "| `{}` | {} | {}/{} | {:.1}% | {} |\n",
            markdown_cell(row["module"].as_str().unwrap_or("?")),
            loads,
            row["matched"].as_u64().unwrap_or(0),
            row["total"].as_u64().unwrap_or(0),
            row["percent"].as_f64().unwrap_or(0.0),
            summarized_names(&row["missing"]),
        ));
    }
    report.push_str("\n## Behavioral cases\n\n");
    report.push_str("| Case | Module | Canaryo | Bun | Source |\n");
    report.push_str("|---|---|---:|---:|---|\n");
    for row in result["cases"].as_array().into_iter().flatten() {
        report.push_str(&format!(
            "| `{}` | `{}` | {} | {} | {} |\n",
            markdown_cell(row["id"].as_str().unwrap_or("?")),
            markdown_cell(row["module"].as_str().unwrap_or("?")),
            status_label(row["status"].as_str().unwrap_or("fail")),
            optional_match_label(&row["bunMatchesNode"]),
            markdown_cell(row["source"].as_str().unwrap_or("?")),
        ));
    }
    report.push_str("\n## Reproduce\n\n```sh\ncargo xtask compat probe\ncargo xtask compat run path\ncargo xtask compat report\n```\n");

    let path = root.join(REPORT_PATH);
    fs::write(&path, report).map_err(|error| format!("cannot write {}: {error}", path.display()))
}

fn display_value(value: &Value) -> String {
    value.as_str().unwrap_or("not available").to_owned()
}

fn summarized_names(value: &Value) -> String {
    let names: Vec<_> = value
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect();
    if names.is_empty() {
        return "None".into();
    }
    let shown = names
        .iter()
        .take(10)
        .copied()
        .collect::<Vec<_>>()
        .join(", ");
    if names.len() > 10 {
        format!("{} (+{})", markdown_cell(&shown), names.len() - 10)
    } else {
        markdown_cell(&shown)
    }
}

fn markdown_cell(value: &str) -> String {
    value.replace('|', "\\|").replace('\n', " ")
}

fn status_label(status: &str) -> &'static str {
    match status {
        "pass" => "Pass",
        "expected-failure" => "Known gap",
        "unexpected-pass" => "Unexpected pass",
        _ => "Fail",
    }
}

fn optional_match_label(value: &Value) -> &'static str {
    match value.as_bool() {
        Some(true) => "Pass",
        Some(false) => "Differs",
        None => "Not installed",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_normalization_removes_platform_and_workspace_noise() {
        let root = Path::new(r"D:\Projects\Canaryo");
        let output = "D:\\Projects\\Canaryo\\src\\case.js\r\n(node:1234) warning\r\n";
        assert_eq!(
            normalize_output(output, root),
            "<ROOT>\\src\\case.js\n(node:<PID>) warning"
        );
    }

    #[test]
    fn case_filter_matches_id_or_module() {
        let case = CompatCase {
            id: "path-platform-variants".into(),
            module: "path".into(),
            fixture: "case.js".into(),
            timeout_ms: 100,
            source: "canaryo".into(),
            expected: "pass".into(),
        };
        assert!(case_matches(&case, None));
        assert!(case_matches(&case, Some("platform")));
        assert!(case_matches(&case, Some("path")));
        assert!(!case_matches(&case, Some("stream")));
    }

    #[test]
    fn json_outputs_match_without_depending_on_object_key_order() {
        assert!(outputs_match(
            r#"{"first":1,"second":{"left":2,"right":3}}"#,
            r#"{"second":{"right":3,"left":2},"first":1}"#,
        ));
        assert!(!outputs_match(r#"{"value":1}"#, r#"{"value":2}"#));
        assert!(!outputs_match("first", "second"));
    }

    #[test]
    fn matching_failures_do_not_count_as_compatibility_passes() {
        let failed = Capture {
            exit_code: 1,
            timed_out: false,
            stdout: "same failure".into(),
            stderr: String::new(),
        };
        let successful = Capture {
            exit_code: 0,
            timed_out: false,
            stdout: "same output".into(),
            stderr: String::new(),
        };
        let timed_out = Capture {
            exit_code: 0,
            timed_out: true,
            stdout: "same output".into(),
            stderr: String::new(),
        };

        assert!(!captures_match(&failed, &failed));
        assert!(captures_match(&successful, &successful));
        assert!(!captures_match(&timed_out, &successful));
    }
}
