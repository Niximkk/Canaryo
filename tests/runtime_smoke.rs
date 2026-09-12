use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    path::Path,
    process::{Child, Command, Stdio},
    sync::Mutex,
    thread,
    time::{Duration, Instant},
};

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

fn read_response(stream: &mut TcpStream) -> String {
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
    stream.read_exact(&mut response[header_length..]).unwrap();
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
                b"HTTP/1.1 200 OK\r\nContent-Length: 11\r\nConnection: close\r\n\r\noutbound-ok",
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
    assert!(response.contains("x-present: yes"));
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
    assert!(response.ends_with("hello from express static\n"));
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
