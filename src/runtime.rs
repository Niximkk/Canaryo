use std::{
    env, fs,
    net::{IpAddr, ToSocketAddrs},
    path::Path,
};

use rquickjs::{Array, CatchResultExt, Context, Function, Module, Object, Promise, Runtime};
use sha1::{Digest, Sha1};

use crate::{esm, http, modules};

const BOOTSTRAP: &str = r#"
globalThis.global = globalThis;
(() => {
globalThis.console = Object.freeze({
    log(...values) {
        __canaryoPrint(values.map(formatValue).join(" "));
    },
    error(...values) {
        __canaryoPrintError(values.map(formatValue).join(" "));
    }
});

function formatValue(value) {
    if (typeof value === "string") return value;
    if (typeof value !== "object" || value === null) return String(value);
    try {
        return JSON.stringify(value);
    } catch {
        return String(value);
    }
}

function IncomingMessage() {}
function ServerResponse() {}
function Socket() {}

IncomingMessage.prototype.setEncoding = function(encoding) {
    this.__canaryoEncoding = String(encoding);
    return this;
};
IncomingMessage.prototype.pause = function() { return this; };
IncomingMessage.prototype.resume = function() { return this; };
IncomingMessage.prototype.__canaryoDeliverBody = function() {
    if (this.readableEnded) return;
    if (this.__canaryoBody.length > 0) {
        const buffer = Buffer.from(this.__canaryoBody);
        const chunk = this.__canaryoEncoding
            ? buffer.toString(this.__canaryoEncoding)
            : buffer;
        this.emit("data", chunk);
    }
    this.readable = false;
    this.readableEnded = true;
    this.complete = true;
    this.emit("end");
};

Socket.prototype.setTimeout = function(value, callback) {
    this.timeout = Number(value);
    if (typeof callback === "function") this.on("timeout", callback);
    return this;
};
Socket.prototype.ref = function() { return this; };
Socket.prototype.unref = function() { return this; };
Socket.prototype.destroy = function() {
    if (!this.destroyed) {
        this.destroyed = true;
        this.emit("close");
    }
    return this;
};

ServerResponse.prototype.setHeader = function(name, value) {
    this.__canaryoHeaders[String(name).toLowerCase()] = String(value);
    return this;
};
ServerResponse.prototype.getHeader = function(name) {
    return this.__canaryoHeaders[String(name).toLowerCase()];
};
ServerResponse.prototype.hasHeader = function(name) {
    return Object.prototype.hasOwnProperty.call(
        this.__canaryoHeaders,
        String(name).toLowerCase()
    );
};
ServerResponse.prototype.removeHeader = function(name) {
    delete this.__canaryoHeaders[String(name).toLowerCase()];
};
ServerResponse.prototype.getHeaders = function() {
    return Object.assign(Object.create(null), this.__canaryoHeaders);
};
ServerResponse.prototype.getHeaderNames = function() {
    return Object.keys(this.__canaryoHeaders);
};
ServerResponse.prototype.getRawHeaderNames = ServerResponse.prototype.getHeaderNames;
ServerResponse.prototype.flushHeaders = function() {
    this.headersSent = true;
};
ServerResponse.prototype.writeHead = function(status, statusMessageOrHeaders, headers) {
    this.statusCode = Number(status);
    if (typeof statusMessageOrHeaders === "string") {
        this.statusMessage = statusMessageOrHeaders;
    }
    const values = typeof statusMessageOrHeaders === "object"
        ? statusMessageOrHeaders
        : headers;
    if (values) {
        for (const [name, value] of Object.entries(values)) {
            this.setHeader(name, value);
        }
    }
    this.headersSent = true;
    return this;
};
function appendResponseChunk(response, chunk) {
    if (chunk === undefined || chunk === null) return;
    const bytes = Buffer.from(chunk);
    for (const byte of bytes) response.__canaryoBody.push(byte);
}
ServerResponse.prototype.write = function(chunk) {
    appendResponseChunk(this, chunk);
    this.headersSent = true;
    return true;
};
ServerResponse.prototype.end = function(chunk) {
    if (this.writableEnded) return this;
    appendResponseChunk(this, chunk);
    this.headersSent = true;
    this.writableEnded = true;
    this.writableFinished = true;
    this.finished = true;
    this.emit("finish");
    return this;
};

const httpModule = Object.freeze({
    IncomingMessage,
    ServerResponse,
    METHODS: ["GET", "HEAD", "POST", "PUT", "DELETE", "CONNECT", "OPTIONS", "TRACE", "PATCH"],
    STATUS_CODES: { 200: "OK", 201: "Created", 204: "No Content", 400: "Bad Request", 404: "Not Found", 500: "Internal Server Error" },
    createServer(optionsOrListener, listener) {
        const requestListener = typeof optionsOrListener === "function"
            ? optionsOrListener
            : listener;
        if (typeof requestListener !== "function") {
            throw new TypeError("createServer requer uma função");
        }

        const server = {
            listening: false,
            __canaryoCloseRequested: false,
            __canaryoCloseCallbacks: [],
            keepAliveTimeout: 5000,
            requestTimeout: 300000,
            timeout: 0,
            listen(port, hostOrCallback, callback) {
                const options = port !== null && typeof port === "object" ? port : null;
                const listenPort = Number(options ? options.port : port);
                const listenHost = options ? options.host : typeof hostOrCallback === "string" ? hostOrCallback : "127.0.0.1";
                const explicitCallback = typeof hostOrCallback === "function"
                    ? hostOrCallback
                    : typeof callback === "function"
                        ? callback
                        : null;
                this.__canaryoAddress = { address: listenHost || "127.0.0.1", family: "IPv4", port: listenPort };
                const currentServer = this;
                currentServer.__canaryoCloseRequested = false;
                globalThis.__canaryoActiveServer = currentServer;
                globalThis.__canaryoPendingServerStart = () => {
                    __canaryoListen(
                        listenPort,
                        requestListener,
                        () => {
                            currentServer.listening = true;
                            currentServer.emit("listening");
                            if (explicitCallback) explicitCallback();
                        },
                        IncomingMessage.prototype,
                        ServerResponse.prototype,
                        Socket.prototype
                    );
                    currentServer.listening = false;
                    const callbacks = currentServer.__canaryoCloseCallbacks.splice(0);
                    for (const closeCallback of callbacks) closeCallback();
                    currentServer.emit("close");
                };
                return this;
            },
            address() { return this.__canaryoAddress || null; },
            setTimeout(value, callback) {
                this.timeout = Number(value);
                if (typeof callback === "function") this.on("timeout", callback);
                return this;
            },
            ref() { return this; },
            unref() { return this; },
            close(callback) {
                if (typeof callback === "function") this.__canaryoCloseCallbacks.push(callback);
                this.__canaryoCloseRequested = true;
                return this;
            },
            closeAllConnections() { this.__canaryoCloseRequested = true; },
            closeIdleConnections() { this.__canaryoCloseRequested = true; }
        };
        const EventEmitter = __canaryoBuiltins.events.EventEmitter;
        Object.setPrototypeOf(IncomingMessage.prototype, EventEmitter.prototype);
        Object.setPrototypeOf(ServerResponse.prototype, EventEmitter.prototype);
        Object.setPrototypeOf(Socket.prototype, EventEmitter.prototype);
        EventEmitter.call(server);
        Object.setPrototypeOf(server, EventEmitter.prototype);
        return server;
    }
});

const moduleCache = Object.create(null);
const resolutionCache = new Map();

function loadModule(filename) {
    if (moduleCache[filename]) return moduleCache[filename].exports;

    const module = {
        id: filename,
        filename,
        exports: {},
        loaded: false,
        children: []
    };
    moduleCache[filename] = module;

    try {
        const source = __canaryoReadFile(filename);
        if (filename.endsWith(".json")) {
            module.exports = JSON.parse(source);
        } else {
            const wrapper = new Function(
                "require",
                "module",
                "exports",
                "__filename",
                "__dirname",
                `${source}\n//# sourceURL=${filename}`
            );
            wrapper(
                createRequire(filename),
                module,
                module.exports,
                filename,
                __canaryoDirname(filename)
            );
        }
        module.loaded = true;
        return module.exports;
    } catch (error) {
        delete moduleCache[filename];
        if (error && typeof error.stack === "string") {
            error.stack += `\n    while loading ${filename}`;
        }
        throw error;
    }
}

function createRequire(parentFilename) {
    function require(name) {
        if (name === "http" || name === "node:http") return httpModule;
        if (name === "https" || name === "node:https") {
            return { createServer() { throw new Error("node:https ainda não é suportado"); } };
        }
        if (name === "http2" || name === "node:http2") {
            return {
                constants: {},
                createServer() { throw new Error("node:http2 ainda não é suportado"); },
                createSecureServer() { throw new Error("node:http2 ainda não é suportado"); }
            };
        }
        const normalized = name.startsWith("node:") ? name.slice(5) : name;
        if (Object.prototype.hasOwnProperty.call(__canaryoBuiltins, normalized)) {
            return __canaryoBuiltins[normalized];
        }
        return loadModule(resolveModule(parentFilename, name));
    }

    require.resolve = name => resolveModule(parentFilename, name);
    require.cache = moduleCache;
    return require;
}

globalThis.__canaryoCreateRequire = createRequire;
globalThis.__canaryoModuleCache = moduleCache;

function resolveModule(parentFilename, name) {
    const separator = Math.max(
        parentFilename.lastIndexOf("/"),
        parentFilename.lastIndexOf("\\")
    );
    const key = `${parentFilename.slice(0, separator + 1)}\0${name}`;
    if (resolutionCache.has(key)) return resolutionCache.get(key);
    const filename = __canaryoResolve(parentFilename, name);
    resolutionCache.set(key, filename);
    return filename;
}

globalThis.__canaryoHttpModule = httpModule;
globalThis.__canaryoLoadCommonJS = filename => loadModule(filename);
globalThis.__canaryoRunMain = filename => loadModule(filename);
globalThis.__canaryoStartPendingServer = () => {
    const start = globalThis.__canaryoPendingServerStart;
    globalThis.__canaryoPendingServerStart = null;
    if (start) start();
};
globalThis.__canaryoServerShouldClose = () => Boolean(
    globalThis.__canaryoActiveServer && globalThis.__canaryoActiveServer.__canaryoCloseRequested
);
})();
"#;

