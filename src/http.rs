use std::{
    collections::{HashMap, VecDeque},
    io::{Read, Write},
    net::{Ipv4Addr, SocketAddr, SocketAddrV4},
    sync::Arc,
    time::{Duration, Instant},
};

use base64::Engine;
use mio::{Events, Interest, Poll, Token, net::TcpStream};
use rquickjs::function::This;
use rquickjs::{Array, Coerced, Ctx, Exception, Function, Object, Result, TypedArray};

const DEFAULT_MAX_REQUEST_SIZE: usize = 1024 * 1024;
const MAX_HEADER_SIZE: usize = 64 * 1024;
const MAX_ASYNC_RESPONSE_WAIT: Duration = Duration::from_secs(30);
const LISTENER: Token = Token(0);
const REQUEST_TOO_LARGE_MESSAGE: &str = "request body exceeds the configured limit";

struct RequestHead {
    method: String,
    url: String,
    version: String,
    headers: Vec<(String, String)>,
}

enum InboundEvent {
    Head(RequestHead),
    Data(Vec<u8>),
    End(Vec<(String, String)>),
}

enum RequestBodyState {
    Head,
    Fixed { remaining: usize },
    Chunked(ChunkedBodyState),
}

enum ChunkedBodyState {
    Size { received: usize },
    Data { remaining: usize, received: usize },
    DataTerminator { received: usize },
    Trailers { received: usize },
}

#[derive(Default)]
struct Response {
    status: u16,
    status_message: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

struct ConnectionReader {
    buffered: Vec<u8>,
    state: RequestBodyState,
    max_request_size: usize,
}

impl Default for ConnectionReader {
    fn default() -> Self {
        Self {
            buffered: Vec::new(),
            state: RequestBodyState::Head,
            max_request_size: DEFAULT_MAX_REQUEST_SIZE,
        }
    }
}

struct ServerOptions {
    max_request_size: usize,
}

impl Default for ServerOptions {
    fn default() -> Self {
        Self {
            max_request_size: DEFAULT_MAX_REQUEST_SIZE,
        }
    }
}

struct Connection<'js> {
    stream: HttpStream,
    socket: Object<'js>,
    incoming_request: Option<Object<'js>>,
    pending_responses: VecDeque<PendingResponse<'js>>,
    reader: ConnectionReader,
    outgoing: Vec<u8>,
    written: usize,
    registered: bool,
    readable_interest: bool,
    writable_interest: bool,
    read_paused: bool,
    close_after_write: bool,
    read_closed: bool,
    tls_shutdown_started: bool,
}

enum HttpStream {
    Plain(TcpStream),
    Tls(Box<rustls::StreamOwned<rustls::ServerConnection, TcpStream>>),
}

impl HttpStream {
    fn raw_mut(&mut self) -> &mut TcpStream {
        match self {
            Self::Plain(stream) => stream,
            Self::Tls(stream) => &mut stream.sock,
        }
    }

    fn wants_write(&self) -> bool {
        match self {
            Self::Plain(_) => false,
            Self::Tls(stream) => stream.conn.wants_write(),
        }
    }

    fn flush_tls(&mut self) -> std::io::Result<()> {
        let Self::Tls(stream) = self else {
            return Ok(());
        };
        while stream.conn.wants_write() {
            match stream.conn.write_tls(&mut stream.sock) {
                Ok(0) => {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::WriteZero,
                        "TLS connection stopped accepting encrypted output",
                    ));
                }
                Ok(_) => {}
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => return Ok(()),
                Err(error) => return Err(error),
            }
        }
        Ok(())
    }
}

impl Read for HttpStream {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        match self {
            Self::Plain(stream) => stream.read(buffer),
            Self::Tls(stream) => loop {
                match stream.conn.reader().read(buffer) {
                    Ok(decrypted) => return Ok(decrypted),
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
                    Err(error) => return Err(error),
                }

                let encrypted = stream.conn.read_tls(&mut stream.sock)?;
                if encrypted == 0 {
                    return Ok(0);
                }
                stream
                    .conn
                    .process_new_packets()
                    .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
            },
        }
    }
}

impl Write for HttpStream {
    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        match self {
            Self::Plain(stream) => stream.write(buffer),
            Self::Tls(stream) => stream.conn.writer().write(buffer),
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        match self {
            Self::Plain(stream) => stream.flush(),
            Self::Tls(_) => self.flush_tls(),
        }
    }
}

struct PendingResponse<'js> {
    object: Object<'js>,
    keep_alive: bool,
    suppress_body: bool,
    deadline: Instant,
}

struct ServerBindings<'js> {
    handler: Function<'js>,
    on_listening: Function<'js>,
    max_connections: Function<'js>,
    connection_dropped: Function<'js>,
    request_prototype: Object<'js>,
    response_prototype: Object<'js>,
    socket_prototype: Object<'js>,
    options: ServerOptions,
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
    let tls_json: Option<String> = context.globals().get("__canaryoServerTlsOptions")?;
    let tls_config = tls_json
        .as_deref()
        .map(|options| tls_server_config(&context, options))
        .transpose()?;
    let options_json: Option<String> = context.globals().get("__canaryoServerOptions")?;
    let options = options_json
        .as_deref()
        .map(|options| server_options(&context, options))
        .transpose()?
        .unwrap_or_default();
    listen_with_config(
        context.clone(),
        port,
        ServerBindings {
            handler,
            on_listening,
            max_connections: context.globals().get("__canaryoServerMaxConnections")?,
            connection_dropped: context.globals().get("__canaryoServerConnectionDropped")?,
            request_prototype,
            response_prototype,
            socket_prototype,
            options,
        },
        tls_config,
    )
}

