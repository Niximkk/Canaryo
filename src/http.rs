use std::{
    borrow::Cow,
    cell::RefCell,
    collections::{HashMap, VecDeque},
    io::{Read, Write},
    net::{Ipv4Addr, SocketAddr, SocketAddrV4},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use base64::Engine;
use mio::{Events, Interest, Poll, Token, net::TcpStream};
use rquickjs::function::This;
use rquickjs::object::Property;
use rquickjs::{
    Array, Coerced, Ctx, Error, Exception, Function, Object, Result, TypedArray, Value,
};
use smallvec::SmallVec;

const DEFAULT_MAX_REQUEST_SIZE: usize = 1024 * 1024;
const MAX_HEADER_SIZE: usize = 64 * 1024;
const MAX_ASYNC_RESPONSE_WAIT: Duration = Duration::from_secs(30);
const KEEP_ALIVE_SPIN_WINDOW: Duration = Duration::from_micros(100);
const LISTENER: Token = Token(0);
const REQUEST_TOO_LARGE_MESSAGE: &str = "request body exceeds the configured limit";
static SERVER_BUSY: AtomicBool = AtomicBool::new(false);
static NATIVE_EXPRESS_PLANS_VALID: AtomicBool = AtomicBool::new(false);
struct ResponseBodyCache {
    next_id: u64,
    bodies: HashMap<u64, Arc<[u8]>>,
    responses: HashMap<u64, CachedResponse>,
    json_envelopes: HashMap<Vec<u8>, JsonEnvelope>,
}

thread_local! {
    static RESPONSE_BODY_CACHE: RefCell<ResponseBodyCache> = RefCell::new(ResponseBodyCache {
        next_id: 1,
        bodies: HashMap::new(),
        responses: HashMap::new(),
        json_envelopes: HashMap::new(),
    });
}
type RequestHeaders = SmallVec<[(Cow<'static, str>, Cow<'static, str>); 4]>;
type ResponseHeaders = SmallVec<[(String, String); 8]>;

pub(crate) fn mark_server_busy() {
    SERVER_BUSY.store(true, Ordering::Relaxed);
}

struct RequestHead {
    method: Cow<'static, str>,
    url: Cow<'static, str>,
    version: Cow<'static, str>,
    headers: RequestHeaders,
}

enum InboundEvent {
    Head(RequestHead),
    Complete(RequestHead),
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

#[derive(Clone)]
struct CachedResponse {
    status: u16,
    status_message: String,
    header_state: u64,
    headers: Arc<[(String, String)]>,
    body: Arc<[u8]>,
}

#[derive(Clone)]
struct JsonEnvelope {
    id: u64,
    length: usize,
    etag: String,
}

#[derive(Default)]
struct Response {
    status: u16,
    status_message: String,
    headers: Arc<[(String, String)]>,
    body: Arc<[u8]>,
}

pub(crate) fn cache_response_body(body: String) -> u64 {
    RESPONSE_BODY_CACHE.with_borrow_mut(|cache| {
        let id = cache.next_id;
        cache.next_id = cache.next_id.wrapping_add(1).max(1);
        cache.bodies.insert(id, Arc::from(body.into_bytes()));
        id
    })
}

pub(crate) fn release_response_body(id: u64) {
    RESPONSE_BODY_CACHE.with_borrow_mut(|cache| {
        cache.bodies.remove(&id);
        cache.responses.remove(&id);
    });
}

pub(crate) fn invalidate_native_express_plans() {
    NATIVE_EXPRESS_PLANS_VALID.store(false, Ordering::Release);
}

fn cached_response_body(id: u64) -> Option<Arc<[u8]>> {
    RESPONSE_BODY_CACHE.with_borrow(|cache| cache.bodies.get(&id).cloned())
}

fn cached_response(id: u64) -> Option<CachedResponse> {
    RESPONSE_BODY_CACHE.with_borrow(|cache| cache.responses.get(&id).cloned())
}

fn cache_response(id: u64, response: CachedResponse) {
    RESPONSE_BODY_CACHE.with_borrow_mut(|cache| {
        cache.responses.insert(id, response);
    });
}

fn cache_json_envelope(canonical: &str) -> Option<JsonEnvelope> {
    RESPONSE_BODY_CACHE.with_borrow_mut(|cache| {
        if let Some(envelope) = cache.json_envelopes.get(canonical.as_bytes()) {
            return Some(envelope.clone());
        }
        if cache.json_envelopes.len() >= 32 {
            return None;
        }
        let mut body = Vec::with_capacity(canonical.len() + 9);
        body.extend_from_slice(b"{\"body\":");
        body.extend_from_slice(canonical.as_bytes());
        body.push(b'}');
        let length = body.len();
        let digest = ring::digest::digest(&ring::digest::SHA1_FOR_LEGACY_USE_ONLY, &body);
        let encoded = base64::engine::general_purpose::STANDARD.encode(digest.as_ref());
        let id = cache.next_id;
        cache.next_id = cache.next_id.wrapping_add(1).max(1);
        cache.bodies.insert(id, Arc::from(body));
        let envelope = JsonEnvelope {
            id,
            length,
            etag: format!("W/\"{length:x}-{}\"", encoded.trim_end_matches('=')),
        };
        cache
            .json_envelopes
            .insert(canonical.as_bytes().to_vec(), envelope.clone());
        Some(envelope)
    })
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
    incoming_json_body: Option<Vec<u8>>,
    pending_responses: VecDeque<PendingResponse<'js>>,
    reader: ConnectionReader,
    inbound_events: Vec<InboundEvent>,
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

struct NativeExpressStep<'js> {
    handle: Function<'js>,
    expects_error: bool,
    terminal_next: bool,
    direct_json_parser: bool,
    route: Option<Object<'js>>,
    route_start: bool,
}

struct NativeExpressPlan<'js> {
    steps: Vec<NativeExpressStep<'js>>,
}

struct NativeExpressRun<'a, 'js> {
    original_url: &'a str,
    start_index: usize,
    error: Option<Value<'js>>,
    initialize: bool,
}

struct ServerBindings<'js> {
    handler: Function<'js>,
    on_listening: Function<'js>,
    max_connections: Function<'js>,
    connection_dropped: Function<'js>,
    request_prototype: Object<'js>,
    response_prototype: Object<'js>,
    socket_template: Object<'js>,
    native_express_plans: HashMap<String, HashMap<String, NativeExpressPlan<'js>>>,
    native_express_next: Option<Function<'js>>,
    native_express_direct_headers: bool,
    native_express_x_powered_by: bool,
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
    let native_plan_values: Option<Array> = handler.get("__canaryoNativePlans")?;
    let mut native_express_plans = HashMap::new();
    if let Some(native_plan_values) = native_plan_values {
        for plan_value in native_plan_values.iter::<Object>() {
            let plan_value = plan_value?;
            let key: String = plan_value.get("key")?;
            let step_values: Array = plan_value.get("steps")?;
            let mut steps = Vec::with_capacity(step_values.len());
            for step_value in step_values.iter::<Object>() {
                let step_value = step_value?;
                steps.push(NativeExpressStep {
                    handle: step_value.get("handle")?,
                    expects_error: step_value.get("expectsError")?,
                    terminal_next: step_value.get("terminalNext")?,
                    direct_json_parser: step_value.get("directJsonParser")?,
                    route: step_value.get("route")?,
                    route_start: step_value.get("routeStart")?,
                });
            }
            if let Some((method, path)) = key.split_once('\0') {
                native_express_plans
                    .entry(method.to_string())
                    .or_insert_with(HashMap::new)
                    .insert(path.to_string(), NativeExpressPlan { steps });
            }
        }
    }
    let native_express_next = handler.get("__canaryoNativeNext")?;
    let native_express_direct_headers = handler
        .get::<_, Option<bool>>("__canaryoNativeDirectHeaders")?
        .unwrap_or(false);
    let native_express_x_powered_by = handler
        .get::<_, Option<bool>>("__canaryoNativeXPoweredBy")?
        .unwrap_or(false);
    let socket_template = socket_template(&context, &socket_prototype, tls_config.is_some())?;
    NATIVE_EXPRESS_PLANS_VALID.store(!native_express_plans.is_empty(), Ordering::Release);
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
            socket_template,
            native_express_plans,
            native_express_next,
            native_express_direct_headers,
            native_express_x_powered_by,
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
    let restore_timer_context: Function = context.globals().get("__canaryoRestoreTimerContext")?;
    let server_tick: Function = context.globals().get("__canaryoServerTick")?;
    let mut closing = false;
    let mut keep_alive_spin_until = None;
    let request_template = request_template(&context, &bindings.request_prototype)?;
    let response_template = response_template(&context, &bindings.response_prototype)?;
    if let Some(next) = bindings.native_express_next.as_ref() {
        request_template.set("next", next.clone())?;
        request_template.set("baseUrl", "")?;
    }
    bindings.on_listening.call::<_, ()>(())?;

    loop {
        while context.execute_pending_job() {}
        let server_was_busy = SERVER_BUSY.swap(false, Ordering::AcqRel);
        let (pending_http_requests, control, timer_result) = if server_was_busy {
            let tick = server_tick.call::<_, i64>(())?;
            let flags = (tick & 15) as u8;
            let pending = flags & 8 != 0;
            let control = flags & 7;
            let timer = (tick >> 4) - 2_147_483_648;
            if pending || control != 0 || timer != -1 {
                SERVER_BUSY.store(true, Ordering::Release);
            }
            (pending, control, timer)
        } else {
            (false, 0, -1)
        };
        let timer_delay = match timer_result {
            -1 | -2 => None,
            value if value < -2 => Some(Duration::from_millis((-value - 3) as u64)),
            value => Some(Duration::from_millis(value as u64)),
        };
        let busy_before_jobs = SERVER_BUSY.load(Ordering::Acquire);
        while context.execute_pending_job() {}
        if timer_result < -1 {
            restore_timer_context.call::<_, ()>(())?;
        }
        if control == 0 && !busy_before_jobs && SERVER_BUSY.load(Ordering::Acquire) {
            continue;
        }
        if connections.values().any(Connection::needs_progress) {
            progress_connections(poll.registry(), &mut connections)?;
        }
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
        let poll_delay = if pending_http_requests {
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
        let poll_delay = if connections.len() == 1
            && keep_alive_spin_until.is_some_and(|deadline| Instant::now() < deadline)
        {
            Some(Duration::ZERO)
        } else {
            keep_alive_spin_until = None;
            poll_delay
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
            if let Some(connection) = connections.get_mut(&token) {
                if event.is_readable() && !remove && !connection.request_paused()? {
                    let read_closed = read_and_dispatch_requests(
                        &context,
                        &bindings,
                        &request_template,
                        &response_template,
                        connection,
                    )?;
                    connection.read_closed |= read_closed || event.is_read_closed();
                    connection.read_paused = connection.request_paused()?;
                    let wrote_response = !connection.outgoing.is_empty();
                    if connection.flush().is_err() {
                        remove = true;
                    } else if wrote_response
                        && connection.outgoing.is_empty()
                        && !connection.close_after_write
                    {
                        keep_alive_spin_until = Some(Instant::now() + KEEP_ALIVE_SPIN_WINDOW);
                    }
                }

                if event.is_writable() && !remove && connection.flush().is_err() {
                    remove = true;
                }
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
                            socket: socket_to_js(context, &bindings.socket_template)?,
                            incoming_request: None,
                            incoming_json_body: None,
                            pending_responses: VecDeque::new(),
                            reader: ConnectionReader::new(bindings.options.max_request_size),
                            inbound_events: Vec::with_capacity(4),
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
        self.inbound_events.clear();
        self.reader.parse_events(&mut self.inbound_events)?;
        if !self.inbound_events.is_empty() {
            return Ok(ConnectionRead::Events);
        }

        let mut chunk = [0_u8; 16 * 1024];
        match self.stream.read(&mut chunk) {
            Ok(0) => Ok(ConnectionRead::Closed),
            Ok(read) => {
                self.reader.push(&chunk[..read])?;
                self.reader.parse_events(&mut self.inbound_events)?;
                if self.inbound_events.is_empty() {
                    Ok(ConnectionRead::Progress)
                } else {
                    Ok(ConnectionRead::Events)
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
    Events,
    Progress,
    WouldBlock,
    Closed,
}

fn read_and_dispatch_requests<'js>(
    context: &Ctx<'js>,
    bindings: &ServerBindings<'js>,
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
                            body: Arc::from(&b"Payload Too Large"[..]),
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
            ConnectionRead::Events => {
                let mut events = std::mem::take(&mut connection.inbound_events);
                let mut progressed_at_end = false;
                for event in events.drain(..) {
                    let ends_request =
                        matches!(&event, InboundEvent::Complete(_) | InboundEvent::End(_));
                    dispatch_inbound_event(
                        context,
                        bindings,
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
                connection.inbound_events = events;
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
    connection.incoming_json_body = None;
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
    bindings: &ServerBindings<'js>,
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
                bindings,
                &request,
                request_prototype,
                response_prototype,
                &connection.socket,
            )?;
            connection.incoming_json_body = request_object
                .contains_key("__canaryoDirectJsonNext")?
                .then(Vec::new);
            connection.incoming_request = Some(request_object);
            connection.pending_responses.push_back(PendingResponse {
                object: response_object,
                keep_alive,
                suppress_body,
                deadline: Instant::now() + MAX_ASYNC_RESPONSE_WAIT,
            });
        }
        InboundEvent::Complete(request) => {
            let keep_alive = request.keep_alive();
            let suppress_body = request.method == "HEAD";
            let (request_object, response_object) = begin_request(
                context,
                bindings,
                &request,
                request_prototype,
                response_prototype,
                &connection.socket,
            )?;
            if request_object.contains_key("__canaryoDirectJsonNext")? {
                if let Some((next_index, error)) =
                    finish_direct_json_body(context, &request_object, Vec::new(), &[])?
                {
                    resume_native_express_plan(
                        context,
                        bindings,
                        next_index,
                        error,
                        &request_object,
                        &response_object,
                    )?;
                }
            } else {
                finish_request_body(context, request_object, &[])?;
            }
            if response_object.get::<_, bool>("writableEnded")? {
                let response = response_from_js(&response_object)?;
                append_response(
                    &mut connection.outgoing,
                    &response,
                    keep_alive,
                    suppress_body,
                );
                connection.close_after_write |= !keep_alive;
            } else {
                connection.pending_responses.push_back(PendingResponse {
                    object: response_object,
                    keep_alive,
                    suppress_body,
                    deadline: Instant::now() + MAX_ASYNC_RESPONSE_WAIT,
                });
            }
        }
        InboundEvent::Data(body) => {
            if let Some(json_body) = connection.incoming_json_body.as_mut() {
                json_body.extend_from_slice(&body);
                return Ok(());
            }
            let request = connection.incoming_request.as_ref().ok_or_else(|| {
                Exception::throw_message(context, "HTTP body arrived without request headers")
            })?;
            deliver_request_chunk(context, request, &body)?;
        }
        InboundEvent::End(trailers) => {
            let request = connection.incoming_request.take().ok_or_else(|| {
                Exception::throw_message(context, "HTTP request ended without request headers")
            })?;
            if let Some(body) = connection.incoming_json_body.take() {
                let resume = finish_direct_json_body(context, &request, body, &trailers)?;
                if let Some((next_index, error)) = resume {
                    let response = connection
                        .pending_responses
                        .back()
                        .ok_or_else(|| {
                            Exception::throw_message(
                                context,
                                "native Express request has no pending response",
                            )
                        })?
                        .object
                        .clone();
                    resume_native_express_plan(
                        context, bindings, next_index, error, &request, &response,
                    )?;
                }
            } else {
                finish_request_body(context, request, &trailers)?;
            }
        }
    }
    Ok(())
}

fn finish_direct_json_body<'js>(
    context: &Ctx<'js>,
    request: &Object<'js>,
    body: Vec<u8>,
    trailers: &[(String, String)],
) -> Result<Option<(usize, Option<Value<'js>>)>> {
    let next: Function = request.get("__canaryoDirectJsonNext")?;
    let native_next_index: Option<usize> = request.get("__canaryoNativePlanIndex")?;
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
    request.set("readable", false)?;
    request.set("readableEnded", true)?;
    request.set("complete", true)?;
    if body.is_empty() {
        request.set("body", Object::new(context.clone())?)?;
        return continue_after_direct_json(next, native_next_index, None);
    }

    let first = body
        .iter()
        .position(|byte| !matches!(byte, b' ' | b'\t' | b'\n' | b'\r'));
    if body.len() > 102400 || !matches!(first.map(|index| body[index]), Some(b'{' | b'[')) {
        let error = Exception::from_message(
            context.clone(),
            if body.len() > 102400 {
                "request entity too large"
            } else {
                "Unexpected token in JSON body"
            },
        )?
        .into_object();
        let status = if body.len() > 102400 { 413 } else { 400 };
        error.set("status", status)?;
        error.set("statusCode", status)?;
        error.set(
            "type",
            if status == 413 {
                "entity.too.large"
            } else {
                "entity.parse.failed"
            },
        )?;
        return continue_after_direct_json(next, native_next_index, Some(error.into_value()));
    }

    let canonical = canonical_flat_json_source(&body);
    match context.json_parse(body) {
        Ok(value) => {
            if let (Some((canonical, property_count)), Some(object)) =
                (canonical, value.as_object())
                && value.as_array().is_none()
            {
                let snapshot = Array::new(context.clone())?;
                let mut index = 0;
                let mut cacheable = true;
                for property in object.props::<String, Value>() {
                    let (name, value) = property?;
                    if name.starts_with("__canaryoCanonicalFlatJson")
                        || (!name.is_empty() && name.bytes().all(|byte| byte.is_ascii_digit()))
                        || !(value.is_null() || value.is_bool() || value.is_string())
                    {
                        cacheable = false;
                        break;
                    }
                    snapshot.set(index, name)?;
                    snapshot.set(index + 1, value)?;
                    index += 2;
                }
                if cacheable && index / 2 == property_count {
                    let envelope = cache_json_envelope(&canonical);
                    object.prop(
                        "__canaryoCanonicalFlatJsonSnapshot",
                        Property::from(snapshot),
                    )?;
                    if let Some(envelope) = envelope {
                        object.prop("__canaryoCanonicalEnvelopeId", Property::from(envelope.id))?;
                        object.prop(
                            "__canaryoCanonicalEnvelopeLength",
                            Property::from(envelope.length),
                        )?;
                        object.prop(
                            "__canaryoCanonicalEnvelopeEtag",
                            Property::from(envelope.etag),
                        )?;
                    }
                }
            }
            request.set("body", value)?;
            continue_after_direct_json(next, native_next_index, None)
        }
        Err(_) => {
            let error = context.catch();
            if let Some(error_object) = error.as_object() {
                error_object.set("status", 400)?;
                error_object.set("statusCode", 400)?;
                error_object.set("type", "entity.parse.failed")?;
            }
            continue_after_direct_json(next, native_next_index, Some(error))
        }
    }
}

fn continue_after_direct_json<'js>(
    next: Function<'js>,
    native_next_index: Option<usize>,
    error: Option<Value<'js>>,
) -> Result<Option<(usize, Option<Value<'js>>)>> {
    if let Some(index) = native_next_index {
        return Ok(Some((index, error)));
    }
    if let Some(error) = error {
        next.call::<_, ()>((error,))?;
    } else {
        next.call::<_, ()>(())?;
    }
    Ok(None)
}

fn canonical_flat_json_source(body: &[u8]) -> Option<(String, usize)> {
    if body.first() != Some(&b'{') || body.last() != Some(&b'}') {
        return None;
    }
    let mut in_string = false;
    let mut escaped = false;
    let mut property_count = 0;
    for &byte in body {
        if in_string {
            if escaped {
                if !matches!(byte, b'"' | b'\\' | b'b' | b'f' | b'n' | b'r' | b't') {
                    return None;
                }
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                in_string = false;
            }
        } else if byte == b'"' {
            in_string = true;
        } else if byte == b':' {
            property_count += 1;
        } else if byte.is_ascii_whitespace() {
            return None;
        }
    }
    (!in_string && !escaped)
        .then(|| String::from_utf8(body.to_vec()).ok())
        .flatten()
        .map(|source| (source, property_count))
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
    bindings: &ServerBindings<'js>,
    request: &RequestHead,
    request_prototype: &Object<'js>,
    response_prototype: &Object<'js>,
    socket: &Object<'js>,
) -> Result<(Object<'js>, Object<'js>)> {
    let request_object = request_to_js(context, request, request_prototype, socket)?;
    let response_object = response_to_js(context, response_prototype)?;
    request_object.set("res", response_object.clone())?;
    response_object.set("req", request_object.clone())?;
    response_object.set("socket", socket.clone())?;
    response_object.set("connection", socket.clone())?;

    let query_index = request.url.find('?').unwrap_or(request.url.len());
    let pathname = &request.url[..query_index];
    let native_plan = bindings
        .native_express_plans
        .get(request.method.as_ref())
        .and_then(|plans| plans.get(pathname));
    if let Some(plan) = native_plan.filter(|_| {
        bindings.native_express_next.is_some() && NATIVE_EXPRESS_PLANS_VALID.load(Ordering::Acquire)
    }) {
        run_native_express_plan(
            context,
            bindings,
            plan,
            &request_object,
            &response_object,
            NativeExpressRun {
                original_url: request.url.as_ref(),
                start_index: 0,
                error: None,
                initialize: true,
            },
        )?;
    } else {
        bindings
            .handler
            .call::<_, ()>((request_object.clone(), response_object.clone()))?;
    }
    Ok((request_object, response_object))
}

fn run_native_express_plan<'js>(
    context: &Ctx<'js>,
    bindings: &ServerBindings<'js>,
    plan: &NativeExpressPlan<'js>,
    request: &Object<'js>,
    response: &Object<'js>,
    run: NativeExpressRun<'_, 'js>,
) -> Result<()> {
    let NativeExpressRun {
        original_url,
        start_index,
        error,
        initialize,
    } = run;
    let mut thrown = error;
    let next = bindings
        .native_express_next
        .as_ref()
        .expect("checked by caller");
    if initialize && bindings.native_express_x_powered_by {
        if bindings.native_express_direct_headers {
            let headers: Object = response.get("__canaryoHeaders")?;
            headers.set("x-powered-by", "Express")?;
        } else {
            let set_header: Function = response.get("setHeader")?;
            set_header.call::<_, ()>((This(response.clone()), "X-Powered-By", "Express"))?;
        }
    }
    if initialize {
        response.set("locals", Object::new_proto(context.clone(), None)?)?;
        request.set("originalUrl", original_url)?;
    }

    for (index, step) in plan.steps.iter().enumerate().skip(start_index) {
        if thrown.is_some() != step.expects_error {
            continue;
        }
        if step.route_start {
            request.set("params", Object::new(context.clone())?)?;
            request.set("route", step.route.clone())?;
        }

        let result = if let Some(error) = thrown.take() {
            step.handle
                .call::<_, Value>((error, request.clone(), response.clone(), next.clone()))
        } else if step.terminal_next || step.direct_json_parser {
            step.handle
                .call::<_, Value>((request.clone(), response.clone(), next.clone()))
        } else {
            step.handle
                .call::<_, Value>((request.clone(), response.clone()))
        };

        match result {
            Ok(value) => {
                if value.is_promise() {
                    return Err(Exception::throw_message(
                        context,
                        "native Express plan unexpectedly returned a promise",
                    ));
                }
                if step.direct_json_parser {
                    if request.contains_key("__canaryoDirectJsonNext")? {
                        request.set("__canaryoNativePlanIndex", index + 1)?;
                    }
                    return Ok(());
                }
                if !step.terminal_next {
                    return Ok(());
                }
            }
            Err(Error::Exception) => thrown = Some(context.catch()),
            Err(error) => return Err(error),
        }
    }

    if !response.get::<_, bool>("writableEnded")? {
        let status = thrown
            .as_ref()
            .and_then(Value::as_object)
            .and_then(|error| error.get::<_, Option<u16>>("statusCode").ok().flatten())
            .filter(|status| (400..=599).contains(status))
            .unwrap_or(if thrown.is_some() { 500 } else { 404 });
        response.set("statusCode", status)?;
        let set_header: Function = response.get("setHeader")?;
        set_header.call::<_, ()>((
            This(response.clone()),
            "content-type",
            "text/plain; charset=utf-8",
        ))?;
        let end: Function = response.get("end")?;
        let message = if thrown.is_some() {
            "Internal Server Error".to_string()
        } else {
            let method: String = request.get("method")?;
            let url: String = request.get("url")?;
            format!("Cannot {method} {url}")
        };
        end.call::<_, ()>((This(response.clone()), message))?;
    }
    Ok(())
}

fn resume_native_express_plan<'js>(
    context: &Ctx<'js>,
    bindings: &ServerBindings<'js>,
    next_index: usize,
    error: Option<Value<'js>>,
    request: &Object<'js>,
    response: &Object<'js>,
) -> Result<()> {
    let method: String = request.get("method")?;
    let url: String = request.get("url")?;
    let pathname = url.split_once('?').map_or(url.as_str(), |(path, _)| path);
    let Some(plan) = bindings
        .native_express_plans
        .get(method.as_str())
        .and_then(|plans| plans.get(pathname))
    else {
        return Err(Exception::throw_message(
            context,
            "native Express plan disappeared while reading the request body",
        ));
    };
    run_native_express_plan(
        context,
        bindings,
        plan,
        request,
        response,
        NativeExpressRun {
            original_url: &url,
            start_index: next_index,
            error,
            initialize: false,
        },
    )
}

fn request_to_js<'js>(
    context: &Ctx<'js>,
    request: &RequestHead,
    prototype: &Object<'js>,
    socket: &Object<'js>,
) -> Result<Object<'js>> {
    let object = Object::new_proto(context.clone(), Some(prototype))?;
    let headers = Object::new(context.clone())?;

    for (name, value) in &request.headers {
        headers.set(name.as_ref(), value.as_ref())?;
    }
    if request.method != "GET" {
        object.set("method", request.method.as_ref())?;
    }
    if request.url != "/" {
        object.set("url", request.url.as_ref())?;
    }
    if request.version != "1.1" {
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
        object.set("httpVersion", request.version.as_ref())?;
        object.set("httpVersionMajor", version_major)?;
        object.set("httpVersionMinor", version_minor)?;
    }
    object.set("headers", headers)?;
    object.set("socket", socket.clone())?;
    object.set("connection", socket.clone())?;
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
    let events: Option<Object> = request.get("_events")?;
    let has_completion_listeners = if let Some(events) = events {
        events.contains_key("end")? || events.contains_key("close")?
    } else {
        false
    };
    if !has_completion_listeners && !request.get::<_, bool>("__canaryoPaused")? {
        request.set("readable", false)?;
        request.set("readableEnded", true)?;
        request.set("complete", true)?;
        return Ok(());
    }
    let finish: Function = request.get("__canaryoFinishBody")?;
    finish.call::<_, ()>((This(request),))
}