const POLYFILLS: &str = include_str!("polyfills.js");

pub fn execute(path: &str, arguments: &[String]) -> Result<(), String> {
    fs::metadata(path).map_err(|error| format!("não foi possível ler {path}: {error}"))?;
    let entry = Path::new(path)
        .canonicalize()
        .map_err(|error| format!("não foi possível resolver {path}: {error}"))?;
    let runtime = Runtime::new().map_err(|error| format!("erro ao criar runtime: {error}"))?;
    runtime.set_gc_threshold(8 * 1024 * 1024);
    runtime.set_loader(esm::NodeResolver, esm::NodeLoader);
    let context =
        Context::full(&runtime).map_err(|error| format!("erro ao criar contexto: {error}"))?;

    context.with(|context| {
        install_host_globals(&context, path, arguments)?;
        context
            .eval::<(), _>(BOOTSTRAP)
            .catch(&context)
            .map_err(|error| error.to_string())?;
        context
            .eval::<(), _>(POLYFILLS)
            .catch(&context)
            .map_err(|error| error.to_string())?;
        let module_evaluation: Option<Promise> = if modules::is_esm_path(&entry) {
            let source = fs::read(&entry).map_err(|error| error.to_string())?;
            Some(
                Module::evaluate(context.clone(), entry.to_string_lossy().as_bytes(), source)
                    .catch(&context)
                    .map_err(|error| error.to_string())?,
            )
        } else {
            let run_main: Function = context
                .globals()
                .get("__canaryoRunMain")
                .map_err(|error| error.to_string())?;
            run_main
                .call::<_, ()>((entry.to_string_lossy().as_ref(),))
                .catch(&context)
                .map_err(|error| error.to_string())?;
            None
        };

        while context.execute_pending_job() {}
        let start_server: Function = context
            .globals()
            .get("__canaryoStartPendingServer")
            .map_err(|error| error.to_string())?;
        start_server
            .call::<_, ()>(())
            .catch(&context)
            .map_err(|error| error.to_string())?;
        if let Some(module_evaluation) = module_evaluation {
            module_evaluation
                .finish::<()>()
                .catch(&context)
                .map_err(|error| error.to_string())?;
        }
        while context.execute_pending_job() {}
        Ok(())
    })
}

