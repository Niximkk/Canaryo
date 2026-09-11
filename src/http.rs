use std::{
    collections::HashMap,
    io::{Read, Write},
    net::{Ipv4Addr, SocketAddr, SocketAddrV4},
};

use mio::{Events, Interest, Poll, Token, net::TcpStream};
use rquickjs::function::This;
use rquickjs::{Array, Coerced, Ctx, Exception, Function, Object, Result};

const MAX_REQUEST_SIZE: usize = 1024 * 1024;
const LISTENER: Token = Token(0);

struct Request {
    method: String,
    url: String,
    version: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

#[derive(Default)]
struct Response {
    status: u16,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

#[derive(Default)]
struct ConnectionReader {
    buffered: Vec<u8>,
}

struct Connection {
    stream: TcpStream,
    reader: ConnectionReader,
    outgoing: Vec<u8>,
    written: usize,
    writable_interest: bool,
    close_after_write: bool,
    read_closed: bool,
}

pub fn listen<'js>(
    context: Ctx<'js>,
    port: u16,
    handler: Function<'js>,
    on_listening: Function<'js>,
    request_prototype: Object<'js>,
    response_prototype: Object<'js>,
    socket_prototype: Object<'js>,
) -> Result<()> {
    let address = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, port));
    let mut listener = mio::net::TcpListener::bind(address)
        .map_err(|error| Exception::throw_message(&context, &error.to_string()))?;
    let mut poll =
        Poll::new().map_err(|error| Exception::throw_message(&context, &error.to_string()))?;
    poll.registry()
        .register(&mut listener, LISTENER, Interest::READABLE)
        .map_err(|error| Exception::throw_message(&context, &error.to_string()))?;
    let mut events = Events::with_capacity(1024);
    let mut connections = HashMap::new();
    let mut next_token = 1;
    on_listening.call::<_, ()>(())?;

    loop {
        poll.poll(&mut events, None)
            .map_err(|error| Exception::throw_message(&context, &error.to_string()))?;

        for event in &events {
            if event.token() == LISTENER {
                accept_connections(
                    &mut listener,
                    poll.registry(),
                    &mut connections,
                    &mut next_token,
                );
                continue;
            }

            let token = event.token();
            let mut remove = event.is_error() || event.is_write_closed();
            if event.is_readable() && !remove {
                let batch = connections
                    .get_mut(&token)
                    .and_then(|connection| connection.read_requests().ok());
                match batch {
                    Some((requests, read_closed)) => {
                        for request in requests {
                            let keep_alive = request.keep_alive();
                            let response = handle_request(
                                &context,
                                &handler,
                                &request,
                                &request_prototype,
                                &response_prototype,
                                &socket_prototype,
                            )?;
                            if let Some(connection) = connections.get_mut(&token) {
                                append_response(&mut connection.outgoing, &response, keep_alive);
                                connection.close_after_write |= !keep_alive;
                            }
                            if !keep_alive {
                                break;
                            }
                        }
                        if let Some(connection) = connections.get_mut(&token) {
                            connection.read_closed |= read_closed || event.is_read_closed();
                            if connection.flush().is_err() {
                                remove = true;
                            }
                        }
                    }
                    None => remove = true,
                }
            }

            if event.is_writable()
                && !remove
                && connections
                    .get_mut(&token)
                    .is_none_or(|connection| connection.flush().is_err())
            {
                remove = true;
            }

            if let Some(connection) = connections.get_mut(&token) {
                if connection.outgoing.is_empty()
                    && (connection.close_after_write || connection.read_closed)
                {
                    remove = true;
                } else if !remove {
                    let wants_write = !connection.outgoing.is_empty();
                    if wants_write != connection.writable_interest {
                        let interest = if wants_write {
                            Interest::READABLE | Interest::WRITABLE
                        } else {
                            Interest::READABLE
                        };
                        if poll
                            .registry()
                            .reregister(&mut connection.stream, token, interest)
                            .is_err()
                        {
                            remove = true;
                        } else {
                            connection.writable_interest = wants_write;
                        }
                    }
                }
            }

            if remove {
                connections.remove(&token);
            }
        }
    }
}

fn accept_connections(
    listener: &mut mio::net::TcpListener,
    registry: &mio::Registry,
    connections: &mut HashMap<Token, Connection>,
    next_token: &mut usize,
) {
    loop {
        match listener.accept() {
            Ok((mut stream, _)) => {
                let token = Token(*next_token);
                *next_token = next_token.wrapping_add(1).max(1);
                let _ = stream.set_nodelay(true);
                if registry
                    .register(&mut stream, token, Interest::READABLE)
                    .is_ok()
                {
                    connections.insert(
                        token,
                        Connection {
                            stream,
                            reader: ConnectionReader::default(),
                            outgoing: Vec::with_capacity(512),
                            written: 0,
                            writable_interest: false,
                            close_after_write: false,
                            read_closed: false,
                        },
                    );
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => return,
            Err(_) => return,
        }
    }
}

impl Connection {
    fn read_requests(&mut self) -> std::io::Result<(Vec<Request>, bool)> {
        let mut requests = Vec::new();

        loop {
            while let Some(request) = self.reader.parse()? {
                requests.push(request);
            }

            let mut chunk = [0_u8; 4096];
            match self.stream.read(&mut chunk) {
                Ok(0) => return Ok((requests, true)),
                Ok(read) => self.reader.push(&chunk[..read])?,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    return Ok((requests, false));
                }
                Err(error) => return Err(error),
            }
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        while self.written < self.outgoing.len() {
            match self.stream.write(&self.outgoing[self.written..]) {
                Ok(0) => {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::WriteZero,
                        "conexão encerrada durante a resposta HTTP",
                    ));
                }
                Ok(written) => self.written += written,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => return Ok(()),
                Err(error) => return Err(error),
            }
        }

        self.outgoing.clear();
        self.written = 0;
        Ok(())
    }
}