fn socket_to_js<'js>(context: &Ctx<'js>, template: &Object<'js>) -> Result<Object<'js>> {
    Object::new_proto(context.clone(), Some(template))
}

fn socket_template<'js>(
    context: &Ctx<'js>,
    prototype: &Object<'js>,
    encrypted: bool,
) -> Result<Object<'js>> {
    let socket = Object::new_proto(context.clone(), Some(prototype))?;
    socket.set("remoteAddress", "127.0.0.1")?;
    socket.set("remoteFamily", "IPv4")?;
    socket.set("localAddress", "127.0.0.1")?;
    socket.set("encrypted", encrypted)?;
    socket.set("destroyed", false)?;
    socket.set("connecting", false)?;
    socket.set("readable", true)?;
    socket.set("writable", true)?;
    Ok(socket)
}

fn response_to_js<'js>(context: &Ctx<'js>, prototype: &Object<'js>) -> Result<Object<'js>> {
    let object = Object::new_proto(context.clone(), Some(prototype))?;
    let headers = Object::new(context.clone())?;
    object.set("__canaryoHeaders", headers)?;
    Ok(object)
}

fn request_template<'js>(context: &Ctx<'js>, prototype: &Object<'js>) -> Result<Object<'js>> {
    let template = Object::new_proto(context.clone(), Some(prototype))?;
    template.set("method", "GET")?;
    template.set("url", "/")?;
    template.set("httpVersion", "1.1")?;
    template.set("httpVersionMajor", 1)?;
    template.set("httpVersionMinor", 1)?;
    template.set("aborted", false)?;
    template.set("complete", false)?;
    template.set("destroyed", false)?;
    template.set("readable", true)?;
    template.set("readableEnded", false)?;
    template.set("__canaryoPaused", false)?;
    Ok(template)
}