fn install_host_globals<'js>(
    context: &rquickjs::Ctx<'js>,
    path: &str,
    arguments: &[String],
) -> Result<(), String> {
    let globals = context.globals();
    let print = Function::new(context.clone(), |message: String| println!("{message}"))
        .map_err(|error| error.to_string())?;
    let print_error = Function::new(context.clone(), |message: String| eprintln!("{message}"))
        .map_err(|error| error.to_string())?;
    globals
        .set("__canaryoPrint", print)
        .map_err(|error| error.to_string())?;
    globals
        .set("__canaryoPrintError", print_error)
        .map_err(|error| error.to_string())?;
    let write = Function::new(context.clone(), |message: String| {
        use std::io::Write;
        print!("{message}");
        let _ = std::io::stdout().flush();
    })
    .map_err(|error| error.to_string())?;
    let write_error = Function::new(context.clone(), |message: String| {
        use std::io::Write;
        eprint!("{message}");
        let _ = std::io::stderr().flush();
    })
    .map_err(|error| error.to_string())?;
    globals
        .set("__canaryoWrite", write)
        .map_err(|error| error.to_string())?;
    globals
        .set("__canaryoWriteError", write_error)
        .map_err(|error| error.to_string())?;
    let cwd = Function::new(context.clone(), cwd).map_err(|error| error.to_string())?;
    globals
        .set("__canaryoCwd", cwd)
        .map_err(|error| error.to_string())?;
    let hash = Function::new(context.clone(), hash).map_err(|error| error.to_string())?;
    globals
        .set("__canaryoHash", hash)
        .map_err(|error| error.to_string())?;
    let byte_length =
        Function::new(context.clone(), byte_length).map_err(|error| error.to_string())?;
    globals
        .set("__canaryoByteLength", byte_length)
        .map_err(|error| error.to_string())?;
    let is_ip = Function::new(context.clone(), is_ip).map_err(|error| error.to_string())?;
    globals
        .set("__canaryoIsIp", is_ip)
        .map_err(|error| error.to_string())?;
    let dns_lookup =
        Function::new(context.clone(), dns_lookup).map_err(|error| error.to_string())?;
    globals
        .set("__canaryoDnsLookup", dns_lookup)
        .map_err(|error| error.to_string())?;
    let os_info = Object::new(context.clone()).map_err(|error| error.to_string())?;
    os_info
        .set("platform", node_platform())
        .map_err(|error| error.to_string())?;
    os_info
        .set("arch", node_arch())
        .map_err(|error| error.to_string())?;
    os_info
        .set("type", os_type())
        .map_err(|error| error.to_string())?;
    os_info
        .set("tempDir", env::temp_dir().to_string_lossy().into_owned())
        .map_err(|error| error.to_string())?;
    os_info
        .set("homeDir", home_dir())
        .map_err(|error| error.to_string())?;
    os_info
        .set("hostname", hostname())
        .map_err(|error| error.to_string())?;
    os_info
        .set(
            "parallelism",
            std::thread::available_parallelism()
                .map(|value| value.get())
                .unwrap_or(1),
        )
        .map_err(|error| error.to_string())?;
    os_info
        .set(
            "endianness",
            if cfg!(target_endian = "little") {
                "LE"
            } else {
                "BE"
            },
        )
        .map_err(|error| error.to_string())?;
    globals
        .set("__canaryoOsInfo", os_info)
        .map_err(|error| error.to_string())?;
    let listen = Function::new(context.clone(), http::listen).map_err(|error| error.to_string())?;
    globals
        .set("__canaryoListen", listen)
        .map_err(|error| error.to_string())?;
    let read_file = Function::new(context.clone(), read_file).map_err(|error| error.to_string())?;
    let resolve = Function::new(context.clone(), resolve).map_err(|error| error.to_string())?;
    let dirname = Function::new(context.clone(), dirname).map_err(|error| error.to_string())?;
    globals
        .set("__canaryoReadFile", read_file)
        .map_err(|error| error.to_string())?;
    globals
        .set("__canaryoResolve", resolve)
        .map_err(|error| error.to_string())?;
    globals
        .set("__canaryoDirname", dirname)
        .map_err(|error| error.to_string())?;
    let fs_read = Function::new(context.clone(), fs_read).map_err(|error| error.to_string())?;
    let fs_write = Function::new(context.clone(), fs_write).map_err(|error| error.to_string())?;
    let fs_stat = Function::new(context.clone(), fs_stat).map_err(|error| error.to_string())?;
    let fs_exists = Function::new(context.clone(), fs_exists).map_err(|error| error.to_string())?;
    let fs_mkdir = Function::new(context.clone(), fs_mkdir).map_err(|error| error.to_string())?;
    let fs_readdir =
        Function::new(context.clone(), fs_readdir).map_err(|error| error.to_string())?;
    globals
        .set("__canaryoFsRead", fs_read)
        .map_err(|error| error.to_string())?;
    globals
        .set("__canaryoFsWrite", fs_write)
        .map_err(|error| error.to_string())?;
    globals
        .set("__canaryoFsStat", fs_stat)
        .map_err(|error| error.to_string())?;
    globals
        .set("__canaryoFsExists", fs_exists)
        .map_err(|error| error.to_string())?;
    globals
        .set("__canaryoFsMkdir", fs_mkdir)
        .map_err(|error| error.to_string())?;
    globals
        .set("__canaryoFsReaddir", fs_readdir)
        .map_err(|error| error.to_string())?;

    let process = Object::new(context.clone()).map_err(|error| error.to_string())?;
    let argv = Array::new(context.clone()).map_err(|error| error.to_string())?;
    argv.set(0, "canaryo").map_err(|error| error.to_string())?;
    argv.set(1, path).map_err(|error| error.to_string())?;
    for (index, argument) in arguments.iter().enumerate() {
        argv.set(index + 2, argument.as_str())
            .map_err(|error| error.to_string())?;
    }

    let environment = Object::new(context.clone()).map_err(|error| error.to_string())?;
    for (key, value) in env::vars() {
        environment
            .set(key, value)
            .map_err(|error| error.to_string())?;
    }

    process
        .set("argv", argv)
        .map_err(|error| error.to_string())?;
    process
        .set("env", environment)
        .map_err(|error| error.to_string())?;
    process
        .set("pid", std::process::id())
        .map_err(|error| error.to_string())?;
    process
        .set(
            "execPath",
            env::current_exe()
                .map(|path| path.to_string_lossy().into_owned())
                .unwrap_or_else(|_| "canaryo".into()),
        )
        .map_err(|error| error.to_string())?;
    globals
        .set("process", process)
        .map_err(|error| error.to_string())?;
    globals
        .set("__filename", path)
        .map_err(|error| error.to_string())?;
    globals
        .set(
            "__dirname",
            Path::new(path)
                .parent()
                .and_then(Path::to_str)
                .unwrap_or("."),
        )
        .map_err(|error| error.to_string())?;

    Ok(())
}