fn server_options<'js>(context: &Ctx<'js>, json: &str) -> Result<ServerOptions> {
    let options: serde_json::Value = serde_json::from_str(json).map_err(|error| {
        Exception::throw_message(context, &format!("invalid HTTP server options: {error}"))
    })?;
    let max_request_size = options
        .get("maxRequestSize")
        .and_then(serde_json::Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
        .filter(|value| *value > 0)
        .unwrap_or(DEFAULT_MAX_REQUEST_SIZE);
    Ok(ServerOptions { max_request_size })
}

fn tls_server_config<'js>(context: &Ctx<'js>, tls_json: &str) -> Result<Arc<rustls::ServerConfig>> {
    let options: serde_json::Value = serde_json::from_str(tls_json).map_err(|error| {
        Exception::throw_message(context, &format!("invalid HTTPS options: {error}"))
    })?;
    let cert_pem = options
        .get("cert")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| Exception::throw_message(context, "https.createServer requires cert"))?;
    let key_pem = options
        .get("key")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| Exception::throw_message(context, "https.createServer requires key"))?;

    let certificates = pem_blocks(cert_pem, "CERTIFICATE")
        .map_err(|message| Exception::throw_message(context, &message))?
        .into_iter()
        .map(rustls::pki_types::CertificateDer::from)
        .collect::<Vec<_>>();
    if certificates.is_empty() {
        return Err(Exception::throw_message(
            context,
            "HTTPS certificate PEM contains no CERTIFICATE block",
        ));
    }

    let read_key = |label| {
        first_pem_block(key_pem, label)
            .map_err(|message| Exception::throw_message(context, &message))
    };
    let private_key = if let Some(key) = read_key("PRIVATE KEY")? {
        rustls::pki_types::PrivateKeyDer::Pkcs8(rustls::pki_types::PrivatePkcs8KeyDer::from(key))
    } else if let Some(key) = read_key("RSA PRIVATE KEY")? {
        rustls::pki_types::PrivateKeyDer::Pkcs1(rustls::pki_types::PrivatePkcs1KeyDer::from(key))
    } else if let Some(key) = read_key("EC PRIVATE KEY")? {
        rustls::pki_types::PrivateKeyDer::Sec1(rustls::pki_types::PrivateSec1KeyDer::from(key))
    } else {
        return Err(Exception::throw_message(
            context,
            "HTTPS private key PEM uses an unsupported format",
        ));
    };

    let config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certificates, private_key)
        .map_err(|error| Exception::throw_message(context, &error.to_string()))?;
    Ok(Arc::new(config))
}

fn first_pem_block(pem: &str, label: &str) -> std::result::Result<Option<Vec<u8>>, String> {
    pem_blocks(pem, label).map(|mut blocks| blocks.drain(..).next())
}

fn pem_blocks(pem: &str, label: &str) -> std::result::Result<Vec<Vec<u8>>, String> {
    let begin = format!("-----BEGIN {label}-----");
    let end = format!("-----END {label}-----");
    let mut remaining = pem;
    let mut blocks = Vec::new();

    while let Some(begin_index) = remaining.find(&begin) {
        remaining = &remaining[begin_index + begin.len()..];
        let end_index = remaining
            .find(&end)
            .ok_or_else(|| format!("unterminated {label} PEM block"))?;
        let encoded = remaining[..end_index]
            .chars()
            .filter(|character| !character.is_whitespace())
            .collect::<String>();
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .map_err(|error| format!("invalid {label} PEM data: {error}"))?;
        blocks.push(decoded);
        remaining = &remaining[end_index + end.len()..];
    }

    Ok(blocks)
}

