use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    path::Path,
    process::{Child, Command, Stdio},
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};

use base64::Engine;

static SERVER_START: Mutex<()> = Mutex::new(());

struct Server(Child);

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn free_port() -> u16 {
    TcpListener::bind(("127.0.0.1", 0))
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

fn start_fixture(fixture: &str) -> (Server, TcpStream) {
    start_fixture_with_args(fixture, &[], Stdio::null())
}

fn start_fixture_with_stdout(fixture: &str, stdout: Stdio) -> (Server, TcpStream) {
    start_fixture_with_args(fixture, &[], stdout)
}

fn start_fixture_with_args(
    fixture: &str,
    arguments: &[String],
    stdout: Stdio,
) -> (Server, TcpStream) {
    let start_guard = SERVER_START.lock().unwrap();
    let port = free_port();
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut server = Server(
        Command::new(env!("CARGO_BIN_EXE_canaryo"))
            .current_dir(root)
            .arg(fixture)
            .arg(port.to_string())
            .args(arguments)
            .stdout(stdout)
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap(),
    );
    let deadline = Instant::now() + Duration::from_secs(10);

    let stream = loop {
        match TcpStream::connect(("127.0.0.1", port)) {
            Ok(stream) => break stream,
            Err(_error) if Instant::now() < deadline => {
                if let Some(status) = server.0.try_wait().unwrap() {
                    panic!("servidor encerrou antes do teste: {status}");
                }
                thread::sleep(Duration::from_millis(25));
            }
            Err(error) => panic!("servidor não abriu a porta {port}: {error}"),
        }
    };

    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    drop(start_guard);
    (server, stream)
}

fn request_fixture(fixture: &str) -> String {
    let (_server, mut stream) = start_fixture(fixture);
    stream
        .write_all(b"GET / HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")
        .unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    response
}

fn run_fixture_to_completion(fixture: &str) -> (String, Duration) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let started = Instant::now();
    let output = Command::new(env!("CARGO_BIN_EXE_canaryo"))
        .current_dir(root)
        .arg(fixture)
        .output()
        .unwrap();
    let elapsed = started.elapsed();

    assert!(
        output.status.success(),
        "fixture failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    (String::from_utf8(output.stdout).unwrap(), elapsed)
}

#[test]
fn keeps_standalone_scripts_alive_for_referenced_timers() {
    let (stdout, elapsed) = run_fixture_to_completion("fixtures/timers/referenced.js");

    assert_eq!(stdout, "done");
    assert!(elapsed >= Duration::from_millis(20));
}

#[test]
fn lets_standalone_scripts_exit_with_only_unreferenced_timers() {
    let (stdout, elapsed) = run_fixture_to_completion("fixtures/timers/unreferenced.js");

    assert_eq!(stdout, "done");
    assert!(elapsed < Duration::from_millis(500));
}

#[test]
fn keeps_intervals_alive_until_they_are_cleared() {
    let (stdout, _elapsed) = run_fixture_to_completion("fixtures/timers/interval.js");

    assert_eq!(stdout, "3");
}

#[test]
fn closes_the_native_http_server_and_exits() {
    let port = free_port();
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut child = Command::new(env!("CARGO_BIN_EXE_canaryo"))
        .current_dir(root)
        .arg("fixtures/http-close/server.js")
        .arg(port.to_string())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);

    loop {
        if let Some(status) = child.try_wait().unwrap() {
            let mut stdout = String::new();
            child
                .stdout
                .take()
                .unwrap()
                .read_to_string(&mut stdout)
                .unwrap();
            assert!(status.success());
            assert_eq!(stdout.trim(), "closed");
            break;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("servidor não encerrou após server.close()");
        }
        thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn waits_for_active_responses_during_graceful_http_close() {
    let (mut server, mut stream) =
        start_fixture_with_stdout("fixtures/http-close/graceful.js", Stdio::piped());
    stream
        .write_all(b"GET / HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n")
        .unwrap();
    let response = read_response(&mut stream);

    assert!(response.starts_with("HTTP/1.1 200 OK"), "{response}");
    assert!(response.ends_with("finished"), "{response}");

    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(status) = server.0.try_wait().unwrap() {
            let mut stdout = String::new();
            server
                .0
                .stdout
                .take()
                .unwrap()
                .read_to_string(&mut stdout)
                .unwrap();
            assert!(status.success());
            assert_eq!(stdout, "closed");
            break;
        }
        if Instant::now() >= deadline {
            panic!("HTTP server did not finish its graceful close");
        }
        thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn closes_idle_connections_without_stopping_the_http_server() {
    let (_server, mut idle) = start_fixture("fixtures/http-close/connections.js");
    let address = idle.peer_addr().unwrap();
    let mut control = TcpStream::connect(address).unwrap();
    control
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    control
        .write_all(b"GET /close-idle HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n")
        .unwrap();
    assert!(read_response(&mut control).ends_with("alive"));

    let mut closed = Vec::new();
    idle.read_to_end(&mut closed).unwrap();
    assert!(closed.is_empty());

    let mut next = TcpStream::connect(address).unwrap();
    next.write_all(b"GET / HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")
        .unwrap();
    assert!(read_response(&mut next).ends_with("alive"));
}

#[test]
fn closes_all_connections_without_stopping_the_http_server() {
    let (_server, mut existing) = start_fixture("fixtures/http-close/connections.js");
    let address = existing.peer_addr().unwrap();
    let mut control = TcpStream::connect(address).unwrap();
    control
        .write_all(b"GET /close-all HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n")
        .unwrap();
    assert!(read_response(&mut control).ends_with("alive"));

    existing
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    let mut closed = Vec::new();
    existing.read_to_end(&mut closed).unwrap();
    assert!(closed.is_empty());

    let mut next = TcpStream::connect(address).unwrap();
    next.write_all(b"GET / HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")
        .unwrap();
    assert!(read_response(&mut next).ends_with("alive"));
}

fn read_response(stream: &mut impl Read) -> String {
    let mut response = Vec::new();

    while !response.ends_with(b"\r\n\r\n") {
        let mut byte = [0];
        stream.read_exact(&mut byte).unwrap();
        response.push(byte[0]);
    }

    let headers = String::from_utf8(response.clone()).unwrap();
    let content_length = headers
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("content-length")
                .then(|| value.trim().parse::<usize>().unwrap())
        })
        .unwrap();
    let header_length = response.len();
    response.resize(header_length + content_length, 0);
    if let Err(error) = stream.read_exact(&mut response[header_length..]) {
        panic!(
            "failed to read HTTP response body: {error}; response so far: {:?}",
            String::from_utf8_lossy(&response)
        );
    }
    String::from_utf8(response).unwrap()
}

#[test]
fn serves_a_native_node_http_application() {
    let response = request_fixture("fixtures/http-basic/server.js");
    let headers = response.to_ascii_lowercase();

    assert!(response.starts_with("HTTP/1.1 200 OK"));
    assert!(headers.contains("content-type: text/plain; charset=utf-8"));
    assert!(response.ends_with("Hello from Canaryo!"));
}

#[test]
fn serves_a_native_esm_http_application() {
    let response = request_fixture("fixtures/http-esm/server.mjs");
    let headers = response.to_ascii_lowercase();

    assert!(response.starts_with("HTTP/1.1 200 OK"));
    assert!(headers.contains("content-type: application/json; charset=utf-8"));
    assert!(response.ends_with(r#"{"runtime":"canaryo","modules":"esm+cjs!","ready":true}"#));
}

#[test]
fn preserves_async_local_storage_across_http_async_boundaries() {
    let (_server, mut stream) = start_fixture("fixtures/async-context/server.js");
    stream
        .write_all(b"GET /request-42 HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")
        .unwrap();
    let response = read_response(&mut stream);

    assert!(
        response.ends_with("/request-42:/request-42:/request-42:/request-42"),
        "{response}"
    );
}

#[test]
fn isolates_async_local_storage_between_concurrent_requests() {
    let (_server, mut slow) = start_fixture("fixtures/async-context/server.js");
    let mut fast = TcpStream::connect(slow.peer_addr().unwrap()).unwrap();
    slow.write_all(b"GET /slow HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")
        .unwrap();
    fast.write_all(b"GET /fast HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")
        .unwrap();

    let fast_response = read_response(&mut fast);
    let slow_response = read_response(&mut slow);
    assert!(
        fast_response.ends_with("/fast:/fast:/fast:/fast"),
        "{fast_response}"
    );
    assert!(
        slow_response.ends_with("/slow:/slow:/slow:/slow"),
        "{slow_response}"
    );
}

#[test]
fn serves_a_native_node_https_application() {
    let (_server, probe) = start_fixture("fixtures/https-basic/server.js");
    let address = probe.peer_addr().unwrap();
    drop(probe);

    let certificate_pem = std::fs::read_to_string("fixtures/https-basic/cert.pem").unwrap();
    let certificate_base64 = certificate_pem
        .lines()
        .filter(|line| !line.starts_with("-----"))
        .collect::<String>();
    let certificate = base64::engine::general_purpose::STANDARD
        .decode(certificate_base64)
        .unwrap();
    let mut roots = rustls::RootCertStore::empty();
    roots
        .add(rustls::pki_types::CertificateDer::from(certificate))
        .unwrap();
    let config = rustls::ClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth();
    let connection = rustls::ClientConnection::new(
        Arc::new(config),
        rustls::pki_types::ServerName::try_from("localhost")
            .unwrap()
            .to_owned(),
    )
    .unwrap();
    let socket = TcpStream::connect(address).unwrap();
    socket
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let mut stream = rustls::StreamOwned::new(connection, socket);

    stream
        .write_all(b"GET /secure HTTP/1.1\r\nHost: localhost\r\n\r\n")
        .unwrap();
    let first = read_response(&mut stream);
    stream
        .write_all(b"GET /again HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
        .unwrap();
    let mut second = String::new();
    stream.read_to_string(&mut second).unwrap();

    assert!(first.starts_with("HTTP/1.1 200 OK"), "{first}");
    assert!(first.contains("Connection: keep-alive"), "{first}");
    assert!(
        first.ends_with(r#"{"secure":true,"method":"GET","url":"/secure"}"#),
        "{first}"
    );
    assert!(second.starts_with("HTTP/1.1 200 OK"), "{second}");
    assert!(
        second.contains("content-type: application/json"),
        "{second}"
    );
    assert!(
        second.ends_with(r#"{"secure":true,"method":"GET","url":"/again"}"#),
        "{second}"
    );
}

#[test]
fn performs_outbound_http_requests() {
    let upstream = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let upstream_port = upstream.local_addr().unwrap().port();
    let upstream_thread = thread::spawn(move || {
        let (mut stream, _) = upstream.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut request = Vec::new();
        while !request.ends_with(b"\r\n\r\n") {
            let mut byte = [0];
            stream.read_exact(&mut byte).unwrap();
            request.push(byte[0]);
        }
        stream
            .write_all(
                b"HTTP/1.1 202 Accepted\r\nContent-Length: 11\r\nX-Upstream: yes\r\nConnection: close\r\n\r\noutbound-ok",
            )
            .unwrap();
        String::from_utf8(request).unwrap()
    });
    let (_server, mut stream) = start_fixture_with_args(
        "fixtures/http-client/server.js",
        &[upstream_port.to_string()],
        Stdio::null(),
    );

    stream
        .write_all(b"GET / HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")
        .unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    let upstream_request = upstream_thread.join().unwrap();

    assert!(upstream_request.starts_with("GET /source?value=42 HTTP/1.1"));
    assert!(
        upstream_request
            .to_ascii_lowercase()
            .contains("x-canaryo-client: yes")
    );
    assert!(response.starts_with("HTTP/1.1 202 Accepted"), "{response}");
    assert!(response.contains("x-upstream: yes"));
    assert!(response.ends_with("outbound-ok"));
}

#[test]
fn streams_outbound_request_bodies_before_end() {
    let upstream = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let upstream_port = upstream.local_addr().unwrap().port();
    let (first_chunk_sent, first_chunk_received) = std::sync::mpsc::sync_channel(1);
    let upstream_thread = thread::spawn(move || {
        let (mut stream, _) = upstream.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut request = Vec::new();
        while !request.ends_with(b"\r\n\r\n") {
            let mut byte = [0];
            stream.read_exact(&mut byte).unwrap();
            request.push(byte[0]);
        }
        let mut body = vec![0; 11];
        stream.read_exact(&mut body[..6]).unwrap();
        first_chunk_sent.send(()).unwrap();
        stream.read_exact(&mut body[6..]).unwrap();
        stream
            .write_all(
                b"HTTP/1.1 200 OK\r\nContent-Length: 11\r\nConnection: close\r\n\r\nhello world",
            )
            .unwrap();
        body
    });
    let (_server, mut stream) = start_fixture_with_args(
        "fixtures/http-client/server.js",
        &[upstream_port.to_string()],
        Stdio::null(),
    );

    let started = Instant::now();
    stream
        .write_all(b"GET /upload HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")
        .unwrap();
    first_chunk_received
        .recv_timeout(Duration::from_millis(250))
        .expect("first upload chunk was buffered until request.end()");
    assert!(started.elapsed() < Duration::from_millis(300));

    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    assert!(response.ends_with("hello world"), "{response}");
    assert_eq!(upstream_thread.join().unwrap(), b"hello world");
}

#[test]
fn isolates_and_reuses_custom_http_agent_connections() {
    fn read_headers(stream: &mut TcpStream) -> String {
        let mut request = Vec::new();
        while !request.ends_with(b"\r\n\r\n") {
            let mut byte = [0];
            stream.read_exact(&mut byte).unwrap();
            request.push(byte[0]);
        }
        String::from_utf8(request).unwrap()
    }

    let upstream = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let upstream_port = upstream.local_addr().unwrap().port();
    let upstream_thread = thread::spawn(move || {
        let (mut pooled_stream, _) = upstream.accept().unwrap();
        pooled_stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let first = read_headers(&mut pooled_stream);
        pooled_stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 1\r\n\r\na")
            .unwrap();
        let second = read_headers(&mut pooled_stream);
        pooled_stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 1\r\n\r\nb")
            .unwrap();

        let (mut separate_stream, _) = upstream.accept().unwrap();
        let third = read_headers(&mut separate_stream);
        separate_stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 1\r\nConnection: close\r\n\r\nc")
            .unwrap();
        (first, second, third)
    });
    let (_server, mut stream) = start_fixture_with_args(
        "fixtures/http-client/server.js",
        &[upstream_port.to_string()],
        Stdio::null(),
    );

    stream
        .write_all(b"GET /agent HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")
        .unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    let (first, second, third) = upstream_thread.join().unwrap();

    assert!(first.starts_with("GET /agent/one HTTP/1.1"));
    assert!(second.starts_with("GET /agent/two HTTP/1.1"));
    assert!(third.starts_with("GET /agent/three HTTP/1.1"));
    assert!(
        response.ends_with(&format!("abc:true:127.0.0.1:{upstream_port}:")),
        "{response}"
    );
}

#[test]
fn returns_redirect_responses_without_following_them() {
    let upstream = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let upstream_port = upstream.local_addr().unwrap().port();
    let upstream_thread = thread::spawn(move || {
        let (mut stream, _) = upstream.accept().unwrap();
        let mut request = Vec::new();
        while !request.ends_with(b"\r\n\r\n") {
            let mut byte = [0];
            stream.read_exact(&mut byte).unwrap();
            request.push(byte[0]);
        }
        stream
            .write_all(
                b"HTTP/1.1 302 Found\r\nLocation: /redirect/final\r\nContent-Length: 8\r\nConnection: close\r\n\r\nredirect",
            )
            .unwrap();
        String::from_utf8(request).unwrap()
    });
    let (_server, mut stream) = start_fixture_with_args(
        "fixtures/http-client/server.js",
        &[upstream_port.to_string()],
        Stdio::null(),
    );

    stream
        .write_all(b"GET /redirect HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")
        .unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();

    assert!(
        upstream_thread
            .join()
            .unwrap()
            .starts_with("GET /redirect/source HTTP/1.1")
    );
    assert!(response.ends_with("302:redirect"), "{response}");
}

#[test]
fn connects_to_tcp_services_from_an_http_handler() {
    let upstream = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let upstream_port = upstream.local_addr().unwrap().port();
    let upstream_thread = thread::spawn(move || {
        let (mut stream, _) = upstream.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut request = String::new();
        stream.read_to_string(&mut request).unwrap();
        stream.write_all(b"pong").unwrap();
        request
    });
    let (_server, mut stream) = start_fixture_with_args(
        "fixtures/net-client/server.js",
        &[upstream_port.to_string()],
        Stdio::null(),
    );

    stream
        .write_all(b"GET / HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")
        .unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();

    assert_eq!(upstream_thread.join().unwrap(), "ping");
    assert!(response.starts_with("HTTP/1.1 200 OK"), "{response}");
    assert!(response.contains("\"body\":\"pong\""), "{response}");
    assert!(response.contains("\"socket\":true"), "{response}");
    assert!(
        response.contains(&format!("\"remotePort\":{upstream_port}")),
        "{response}"
    );
    assert!(response.contains("\"bytesRead\":4"), "{response}");
    assert!(response.contains("\"bytesWritten\":4"), "{response}");
}

#[test]
fn keeps_the_standalone_event_loop_alive_for_tcp_clients() {
    let upstream = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let upstream_port = upstream.local_addr().unwrap().port();
    let upstream_thread = thread::spawn(move || {
        let (mut stream, _) = upstream.accept().unwrap();
        let mut request = String::new();
        stream.read_to_string(&mut request).unwrap();
        stream.write_all(b"complete").unwrap();
        request
    });
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut child = Command::new(env!("CARGO_BIN_EXE_canaryo"))
        .current_dir(root)
        .arg("fixtures/net-client/client.js")
        .arg(upstream_port.to_string())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);

    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("standalone TCP client did not finish");
        }
        thread::sleep(Duration::from_millis(10));
    };
    let mut stdout = String::new();
    child
        .stdout
        .take()
        .unwrap()
        .read_to_string(&mut stdout)
        .unwrap();

    assert!(status.success());
    assert_eq!(upstream_thread.join().unwrap(), "standalone");
    assert_eq!(stdout, "complete");
}

#[test]
fn serves_duplex_tcp_connections_and_closes_gracefully() {
    let (mut server, mut stream) = start_fixture("fixtures/net-server/server.js");
    let server_port = stream.peer_addr().unwrap().port();
    stream.write_all(b"canaryo").unwrap();
    stream.shutdown(std::net::Shutdown::Write).unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();

    assert!(response.contains("\"body\":\"canaryo\""), "{response}");
    assert!(
        response.contains("\"remoteAddress\":\"127.0.0.1\""),
        "{response}"
    );
    assert!(
        response.contains(&format!("\"localPort\":{server_port}")),
        "{response}"
    );
    assert!(
        response.contains(&format!("\"serverPort\":{server_port}")),
        "{response}"
    );

    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        if let Some(status) = server.0.try_wait().unwrap() {
            assert!(status.success());
            break;
        }
        if Instant::now() >= deadline {
            panic!("TCP server did not exit after server.close()");
        }
        thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn serves_other_tcp_connections_while_one_is_incomplete() {
    let (_server, mut slow_stream) = start_fixture("fixtures/net-server/server.js");
    let address = slow_stream.peer_addr().unwrap();
    let mut fast_stream = TcpStream::connect(address).unwrap();
    fast_stream
        .set_read_timeout(Some(Duration::from_millis(500)))
        .unwrap();
    slow_stream.write_all(b"slow").unwrap();

    let started = Instant::now();
    fast_stream.write_all(b"fast").unwrap();
    fast_stream.shutdown(std::net::Shutdown::Write).unwrap();
    let mut fast_response = String::new();
    fast_stream.read_to_string(&mut fast_response).unwrap();

    assert!(started.elapsed() < Duration::from_millis(500));
    assert!(fast_response.contains("\"body\":\"fast\""));

    slow_stream.shutdown(std::net::Shutdown::Write).unwrap();
    let mut slow_response = String::new();
    slow_stream.read_to_string(&mut slow_response).unwrap();
    assert!(slow_response.contains("\"body\":\"slow\""));
}

#[test]
fn queues_requests_at_the_custom_agent_socket_limit() {
    fn read_headers(stream: &mut TcpStream) -> String {
        let mut request = Vec::new();
        while !request.ends_with(b"\r\n\r\n") {
            let mut byte = [0];
            stream.read_exact(&mut byte).unwrap();
            request.push(byte[0]);
        }
        String::from_utf8(request).unwrap()
    }

    let upstream = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let upstream_port = upstream.local_addr().unwrap().port();
    let upstream_thread = thread::spawn(move || {
        let (mut pooled_stream, _) = upstream.accept().unwrap();
        pooled_stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let first = read_headers(&mut pooled_stream);

        upstream.set_nonblocking(true).unwrap();
        thread::sleep(Duration::from_millis(150));
        match upstream.accept() {
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
            Ok(_) => panic!("maxSockets allowed a second simultaneous connection"),
            Err(error) => panic!("unexpected accept error: {error}"),
        }
        pooled_stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 1\r\n\r\na")
            .unwrap();
        let second = read_headers(&mut pooled_stream);
        pooled_stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 1\r\nConnection: close\r\n\r\nb")
            .unwrap();
        (first, second)
    });
    let (_server, mut stream) = start_fixture_with_args(
        "fixtures/http-client/server.js",
        &[upstream_port.to_string()],
        Stdio::null(),
    );

    stream
        .write_all(b"GET /agent-limit HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")
        .unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    let (first, second) = upstream_thread.join().unwrap();

    assert!(first.contains("/limit/one") || first.contains("/limit/two"));
    assert!(second.contains("/limit/one") || second.contains("/limit/two"));
    assert_ne!(first, second);
    assert!(response.ends_with("ab"), "{response}");
}

#[test]
fn keeps_serving_while_an_outbound_request_is_pending() {
    let upstream = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let upstream_port = upstream.local_addr().unwrap().port();
    let (request_started, wait_for_request) = std::sync::mpsc::sync_channel(1);
    let upstream_thread = thread::spawn(move || {
        let (mut stream, _) = upstream.accept().unwrap();
        let mut request = Vec::new();
        while !request.ends_with(b"\r\n\r\n") {
            let mut byte = [0];
            stream.read_exact(&mut byte).unwrap();
            request.push(byte[0]);
        }
        request_started.send(()).unwrap();
        thread::sleep(Duration::from_millis(300));
        stream
            .write_all(
                b"HTTP/1.1 200 OK\r\nContent-Length: 11\r\nX-Upstream: yes\r\nConnection: close\r\n\r\noutbound-ok",
            )
            .unwrap();
    });
    let (_server, mut slow_stream) = start_fixture_with_args(
        "fixtures/http-client/server.js",
        &[upstream_port.to_string()],
        Stdio::null(),
    );
    let server_address = slow_stream.peer_addr().unwrap();
    let slow_request = thread::spawn(move || {
        slow_stream
            .write_all(b"GET /proxy HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")
            .unwrap();
        let mut response = String::new();
        slow_stream.read_to_string(&mut response).unwrap();
        response
    });
    wait_for_request
        .recv_timeout(Duration::from_secs(5))
        .unwrap();

    let started = Instant::now();
    let mut health_stream = TcpStream::connect(server_address).unwrap();
    health_stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    health_stream
        .write_all(b"GET /health HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")
        .unwrap();
    let mut health_response = String::new();
    health_stream.read_to_string(&mut health_response).unwrap();
    let health_elapsed = started.elapsed();

    assert!(health_response.ends_with("healthy"));
    assert!(
        health_elapsed < Duration::from_millis(200),
        "health request took {health_elapsed:?} while outbound I/O was pending"
    );
    assert!(slow_request.join().unwrap().ends_with("outbound-ok"));
    upstream_thread.join().unwrap();
}

#[test]
fn streams_and_pauses_outbound_response_bodies() {
    let upstream = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let upstream_port = upstream.local_addr().unwrap().port();
    let upstream_thread = thread::spawn(move || {
        let (mut stream, _) = upstream.accept().unwrap();
        let mut request = Vec::new();
        while !request.ends_with(b"\r\n\r\n") {
            let mut byte = [0];
            stream.read_exact(&mut byte).unwrap();
            request.push(byte[0]);
        }
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 40000\r\nConnection: close\r\n\r\n")
            .unwrap();
        stream.write_all(&vec![b'x'; 40_000]).unwrap();
    });
    let (_server, mut stream) = start_fixture_with_args(
        "fixtures/http-client/server.js",
        &[upstream_port.to_string()],
        Stdio::null(),
    );

    stream
        .write_all(b"GET /stream HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")
        .unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    upstream_thread.join().unwrap();
    let body = response.rsplit("\r\n\r\n").next().unwrap();
    let (chunks, bytes) = body.split_once(':').unwrap();

    assert!(chunks.parse::<usize>().unwrap() >= 3);
    assert_eq!(bytes, "40000");
}

#[test]
fn times_out_and_aborts_outbound_requests() {
    let upstream = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let upstream_port = upstream.local_addr().unwrap().port();
    let upstream_thread = thread::spawn(move || {
        let (mut stream, _) = upstream.accept().unwrap();
        let mut request = Vec::new();
        while !request.ends_with(b"\r\n\r\n") {
            let mut byte = [0];
            stream.read_exact(&mut byte).unwrap();
            request.push(byte[0]);
        }
        thread::sleep(Duration::from_millis(100));
    });
    let (_server, mut stream) = start_fixture_with_args(
        "fixtures/http-client/server.js",
        &[upstream_port.to_string()],
        Stdio::null(),
    );

    stream
        .write_all(b"GET /timeout HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")
        .unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    upstream_thread.join().unwrap();

    assert!(response.starts_with("HTTP/1.1 504"), "{response}");
    assert!(response.ends_with("outbound-timeout"));
}

#[test]
fn aborts_outbound_requests_with_a_signal() {
    let upstream = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let upstream_port = upstream.local_addr().unwrap().port();
    let (_server, mut stream) = start_fixture_with_args(
        "fixtures/http-client/server.js",
        &[upstream_port.to_string()],
        Stdio::null(),
    );

    stream
        .write_all(b"GET /signal HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")
        .unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();

    assert!(
        response.ends_with("AbortError:ABORT_ERR:request-reason"),
        "{response}"
    );
}

#[test]
fn serves_multiple_requests_on_a_persistent_connection() {
    let (_server, mut stream) = start_fixture("fixtures/http-basic/server.js");

    stream
        .write_all(b"GET / HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n")
        .unwrap();
    let first = read_response(&mut stream);
    stream
        .write_all(b"GET / HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")
        .unwrap();
    let second = read_response(&mut stream);

    assert!(first.contains("Connection: keep-alive"));
    assert!(first.ends_with("Hello from Canaryo!"));
    assert!(second.contains("Connection: close"));
    assert!(second.ends_with("Hello from Canaryo!"));
}

#[test]
fn decodes_chunked_requests_and_exposes_trailers() {
    let (_server, mut stream) = start_fixture("fixtures/http-basic/server.js");

    stream
        .write_all(
            b"POST /echo HTTP/1.1\r\nHost: 127.0.0.1\r\nTransfer-Encoding: chunked\r\nTrailer: X-Checksum\r\nConnection: close\r\n\r\n4\r\nWiki\r\n5;source=test\r\npedia\r\n0\r\nX-Checksum: abc123\r\n\r\n",
        )
        .unwrap();
    let response = read_response(&mut stream);

    assert!(response.starts_with("HTTP/1.1 200 OK"));
    assert!(response.contains("x-request-trailer: abc123"));
    assert!(response.ends_with("Wikipedia"));
}

#[test]
fn dispatches_inbound_body_chunks_before_the_request_ends() {
    let (_server, mut stream) = start_fixture("fixtures/http-basic/server.js");
    stream
        .set_read_timeout(Some(Duration::from_millis(500)))
        .unwrap();
    let started = Instant::now();
    stream
        .write_all(
            b"POST /first-chunk HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Length: 10\r\n\r\nhello",
        )
        .unwrap();

    let response = read_response(&mut stream);

    assert!(started.elapsed() < Duration::from_millis(500));
    assert!(response.ends_with("first:hello"), "{response}");
}

#[test]
fn iterates_inbound_request_bodies_asynchronously() {
    let (_server, mut stream) = start_fixture("fixtures/http-basic/server.js");
    stream
        .write_all(
            b"POST /async-iterate HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Length: 11\r\nConnection: close\r\n\r\nhello ",
        )
        .unwrap();
    thread::sleep(Duration::from_millis(20));
    stream.write_all(b"world").unwrap();

    let response = read_response(&mut stream);

    assert!(response.starts_with("HTTP/1.1 200 OK"), "{response}");
    assert!(response.ends_with("hello world"), "{response}");
}

#[test]
fn stops_reading_the_socket_while_an_inbound_request_is_paused() {
    let (_server, mut stream) = start_fixture("fixtures/http-backpressure/server.js");
    let body = vec![b'x'; 64 * 1024];
    let request = format!(
        "POST /inspect HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(request.as_bytes()).unwrap();
    stream.write_all(&body).unwrap();
    let response = read_response(&mut stream);

    assert!(response.ends_with('0'), "{response}");
}

#[test]
fn resumes_reading_a_paused_inbound_request() {
    let (_server, mut stream) = start_fixture("fixtures/http-backpressure/server.js");
    let body = vec![b'x'; 64 * 1024];
    let request = format!(
        "POST /resume HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(request.as_bytes()).unwrap();
    stream.write_all(&body).unwrap();
    let response = read_response(&mut stream);

    assert!(response.ends_with(&body.len().to_string()), "{response}");
}

#[test]
fn rejects_request_bodies_above_the_configured_limit() {
    let (_server, mut stream) = start_fixture("fixtures/http-limits/server.js");
    stream
        .write_all(
            b"POST / HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Length: 9\r\nConnection: close\r\n\r\ntoo-large",
        )
        .unwrap();
    let response = read_response(&mut stream);

    assert!(
        response.starts_with("HTTP/1.1 413 Payload Too Large"),
        "{response}"
    );
    assert!(response.ends_with("Payload Too Large"), "{response}");
}

#[test]
fn accepts_request_bodies_at_the_configured_limit() {
    let (_server, mut stream) = start_fixture("fixtures/http-limits/server.js");
    stream
        .write_all(
            b"POST / HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Length: 8\r\nConnection: close\r\n\r\naccepted",
        )
        .unwrap();
    let response = read_response(&mut stream);

    assert!(response.starts_with("HTTP/1.1 200 OK"), "{response}");
    assert!(response.ends_with("accepted"), "{response}");
}

#[test]
fn applies_the_request_limit_to_chunked_bodies() {
    let (_server, mut stream) = start_fixture("fixtures/http-limits/server.js");
    stream
        .write_all(
            b"POST / HTTP/1.1\r\nHost: 127.0.0.1\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n5\r\nfirst\r\n4\r\nmore\r\n0\r\n\r\n",
        )
        .unwrap();
    let response = read_response(&mut stream);

    assert!(
        response.starts_with("HTTP/1.1 413 Payload Too Large"),
        "{response}"
    );
}

#[test]
fn enforces_the_http_server_connection_limit() {
    let (_server, mut held_connection) = start_fixture("fixtures/http-limits/server.js");
    let address = held_connection.peer_addr().unwrap();
    let mut rejected = TcpStream::connect(address).unwrap();
    rejected
        .set_read_timeout(Some(Duration::from_secs(1)))
        .unwrap();
    rejected
        .write_all(b"GET / HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")
        .unwrap();
    let mut response = Vec::new();
    match rejected.read_to_end(&mut response) {
        Ok(_) => {}
        Err(error)
            if matches!(
                error.kind(),
                std::io::ErrorKind::ConnectionReset | std::io::ErrorKind::ConnectionAborted
            ) => {}
        Err(error) => panic!("limited connection was not rejected: {error}"),
    }

    assert!(response.is_empty());

    held_connection
        .write_all(b"GET /drops HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")
        .unwrap();
    let response = read_response(&mut held_connection);
    assert!(response.ends_with('1'), "{response}");
}

#[test]
fn preserves_binary_response_bytes() {
    let (_server, mut stream) = start_fixture("fixtures/http-basic/server.js");
    stream
        .write_all(b"GET /binary HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")
        .unwrap();
    let mut response = Vec::new();
    stream.read_to_end(&mut response).unwrap();
    let body_start = response
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .unwrap()
        + 4;

    assert!(
        response[..body_start]
            .windows(b"Content-Length: 4".len())
            .any(|window| window == b"Content-Length: 4")
    );
    assert_eq!(&response[body_start..], &[0, 255, 128, 65]);
}

#[test]
fn omits_the_response_body_for_head_requests() {
    let (_server, mut stream) = start_fixture("fixtures/http-basic/server.js");
    stream
        .write_all(b"HEAD / HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")
        .unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();

    assert!(response.contains("Content-Length: 19"));
    assert!(response.ends_with("\r\n\r\n"));
}

#[test]
fn supports_response_header_introspection_and_custom_status_messages() {
    let (_server, mut stream) = start_fixture("fixtures/http-basic/server.js");
    stream
        .write_all(b"GET /headers HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")
        .unwrap();
    let response = read_response(&mut stream);

    assert!(response.starts_with("HTTP/1.1 201 Canaryo Created"));
    assert!(response.contains("x-present: yes,again"));
    assert!(response.contains("x-map: ready"));
    assert!(!response.contains("x-removed"));
    assert!(response.ends_with("true"));
}

#[test]
#[ignore = "requires npm ci in fixtures/express-basic"]
fn serves_a_native_express_application() {
    let response = request_fixture("fixtures/express-basic/server.js");
    let headers = response.to_ascii_lowercase();

    assert!(response.starts_with("HTTP/1.1 200 OK"));
    assert!(headers.contains("content-type: application/json; charset=utf-8"));
    assert!(response.ends_with(r#"{"runtime":"canaryo","status":"ok"}"#));
}

#[test]
#[ignore = "requires npm ci in fixtures/express-basic"]
fn compresses_express_responses_with_standard_middleware() {
    let (_server, mut stream) = start_fixture("fixtures/express-basic/compression-server.js");
    stream
        .write_all(
            b"GET / HTTP/1.1\r\nHost: 127.0.0.1\r\nAccept-Encoding: gzip\r\nConnection: close\r\n\r\n",
        )
        .unwrap();
    let mut response = Vec::new();
    stream.read_to_end(&mut response).unwrap();
    let body_start = response
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .unwrap()
        + 4;
    let headers = String::from_utf8_lossy(&response[..body_start]).to_ascii_lowercase();
    let mut decoder = flate2::read::GzDecoder::new(&response[body_start..]);
    let mut body = String::new();
    decoder.read_to_string(&mut body).unwrap();

    assert!(response.starts_with(b"HTTP/1.1 200 OK"));
    assert!(headers.contains("content-encoding: gzip"), "{headers}");
    assert_eq!(
        body,
        format!(r#"{{"payload":"{}"}}"#, "canaryo-compression-".repeat(100))
    );
}

#[test]
#[ignore = "requires npm ci in fixtures/express-basic"]
fn serves_static_files_from_express() {
    let (_server, mut stream) = start_fixture("fixtures/express-static/server.js");
    stream
        .write_all(b"GET /hello.txt HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")
        .unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();

    assert!(response.starts_with("HTTP/1.1 200 OK"));
    assert!(
        response
            .to_ascii_lowercase()
            .contains("content-type: text/plain")
    );
    let body = response.split_once("\r\n\r\n").unwrap().1.as_bytes();
    let expected = std::fs::read("fixtures/express-static/public/hello.txt").unwrap();
    assert_eq!(body, expected);
}

#[test]
#[ignore = "requires npm ci in fixtures/express-basic"]
fn parses_an_express_json_request_body() {
    let (_server, mut stream) = start_fixture("fixtures/express-basic/server.js");

    stream
        .write_all(
            b"POST /echo HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: 19\r\nConnection: close\r\n\r\n{\"message\":\"hello\"}",
        )
        .unwrap();
    let response = read_response(&mut stream);

    assert!(response.starts_with("HTTP/1.1 200 OK"));
    assert!(
        response.ends_with(r#"{"body":{"message":"hello"}}"#),
        "unexpected response: {response}"
    );
}

#[test]
#[ignore = "requires npm ci in fixtures/express-basic"]
fn parses_a_chunked_express_json_request_body() {
    let (_server, mut stream) = start_fixture("fixtures/express-basic/server.js");

    stream
        .write_all(
            b"POST /echo HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n5\r\n{\"mes\r\ne\r\nsage\":\"hello\"}\r\n0\r\n\r\n",
        )
        .unwrap();
    let response = read_response(&mut stream);

    assert!(response.starts_with("HTTP/1.1 200 OK"));
    assert!(
        response.ends_with(r#"{"body":{"message":"hello"}}"#),
        "unexpected response: {response}"
    );
}

#[test]
#[ignore = "requires npm ci in fixtures/fastify-basic"]
fn serves_a_native_fastify_application() {
    let response = request_fixture("fixtures/fastify-basic/server.js");
    let headers = response.to_ascii_lowercase();

    assert!(response.starts_with("HTTP/1.1 200 OK"));
    assert!(headers.contains("content-type: application/json; charset=utf-8"));
    assert!(response.ends_with(r#"{"runtime":"canaryo","framework":"fastify","status":"ok"}"#));
}

#[test]
#[ignore = "requires npm ci in fixtures/fastify-basic"]
fn serves_a_fastify_web_stream_response() {
    let (_server, mut stream) = start_fixture("fixtures/fastify-basic/server.js");
    stream
        .write_all(b"GET /web-stream HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")
        .unwrap();
    let response = read_response(&mut stream);

    assert!(response.starts_with("HTTP/1.1 200 OK"));
    assert!(
        response.ends_with("canaryo"),
        "unexpected response: {response}"
    );
}

#[test]
#[ignore = "requires npm ci in fixtures/fastify-basic"]
fn serves_a_fastify_web_response() {
    let (_server, mut stream) = start_fixture("fixtures/fastify-basic/server.js");
    stream
        .write_all(b"GET /web-response HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")
        .unwrap();
    let response = read_response(&mut stream);
    let headers = response.to_ascii_lowercase();

    assert!(response.starts_with("HTTP/1.1 201 Created"));
    assert!(headers.contains("x-canaryo-web: response"));
    assert!(response.ends_with("web-response"));
}

#[test]
#[ignore = "requires npm ci in fixtures/fastify-basic"]
fn serves_a_fastify_fetch_response() {
    let (_server, mut stream) = start_fixture("fixtures/fastify-basic/server.js");
    stream
        .write_all(b"GET /fetch-response HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")
        .unwrap();
    let response = read_response(&mut stream);
    let headers = response.to_ascii_lowercase();

    assert!(response.starts_with("HTTP/1.1 200 OK"));
    assert!(headers.contains("content-type: application/json; charset=utf-8"));
    assert!(response.ends_with(r#"{"id":"73","active":"fetch"}"#));
}

#[test]
#[ignore = "requires npm ci in fixtures/fastify-basic"]
fn fetches_post_bodies_and_follows_relative_redirects() {
    let (_server, mut stream) = start_fixture("fixtures/fastify-basic/server.js");

    stream
        .write_all(b"GET /fetch-post HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n")
        .unwrap();
    let post = read_response(&mut stream);
    assert!(post.starts_with("HTTP/1.1 200 OK"));
    assert!(post.ends_with(r#"{"body":{"source":"fetch"},"contentType":"application/json"}"#));

    stream
        .write_all(b"GET /fetch-redirect HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")
        .unwrap();
    let redirect = read_response(&mut stream);
    assert!(redirect.starts_with("HTTP/1.1 200 OK"));
    assert!(redirect.ends_with(r#"{"id":"91","active":"redirect"}"#));
}

#[test]
#[ignore = "requires npm ci in fixtures/fastify-basic"]
fn serves_fastify_with_pino_logging_enabled() {
    let (mut server, mut stream) =
        start_fixture_with_stdout("fixtures/fastify-logger/server.js", Stdio::piped());
    stream
        .write_all(b"GET / HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")
        .unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    thread::sleep(Duration::from_millis(25));
    server.0.kill().unwrap();
    server.0.wait().unwrap();
    let mut logs = String::new();
    server
        .0
        .stdout
        .take()
        .unwrap()
        .read_to_string(&mut logs)
        .unwrap();

    assert!(response.starts_with("HTTP/1.1 200 OK"));
    assert!(response.ends_with(r#"{"runtime":"canaryo","logger":"pino","status":"ok"}"#));
    assert!(logs.contains(r#""runtime":"canaryo""#));
    assert!(logs.contains("logger fixture handled request"));
}

#[test]
#[ignore = "requires npm ci in fixtures/fastify-basic"]
fn supports_fastify_params_query_and_response_hooks() {
    let (_server, mut stream) = start_fixture("fixtures/fastify-basic/server.js");

    stream
        .write_all(b"GET /users/42?active=true HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n")
        .unwrap();
    let route = read_response(&mut stream);
    assert!(route.ends_with(r#"{"id":"42","active":"true"}"#));

    stream
        .write_all(b"GET /hooks HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n")
        .unwrap();
    let first_hook = read_response(&mut stream);
    assert!(
        first_hook
            .to_ascii_lowercase()
            .contains("x-canaryo-hook: on-request")
    );
    assert!(first_hook.ends_with(r#"{"completedResponses":0}"#));

    stream
        .write_all(b"GET /hooks HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")
        .unwrap();
    let second_hook = read_response(&mut stream);
    assert!(second_hook.ends_with(r#"{"completedResponses":1}"#));
}

#[test]
#[ignore = "requires npm ci in fixtures/fastify-basic"]
fn parses_a_fastify_json_request_body() {
    let (_server, mut stream) = start_fixture("fixtures/fastify-basic/server.js");

    stream
        .write_all(
            b"POST /echo HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: 19\r\nConnection: close\r\n\r\n{\"message\":\"hello\"}",
        )
        .unwrap();
    let response = read_response(&mut stream);

    assert!(response.starts_with("HTTP/1.1 200 OK"));
    assert!(response.ends_with(r#"{"body":{"message":"hello"},"contentType":"application/json"}"#));
}

#[test]
#[ignore = "requires npm ci in fixtures/fastify-basic"]
fn waits_for_a_fastify_timer_before_responding() {
    let (_server, mut stream) = start_fixture("fixtures/fastify-basic/server.js");

    stream
        .write_all(b"GET /delayed HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")
        .unwrap();
    let response = read_response(&mut stream);

    assert!(response.starts_with("HTTP/1.1 200 OK"));
    assert!(
        response.ends_with(r#"{"delayed":true}"#),
        "unexpected response: {response}"
    );
}
