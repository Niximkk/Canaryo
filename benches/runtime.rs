use std::{
    env,
    io::{BufRead, BufReader, Read, Write},
    net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener, TcpStream},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{Arc, Barrier},
    thread,
    time::{Duration, Instant},
};

const CLOSE_REQUEST: &[u8] = b"GET / HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n";
const KEEP_ALIVE_REQUEST: &[u8] = b"GET / HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n";

#[derive(Clone, Copy)]
enum Runtime {
    Node,
    Canaryo,
}

impl Runtime {
    fn name(self) -> &'static str {
        match self {
            Self::Node => "Node.js",
            Self::Canaryo => "Canaryo",
        }
    }
}

struct Config {
    duration: Duration,
    runs: usize,
    startup_runs: usize,
    startup_only: bool,
    keep_alive: bool,
    body_size: Option<usize>,
    case_filter: Option<String>,
}

struct Server(Child);

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

struct LoadResult {
    errors: usize,
    requests_per_second: f64,
    p50_ms: f64,
    p95_ms: f64,
    p99_ms: f64,
}

struct Sample {
    load: LoadResult,
    memory_mib: Option<f64>,
}

struct PersistentClient {
    stream: BufReader<TcpStream>,
}

fn main() {
    let config = parse_config();
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let canaryo = canaryo_binary();

    if !canaryo.is_file() {
        panic!(
            "release binary not found at {}; run cargo build --release first",
            canaryo.display()
        );
    }

    let cases = [
        ("node:http", "fixtures/http-basic/server.js"),
        ("Express 5.2.1", "fixtures/express-basic/server.js"),
        ("Fastify 5.12.3", "fixtures/fastify-basic/server.js"),
    ]
    .into_iter()
    .filter(|(label, _)| {
        config
            .case_filter
            .as_ref()
            .is_none_or(|filter| label.to_ascii_lowercase().contains(filter))
    })
    .collect::<Vec<_>>();
    assert!(!cases.is_empty(), "no benchmark case matched --case");

    println!("# Canaryo benchmark\n");
    println!("- OS: {} {}", os_name(), env::consts::ARCH);
    println!("- CPU: {}", cpu_name());
    println!("- Logical processors: {}", logical_processors());
    println!("- Node.js: {}", command_version("node", "--version"));
    println!("- Canaryo: {}", command_version(&canaryo, "--version"));
    println!("- Samples per result: {}", config.runs);
    println!("- Startup samples: {}", config.startup_runs);
    println!(
        "- Load duration per sample: {} s",
        config.duration.as_secs()
    );
    println!(
        "- HTTP mode: {}\n",
        if config.keep_alive {
            "persistent connection per worker"
        } else {
            "new TCP connection per request"
        }
    );
    if let Some(body_size) = config.body_size {
        println!("- Request: POST /echo with a {body_size}-byte JSON body\n");
    }

    println!("## Startup\n");
    println!("| Application | Runtime | Median | Minimum | Maximum |");
    println!("|---|---:|---:|---:|---:|");
    for &(label, fixture) in &cases {
        for runtime in [Runtime::Node, Runtime::Canaryo] {
            let mut times = Vec::with_capacity(config.startup_runs);
            for _ in 0..config.startup_runs {
                times.push(measure_startup(runtime, root, fixture, &canaryo));
            }
            times.sort_unstable();
            println!(
                "| {label} | {} | {:.2} ms | {:.2} ms | {:.2} ms |",
                runtime.name(),
                duration_ms(times[times.len() / 2]),
                duration_ms(times[0]),
                duration_ms(*times.last().unwrap())
            );
        }
    }

    if config.startup_only {
        return;
    }

    for concurrency in [1, 16] {
        println!("\n## HTTP throughput, concurrency {concurrency}\n");
        println!("| Application | Runtime | Requests/s | p50 | p95 | p99 | Errors | RSS |");
        println!("|---|---:|---:|---:|---:|---:|---:|---:|");

        for &(label, fixture) in &cases {
            for runtime in [Runtime::Node, Runtime::Canaryo] {
                let mut samples = Vec::with_capacity(config.runs);
                for _ in 0..config.runs {
                    samples.push(measure_load(
                        runtime,
                        root,
                        fixture,
                        &canaryo,
                        concurrency,
                        &config,
                    ));
                }
                samples.sort_by(|a, b| {
                    a.load
                        .requests_per_second
                        .total_cmp(&b.load.requests_per_second)
                });
                let sample = &samples[samples.len() / 2];
                let memory = sample
                    .memory_mib
                    .map(|value| format!("{value:.1} MiB"))
                    .unwrap_or_else(|| "n/a".into());
                println!(
                    "| {label} | {} | {:.0} | {:.2} ms | {:.2} ms | {:.2} ms | {} | {memory} |",
                    runtime.name(),
                    sample.load.requests_per_second,
                    sample.load.p50_ms,
                    sample.load.p95_ms,
                    sample.load.p99_ms,
                    sample.load.errors
                );
            }
        }
    }
}