fn listen_with_config<'js>(
    context: Ctx<'js>,
    port: u16,
    bindings: ServerBindings<'js>,
    tls_config: Option<Arc<rustls::ServerConfig>>,
) -> Result<()> {
    let address = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, port));
    let mut listener = mio::net::TcpListener::bind(address)
        .map_err(|error| Exception::throw_message(&context, &error.to_string()))?;
    let mut poll =
        Poll::new().map_err(|error| Exception::throw_message(&context, &error.to_string()))?;
    poll.registry()
        .register(&mut listener, LISTENER, Interest::READABLE)
        .map_err(|error| Exception::throw_message(&context, &error.to_string()))?;
    let mut listener = Some(listener);
    let mut events = Events::with_capacity(1024);
    let mut connections = HashMap::new();
    let mut next_token = 1;
    let run_timers: Function = context.globals().get("__canaryoRunTimers")?;
    let restore_timer_context: Function = context.globals().get("__canaryoRestoreTimerContext")?;
    let poll_http_requests: Function = context.globals().get("__canaryoPollHttpRequests")?;
    let server_control: Function = context.globals().get("__canaryoServerControl")?;
    let mut closing = false;
    bindings.on_listening.call::<_, ()>(())?;

    loop {
        while context.execute_pending_job() {}
        let pending_http_requests = poll_http_requests.call::<_, usize>(())?;
        while context.execute_pending_job() {}
        let timer_result = run_timers.call::<_, i64>(())?;
        let timer_delay = match timer_result {
            -1 | -2 => None,
            value if value < -2 => Some(Duration::from_millis((-value - 3) as u64)),
            value => Some(Duration::from_millis(value as u64)),
        };
        while context.execute_pending_job() {}
        if timer_result < -1 {
            restore_timer_context.call::<_, ()>(())?;
        }
        if connections.values().any(Connection::needs_progress) {
            progress_connections(poll.registry(), &mut connections)?;
        }
        let control = server_control.call::<_, u8>(())?;
        if apply_connection_action(control, &mut connections)? {
            progress_connections(poll.registry(), &mut connections)?;
        }
        if !closing && control & 1 != 0 {
            closing = true;
            drop(listener.take());
            for connection in connections.values_mut() {
                connection.close_after_write = true;
            }
            progress_connections(poll.registry(), &mut connections)?;
        }
        if closing && connections.is_empty() {
            break;
        }
        let response_delay = next_response_deadline(&connections);
        let poll_delay = if pending_http_requests > 0 {
            Some(
                timer_delay
                    .unwrap_or(Duration::from_millis(2))
                    .min(Duration::from_millis(2)),
            )
        } else {
            timer_delay
        };
        let poll_delay = match (poll_delay, response_delay) {
            (Some(left), Some(right)) => Some(left.min(right)),
            (None, right) => right,
            (left, None) => left,
        };
        poll.poll(&mut events, poll_delay)
            .map_err(|error| Exception::throw_message(&context, &error.to_string()))?;

        for event in &events {
            if event.token() == LISTENER {
                if let Some(listener) = listener.as_mut() {
                    accept_connections(
                        &context,
                        listener,
                        poll.registry(),
                        &mut connections,
                        &mut next_token,
                        tls_config.as_ref(),
                        &bindings,
                    )?;
                }
                continue;
            }

            let token = event.token();
            let mut remove = event.is_error() || event.is_write_closed();
            if event.is_readable()
                && !remove
                && let Some(connection) = connections.get_mut(&token)
                && !connection.request_paused()?
            {
                let read_closed = read_and_dispatch_requests(
                    &context,
                    &bindings.handler,
                    &bindings.request_prototype,
                    &bindings.response_prototype,
                    connection,
                )?;
                connection.read_closed |= read_closed || event.is_read_closed();
                connection.read_paused = connection.request_paused()?;
                if connection.flush().is_err() {
                    remove = true;
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
                    && connection.pending_responses.is_empty()
                    && (connection.close_after_write || connection.read_closed)
                {
                    remove = connection.begin_shutdown().unwrap_or(true);
                } else if !remove
                    && connection
                        .sync_registration(poll.registry(), token)
                        .is_err()
                {
                    remove = true;
                }
            }

            if remove && let Some(mut connection) = connections.remove(&token) {
                close_connection_objects(&mut connection)?;
            }
        }
    }

    Ok(())
}

fn apply_connection_action<'js>(
    action: u8,
    connections: &mut HashMap<Token, Connection<'js>>,
) -> Result<bool> {
    let applied = if action & 4 != 0 {
        for (_, mut connection) in connections.drain() {
            close_connection_objects(&mut connection)?;
        }
        true
    } else if action & 2 != 0 {
        for connection in connections.values_mut() {
            if connection.incoming_request.is_none() && connection.pending_responses.is_empty() {
                connection.close_after_write = true;
            }
        }
        true
    } else {
        false
    };
    Ok(applied)
}