fn read_file<'js>(context: rquickjs::Ctx<'js>, path: String) -> rquickjs::Result<String> {
    fs::read_to_string(path)
        .map_err(|error| rquickjs::Exception::throw_message(&context, &error.to_string()))
}

fn is_ip(value: String) -> u8 {
    match value.parse::<IpAddr>() {
        Ok(IpAddr::V4(_)) => 4,
        Ok(IpAddr::V6(_)) => 6,
        Err(_) => 0,
    }
}

fn node_platform() -> &'static str {
    match env::consts::OS {
        "windows" => "win32",
        "macos" => "darwin",
        value => value,
    }
}

fn node_arch() -> &'static str {
    match env::consts::ARCH {
        "x86_64" => "x64",
        "x86" => "ia32",
        "aarch64" => "arm64",
        value => value,
    }
}

fn os_type() -> &'static str {
    match env::consts::OS {
        "windows" => "Windows_NT",
        "macos" => "Darwin",
        "linux" => "Linux",
        value => value,
    }
}

fn home_dir() -> String {
    env::var_os("USERPROFILE")
        .or_else(|| env::var_os("HOME"))
        .map(|value| value.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn hostname() -> String {
    env::var("COMPUTERNAME")
        .or_else(|_| env::var("HOSTNAME"))
        .unwrap_or_else(|_| "localhost".into())
}

fn dns_lookup<'js>(
    context: rquickjs::Ctx<'js>,
    hostname: String,
    family: u8,
) -> rquickjs::Result<Array<'js>> {
    if !matches!(family, 0 | 4 | 6) {
        return Err(rquickjs::Exception::throw_message(
            &context,
            "family must be 0, 4, or 6",
        ));
    }

    let addresses = (hostname.as_str(), 0)
        .to_socket_addrs()
        .map_err(|error| rquickjs::Exception::throw_message(&context, &error.to_string()))?;
    let result = Array::new(context.clone())?;
    let mut unique = Vec::new();

    for address in addresses {
        let address = address.ip();
        let address_family = match address {
            IpAddr::V4(_) => 4,
            IpAddr::V6(_) => 6,
        };
        if family != 0 && family != address_family {
            continue;
        }
        let value = address.to_string();
        if unique.contains(&value) {
            continue;
        }
        unique.push(value.clone());
        let entry = Object::new(context.clone())?;
        entry.set("address", value)?;
        entry.set("family", address_family)?;
        result.set(result.len(), entry)?;
    }

    if result.is_empty() {
        return Err(rquickjs::Exception::throw_message(
            &context,
            "no addresses matched the requested family",
        ));
    }
    Ok(result)
}

