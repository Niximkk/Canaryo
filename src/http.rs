use std::{
    cell::RefCell,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    rc::Rc,
    time::Duration,
};

use rquickjs::{Ctx, Exception, Function, Object, Result, prelude::Opt};

const MAX_REQUEST_SIZE: usize = 1024 * 1024;

struct Request {
    method: String,
    url: String,
    version: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

struct Response {
    status: u16,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

pub fn listen<'js>(
    context: Ctx<'js>,
    port: u16,
    handler: Function<'js>,
    on_listening: Function<'js>,
) -> Result<()> {
    let listener = TcpListener::bind(("127.0.0.1", port))
        .map_err(|error| Exception::throw_message(&context, &error.to_string()))?;

    on_listening.call::<_, ()>(())?;

    for stream in listener.incoming() {
        let mut stream =
            stream.map_err(|error| Exception::throw_message(&context, &error.to_string()))?;
        stream
            .set_read_timeout(Some(Duration::from_secs(30)))
            .map_err(|error| Exception::throw_message(&context, &error.to_string()))?;

        handle_connection(&context, &handler, &mut stream)?;
    }

    Ok(())
}

fn handle_connection<'js>(
    context: &Ctx<'js>,
    handler: &Function<'js>,
    stream: &mut TcpStream,
) -> Result<()> {
    let request = read_request(stream)
        .map_err(|error| Exception::throw_message(context, &error.to_string()))?;
    let request_object = request_to_js(context, &request)?;
    let state = Rc::new(RefCell::new(Response {
        status: 200,
        headers: Vec::new(),
        body: Vec::new(),
    }));
    let response_object = response_to_js(context, Rc::clone(&state))?;

    handler.call::<_, ()>((request_object, response_object.clone()))?;

    if let Ok(status) = response_object.get::<_, u16>("statusCode") {
        state.borrow_mut().status = status;
    }

    write_response(stream, &state.borrow())
        .map_err(|error| Exception::throw_message(context, &error.to_string()))
}

fn request_to_js<'js>(context: &Ctx<'js>, request: &Request) -> Result<Object<'js>> {
    let object = Object::new(context.clone())?;
    let headers = Object::new(context.clone())?;

    for (name, value) in &request.headers {
        headers.set(name.as_str(), value.as_str())?;
    }

    object.set("method", request.method.as_str())?;
    object.set("url", request.url.as_str())?;
    object.set("httpVersion", request.version.as_str())?;
    object.set("headers", headers)?;
    object.set("body", String::from_utf8_lossy(&request.body).as_ref())?;
    Ok(object)
}

fn response_to_js<'js>(context: &Ctx<'js>, state: Rc<RefCell<Response>>) -> Result<Object<'js>> {
    let object = Object::new(context.clone())?;
    object.set("statusCode", 200)?;

    let header_state = Rc::clone(&state);
    object.set(
        "setHeader",
        Function::new(context.clone(), move |name: String, value: String| {
            set_header(&mut header_state.borrow_mut().headers, name, value);
        })?,
    )?;

    let head_state = Rc::clone(&state);
    object.set(
        "writeHead",
        Function::new(
            context.clone(),
            move |status: u16, headers: Opt<Object>| -> Result<()> {
                head_state.borrow_mut().status = status;
                if let Some(headers) = headers.0 {
                    for property in headers.props::<String, String>() {
                        let (name, value) = property?;
                        set_header(&mut head_state.borrow_mut().headers, name, value);
                    }
                }
                Ok(())
            },
        )?,
    )?;

    let write_state = Rc::clone(&state);
    object.set(
        "write",
        Function::new(context.clone(), move |chunk: String| {
            write_state
                .borrow_mut()
                .body
                .extend_from_slice(chunk.as_bytes());
            true
        })?,
    )?;

    object.set(
        "end",
        Function::new(context.clone(), move |chunk: Opt<String>| {
            if let Some(chunk) = chunk.0 {
                state.borrow_mut().body.extend_from_slice(chunk.as_bytes());
            }
        })?,
    )?;

    Ok(object)
}

fn set_header(headers: &mut Vec<(String, String)>, name: String, value: String) {
    if let Some(header) = headers
        .iter_mut()
        .find(|(existing, _)| existing.eq_ignore_ascii_case(&name))
    {
        header.1 = value;
    } else {
        headers.push((name, value));
    }
}

fn read_request(stream: &mut TcpStream) -> std::io::Result<Request> {
    let mut data = Vec::new();
    let mut chunk = [0_u8; 4096];
    let header_end;

    loop {
        let read = stream.read(&mut chunk)?;
        if read == 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "conexão encerrada antes do cabeçalho HTTP",
            ));
        }
        data.extend_from_slice(&chunk[..read]);

        if data.len() > MAX_REQUEST_SIZE {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "requisição excede o limite de 1 MiB",
            ));
        }

        if let Some(position) = find_bytes(&data, b"\r\n\r\n") {
            header_end = position + 4;
            break;
        }
    }

    let head = std::str::from_utf8(&data[..header_end - 4])
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
        .and_then(|(_, value)| value.parse::<usize>().ok())
        .unwrap_or(0);
    let expected_size = header_end + content_length;

    while data.len() < expected_size {
        let read = stream.read(&mut chunk)?;
        if read == 0 {
            break;
        }
        data.extend_from_slice(&chunk[..read]);
        if data.len() > MAX_REQUEST_SIZE {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "requisição excede o limite de 1 MiB",
            ));
        }
    }

    Ok(Request {
        method,
        url,
        version,
        headers,
        body: data[header_end..data.len().min(expected_size)].to_vec(),
    })
}

fn required_part<'a>(part: Option<&'a str>, message: &str) -> std::io::Result<&'a str> {
    part.ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidData, message))
}

fn write_response(stream: &mut TcpStream, response: &Response) -> std::io::Result<()> {
    let mut head = format!(
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
            head.push_str(&format!("{name}: {value}\r\n"));
        }
    }

    if !has_content_type {
        head.push_str("Content-Type: text/plain; charset=utf-8\r\n");
    }
    head.push_str(&format!("Content-Length: {}\r\n", response.body.len()));
    head.push_str("Connection: close\r\n\r\n");

    stream.write_all(head.as_bytes())?;
    stream.write_all(&response.body)?;
    stream.flush()
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
    fn replaces_headers_case_insensitively() {
        let mut headers = vec![("Content-Type".into(), "text/plain".into())];

        set_header(
            &mut headers,
            "content-type".into(),
            "application/json".into(),
        );

        assert_eq!(headers.len(), 1);
        assert_eq!(headers[0].1, "application/json");
    }
}
