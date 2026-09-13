use std::{
    collections::{HashMap, VecDeque},
    io::{Read, Write},
    net::ToSocketAddrs,
    time::Duration,
};

use base64::Engine;
use mio::{Events, Interest, Poll, Token, net::TcpStream};
use rquickjs::function::This;
use rquickjs::{Array, Ctx, Exception, Function, Object, Result};

const LISTENER: Token = Token(0);

struct Connection<'js> {
    stream: TcpStream,
    socket: Object<'js>,
    outgoing: VecDeque<Vec<u8>>,
    written: usize,
    read_closed: bool,
    write_closed: bool,
    registered: bool,
}

pub fn listen<'js>(
    context: Ctx<'js>,
    port: u16,
    host: String,
    on_connection: Function<'js>,
    on_listening: Function<'js>,
    socket_factory: Function<'js>,
) -> Result<()> {
    let address = (host.as_str(), port)
        .to_socket_addrs()
        .map_err(|error| Exception::throw_message(&context, &error.to_string()))?
        .next()
        .ok_or_else(|| Exception::throw_message(&context, "TCP listen address not found"))?;
    let mut listener = mio::net::TcpListener::bind(address)
        .map_err(|error| Exception::throw_message(&context, &error.to_string()))?;
    let bound_address = listener
        .local_addr()
        .map_err(|error| Exception::throw_message(&context, &error.to_string()))?;
    let mut poll =
        Poll::new().map_err(|error| Exception::throw_message(&context, &error.to_string()))?;
    poll.registry()
        .register(&mut listener, LISTENER, Interest::READABLE)
        .map_err(|error| Exception::throw_message(&context, &error.to_string()))?;
    let mut events = Events::with_capacity(1024);
    let mut connections = HashMap::new();
    let mut next_token = 1_usize;
    let mut closing = false;
    let run_timers: Function = context.globals().get("__canaryoRunTimers")?;
    let restore_timer_context: Function = context.globals().get("__canaryoRestoreTimerContext")?;
    let poll_async_io: Function = context.globals().get("__canaryoPollHttpRequests")?;
    let should_close: Function = context.globals().get("__canaryoServerShouldClose")?;
    on_listening.call::<_, ()>((
        bound_address.ip().to_string(),
        if bound_address.is_ipv6() {
            "IPv6"
        } else {
            "IPv4"
        },
        bound_address.port(),
    ))?;

    loop {
        while context.execute_pending_job() {}
        let pending_async_io = poll_async_io.call::<_, usize>(())?;
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
        progress_connections(&context, poll.registry(), &mut connections)?;

        if !closing && should_close.call::<_, bool>(())? {
            closing = true;
            let _ = poll.registry().deregister(&mut listener);
        }
        if closing && connections.is_empty() {
            break;
        }

        let poll_delay = if pending_async_io > 0 {
            Some(
                timer_delay
                    .unwrap_or(Duration::from_millis(2))
                    .min(Duration::from_millis(2)),
            )
        } else {
            timer_delay
        };
        poll.poll(&mut events, poll_delay)
            .map_err(|error| Exception::throw_message(&context, &error.to_string()))?;

        for event in &events {
            if event.token() == LISTENER {
                if !closing {
                    accept_connections(
                        &context,
                        &socket_factory,
                        &on_connection,
                        &mut listener,
                        poll.registry(),
                        &mut connections,
                        &mut next_token,
                    )?;
                }
                continue;
            }

            let token = event.token();
            let mut remove = event.is_error();
            if let Some(connection) = connections.get_mut(&token) {
                if event.is_readable() && !connection.read_closed {
                    remove |= read_connection(&context, connection)?;
                }
                collect_socket_commands(&context, connection)?;
                if event.is_writable() && !remove {
                    remove |= flush_connection(&context, connection)?;
                }
                finish_writable_side(connection);
                remove |= connection.read_closed && connection.write_closed;
            }
            if remove {
                remove_connection(&mut connections, token)?;
            } else if let Some(connection) = connections.get_mut(&token) {
                update_interest(poll.registry(), token, connection)?;
            }
        }
    }

    Ok(())
}

fn accept_connections<'js>(
    context: &Ctx<'js>,
    socket_factory: &Function<'js>,
    on_connection: &Function<'js>,
    listener: &mut mio::net::TcpListener,
    registry: &mio::Registry,
    connections: &mut HashMap<Token, Connection<'js>>,
    next_token: &mut usize,
) -> Result<()> {
    loop {
        match listener.accept() {
            Ok((mut stream, remote)) => {
                let token = Token(*next_token);
                *next_token = next_token.wrapping_add(1).max(1);
                let local = stream.local_addr().ok();
                let _ = stream.set_nodelay(true);
                registry
                    .register(&mut stream, token, Interest::READABLE)
                    .map_err(|error| Exception::throw_message(context, &error.to_string()))?;
                let family = if remote.is_ipv6() { "IPv6" } else { "IPv4" };
                let socket = socket_factory.call::<_, Object>((
                    remote.ip().to_string(),
                    family,
                    remote.port(),
                    local
                        .map(|address| address.ip().to_string())
                        .unwrap_or_default(),
                    local.map_or(0, |address| address.port()),
                ))?;
                connections.insert(
                    token,
                    Connection {
                        stream,
                        socket: socket.clone(),
                        outgoing: VecDeque::new(),
                        written: 0,
                        read_closed: false,
                        write_closed: false,
                        registered: true,
                    },
                );
                on_connection.call::<_, ()>((socket,))?;
                while context.execute_pending_job() {}
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => return Ok(()),
            Err(error) => {
                return Err(Exception::throw_message(context, &error.to_string()));
            }
        }
    }
}

fn progress_connections<'js>(
    context: &Ctx<'js>,
    registry: &mio::Registry,
    connections: &mut HashMap<Token, Connection<'js>>,
) -> Result<()> {
    let tokens = connections.keys().copied().collect::<Vec<_>>();
    for token in tokens {
        let mut remove = false;
        if let Some(connection) = connections.get_mut(&token) {
            collect_socket_commands(context, connection)?;
            remove |= flush_connection(context, connection)?;
            finish_writable_side(connection);
            remove |= connection.read_closed && connection.write_closed;
            if !remove {
                update_interest(registry, token, connection)?;
            }
        }
        if remove {
            remove_connection(connections, token)?;
        }
    }
    Ok(())
}

