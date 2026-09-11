use std::{env, fs, path::Path};

use rquickjs::{Array, CatchResultExt, Context, Function, Object, Runtime};
use sha1::{Digest, Sha1};

use crate::{http, modules};

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
ServerResponse.prototype.writeHead = function(status, statusMessageOrHeaders, headers) {
    this.statusCode = Number(status);
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
ServerResponse.prototype.write = function(chunk) {
    if (chunk !== undefined && chunk !== null) {
        this.__canaryoBody += typeof chunk === "string" ? chunk : chunk.toString();
    }
    this.headersSent = true;
    return true;
};
ServerResponse.prototype.end = function(chunk) {
    if (chunk !== undefined && chunk !== null) {
        this.__canaryoBody += typeof chunk === "string" ? chunk : chunk.toString();
    }
    this.headersSent = true;
    this.writableEnded = true;
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
                    ServerResponse.prototype
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
        let run_main: Function = context
            .globals()
            .get("__canaryoRunMain")
            .map_err(|error| error.to_string())?;
        run_main
            .call::<_, ()>((entry.to_string_lossy().as_ref(),))
            .catch(&context)
            .map_err(|error| error.to_string())?;

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
                    Buffer.byteLength(new Uint8Array([1, 2, 3])) === 3 &&
                    Buffer.byteLength(new Uint8Array([1, 2, 3]).subarray(1)) === 2 &&
                    Buffer.byteLength(new ArrayBuffer(4)) === 4
                    "#,
                )
                .unwrap()
        });

        assert!(matches_node);
    }
}
