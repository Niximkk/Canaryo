use std::{env, fs, path::Path};

use rquickjs::{Array, CatchResultExt, Context, Function, Module, Object, Runtime};
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
                __canaryoListen(
                    listenPort,
                    requestListener,
                    () => {
                        this.listening = true;
                        this.emit("listening");
                        if (explicitCallback) explicitCallback();
                    },
                    IncomingMessage.prototype,
                    ServerResponse.prototype,
                    Socket.prototype
                );
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
                this.listening = false;
                if (typeof callback === "function") callback();
                this.emit("close");
                return this;
            }
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
        if modules::is_esm_path(&entry) {
            let source = fs::read(&entry).map_err(|error| error.to_string())?;
            Module::evaluate(context.clone(), entry.to_string_lossy().as_bytes(), source)
                .catch(&context)
                .map_err(|error| error.to_string())?
                .finish::<()>()
                .catch(&context)
                .map_err(|error| error.to_string())?;
        } else {
            let run_main: Function = context
                .globals()
                .get("__canaryoRunMain")
                .map_err(|error| error.to_string())?;
            run_main
                .call::<_, ()>((entry.to_string_lossy().as_ref(),))
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
}