fn accept_connections<'js>(
    context: &Ctx<'js>,
    listener: &mut mio::net::TcpListener,
    registry: &mio::Registry,
    connections: &mut HashMap<Token, Connection<'js>>,
    next_token: &mut usize,
    tls_config: Option<&Arc<rustls::ServerConfig>>,
    bindings: &ServerBindings<'js>,
) -> Result<()> {
    loop {
        match listener.accept() {
            Ok((mut socket, remote_address)) => {
                let connection_limit = bindings.max_connections.call::<_, Option<usize>>(())?;
                if connection_limit.is_some_and(|limit| connections.len() >= limit) {
                    let local_address = socket.local_addr().ok();
                    bindings.connection_dropped.call::<_, ()>((
                        remote_address.ip().to_string(),
                        remote_address.port(),
                        local_address
                            .map(|address| address.ip().to_string())
                            .unwrap_or_default(),
                        local_address
                            .map(|address| address.port())
                            .unwrap_or_default(),
                    ))?;
                    continue;
                }
                let token = Token(*next_token);
                *next_token = next_token.wrapping_add(1).max(1);
                let _ = socket.set_nodelay(true);
                if registry
                    .register(&mut socket, token, Interest::READABLE)
                    .is_ok()
                {
                    let stream = match tls_config {
                        Some(config) => {
                            let connection = rustls::ServerConnection::new(Arc::clone(config))
                                .map_err(|error| {
                                    Exception::throw_message(context, &error.to_string())
                                })?;
                            HttpStream::Tls(Box::new(rustls::StreamOwned::new(connection, socket)))
                        }
                        None => HttpStream::Plain(socket),
                    };
                    connections.insert(
                        token,
                        Connection {
                            stream,
                            socket: socket_to_js(
                                context,
                                &bindings.socket_prototype,
                                tls_config.is_some(),
                            )?,
                            incoming_request: None,
                            pending_responses: VecDeque::new(),
                            reader: ConnectionReader::new(bindings.options.max_request_size),
                            outgoing: Vec::with_capacity(512),
                            written: 0,
                            registered: true,
                            readable_interest: true,
                            writable_interest: false,
                            read_paused: false,
                            close_after_write: false,
                            read_closed: false,
                            tls_shutdown_started: false,
                        },
                    );
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => return Ok(()),
            Err(_) => return Ok(()),
        }
    }
}

impl Connection<'_> {
    fn needs_progress(&self) -> bool {
        self.read_paused
            || !self.pending_responses.is_empty()
            || !self.outgoing.is_empty()
            || self.stream.wants_write()
            || self.close_after_write
            || self.read_closed
    }

    fn request_paused(&self) -> Result<bool> {
        self.incoming_request
            .as_ref()
            .map(|request| request.get("__canaryoPaused"))
            .transpose()
            .map(Option::unwrap_or_default)
    }

    fn sync_registration(&mut self, registry: &mio::Registry, token: Token) -> std::io::Result<()> {
        let wants_read = !self.read_paused && !self.read_closed;
        let wants_write = !self.outgoing.is_empty() || self.stream.wants_write();
        let interest = match (wants_read, wants_write) {
            (true, true) => Some(Interest::READABLE | Interest::WRITABLE),
            (true, false) => Some(Interest::READABLE),
            (false, true) => Some(Interest::WRITABLE),
            (false, false) => None,
        };

        match (self.registered, interest) {
            (true, None) => registry.deregister(self.stream.raw_mut())?,
            (false, Some(interest)) => registry.register(self.stream.raw_mut(), token, interest)?,
            (true, Some(interest))
                if wants_read != self.readable_interest
                    || wants_write != self.writable_interest =>
            {
                registry.reregister(self.stream.raw_mut(), token, interest)?;
            }
            _ => {}
        }
        self.registered = interest.is_some();
        self.readable_interest = wants_read;
        self.writable_interest = wants_write;
        Ok(())
    }

    fn next_inbound(&mut self) -> std::io::Result<ConnectionRead> {
        let events = self.reader.parse_events()?;
        if !events.is_empty() {
            return Ok(ConnectionRead::Events(events));
        }

        let mut chunk = [0_u8; 16 * 1024];
        match self.stream.read(&mut chunk) {
            Ok(0) => Ok(ConnectionRead::Closed),
            Ok(read) => {
                self.reader.push(&chunk[..read])?;
                let events = self.reader.parse_events()?;
                if events.is_empty() {
                    Ok(ConnectionRead::Progress)
                } else {
                    Ok(ConnectionRead::Events(events))
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                Ok(ConnectionRead::WouldBlock)
            }
            Err(error) => Err(error),
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
        self.stream.flush_tls()
    }

    fn begin_shutdown(&mut self) -> std::io::Result<bool> {
        if let HttpStream::Tls(stream) = &mut self.stream
            && !self.tls_shutdown_started
        {
            stream.conn.send_close_notify();
            self.tls_shutdown_started = true;
        }
        self.stream.flush_tls()?;
        Ok(!self.stream.wants_write())
    }
}

enum ConnectionRead {
    Events(Vec<InboundEvent>),
    Progress,
    WouldBlock,
    Closed,
}

fn read_and_dispatch_requests<'js>(
    context: &Ctx<'js>,
    handler: &Function<'js>,
    request_prototype: &Object<'js>,
    response_prototype: &Object<'js>,
    connection: &mut Connection<'js>,
) -> Result<bool> {
    loop {
        let read = match connection.next_inbound() {
            Ok(read) => read,
            Err(error) => {
                abort_incoming_request(connection)?;
                if error.to_string() == REQUEST_TOO_LARGE_MESSAGE {
                    connection.pending_responses.clear();
                    connection.outgoing.clear();
                    connection.written = 0;
                    append_response(
                        &mut connection.outgoing,
                        &Response {
                            status: 413,
                            body: b"Payload Too Large".to_vec(),
                            ..Response::default()
                        },
                        false,
                        false,
                    );
                    connection.close_after_write = true;
                    return Ok(false);
                }
                return Ok(true);
            }
        };
        match read {
            ConnectionRead::Events(events) => {
                let mut progressed_at_end = false;
                for event in events {
                    let ends_request = matches!(&event, InboundEvent::End(_));
                    dispatch_inbound_event(
                        context,
                        handler,
                        request_prototype,
                        response_prototype,
                        connection,
                        event,
                    )?;
                    if ends_request {
                        while context.execute_pending_job() {}
                        collect_completed_responses(connection)?;
                        progressed_at_end = true;
                    } else {
                        progressed_at_end = false;
                    }
                }
                if !progressed_at_end {
                    while context.execute_pending_job() {}
                    collect_completed_responses(connection)?;
                }
                if connection.request_paused()? {
                    return Ok(false);
                }
            }
            ConnectionRead::Progress => {}
            ConnectionRead::WouldBlock => return Ok(false),
            ConnectionRead::Closed => {
                abort_incoming_request(connection)?;
                return Ok(true);
            }
        }
    }
}

fn abort_incoming_request(connection: &mut Connection<'_>) -> Result<()> {
    let Some(request) = connection.incoming_request.take() else {
        return Ok(());
    };
    request.set("aborted", true)?;
    request.set("destroyed", true)?;
    request.set("readable", false)?;
    let emit: Function = request.get("emit")?;
    emit.call::<_, bool>((This(request.clone()), "aborted"))?;
    emit.call::<_, bool>((This(request), "close"))?;
    Ok(())
}

fn close_connection_objects(connection: &mut Connection<'_>) -> Result<()> {
    abort_incoming_request(connection)?;
    if !connection.socket.get::<_, bool>("destroyed")? {
        connection.socket.set("destroyed", true)?;
        connection.socket.set("readable", false)?;
        connection.socket.set("writable", false)?;
        let emit: Function = connection.socket.get("emit")?;
        emit.call::<_, bool>((This(connection.socket.clone()), "close"))?;
    }
    Ok(())
}

fn dispatch_inbound_event<'js>(
    context: &Ctx<'js>,
    handler: &Function<'js>,
    request_prototype: &Object<'js>,
    response_prototype: &Object<'js>,
    connection: &mut Connection<'js>,
    event: InboundEvent,
) -> Result<()> {
    match event {
        InboundEvent::Head(request) => {
            let keep_alive = request.keep_alive();
            let suppress_body = request.method == "HEAD";
            let (request_object, response_object) = begin_request(
                context,
                handler,
                &request,
                request_prototype,
                response_prototype,
                &connection.socket,
            )?;
            connection.incoming_request = Some(request_object);
            connection.pending_responses.push_back(PendingResponse {
                object: response_object,
                keep_alive,
                suppress_body,
                deadline: Instant::now() + MAX_ASYNC_RESPONSE_WAIT,
            });
        }
        InboundEvent::Data(body) => {
            let request = connection.incoming_request.as_ref().ok_or_else(|| {
                Exception::throw_message(context, "HTTP body arrived without request headers")
            })?;
            deliver_request_chunk(context, request, &body)?;
        }
        InboundEvent::End(trailers) => {
            let request = connection.incoming_request.take().ok_or_else(|| {
                Exception::throw_message(context, "HTTP request ended without request headers")
            })?;
            finish_request_body(context, request, &trailers)?;
        }
    }
    Ok(())
}

fn progress_connections<'js>(
    registry: &mio::Registry,
    connections: &mut HashMap<Token, Connection<'js>>,
) -> Result<()> {
    let tokens = connections.keys().copied().collect::<Vec<_>>();
    for token in tokens {
        let mut remove = false;
        if let Some(connection) = connections.get_mut(&token) {
            if connection.read_paused && !connection.request_paused()? {
                connection.read_paused = false;
            }
            collect_completed_responses(connection)?;

            let flush_failed = connection.flush().is_err();
            let should_close = connection.outgoing.is_empty()
                && connection.pending_responses.is_empty()
                && (connection.close_after_write || connection.read_closed);
            let closed = should_close && connection.begin_shutdown().unwrap_or(true);
            if flush_failed || closed || connection.sync_registration(registry, token).is_err() {
                remove = true;
            }
        }
        if remove && let Some(mut connection) = connections.remove(&token) {
            close_connection_objects(&mut connection)?;
        }
    }
    Ok(())
}