fn parse_config() -> Config {
    let mut config = Config {
        duration: Duration::from_secs(5),
        runs: 3,
        startup_runs: 7,
        startup_only: false,
        keep_alive: false,
        body_size: None,
        case_filter: None,
    };
    let arguments: Vec<String> = env::args().skip(1).collect();
    let mut index = 0;

    while index < arguments.len() {
        if arguments[index] == "--bench" {
            index += 1;
            continue;
        }
        if arguments[index] == "--startup-only" {
            config.startup_only = true;
            index += 1;
            continue;
        }
        if arguments[index] == "--keep-alive" {
            config.keep_alive = true;
            index += 1;
            continue;
        }
        let value = arguments
            .get(index + 1)
            .unwrap_or_else(|| panic!("missing value for {}", arguments[index]));
        match arguments[index].as_str() {
            "--duration" => config.duration = Duration::from_secs(parse_number(value)),
            "--runs" => config.runs = parse_number(value),
            "--startup-runs" => config.startup_runs = parse_number(value),
            "--case" => config.case_filter = Some(value.to_ascii_lowercase()),
            "--body-size" => config.body_size = Some(parse_number(value)),
            option => panic!("unknown option: {option}"),
        }
        index += 2;
    }

    assert!(config.duration.as_secs() > 0, "duration must be positive");
    assert!(config.runs > 0, "runs must be positive");
    assert!(config.startup_runs > 0, "startup runs must be positive");
    config
}

fn parse_number<T>(value: &str) -> T
where
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{
    value
        .parse()
        .unwrap_or_else(|error| panic!("invalid number {value}: {error}"))
}

fn canaryo_binary() -> PathBuf {
    let suffix = env::consts::EXE_SUFFIX;
    env::current_exe()
        .unwrap()
        .parent()
        .and_then(Path::parent)
        .unwrap()
        .join(format!("canaryo{suffix}"))
}