fn response_template<'js>(context: &Ctx<'js>, prototype: &Object<'js>) -> Result<Object<'js>> {
    let template = Object::new_proto(context.clone(), Some(prototype))?;
    template.set("statusCode", 200)?;
    template.set("statusMessage", "")?;
    template.set("headersSent", false)?;
    template.set("writableEnded", false)?;
    template.set("writableFinished", false)?;
    template.set("finished", false)?;
    template.set("destroyed", false)?;
    template.set("__canaryoTextBody", "")?;
    template.set("__canaryoBody", Option::<Array>::None)?;
    Ok(template)
}

fn response_from_js(response: &Object<'_>) -> Result<Response> {
    let cached_body_id: Option<u64> = response.get("__canaryoCachedBodyId")?;
    let status: u16 = response.get("statusCode")?;
    let status_message: String = response.get("statusMessage")?;
    if let Some(cached) = cached_body_id.and_then(cached_response)
        && cached.status == status
        && cached.status_message == status_message
        && cached.header_state == response.get::<_, u64>("__canaryoHeaderState")?
    {
        return Ok(Response {
            status,
            status_message,
            headers: cached.headers,
            body: cached.body,
        });
    }

    let headers_object: Object = response.get("__canaryoHeaders")?;
    let mut headers = ResponseHeaders::new();
    let mut cacheable_headers = true;
    for property in headers_object.props::<String, rquickjs::Value>() {
        let (name, value) = property?;
        if name.eq_ignore_ascii_case("content-length") || name.eq_ignore_ascii_case("connection") {
            continue;
        }
        if let Some(values) = value.as_array() {
            cacheable_headers = false;
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
    let body = if let Some(body) = cached_body_id.and_then(cached_response_body) {
        body
    } else {
        let text_body: String = response.get("__canaryoTextBody")?;
        if text_body.is_empty() {
            let body_chunks: Option<Array> = response.get("__canaryoBody")?;
            let mut body = Vec::new();
            if let Some(body_chunks) = body_chunks {
                for chunk in body_chunks.iter::<TypedArray<u8>>() {
                    let chunk = chunk?;
                    let bytes = chunk.as_bytes().ok_or_else(|| {
                        Exception::throw_message(
                            response.ctx(),
                            "response contains a detached buffer",
                        )
                    })?;
                    body.extend_from_slice(bytes);
                }
            }
            Arc::from(body)
        } else {
            Arc::from(text_body.into_bytes())
        }
    };

    let headers: Arc<[(String, String)]> = Arc::from(headers.into_vec());
    if let Some(id) = cached_body_id
        && cacheable_headers
    {
        cache_response(
            id,
            CachedResponse {
                status,
                status_message: status_message.clone(),
                header_state: response.get("__canaryoHeaderState")?,
                headers: Arc::clone(&headers),
                body: Arc::clone(&body),
            },
        );
    }

    Ok(Response {
        status,
        status_message,
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
            .map(|(_, value)| value.as_ref());
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

    fn parse_events(&mut self, events: &mut Vec<InboundEvent>) -> std::io::Result<()> {
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
                    if matches!(body_state, RequestBodyState::Fixed { remaining: 0 }) {
                        self.state = RequestBodyState::Head;
                        events.push(InboundEvent::Complete(request));
                    } else {
                        self.state = body_state;
                        events.push(InboundEvent::Head(request));
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
        Ok(())
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
    let method = match required_part(request_parts.next(), "método HTTP ausente")? {
        "GET" => Cow::Borrowed("GET"),
        "POST" => Cow::Borrowed("POST"),
        "HEAD" => Cow::Borrowed("HEAD"),
        method => Cow::Owned(method.to_string()),
    };
    let url = match required_part(request_parts.next(), "URL ausente")? {
        "/" => Cow::Borrowed("/"),
        url => Cow::Owned(url.to_string()),
    };
    let version = match required_part(request_parts.next(), "versão HTTP ausente")?
        .trim_start_matches("HTTP/")
    {
        "1.1" => Cow::Borrowed("1.1"),
        "1.0" => Cow::Borrowed("1.0"),
        version => Cow::Owned(version.to_string()),
    };
    let mut headers = RequestHeaders::new();
    for line in lines {
        let (name, value) = line.split_once(':').ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, "cabeçalho HTTP inválido")
        })?;
        let name = name.trim();
        let name = if name.eq_ignore_ascii_case("host") {
            Cow::Borrowed("host")
        } else if name.eq_ignore_ascii_case("connection") {
            Cow::Borrowed("connection")
        } else if name.eq_ignore_ascii_case("content-type") {
            Cow::Borrowed("content-type")
        } else if name.eq_ignore_ascii_case("content-length") {
            Cow::Borrowed("content-length")
        } else if name.eq_ignore_ascii_case("transfer-encoding") {
            Cow::Borrowed("transfer-encoding")
        } else {
            Cow::Owned(name.to_ascii_lowercase())
        };
        let value = match value.trim() {
            "127.0.0.1" => Cow::Borrowed("127.0.0.1"),
            "keep-alive" => Cow::Borrowed("keep-alive"),
            "close" => Cow::Borrowed("close"),
            "application/json" => Cow::Borrowed("application/json"),
            "chunked" => Cow::Borrowed("chunked"),
            value => Cow::Owned(value.to_string()),
        };
        headers.push((name, value));
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

    for (name, value) in response.headers.iter() {
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
        buffer.extend_from_slice(response.body.as_ref());
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
    fn recognizes_canonical_flat_json_sources() {
        assert_eq!(
            canonical_flat_json_source(br#"{"data":"canaryo","ok":true,"empty":null}"#),
            Some((r#"{"data":"canaryo","ok":true,"empty":null}"#.into(), 3))
        );
        assert!(canonical_flat_json_source(br#"{ "data": "canaryo" }"#).is_none());
        assert!(canonical_flat_json_source(br#"{"data":"\u0063anaryo"}"#).is_none());
    }

    #[test]
    fn keeps_http_11_connections_open_by_default() {
        let request = RequestHead {
            method: "GET".into(),
            url: "/".into(),
            version: "1.1".into(),
            headers: SmallVec::new(),
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

        let mut events = Vec::new();
        reader.parse_events(&mut events).unwrap();
        let heads = events
            .iter()
            .filter_map(|event| match event {
                InboundEvent::Head(request) | InboundEvent::Complete(request) => Some(request),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(heads.len(), 2);
        assert_eq!(heads[0].url, "/first");
        assert!(heads[0].keep_alive());
        assert_eq!(heads[1].url, "/second");
        assert!(!heads[1].keep_alive());
        events.clear();
        reader.parse_events(&mut events).unwrap();
        assert!(events.is_empty());
    }

    #[test]
    fn parses_chunked_bodies_extensions_and_trailers() {
        let mut reader = ConnectionReader::default();
        reader
            .push(
                b"POST /echo HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n4\r\nWiki\r\n5;source=test\r\npedia\r\n0\r\nX-Checksum: abc123\r\n\r\nGET /next HTTP/1.1\r\n\r\n",
            )
            .unwrap();

        let mut events = Vec::new();
        reader.parse_events(&mut events).unwrap();
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
            InboundEvent::Head(request) | InboundEvent::Complete(request)
                if request.url == "/next" =>
            {
                Some(request)
            }
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