fn collect_completed_responses(connection: &mut Connection<'_>) -> Result<()> {
    while let Some(pending) = connection.pending_responses.front() {
        if !pending.object.get::<_, bool>("writableEnded")? {
            if Instant::now() >= pending.deadline {
                return Err(Exception::throw_message(
                    pending.object.ctx(),
                    "a resposta assíncrona excedeu o limite de 30 segundos",
                ));
            }
            break;
        }
        let response = response_from_js(&pending.object)?;
        append_response(
            &mut connection.outgoing,
            &response,
            pending.keep_alive,
            pending.suppress_body,
        );
        connection.close_after_write |= !pending.keep_alive;
        connection.pending_responses.pop_front();
    }
    Ok(())
}

fn next_response_deadline(connections: &HashMap<Token, Connection<'_>>) -> Option<Duration> {
    let now = Instant::now();
    connections
        .values()
        .filter_map(|connection| connection.pending_responses.front())
        .map(|pending| pending.deadline.saturating_duration_since(now))
        .min()
}

fn begin_request<'js>(
    context: &Ctx<'js>,
    handler: &Function<'js>,
    request: &RequestHead,
    request_prototype: &Object<'js>,
    response_prototype: &Object<'js>,
    socket: &Object<'js>,
) -> Result<(Object<'js>, Object<'js>)> {
    let request_object = request_to_js(context, request, request_prototype, socket)?;
    let response_object = response_to_js(context, response_prototype)?;
    let socket: Object = request_object.get("socket")?;
    request_object.set("res", response_object.clone())?;
    response_object.set("req", request_object.clone())?;
    response_object.set("socket", socket.clone())?;
    response_object.set("connection", socket)?;

    handler.call::<_, ()>((request_object.clone(), response_object.clone()))?;
    Ok((request_object, response_object))
}

fn request_to_js<'js>(
    context: &Ctx<'js>,
    request: &RequestHead,
    prototype: &Object<'js>,
    socket: &Object<'js>,
) -> Result<Object<'js>> {
    let object = Object::new(context.clone())?;
    let headers = Object::new(context.clone())?;
    let raw_headers = Array::new(context.clone())?;

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
    object.set("method", request.method.as_str())?;
    object.set("url", request.url.as_str())?;
    object.set("httpVersion", request.version.as_str())?;
    object.set("httpVersionMajor", version_major)?;
    object.set("httpVersionMinor", version_minor)?;
    object.set("headers", headers)?;
    object.set("rawHeaders", raw_headers)?;
    object.set("trailers", Object::new(context.clone())?)?;
    object.set("rawTrailers", Array::new(context.clone())?)?;
    object.set("aborted", false)?;
    object.set("complete", false)?;
    object.set("destroyed", false)?;
    object.set("readable", true)?;
    object.set("readableEnded", false)?;
    object.set("__canaryoPaused", false)?;
    object.set("socket", socket.clone())?;
    object.set("connection", socket.clone())?;
    object.set_prototype(Some(prototype))?;
    Ok(object)
}

fn deliver_request_chunk<'js>(
    context: &Ctx<'js>,
    request: &Object<'js>,
    body: &[u8],
) -> Result<()> {
    if body.is_empty() {
        return Ok(());
    }
    let bytes = TypedArray::<u8>::new_copy(context.clone(), body)?;
    let deliver: Function = request.get("__canaryoDeliverBytes")?;
    deliver.call::<_, ()>((This(request.clone()), bytes))
}

