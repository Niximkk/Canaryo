use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    path::Path,
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};

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
    let port = free_port();
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut server = Server(
        Command::new(env!("CARGO_BIN_EXE_canaryo"))
            .current_dir(root)
            .arg(fixture)
            .arg(port.to_string())
            .stdout(Stdio::null())
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