fn free_port() -> u16 {
    TcpListener::bind(("127.0.0.1", 0))
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

fn spawn_server(runtime: Runtime, root: &Path, fixture: &str, canaryo: &Path, port: u16) -> Server {
    let executable = match runtime {
        Runtime::Node => Path::new("node"),
        Runtime::Canaryo => canaryo,
    };
    let child = Command::new(executable)
        .current_dir(root)
        .arg(fixture)
        .arg(port.to_string())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap_or_else(|error| panic!("failed to start {}: {error}", runtime.name()));
    Server(child)
}

fn wait_until_ready(server: &mut Server, port: u16) -> Duration {
    let started = Instant::now();
    let deadline = started + Duration::from_secs(15);

    loop {
        if readiness_request(port).is_ok() {
            return started.elapsed();
        }
        if let Some(status) = server.0.try_wait().unwrap() {
            panic!("server exited before listening: {status}");
        }
        if Instant::now() >= deadline {
            panic!("server did not listen on port {port}");
        }
        thread::sleep(Duration::from_millis(2));
    }
}

fn measure_startup(runtime: Runtime, root: &Path, fixture: &str, canaryo: &Path) -> Duration {
    let port = free_port();
    let mut server = spawn_server(runtime, root, fixture, canaryo, port);
    wait_until_ready(&mut server, port)
}

fn measure_load(
    runtime: Runtime,
    root: &Path,
    fixture: &str,
    canaryo: &Path,
    concurrency: usize,
    config: &Config,
) -> Sample {
    let port = free_port();
    let mut server = spawn_server(runtime, root, fixture, canaryo, port);
    wait_until_ready(&mut server, port);
    let _ = run_load(
        port,
        concurrency,
        Duration::from_millis(500),
        config.keep_alive,
        config.body_size,
    );
    let memory_mib = process_rss_mib(server.0.id());
    let load = run_load(
        port,
        concurrency,
        config.duration,
        config.keep_alive,
        config.body_size,
    );

    if let Some(status) = server.0.try_wait().unwrap() {
        panic!("{} exited during load: {status}", runtime.name());
    }

    Sample { load, memory_mib }
}

fn run_load(
    port: u16,
    concurrency: usize,
    duration: Duration,
    keep_alive: bool,
    body_size: Option<usize>,
) -> LoadResult {
    let barrier = Arc::new(Barrier::new(concurrency + 1));
    let request = Arc::new(load_request(keep_alive, body_size));
    let mut workers = Vec::with_capacity(concurrency);

    for _ in 0..concurrency {
        let barrier = Arc::clone(&barrier);
        let request = Arc::clone(&request);
        workers.push(thread::spawn(move || {
            let mut persistent = keep_alive.then(|| persistent_connection(port));
            barrier.wait();
            let deadline = Instant::now() + duration;
            let mut latencies = Vec::new();
            let mut errors = 0;

            while Instant::now() < deadline {
                let started = Instant::now();
                let result = if let Some(stream) = persistent.as_mut() {
                    persistent_request(stream, &request)
                } else {
                    close_request(port, &request)
                };
                match result {
                    Ok(()) => latencies.push(started.elapsed()),
                    Err(()) => {
                        errors += 1;
                        if keep_alive {
                            persistent = Some(persistent_connection(port));
                        }
                    }
                }
            }
            (latencies, errors)
        }));
    }

    let started = Instant::now();
    barrier.wait();
    let mut latencies = Vec::new();
    let mut errors = 0;
    for worker in workers {
        let (mut worker_latencies, worker_errors) = worker.join().unwrap();
        latencies.append(&mut worker_latencies);
        errors += worker_errors;
    }
    let elapsed = started.elapsed();
    latencies.sort_unstable();

    LoadResult {
        errors,
        requests_per_second: latencies.len() as f64 / elapsed.as_secs_f64(),
        p50_ms: percentile_ms(&latencies, 0.50),
        p95_ms: percentile_ms(&latencies, 0.95),
        p99_ms: percentile_ms(&latencies, 0.99),
    }
}

fn close_request(port: u16, request: &[u8]) -> Result<(), ()> {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).map_err(|_| ())?;
    exchange_close_request(&mut stream, request)
}

fn readiness_request(port: u16) -> Result<(), ()> {
    let address = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port);
    let mut stream =
        TcpStream::connect_timeout(&address, Duration::from_millis(5)).map_err(|_| ())?;
    exchange_close_request(&mut stream, CLOSE_REQUEST)
}

fn exchange_close_request(stream: &mut TcpStream, request: &[u8]) -> Result<(), ()> {
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .map_err(|_| ())?;
    stream.write_all(request).map_err(|_| ())?;
    let mut response = Vec::new();
    stream.read_to_end(&mut response).map_err(|_| ())?;

    if response.starts_with(b"HTTP/1.1 200") {
        Ok(())
    } else {
        Err(())
    }
}

fn persistent_connection(port: u16) -> PersistentClient {
    let stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    stream.set_nodelay(true).unwrap();
    PersistentClient {
        stream: BufReader::new(stream),
    }
}

fn persistent_request(client: &mut PersistentClient, request: &[u8]) -> Result<(), ()> {
    client.stream.get_mut().write_all(request).map_err(|_| ())?;
    let mut line = String::new();
    client.stream.read_line(&mut line).map_err(|_| ())?;
    if !line.starts_with("HTTP/1.1 200") {
        return Err(());
    }

    let mut content_length = None;
    let mut chunked = false;
    loop {
        line.clear();
        client.stream.read_line(&mut line).map_err(|_| ())?;
        if line == "\r\n" {
            break;
        }
        let (name, value) = line.split_once(':').ok_or(())?;
        if name.eq_ignore_ascii_case("content-length") {
            content_length = Some(value.trim().parse::<usize>().map_err(|_| ())?);
        }
        if name.eq_ignore_ascii_case("transfer-encoding")
            && value
                .split(',')
                .any(|value| value.trim().eq_ignore_ascii_case("chunked"))
        {
            chunked = true;
        }
    }

    if let Some(content_length) = content_length {
        read_exact_bytes(&mut client.stream, content_length)
    } else if chunked {
        read_chunked_body(&mut client.stream)
    } else {
        Err(())
    }
}