fn finish_request_body<'js>(
    context: &Ctx<'js>,
    request: Object<'js>,
    trailers: &[(String, String)],
) -> Result<()> {
    if !trailers.is_empty() {
        let trailer_object = Object::new(context.clone())?;
        let raw_trailers = Array::new(context.clone())?;
        for (index, (name, value)) in trailers.iter().enumerate() {
            trailer_object.set(name.as_str(), value.as_str())?;
            raw_trailers.set(index * 2, name.as_str())?;
            raw_trailers.set(index * 2 + 1, value.as_str())?;
        }
        request.set("trailers", trailer_object)?;
        request.set("rawTrailers", raw_trailers)?;
    }
    let finish: Function = request.get("__canaryoFinishBody")?;
    finish.call::<_, ()>((This(request),))
}

fn socket_to_js<'js>(
    context: &Ctx<'js>,
    prototype: &Object<'js>,
    encrypted: bool,
) -> Result<Object<'js>> {
    let socket = Object::new(context.clone())?;
    socket.set("remoteAddress", "127.0.0.1")?;
    socket.set("remoteFamily", "IPv4")?;
    socket.set("localAddress", "127.0.0.1")?;
    socket.set("encrypted", encrypted)?;
    socket.set("destroyed", false)?;
    socket.set("connecting", false)?;
    socket.set("readable", true)?;
    socket.set("writable", true)?;
    socket.set_prototype(Some(prototype))?;
    Ok(socket)
}

fn response_to_js<'js>(context: &Ctx<'js>, prototype: &Object<'js>) -> Result<Object<'js>> {
    let object = Object::new(context.clone())?;
    let headers = Object::new(context.clone())?;
    object.set("statusCode", 200)?;
    object.set("statusMessage", "")?;
    object.set("headersSent", false)?;
    object.set("writableEnded", false)?;
    object.set("writableFinished", false)?;
    object.set("finished", false)?;
    object.set("destroyed", false)?;
    object.set("__canaryoHeaders", headers)?;
    object.set("__canaryoBody", Array::new(context.clone())?)?;
    object.set("__canaryoTextBody", "")?;
    object.set_prototype(Some(prototype))?;
    Ok(object)
}

fn response_from_js(response: &Object<'_>) -> Result<Response> {
    let headers_object: Object = response.get("__canaryoHeaders")?;
    let mut headers = Vec::new();
    for property in headers_object.props::<String, rquickjs::Value>() {
        let (name, value) = property?;
        if let Some(values) = value.as_array() {
            let values = values
                .iter::<Coerced<String>>()
                .collect::<Result<Vec<_>>>()?
                .into_iter()
                .map(|value| value.0)
                .collect::<Vec<_>>();
            if name.eq_ignore_ascii_case("set-cookie") {
                headers.extend(values.into_iter().map(|value| (name.clone(), value)));
            } else {
                headers.push((name, values.join(", ")));
            }
        } else {
            let value: Coerced<String> = headers_object.get(name.as_str())?;
            headers.push((name, value.0));
        }
    }
    let text_body: String = response.get("__canaryoTextBody")?;
    let body = if text_body.is_empty() {
        let body_chunks: Array = response.get("__canaryoBody")?;
        let mut body = Vec::new();
        for chunk in body_chunks.iter::<TypedArray<u8>>() {
            let chunk = chunk?;
            let bytes = chunk.as_bytes().ok_or_else(|| {
                Exception::throw_message(response.ctx(), "response contains a detached buffer")
            })?;
            body.extend_from_slice(bytes);
        }
        body
    } else {
        text_body.into_bytes()
    };

    Ok(Response {
        status: response.get("statusCode")?,
        status_message: response.get("statusMessage")?,
        headers,
        body,
    })
}

impl RequestHead {
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
    fn new(max_request_size: usize) -> Self {
        Self {
            buffered: Vec::new(),
            state: RequestBodyState::Head,
            max_request_size,
        }
    }