fn fs_read<'js>(context: rquickjs::Ctx<'js>, path: String) -> rquickjs::Result<Array<'js>> {
    let bytes = fs::read(path)
        .map_err(|error| rquickjs::Exception::throw_message(&context, &error.to_string()))?;
    let result = Array::new(context.clone())?;
    for (index, byte) in bytes.into_iter().enumerate() {
        result.set(index, byte)?;
    }
    Ok(result)
}

fn fs_write<'js>(
    context: rquickjs::Ctx<'js>,
    path: String,
    bytes: Vec<u8>,
    append: bool,
) -> rquickjs::Result<()> {
    use std::io::Write;

    let mut options = fs::OpenOptions::new();
    options.create(true).write(true);
    if append {
        options.append(true);
    } else {
        options.truncate(true);
    }
    let mut file = options
        .open(path)
        .map_err(|error| rquickjs::Exception::throw_message(&context, &error.to_string()))?;
    file.write_all(&bytes)
        .map_err(|error| rquickjs::Exception::throw_message(&context, &error.to_string()))
}

fn fs_stat<'js>(context: rquickjs::Ctx<'js>, path: String) -> rquickjs::Result<Object<'js>> {
    use std::time::UNIX_EPOCH;

    let metadata = fs::metadata(path)
        .map_err(|error| rquickjs::Exception::throw_message(&context, &error.to_string()))?;
    let result = Object::new(context.clone())?;
    result.set("size", metadata.len() as f64)?;
    result.set("file", metadata.is_file())?;
    result.set("directory", metadata.is_dir())?;
    result.set("symlink", metadata.file_type().is_symlink())?;
    let modified = metadata
        .modified()
        .ok()
        .and_then(|value| value.duration_since(UNIX_EPOCH).ok())
        .map(|value| value.as_secs_f64() * 1000.0)
        .unwrap_or(0.0);
    result.set("mtimeMs", modified)?;
    Ok(result)
}

fn fs_exists(path: String) -> bool {
    Path::new(&path).exists()
}

fn fs_mkdir<'js>(
    context: rquickjs::Ctx<'js>,
    path: String,
    recursive: bool,
) -> rquickjs::Result<()> {
    let result = if recursive {
        fs::create_dir_all(path)
    } else {
        fs::create_dir(path)
    };
    result.map_err(|error| rquickjs::Exception::throw_message(&context, &error.to_string()))
}

fn fs_readdir<'js>(context: rquickjs::Ctx<'js>, path: String) -> rquickjs::Result<Array<'js>> {
    let entries = fs::read_dir(path)
        .map_err(|error| rquickjs::Exception::throw_message(&context, &error.to_string()))?;
    let result = Array::new(context.clone())?;
    for (index, entry) in entries.enumerate() {
        let entry = entry
            .map_err(|error| rquickjs::Exception::throw_message(&context, &error.to_string()))?;
        result.set(index, entry.file_name().to_string_lossy().as_ref())?;
    }
    Ok(result)
}

fn resolve<'js>(
    context: rquickjs::Ctx<'js>,
    parent: String,
    specifier: String,
) -> rquickjs::Result<String> {
    modules::resolve(&parent, &specifier)
        .map(|path| path.to_string_lossy().into_owned())
        .map_err(|error| rquickjs::Exception::throw_message(&context, &error.to_string()))
}

fn dirname(path: String) -> String {
    Path::new(&path)
        .parent()
        .map(|path| path.to_string_lossy().into_owned())
        .unwrap_or_else(|| ".".into())
}

fn byte_length(value: rquickjs::String<'_>) -> rquickjs::Result<usize> {
    Ok(value.to_cstring()?.len())
}

fn cwd() -> String {
    env::current_dir()
        .map(|path| path.to_string_lossy().into_owned())
        .unwrap_or_else(|_| ".".into())
}