fn load_request(keep_alive: bool, body_size: Option<usize>) -> Vec<u8> {
    let Some(body_size) = body_size else {
        return if keep_alive {
            KEEP_ALIVE_REQUEST.to_vec()
        } else {
            CLOSE_REQUEST.to_vec()
        };
    };
    let body_size = body_size.max(11);
    let body = format!("{{\"data\":\"{}\"}}", "x".repeat(body_size - 11));
    let connection = if keep_alive { "keep-alive" } else { "close" };
    format!(
        "POST /echo HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: {connection}\r\n\r\n{body}",
        body.len()
    )
    .into_bytes()
}

fn read_exact_bytes(reader: &mut impl Read, length: usize) -> Result<(), ()> {
    let mut remaining = length;
    let mut buffer = [0; 4096];

    while remaining > 0 {
        let size = remaining.min(buffer.len());
        reader.read_exact(&mut buffer[..size]).map_err(|_| ())?;
        remaining -= size;
    }
    Ok(())
}

fn read_chunked_body(reader: &mut impl BufRead) -> Result<(), ()> {
    let mut line = String::new();

    loop {
        line.clear();
        reader.read_line(&mut line).map_err(|_| ())?;
        let size = line
            .trim_end()
            .split(';')
            .next()
            .and_then(|value| usize::from_str_radix(value, 16).ok())
            .ok_or(())?;
        if size == 0 {
            line.clear();
            reader.read_line(&mut line).map_err(|_| ())?;
            return (line == "\r\n").then_some(()).ok_or(());
        }
        read_exact_bytes(reader, size + 2)?;
    }
}

fn percentile_ms(values: &[Duration], percentile: f64) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let index = ((values.len() - 1) as f64 * percentile).round() as usize;
    duration_ms(values[index])
}

fn duration_ms(value: Duration) -> f64 {
    value.as_secs_f64() * 1000.0
}

fn process_rss_mib(process_id: u32) -> Option<f64> {
    let bytes = if cfg!(target_os = "windows") {
        let output = Command::new("powershell")
            .args([
                "-NoProfile",
                "-Command",
                &format!("(Get-Process -Id {process_id}).WorkingSet64"),
            ])
            .output()
            .ok()?;
        String::from_utf8(output.stdout)
            .ok()?
            .trim()
            .parse::<u64>()
            .ok()?
    } else if cfg!(target_os = "linux") {
        let status = std::fs::read_to_string(format!("/proc/{process_id}/status")).ok()?;
        let kib = status
            .lines()
            .find(|line| line.starts_with("VmRSS:"))?
            .split_whitespace()
            .nth(1)?
            .parse::<u64>()
            .ok()?;
        kib * 1024
    } else {
        let output = Command::new("ps")
            .args(["-o", "rss=", "-p", &process_id.to_string()])
            .output()
            .ok()?;
        let kib = String::from_utf8(output.stdout)
            .ok()?
            .trim()
            .parse::<u64>()
            .ok()?;
        kib * 1024
    };

    Some(bytes as f64 / 1024.0 / 1024.0)
}

fn cpu_name() -> String {
    if cfg!(target_os = "windows")
        && let Ok(output) = Command::new("powershell")
            .args([
                "-NoProfile",
                "-Command",
                "(Get-ItemProperty 'HKLM:\\HARDWARE\\DESCRIPTION\\System\\CentralProcessor\\0').ProcessorNameString",
            ])
            .output()
        && output.status.success()
        && let Ok(name) = String::from_utf8(output.stdout)
        && !name.trim().is_empty()
    {
        return name.trim().to_string();
    }

    env::var("PROCESSOR_IDENTIFIER")
        .or_else(|_| env::var("HOSTTYPE"))
        .unwrap_or_else(|_| "unknown".into())
}

fn os_name() -> String {
    if cfg!(target_os = "windows")
        && let Ok(output) = Command::new("powershell")
            .args([
                "-NoProfile",
                "-Command",
                "[Environment]::OSVersion.Version.Build",
            ])
            .output()
        && output.status.success()
        && let Ok(build) = String::from_utf8(output.stdout)
        && let Ok(build) = build.trim().parse::<u32>()
    {
        let version = if build >= 22_000 {
            "Windows 11"
        } else {
            "Windows 10"
        };
        return format!("{version} build {build}");
    }

    env::consts::OS.into()
}

fn logical_processors() -> usize {
    thread::available_parallelism()
        .map(usize::from)
        .unwrap_or(1)
}

fn command_version(executable: impl AsRef<Path>, argument: &str) -> String {
    Command::new(executable.as_ref())
        .arg(argument)
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|version| version.trim().to_string())
        .unwrap_or_else(|| "unknown".into())
}