    fn push(&mut self, data: &[u8]) -> std::io::Result<()> {
        self.buffered.extend_from_slice(data);
        if matches!(self.state, RequestBodyState::Head)
            && self.buffered.len() > MAX_HEADER_SIZE
            && find_bytes(&self.buffered, b"\r\n\r\n").is_none()
        {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "HTTP headers exceed the 64 KiB limit",
            ));
        }
        Ok(())
    }

    fn parse_events(&mut self) -> std::io::Result<Vec<InboundEvent>> {
        let mut events = Vec::new();
        loop {
            match self.state {
                RequestBodyState::Head => {
                    let Some(header_position) = find_bytes(&self.buffered, b"\r\n\r\n") else {
                        break;
                    };
                    let header_end = header_position + 4;
                    let (request, body_state) = parse_request_head(
                        &self.buffered[..header_position],
                        self.max_request_size,
                    )?;
                    self.buffered.drain(..header_end);
                    self.state = body_state;
                    events.push(InboundEvent::Head(request));
                    if matches!(self.state, RequestBodyState::Fixed { remaining: 0 }) {
                        self.state = RequestBodyState::Head;
                        events.push(InboundEvent::End(Vec::new()));
                    }
                }
                RequestBodyState::Fixed { remaining } => {
                    if self.buffered.is_empty() {
                        break;
                    }
                    let length = remaining.min(self.buffered.len()).min(16 * 1024);
                    events.push(InboundEvent::Data(self.buffered.drain(..length).collect()));
                    let remaining = remaining - length;
                    if remaining == 0 {
                        self.state = RequestBodyState::Head;
                        events.push(InboundEvent::End(Vec::new()));
                    } else {
                        self.state = RequestBodyState::Fixed { remaining };
                    }
                }
                RequestBodyState::Chunked(ChunkedBodyState::Size { received }) => {
                    let Some(line_length) = find_bytes(&self.buffered, b"\r\n") else {
                        break;
                    };
                    let size_line =
                        std::str::from_utf8(&self.buffered[..line_length]).map_err(|error| {
                            std::io::Error::new(std::io::ErrorKind::InvalidData, error)
                        })?;
                    let size =
                        usize::from_str_radix(size_line.split(';').next().unwrap_or("").trim(), 16)
                            .map_err(|error| {
                                std::io::Error::new(std::io::ErrorKind::InvalidData, error)
                            })?;
                    self.buffered.drain(..line_length + 2);
                    self.state = if size == 0 {
                        RequestBodyState::Chunked(ChunkedBodyState::Trailers { received })
                    } else {
                        RequestBodyState::Chunked(ChunkedBodyState::Data {
                            remaining: size,
                            received,
                        })
                    };
                }
                RequestBodyState::Chunked(ChunkedBodyState::Data {
                    remaining,
                    received,
                }) => {
                    if self.buffered.is_empty() {
                        break;
                    }
                    let length = remaining.min(self.buffered.len()).min(16 * 1024);
                    let received = received.checked_add(length).ok_or_else(request_too_large)?;
                    if received > self.max_request_size {
                        return Err(request_too_large());
                    }
                    events.push(InboundEvent::Data(self.buffered.drain(..length).collect()));
                    let remaining = remaining - length;
                    self.state = if remaining == 0 {
                        RequestBodyState::Chunked(ChunkedBodyState::DataTerminator { received })
                    } else {
                        RequestBodyState::Chunked(ChunkedBodyState::Data {
                            remaining,
                            received,
                        })
                    };
                }
                RequestBodyState::Chunked(ChunkedBodyState::DataTerminator { received }) => {
                    if self.buffered.len() < 2 {
                        break;
                    }
                    if &self.buffered[..2] != b"\r\n" {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            "chunk HTTP sem terminador CRLF",
                        ));
                    }
                    self.buffered.drain(..2);
                    self.state = RequestBodyState::Chunked(ChunkedBodyState::Size { received });
                }
                RequestBodyState::Chunked(ChunkedBodyState::Trailers { received }) => {
                    if self.buffered.starts_with(b"\r\n") {
                        self.buffered.drain(..2);
                        self.state = RequestBodyState::Head;
                        events.push(InboundEvent::End(Vec::new()));
                        continue;
                    }
                    let Some(trailer_length) = find_bytes(&self.buffered, b"\r\n\r\n") else {
                        self.state =
                            RequestBodyState::Chunked(ChunkedBodyState::Trailers { received });
                        break;
                    };
                    let trailer_text = std::str::from_utf8(&self.buffered[..trailer_length])
                        .map_err(|error| {
                            std::io::Error::new(std::io::ErrorKind::InvalidData, error)
                        })?;
                    let trailers = parse_trailers(trailer_text)?;
                    self.buffered.drain(..trailer_length + 4);
                    self.state = RequestBodyState::Head;
                    events.push(InboundEvent::End(trailers));
                }
            }
        }
        Ok(events)
    }
}

fn parse_request_head(
    data: &[u8],
    max_request_size: usize,
) -> std::io::Result<(RequestHead, RequestBodyState)> {
    let head = std::str::from_utf8(data)
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
        .transpose()?;
    let chunked = headers.iter().any(|(name, value)| {
        name == "transfer-encoding"
            && value
                .split(',')
                .any(|token| token.trim().eq_ignore_ascii_case("chunked"))
    });
    if chunked && content_length.is_some() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "content-length e transfer-encoding chunked não podem ser combinados",
        ));
    }
    if content_length.is_some_and(|length| length > max_request_size) {
        return Err(request_too_large());
    }
    let state = if chunked {
        RequestBodyState::Chunked(ChunkedBodyState::Size { received: 0 })
    } else {
        RequestBodyState::Fixed {
            remaining: content_length.unwrap_or(0),
        }
    };
    Ok((
        RequestHead {
            method,
            url,
            version,
            headers,
        },
        state,
    ))
}

fn parse_trailers(data: &str) -> std::io::Result<Vec<(String, String)>> {
    data.split("\r\n")
        .map(|line| {
            let (name, value) = line.split_once(':').ok_or_else(|| {
                std::io::Error::new(std::io::ErrorKind::InvalidData, "trailer HTTP inválido")
            })?;
            Ok((name.trim().to_ascii_lowercase(), value.trim().to_string()))
        })
        .collect()
}

fn request_too_large() -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidData, REQUEST_TOO_LARGE_MESSAGE)
}

fn required_part<'a>(part: Option<&'a str>, message: &str) -> std::io::Result<&'a str> {
    part.ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidData, message))
}