fn handle_request<'js>(
    context: &Ctx<'js>,
    handler: &Function<'js>,
    request: &Request,
    request_prototype: &Object<'js>,
    response_prototype: &Object<'js>,
    socket_prototype: &Object<'js>,
) -> Result<Response> {
    let request_object = request_to_js(context, request, request_prototype, socket_prototype)?;
    let response_object = response_to_js(context, response_prototype)?;
    let socket: Object = request_object.get("socket")?;
    request_object.set("res", response_object.clone())?;
    response_object.set("req", request_object.clone())?;
    response_object.set("socket", socket.clone())?;
    response_object.set("connection", socket)?;

    handler.call::<_, ()>((request_object.clone(), response_object.clone()))?;
    let deliver_body: Function = request_object.get("__canaryoDeliverBody")?;
    deliver_body.call::<_, ()>((This(request_object),))?;
    while context.execute_pending_job() {}

    response_from_js(&response_object)
}

fn request_to_js<'js>(
    context: &Ctx<'js>,
    request: &Request,
    prototype: &Object<'js>,
    socket_prototype: &Object<'js>,
) -> Result<Object<'js>> {
    let object = Object::new(context.clone())?;
    let headers = Object::new(context.clone())?;
    let raw_headers = Array::new(context.clone())?;
    let socket = Object::new(context.clone())?;

    for (index, (name, value)) in request.headers.iter().enumerate() {
        headers.set(name.as_str(), value.as_str())?;
        raw_headers.set(index * 2, name.as_str())?;
        raw_headers.set(index * 2 + 1, value.as_str())?;
    }

    let mut version_parts = request.version.split('.');
    let version_major = version_parts
        .next()
        .unwrap_or("1")
        .parse::<u8>()
        .unwrap_or(1);
    let version_minor = version_parts
        .next()
        .unwrap_or("1")
        .parse::<u8>()
        .unwrap_or(1);
    socket.set("remoteAddress", "127.0.0.1")?;
    socket.set("remoteFamily", "IPv4")?;
    socket.set("localAddress", "127.0.0.1")?;
    socket.set("encrypted", false)?;
    socket.set("destroyed", false)?;
    socket.set("connecting", false)?;
    socket.set("readable", true)?;
    socket.set("writable", true)?;
    socket.set_prototype(Some(socket_prototype))?;

    object.set("method", request.method.as_str())?;
    object.set("url", request.url.as_str())?;
    object.set("httpVersion", request.version.as_str())?;
    object.set("httpVersionMajor", version_major)?;
    object.set("httpVersionMinor", version_minor)?;
    object.set("headers", headers)?;
    object.set("rawHeaders", raw_headers)?;
    let body = String::from_utf8_lossy(&request.body);
    object.set("body", body.as_ref())?;
    object.set("__canaryoBody", body.as_ref())?;
    object.set("aborted", false)?;
    object.set("complete", false)?;
    object.set("destroyed", false)?;
    object.set("readable", true)?;
    object.set("readableEnded", false)?;
    object.set("socket", socket.clone())?;
    object.set("connection", socket)?;
    object.set_prototype(Some(prototype))?;
    Ok(object)
}

fn response_to_js<'js>(context: &Ctx<'js>, prototype: &Object<'js>) -> Result<Object<'js>> {
    let object = Object::new(context.clone())?;
    let headers = Object::new(context.clone())?;
    object.set("statusCode", 200)?;
    object.set("headersSent", false)?;
    object.set("writableEnded", false)?;
    object.set("writableFinished", false)?;
    object.set("finished", false)?;
    object.set("destroyed", false)?;
    object.set("__canaryoHeaders", headers)?;
    object.set("__canaryoBody", "")?;
    object.set_prototype(Some(prototype))?;
    Ok(object)
}

fn response_from_js(response: &Object<'_>) -> Result<Response> {
    let headers_object: Object = response.get("__canaryoHeaders")?;
    let mut headers = Vec::new();
    for property in headers_object.props::<String, Coerced<String>>() {
        let (name, value) = property?;
        headers.push((name, value.0));
    }
    let body: String = response.get("__canaryoBody")?;

    Ok(Response {
        status: response.get("statusCode")?,
        headers,
        body: body.into_bytes(),
    })
}