fn collect_socket_commands<'js>(
    context: &Ctx<'js>,
    connection: &mut Connection<'js>,
) -> Result<()> {
    let chunks: Array = connection.socket.get("__canaryoServerOutgoing")?;
    connection
        .socket
        .set("__canaryoServerOutgoing", Array::new(context.clone())?)?;
    for chunk in chunks.iter::<String>() {
        let body = base64::engine::general_purpose::STANDARD
            .decode(chunk?)
            .map_err(|error| Exception::throw_message(context, &error.to_string()))?;
        if !body.is_empty() {
            connection.outgoing.push_back(body);
        }
    }
    if connection.socket.get::<_, bool>("__canaryoServerEnd")? {
        connection.socket.set("__canaryoServerEnd", false)?;
        connection.socket.set("__canaryoServerEnding", true)?;
    }
    if connection.socket.get::<_, bool>("__canaryoServerDestroy")? {
        connection.read_closed = true;
        connection.write_closed = true;
    }
    Ok(())
}

fn read_connection<'js>(context: &Ctx<'js>, connection: &mut Connection<'js>) -> Result<bool> {
    let mut input = [0_u8; 16 * 1024];
    loop {
        match connection.stream.read(&mut input) {
            Ok(0) => {
                connection.read_closed = true;
                let receive_end: Function = connection.socket.get("__canaryoNetReceiveEnd")?;
                receive_end.call::<_, ()>((This(connection.socket.clone()),))?;
                while context.execute_pending_job() {}
                return Ok(false);
            }
            Ok(length) => {
                let bytes = Array::new(context.clone())?;
                for (index, byte) in input[..length].iter().enumerate() {
                    bytes.set(index, *byte)?;
                }
                let receive: Function = connection.socket.get("__canaryoNetReceiveBytes")?;
                receive.call::<_, ()>((This(connection.socket.clone()), bytes))?;
                while context.execute_pending_job() {}
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => return Ok(false),
            Err(error) => {
                emit_socket_error(context, &connection.socket, &error.to_string())?;
                return Ok(true);
            }
        }
    }
}

fn flush_connection<'js>(context: &Ctx<'js>, connection: &mut Connection<'js>) -> Result<bool> {
    while let Some(chunk) = connection.outgoing.front() {
        match connection.stream.write(&chunk[connection.written..]) {
            Ok(0) => return Ok(true),
            Ok(length) => {
                connection.written += length;
                if connection.written == chunk.len() {
                    connection.outgoing.pop_front();
                    connection.written = 0;
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => return Ok(false),
            Err(error) => {
                emit_socket_error(context, &connection.socket, &error.to_string())?;
                return Ok(true);
            }
        }
    }
    Ok(false)
}

fn finish_writable_side(connection: &mut Connection<'_>) {
    if connection
        .socket
        .get::<_, bool>("__canaryoServerEnding")
        .unwrap_or(false)
        && connection.outgoing.is_empty()
        && !connection.write_closed
    {
        let _ = connection.stream.shutdown(std::net::Shutdown::Write);
        connection.write_closed = true;
    }
}

fn update_interest(
    registry: &mio::Registry,
    token: Token,
    connection: &mut Connection<'_>,
) -> Result<()> {
    let wants_read = !connection.read_closed;
    let wants_write = !connection.outgoing.is_empty();
    let interest = match (wants_read, wants_write) {
        (true, true) => Some(Interest::READABLE | Interest::WRITABLE),
        (true, false) => Some(Interest::READABLE),
        (false, true) => Some(Interest::WRITABLE),
        (false, false) => None,
    };
    match (connection.registered, interest) {
        (true, Some(interest)) => registry
            .reregister(&mut connection.stream, token, interest)
            .map_err(|error| Exception::throw_message(connection.socket.ctx(), &error.to_string())),
        (false, Some(interest)) => {
            registry
                .register(&mut connection.stream, token, interest)
                .map_err(|error| {
                    Exception::throw_message(connection.socket.ctx(), &error.to_string())
                })?;
            connection.registered = true;
            Ok(())
        }
        (true, None) => {
            registry
                .deregister(&mut connection.stream)
                .map_err(|error| {
                    Exception::throw_message(connection.socket.ctx(), &error.to_string())
                })?;
            connection.registered = false;
            Ok(())
        }
        (false, None) => Ok(()),
    }
}

fn emit_socket_error<'js>(context: &Ctx<'js>, socket: &Object<'js>, message: &str) -> Result<()> {
    let emit_error: Function = socket.get("__canaryoNetError")?;
    emit_error.call::<_, ()>((This(socket.clone()), message))?;
    while context.execute_pending_job() {}
    Ok(())
}

fn remove_connection(connections: &mut HashMap<Token, Connection<'_>>, token: Token) -> Result<()> {
    if let Some(connection) = connections.remove(&token) {
        let close: Function = connection.socket.get("__canaryoNetClose")?;
        close.call::<_, ()>((This(connection.socket),))?;
    }
    Ok(())
}