fn append_response(
    buffer: &mut Vec<u8>,
    response: &Response,
    keep_alive: bool,
    suppress_body: bool,
) {
    buffer.extend_from_slice(b"HTTP/1.1 ");
    append_decimal(buffer, usize::from(response.status));
    buffer.push(b' ');
    buffer.extend_from_slice(
        if response.status_message.is_empty() {
            reason_phrase(response.status)
        } else {
            &response.status_message
        }
        .as_bytes(),
    );
    buffer.extend_from_slice(b"\r\n");
    let has_content_type = response
        .headers
        .iter()
        .any(|(name, _)| name.eq_ignore_ascii_case("content-type"));

    for (name, value) in &response.headers {
        if !name.eq_ignore_ascii_case("content-length") && !name.eq_ignore_ascii_case("connection")
        {
            buffer.extend_from_slice(name.as_bytes());
            buffer.extend_from_slice(b": ");
            buffer.extend_from_slice(value.as_bytes());
            buffer.extend_from_slice(b"\r\n");
        }
    }

    if !has_content_type {
        buffer.extend_from_slice(b"Content-Type: text/plain; charset=utf-8\r\n");
    }
    let content_length = if response.status == 204 || response.status == 304 {
        0
    } else {
        response.body.len()
    };
    buffer.extend_from_slice(b"Content-Length: ");
    append_decimal(buffer, content_length);
    buffer.extend_from_slice(b"\r\n");
    buffer.extend_from_slice(if keep_alive {
        b"Connection: keep-alive\r\n\r\n"
    } else {
        b"Connection: close\r\n\r\n"
    });
    if !suppress_body && response.status != 204 && response.status != 304 {
        buffer.extend_from_slice(&response.body);
    }
}

fn append_decimal(buffer: &mut Vec<u8>, mut value: usize) {
    if value == 0 {
        buffer.push(b'0');
        return;
    }

    let mut digits = [0_u8; 20];
    let mut index = digits.len();
    while value > 0 {
        index -= 1;
        digits[index] = b'0' + (value % 10) as u8;
        value /= 10;
    }
    buffer.extend_from_slice(&digits[index..]);
}

fn reason_phrase(status: u16) -> &'static str {
    match status {
        100 => "Continue",
        101 => "Switching Protocols",
        102 => "Processing",
        103 => "Early Hints",
        200 => "OK",
        201 => "Created",
        202 => "Accepted",
        203 => "Non-Authoritative Information",
        204 => "No Content",
        205 => "Reset Content",
        206 => "Partial Content",
        207 => "Multi-Status",
        208 => "Already Reported",
        226 => "IM Used",
        300 => "Multiple Choices",
        301 => "Moved Permanently",
        302 => "Found",
        303 => "See Other",
        304 => "Not Modified",
        305 => "Use Proxy",
        307 => "Temporary Redirect",
        308 => "Permanent Redirect",
        400 => "Bad Request",
        401 => "Unauthorized",
        402 => "Payment Required",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        406 => "Not Acceptable",
        407 => "Proxy Authentication Required",
        408 => "Request Timeout",
        409 => "Conflict",
        410 => "Gone",
        411 => "Length Required",
        412 => "Precondition Failed",
        413 => "Payload Too Large",
        414 => "URI Too Long",
        415 => "Unsupported Media Type",
        416 => "Range Not Satisfiable",
        417 => "Expectation Failed",
        418 => "I'm a Teapot",
        421 => "Misdirected Request",
        422 => "Unprocessable Entity",
        423 => "Locked",
        424 => "Failed Dependency",
        425 => "Too Early",
        426 => "Upgrade Required",
        428 => "Precondition Required",
        429 => "Too Many Requests",
        431 => "Request Header Fields Too Large",
        451 => "Unavailable For Legal Reasons",
        500 => "Internal Server Error",
        501 => "Not Implemented",
        502 => "Bad Gateway",
        503 => "Service Unavailable",
        504 => "Gateway Timeout",
        505 => "HTTP Version Not Supported",
        506 => "Variant Also Negotiates",
        507 => "Insufficient Storage",
        508 => "Loop Detected",
        509 => "Bandwidth Limit Exceeded",
        510 => "Not Extended",
        511 => "Network Authentication Required",
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
    fn returns_standard_reason_phrases_used_by_express() {
        assert_eq!(reason_phrase(302), "Found");
        assert_eq!(reason_phrase(418), "I'm a Teapot");
        assert_eq!(reason_phrase(503), "Service Unavailable");
    }

    #[test]
    fn keeps_http_11_connections_open_by_default() {
        let request = RequestHead {
            method: "GET".into(),
            url: "/".into(),
            version: "1.1".into(),
            headers: Vec::new(),
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

        let events = reader.parse_events().unwrap();
        let heads = events
            .iter()
            .filter_map(|event| match event {
                InboundEvent::Head(request) => Some(request),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(heads.len(), 2);
        assert_eq!(heads[0].url, "/first");
        assert!(heads[0].keep_alive());
        assert_eq!(heads[1].url, "/second");
        assert!(!heads[1].keep_alive());
        assert!(reader.parse_events().unwrap().is_empty());
    }

    #[test]
    fn parses_chunked_bodies_extensions_and_trailers() {
        let mut reader = ConnectionReader::default();
        reader
            .push(
                b"POST /echo HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n4\r\nWiki\r\n5;source=test\r\npedia\r\n0\r\nX-Checksum: abc123\r\n\r\nGET /next HTTP/1.1\r\n\r\n",
            )
            .unwrap();

        let events = reader.parse_events().unwrap();
        let body = events
            .iter()
            .filter_map(|event| match event {
                InboundEvent::Data(chunk) => Some(chunk.as_slice()),
                _ => None,
            })
            .flatten()
            .copied()
            .collect::<Vec<_>>();
        let trailers = events.iter().find_map(|event| match event {
            InboundEvent::End(trailers) if !trailers.is_empty() => Some(trailers),
            _ => None,
        });
        let next = events.iter().find_map(|event| match event {
            InboundEvent::Head(request) if request.url == "/next" => Some(request),
            _ => None,
        });

        assert_eq!(body, b"Wikipedia");
        assert_eq!(
            trailers.unwrap(),
            &vec![("x-checksum".into(), "abc123".into())]
        );
        assert_eq!(next.unwrap().url, "/next");
    }
}