impl Request {
    fn keep_alive(&self) -> bool {
        let connection = self
            .headers
            .iter()
            .find(|(name, _)| name == "connection")
            .map(|(_, value)| value.as_str());
        let has_token = |token: &str| {
            connection.is_some_and(|value| {
                value
                    .split(',')
                    .any(|value| value.trim().eq_ignore_ascii_case(token))
            })
        };

        if self.version == "1.1" {
            !has_token("close")
        } else {
            has_token("keep-alive")
        }
    }
}

impl ConnectionReader {
    fn push(&mut self, data: &[u8]) -> std::io::Result<()> {
        self.buffered.extend_from_slice(data);
        if self.buffered.len() > MAX_REQUEST_SIZE {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "requisição excede o limite de 1 MiB",
            ));
        }
        Ok(())
    }

    fn parse(&mut self) -> std::io::Result<Option<Request>> {
        let Some(header_position) = find_bytes(&self.buffered, b"\r\n\r\n") else {
            return Ok(None);
        };
        let header_end = header_position + 4;

        let head = std::str::from_utf8(&self.buffered[..header_end - 4])
            .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
        let mut lines = head.split("\r\n");
        let request_line = lines.next().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, "linha HTTP ausente")
        })?;
        let mut request_parts = request_line.split_whitespace();
        let method = required_part(request_parts.next(), "método HTTP ausente")?.to_string();
        let url = required_part(request_parts.next(), "URL ausente")?.to_string();
        let version = required_part(request_parts.next(), "versão HTTP ausente")?
            .trim_start_matches("HTTP/")
            .to_string();
        let mut headers = Vec::new();

        for line in lines {
            let (name, value) = line.split_once(':').ok_or_else(|| {
                std::io::Error::new(std::io::ErrorKind::InvalidData, "cabeçalho HTTP inválido")
            })?;
            headers.push((name.trim().to_ascii_lowercase(), value.trim().to_string()));
        }

        let content_length = headers
            .iter()
            .find(|(name, _)| name == "content-length")
            .map(|(_, value)| {
                value
                    .parse::<usize>()
                    .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))
            })
            .transpose()?
            .unwrap_or(0);
        let expected_size = header_end + content_length;

        if expected_size > MAX_REQUEST_SIZE {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "requisição excede o limite de 1 MiB",
            ));
        }

        if self.buffered.len() < expected_size {
            return Ok(None);
        }

        let remaining = self.buffered.split_off(expected_size);
        let request_data = std::mem::replace(&mut self.buffered, remaining);
        Ok(Some(Request {
            method,
            url,
            version,
            headers,
            body: request_data[header_end..expected_size].to_vec(),
        }))
    }
}

fn required_part<'a>(part: Option<&'a str>, message: &str) -> std::io::Result<&'a str> {
    part.ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidData, message))
}

fn append_response(buffer: &mut Vec<u8>, response: &Response, keep_alive: bool) {
    let _ = write!(
        buffer,
        "HTTP/1.1 {} {}\r\n",
        response.status,
        reason_phrase(response.status)
    );
    let has_content_type = response
        .headers
        .iter()
        .any(|(name, _)| name.eq_ignore_ascii_case("content-type"));

    for (name, value) in &response.headers {
        if !name.eq_ignore_ascii_case("content-length") && !name.eq_ignore_ascii_case("connection")
        {
            let _ = write!(buffer, "{name}: {value}\r\n");
        }
    }

    if !has_content_type {
        buffer.extend_from_slice(b"Content-Type: text/plain; charset=utf-8\r\n");
    }
    let _ = write!(buffer, "Content-Length: {}\r\n", response.body.len());
    buffer.extend_from_slice(if keep_alive {
        b"Connection: keep-alive\r\n\r\n"
    } else {
        b"Connection: close\r\n\r\n"
    });
    buffer.extend_from_slice(&response.body);
}

fn reason_phrase(status: u16) -> &'static str {
    match status {
        200 => "OK",
        201 => "Created",
        204 => "No Content",
        400 => "Bad Request",
        404 => "Not Found",
        500 => "Internal Server Error",
        _ => "Unknown",
    }
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_end_of_http_headers() {
        assert_eq!(find_bytes(b"GET / HTTP/1.1\r\n\r\n", b"\r\n\r\n"), Some(14));
    }

    #[test]
    fn keeps_http_11_connections_open_by_default() {
        let request = Request {
            method: "GET".into(),
            url: "/".into(),
            version: "1.1".into(),
            headers: Vec::new(),
            body: Vec::new(),
        };

        assert!(request.keep_alive());
    }

    #[test]
    fn parses_pipelined_requests_without_losing_bytes() {
        let mut reader = ConnectionReader::default();
        reader
            .push(
                b"GET /first HTTP/1.1\r\nHost: localhost\r\n\r\nGET /second HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n",
            )
            .unwrap();

        let first = reader.parse().unwrap().unwrap();
        let second = reader.parse().unwrap().unwrap();

        assert_eq!(first.url, "/first");
        assert!(first.keep_alive());
        assert_eq!(second.url, "/second");
        assert!(!second.keep_alive());
        assert!(reader.parse().unwrap().is_none());
    }
}
