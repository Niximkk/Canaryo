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

fn request_fixture(fixture: &str) -> String {
    let port = free_port();
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut server = Server(
        Command::new(env!("CARGO_BIN_EXE_canaryo"))
            .current_dir(root)
            .arg(fixture)
            .arg(port.to_string())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let deadline = Instant::now() + Duration::from_secs(10);

    let mut stream = loop {
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
    stream
        .write_all(b"GET / HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")
        .unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    response
}

#[test]
fn serves_a_native_node_http_application() {
    let response = request_fixture("fixtures/http-basic/server.js");

    assert!(response.starts_with("HTTP/1.1 200 OK"));
    assert!(response.contains("Content-Type: text/plain; charset=utf-8"));
    assert!(response.ends_with("Hello from Canaryo!"));
}

#[test]
#[ignore = "requires npm ci in fixtures/express-basic"]
fn serves_a_native_express_application() {
    let response = request_fixture("fixtures/express-basic/server.js");

    assert!(response.starts_with("HTTP/1.1 200 OK"));
    assert!(response.contains("Content-Type: application/json; charset=utf-8"));
    assert!(response.ends_with(r#"{"runtime":"canaryo","status":"ok"}"#));
}