fn hash<'js>(
    context: rquickjs::Ctx<'js>,
    algorithm: String,
    contents: String,
    encoding: String,
) -> rquickjs::Result<String> {
    if !algorithm.eq_ignore_ascii_case("sha1") {
        return Err(rquickjs::Exception::throw_message(
            &context,
            &format!("algoritmo de hash não suportado: {algorithm}"),
        ));
    }

    let digest = Sha1::digest(contents.as_bytes());
    match encoding.as_str() {
        "hex" => Ok(digest.iter().map(|byte| format!("{byte:02x}")).collect()),
        "base64" => {
            use base64::Engine;
            Ok(base64::engine::general_purpose::STANDARD.encode(digest))
        }
        _ => Err(rquickjs::Exception::throw_message(
            &context,
            &format!("codificação de hash não suportada: {encoding}"),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn executes_javascript() {
        let runtime = Runtime::new().unwrap();
        let context = Context::full(&runtime).unwrap();

        let result = context.with(|context| context.eval::<i32, _>("21 * 2").unwrap());

        assert_eq!(result, 42);
    }

    #[test]
    fn reports_javascript_exceptions() {
        let runtime = Runtime::new().unwrap();
        let context = Context::full(&runtime).unwrap();

        let error = context.with(|context| {
            context
                .eval::<(), _>("throw new Error('boom')")
                .catch(&context)
                .unwrap_err()
                .to_string()
        });

        assert!(error.contains("boom"));
    }

    #[test]
    fn buffer_byte_length_counts_utf8_without_allocating_a_buffer() {
        let runtime = Runtime::new().unwrap();
        let context = Context::full(&runtime).unwrap();

        let matches_node = context.with(|context| {
            install_host_globals(&context, "fixture.js", &[]).unwrap();
            context.eval::<(), _>(POLYFILLS).unwrap();
            context
                .eval::<bool, _>(
                    r#"
                    Buffer.byteLength("Canaryo") === 7 &&
                    Buffer.byteLength("can\u00e1rio \ud83d\udc24") === 13 &&
                    Buffer.byteLength("\ud800") === 3 &&
                    Buffer.from("hello").toString() === "hello" &&
                    Buffer.from("ff00a5", "hex").toString("hex") === "ff00a5" &&
                    Buffer.from("Canaryo", "utf8").toString("base64") === "Q2FuYXJ5bw==" &&
                    Buffer.from("Q2FuYXJ5bw==", "base64").toString() === "Canaryo" &&
                    Buffer.from("canário").equals(Buffer.from("canário")) &&
                    Buffer.compare(Buffer.from("a"), Buffer.from("b")) < 0 &&
                    Buffer.isEncoding("utf-16le") && !Buffer.isEncoding("unknown") &&
                    Buffer.from("Canaryo").includes("aryo") &&
                    Buffer.from("Canaryo").indexOf("nar") === 2 &&
                    Buffer.from([0x78, 0x56, 0x34, 0x12]).readUInt32LE() === 0x12345678 &&
                    Buffer.alloc(4).writeUInt32BE(0x12345678) === 4 &&
                    JSON.stringify(Buffer.from([1, 2])) === '{"type":"Buffer","data":[1,2]}' &&
                    new __canaryoBuiltins.string_decoder.StringDecoder("utf-8")
                        .write(Buffer.from("hello")) === "hello" &&
                    Buffer.byteLength(new Uint8Array([1, 2, 3])) === 3 &&
                    Buffer.byteLength(new Uint8Array([1, 2, 3]).subarray(1)) === 2 &&
                    Buffer.byteLength(new ArrayBuffer(4)) === 4
                    "#,
                )
                .unwrap()
        });

        assert!(matches_node);
    }

    #[test]
    fn schedules_and_cancels_timers() {
        let runtime = Runtime::new().unwrap();
        let context = Context::full(&runtime).unwrap();

        context.with(|context| {
            install_host_globals(&context, "fixture.js", &[]).unwrap();
            context.eval::<(), _>(POLYFILLS).unwrap();
            let timer_created = context
                .eval::<bool, _>(
                    r#"
                    globalThis.timerResult = 0;
                    const cancelled = setTimeout(() => { timerResult = -1; }, 0);
                    clearTimeout(cancelled);
                    const handle = setTimeout((left, right) => {
                        timerResult = left + right;
                    }, 0, 20, 22);
                    handle.hasRef() && Number(handle) > 0
                    "#,
                )
                .unwrap();
            assert!(timer_created);

            let run_timers: Function = context.globals().get("__canaryoRunTimers").unwrap();
            assert_eq!(run_timers.call::<_, Option<u64>>(()).unwrap(), None);
            assert_eq!(context.globals().get::<_, i32>("timerResult").unwrap(), 42);
        });
    }

    #[test]
    fn reads_and_writes_files_through_node_apis() {
        use std::time::{SystemTime, UNIX_EPOCH};

        let id = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory = env::temp_dir().join(format!("canaryo-fs-{}-{id}", std::process::id()));
        fs::create_dir_all(&directory).unwrap();
        let filename = directory.join("message.txt");
        let runtime = Runtime::new().unwrap();
        let context = Context::full(&runtime).unwrap();

        context.with(|context| {
            install_host_globals(&context, "fixture.js", &[]).unwrap();
            context
                .globals()
                .set("fixturePath", filename.to_string_lossy().as_ref())
                .unwrap();
            context.eval::<(), _>(POLYFILLS).unwrap();
            let synchronous = context
                .eval::<bool, _>(
                    r#"
                    const fs = __canaryoBuiltins.fs;
                    fs.writeFileSync(fixturePath, "canário");
                    fs.appendFileSync(fixturePath, "!");
                    const raw = fs.readFileSync(fixturePath);
                    globalThis.fsPromiseResult = false;
                    fs.promises.readFile(fixturePath, "utf8").then(value => {
                        fsPromiseResult = value === "canário!";
                    });
                    Buffer.isBuffer(raw) && raw.toString() === "canário!" &&
                        fs.existsSync(fixturePath) && fs.statSync(fixturePath).isFile()
                    "#,
                )
                .unwrap();
            while context.execute_pending_job() {}

            assert!(synchronous);
            assert!(context.globals().get::<_, bool>("fsPromiseResult").unwrap());
        });

        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn supports_event_emitter_ordering_and_helpers() {
        let runtime = Runtime::new().unwrap();
        let context = Context::full(&runtime).unwrap();

        context.with(|context| {
            install_host_globals(&context, "fixture.js", &[]).unwrap();
            context.eval::<(), _>(POLYFILLS).unwrap();
            let synchronous = context
                .eval::<bool, _>(
                    r#"
                    const EventEmitter = __canaryoBuiltins.events;
                    const emitter = new EventEmitter();
                    const order = [];
                    function regular(value) { order.push(`regular:${value}`); }
                    emitter.on("value", regular);
                    emitter.prependOnceListener("value", value => order.push(`first:${value}`));
                    emitter.emit("value", 1);
                    emitter.emit("value", 2);
                    globalThis.eventPromiseResolved = false;
                    EventEmitter.once(emitter, "done").then(([value]) => {
                        eventPromiseResolved = value === 42;
                    });
                    emitter.emit("done", 42);
                    let errorThrown = false;
                    try { emitter.emit("error", new Error("boom")); }
                    catch (error) { errorThrown = error.message === "boom"; }
                    order.join(",") === "first:1,regular:1,regular:2" &&
                        emitter.listeners("value")[0] === regular &&
                        emitter.rawListeners("value")[0] === regular &&
                        emitter.eventNames().includes("value") && errorThrown
                    "#,
                )
                .unwrap();
            while context.execute_pending_job() {}

            assert!(synchronous);
            assert!(
                context
                    .globals()
                    .get::<_, bool>("eventPromiseResolved")
                    .unwrap()
            );
        });
    }

    #[test]
    fn pipes_data_through_node_streams() {
        let runtime = Runtime::new().unwrap();
        let context = Context::full(&runtime).unwrap();

        let streamed = context.with(|context| {
            install_host_globals(&context, "fixture.js", &[]).unwrap();
            context.eval::<(), _>(POLYFILLS).unwrap();
            context
                .eval::<bool, _>(
                    r#"
                    const { Readable, Writable, PassThrough, pipeline } = __canaryoBuiltins.stream;
                    const chunks = [];
                    let completed = false;
                    const source = new Readable();
                    const destination = new Writable({
                        write(chunk, _encoding, callback) {
                            chunks.push(Buffer.from(chunk).toString());
                            callback();
                        }
                    });
                    pipeline(source, new PassThrough(), destination, error => {
                        if (error) throw error;
                        completed = true;
                    });
                    source.push("Canar");
                    source.push("yo");
                    source.push(null);
                    completed && chunks.join("") === "Canaryo" &&
                        destination.writableFinished
                    "#,
                )
                .unwrap()
        });

        assert!(streamed);
    }

    #[test]
    fn supports_web_and_node_url_apis() {
        let runtime = Runtime::new().unwrap();
        let context = Context::full(&runtime).unwrap();

        let supported = context.with(|context| {
            install_host_globals(&context, "fixture.js", &[]).unwrap();
            context.eval::<(), _>(POLYFILLS).unwrap();
            context
                .eval::<bool, _>(
                    r#"
                    const url = new URL("/users?id=1&id=2", "https://example.com/base");
                    const relative = new URL("../teams", "https://example.com/api/users/");
                    url.searchParams.append("active", "true");
                    const nodeUrl = __canaryoBuiltins.url;
                    url.origin === "https://example.com" &&
                        url.pathname === "/users" &&
                        url.searchParams.getAll("id").join(",") === "1,2" &&
                        url.href === "https://example.com/users?id=1&id=2&active=true" &&
                        relative.href === "https://example.com/api/teams" &&
                        nodeUrl.fileURLToPath(nodeUrl.pathToFileURL(".")) === process.cwd()
                    "#,
                )
                .unwrap()
        });

        assert!(supported);
    }

    #[test]
    fn exposes_common_node_process_metadata() {
        let runtime = Runtime::new().unwrap();
        let context = Context::full(&runtime).unwrap();

        let supported = context.with(|context| {
            install_host_globals(&context, "fixture.js", &["argument".into()]).unwrap();
            context.eval::<(), _>(POLYFILLS).unwrap();
            context
                .eval::<bool, _>(
                    r#"
                    let warned = false;
                    process.once("warning", warning => { warned = warning.message === "careful"; });
                    process.emitWarning("careful");
                    const elapsed = process.hrtime();
                    process.pid > 0 && process.arch === __canaryoOsInfo.arch &&
                        process.platform === __canaryoOsInfo.platform &&
                        process.argv[2] === "argument" && typeof process.execPath === "string" &&
                        process.uptime() >= 0 && elapsed.length === 2 &&
                        typeof process.hrtime.bigint() === "bigint" && warned &&
                        process.getBuiltinModule("node:path") === __canaryoBuiltins.path
                    "#,
                )
                .unwrap()
        });

        assert!(supported);
    }

    #[test]
    fn resolves_dns_and_identifies_ip_addresses() {
        let runtime = Runtime::new().unwrap();
        let context = Context::full(&runtime).unwrap();

        let supported = context.with(|context| {
            install_host_globals(&context, "fixture.js", &[]).unwrap();
            context.eval::<(), _>(POLYFILLS).unwrap();
            context
                .eval::<(), _>(
                    r#"
                    globalThis.callbackLookup = false;
                    globalThis.allLookup = false;
                    globalThis.promiseLookup = false;
                    const dns = __canaryoBuiltins.dns;
                    dns.lookup("127.0.0.1", (error, address, family) => {
                        callbackLookup = !error && address === "127.0.0.1" && family === 4;
                    });
                    dns.lookup("::1", { family: 6, all: true }, (error, addresses) => {
                        allLookup = !error && addresses.length === 1 &&
                            addresses[0].address === "::1" && addresses[0].family === 6;
                    });
                    __canaryoBuiltins["dns/promises"].lookup("127.0.0.1").then(result => {
                        promiseLookup = result.address === "127.0.0.1" && result.family === 4;
                    });
                    "#,
                )
                .unwrap();
            while context.execute_pending_job() {}
            context
                .eval::<bool, _>(
                    r#"
                    callbackLookup && allLookup && promiseLookup &&
                        __canaryoBuiltins.net.isIP("127.0.0.1") === 4 &&
                        __canaryoBuiltins.net.isIPv4("127.0.0.1") &&
                        __canaryoBuiltins.net.isIPv6("2001:db8::1") &&
                        __canaryoBuiltins.net.isIP("not-an-address") === 0
                    "#,
                )
                .unwrap()
        });

        assert!(supported);
    }

    #[test]
    fn exposes_node_module_helpers() {
        let runtime = Runtime::new().unwrap();
        let context = Context::full(&runtime).unwrap();

        let supported = context.with(|context| {
            install_host_globals(&context, "fixture.js", &[]).unwrap();
            context.eval::<(), _>(BOOTSTRAP).unwrap();
            context.eval::<(), _>(POLYFILLS).unwrap();
            context
                .eval::<bool, _>(
                    r#"
                    const Module = __canaryoBuiltins.module;
                    const localRequire = Module.createRequire(__filename);
                    Module === Module.Module && Module.isBuiltin("node:http") &&
                        !Module.isBuiltin("left-pad") &&
                        Module.builtinModules.includes("fs/promises") &&
                        localRequire("node:path") === __canaryoBuiltins.path &&
                        Module._cache === localRequire.cache
                    "#,
                )
                .unwrap()
        });

        assert!(supported);
    }

    #[test]
    fn exposes_host_operating_system_metadata() {
        let runtime = Runtime::new().unwrap();
        let context = Context::full(&runtime).unwrap();

        let supported = context.with(|context| {
            install_host_globals(&context, "fixture.js", &[]).unwrap();
            context.eval::<(), _>(POLYFILLS).unwrap();
            context
                .eval::<bool, _>(
                    r#"
                    const os = __canaryoBuiltins.os;
                    os.platform() === process.platform && os.arch() === process.arch &&
                        typeof os.type() === "string" && os.type().length > 0 &&
                        typeof os.tmpdir() === "string" && os.tmpdir().length > 0 &&
                        typeof os.homedir() === "string" && typeof os.hostname() === "string" &&
                        ["LE", "BE"].includes(os.endianness()) &&
                        os.availableParallelism() >= 1 &&
                        os.cpus().length === os.availableParallelism() &&
                        os.uptime() >= 0 && typeof os.userInfo().username === "string"
                    "#,
                )
                .unwrap()
        });

        assert!(supported);
    }

    #[test]
    fn uses_platform_specific_path_semantics() {
        let runtime = Runtime::new().unwrap();
        let context = Context::full(&runtime).unwrap();

        let supported = context.with(|context| {
            install_host_globals(&context, "fixture.js", &[]).unwrap();
            context.eval::<(), _>(POLYFILLS).unwrap();
            context
                .eval::<bool, _>(
                    r#"
                    const path = __canaryoBuiltins.path;
                    const shared = path.win32.normalize("C:\\one\\..\\two") === "C:\\two" &&
                        path.win32.join("C:\\one", "two", "..", "file.js") === "C:\\one\\file.js" &&
                        path.win32.dirname("C:\\one\\file.js") === "C:\\one" &&
                        path.win32.parse("C:\\one\\file.js").root === "C:\\" &&
                        path.posix.normalize("/one/../two") === "/two" &&
                        path.posix.join("/one", "two") === "/one/two" &&
                        path.posix.delimiter === ":";
                    shared && (process.platform === "win32" ? path.sep === "\\" : path.sep === "/")
                    "#,
                )
                .unwrap()
        });

        assert!(supported);
    }
}
