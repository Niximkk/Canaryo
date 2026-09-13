use std::{fs, path::Path};

use rquickjs::{
    Ctx, Error, Module, Result,
    loader::{ImportAttributes, Loader, Resolver},
    module::Declared,
};

use crate::modules;

pub struct NodeResolver;
pub struct NodeLoader;

impl Resolver for NodeResolver {
    fn resolve<'js>(
        &mut self,
        _context: &Ctx<'js>,
        base: &str,
        name: &str,
        _attributes: Option<ImportAttributes<'js>>,
    ) -> Result<String> {
        if is_builtin(name) {
            return Ok(if name.starts_with("node:") {
                name.to_string()
            } else {
                format!("node:{name}")
            });
        }

        modules::resolve_import(base, name)
            .map(|path| path.to_string_lossy().into_owned())
            .map_err(|error| Error::new_resolving_message(base, name, error.to_string()))
    }
}

impl Loader for NodeLoader {
    fn load<'js>(
        &mut self,
        context: &Ctx<'js>,
        name: &str,
        _attributes: Option<ImportAttributes<'js>>,
    ) -> Result<Module<'js, Declared>> {
        if let Some(source) = builtin_source(name) {
            return Module::declare(context.clone(), name, source);
        }

        let path = Path::new(name);
        if path.extension().and_then(|extension| extension.to_str()) == Some("json") {
            let json = fs::read_to_string(path)
                .map_err(|error| Error::new_loading_message(name, error.to_string()))?;
            let source = format!("export default {json};");
            return Module::declare(context.clone(), name, source);
        }

        if modules::is_esm_path(path) {
            let source = fs::read(path)
                .map_err(|error| Error::new_loading_message(name, error.to_string()))?;
            return Module::declare(context.clone(), name, source);
        }

        let commonjs_source = fs::read_to_string(path)
            .map_err(|error| Error::new_loading_message(name, error.to_string()))?;
        let filename = serde_json::to_string(name)
            .map_err(|error| Error::new_loading_message(name, error.to_string()))?;
        let named_exports = commonjs_named_exports(&commonjs_source)
            .into_iter()
            .map(|export| format!("export const {export} = value.{export};"))
            .collect::<String>();
        Module::declare(
            context.clone(),
            name,
            format!(
                "const value = globalThis.__canaryoLoadCommonJS({filename}); export default value; {named_exports}"
            ),
        )
    }
}

fn commonjs_named_exports(source: &str) -> Vec<String> {
    let mut exports = Vec::new();
    for marker in ["exports.", "module.exports."] {
        let mut remaining = source;
        while let Some(position) = remaining.find(marker) {
            remaining = &remaining[position + marker.len()..];
            let name = remaining
                .chars()
                .take_while(|character| {
                    character.is_ascii_alphanumeric() || matches!(character, '_' | '$')
                })
                .collect::<String>();
            if is_export_identifier(&name) && !exports.contains(&name) {
                exports.push(name);
            }
        }
    }
    exports.sort();
    exports
}

fn is_export_identifier(name: &str) -> bool {
    let mut characters = name.chars();
    let Some(first) = characters.next() else {
        return false;
    };
    (first.is_ascii_alphabetic() || matches!(first, '_' | '$'))
        && characters
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '_' | '$'))
        && !matches!(
            name,
            "await"
                | "break"
                | "case"
                | "catch"
                | "class"
                | "const"
                | "continue"
                | "debugger"
                | "default"
                | "delete"
                | "do"
                | "else"
                | "export"
                | "extends"
                | "finally"
                | "for"
                | "function"
                | "if"
                | "import"
                | "in"
                | "instanceof"
                | "let"
                | "new"
                | "return"
                | "super"
                | "switch"
                | "this"
                | "throw"
                | "try"
                | "typeof"
                | "var"
                | "void"
                | "while"
                | "with"
                | "yield"
        )
}

fn is_builtin(name: &str) -> bool {
    matches!(
        name.strip_prefix("node:").unwrap_or(name),
        "assert"
            | "async_hooks"
            | "buffer"
            | "console"
            | "crypto"
            | "diagnostics_channel"
            | "dns"
            | "events"
            | "fs"
            | "fs/promises"
            | "http"
            | "https"
            | "net"
            | "os"
            | "path"
            | "perf_hooks"
            | "process"
            | "querystring"
            | "stream"
            | "stream/consumers"
            | "stream/promises"
            | "string_decoder"
            | "timers"
            | "timers/promises"
            | "tty"
            | "url"
            | "util"
            | "worker_threads"
            | "zlib"
    )
}

fn builtin_source(name: &str) -> Option<String> {
    let normalized = name.strip_prefix("node:").unwrap_or(name);
    if !is_builtin(normalized) {
        return None;
    }
    let expression = if normalized == "http" {
        "globalThis.__canaryoHttpModule".to_string()
    } else if normalized == "https" {
        "globalThis.__canaryoHttpsModule".to_string()
    } else {
        format!(
            "globalThis.__canaryoBuiltins[{}]",
            serde_json::to_string(normalized).ok()?
        )
    };
    let names: &[&str] = match normalized {
        "async_hooks" => &[
            "AsyncLocalStorage",
            "AsyncResource",
            "createHook",
            "executionAsyncId",
            "triggerAsyncId",
            "executionAsyncResource",
        ],
        "http" => &[
            "createServer",
            "request",
            "get",
            "IncomingMessage",
            "ServerResponse",
            "ClientRequest",
            "Agent",
            "globalAgent",
            "METHODS",
            "STATUS_CODES",
        ],
        "https" => &[
            "createServer",
            "request",
            "get",
            "IncomingMessage",
            "ServerResponse",
            "ClientRequest",
            "Agent",
            "globalAgent",
            "METHODS",
            "STATUS_CODES",
        ],
        "buffer" => &[
            "Buffer",
            "SlowBuffer",
            "INSPECT_MAX_BYTES",
            "kMaxLength",
            "constants",
        ],
        "console" => &["Console", "log", "info", "debug", "warn", "error"],
        "crypto" => &[
            "createHash",
            "createHmac",
            "randomBytes",
            "randomFill",
            "randomFillSync",
            "randomUUID",
            "timingSafeEqual",
            "getHashes",
            "webcrypto",
            "constants",
        ],
        "diagnostics_channel" => &[
            "channel",
            "hasSubscribers",
            "subscribe",
            "unsubscribe",
            "tracingChannel",
        ],
        "dns" => &[
            "lookup",
            "resolve",
            "resolve4",
            "resolve6",
            "promises",
            "getDefaultResultOrder",
            "setDefaultResultOrder",
        ],
        "dns/promises" => &["lookup", "resolve"],
        "events" => &[
            "EventEmitter",
            "EventEmitterAsyncResource",
            "once",
            "on",
            "getEventListeners",
            "listenerCount",
            "setMaxListeners",
            "getMaxListeners",
            "addAbortListener",
            "captureRejectionSymbol",
            "errorMonitor",
        ],
        "fs" => &[
            "readFile",
            "readFileSync",
            "writeFile",
            "writeFileSync",
            "appendFile",
            "appendFileSync",
            "stat",
            "statSync",
            "existsSync",
            "accessSync",
            "mkdir",
            "mkdirSync",
            "readdir",
            "readdirSync",
            "unlink",
            "unlinkSync",
            "rename",
            "renameSync",
            "copyFile",
            "copyFileSync",
            "rm",
            "rmSync",
            "rmdir",
            "rmdirSync",
            "realpath",
            "realpathSync",
            "createReadStream",
            "createWriteStream",
            "ReadStream",
            "WriteStream",
            "promises",
            "constants",
            "Stats",
        ],
        "fs/promises" => &[
            "readFile",
            "writeFile",
            "appendFile",
            "stat",
            "access",
            "mkdir",
            "readdir",
            "unlink",
            "rename",
            "copyFile",
            "rm",
            "rmdir",
            "realpath",
        ],
        "module" => &[
            "Module",
            "builtinModules",
            "isBuiltin",
            "createRequire",
            "syncBuiltinESMExports",
        ],
        "net" => &[
            "Server",
            "Socket",
            "Stream",
            "connect",
            "createConnection",
            "createServer",
            "isIP",
            "isIPv4",
            "isIPv6",
        ],
        "os" => &[
            "EOL",
            "constants",
            "arch",
            "platform",
            "type",
            "endianness",
            "homedir",
            "tmpdir",
            "hostname",
            "availableParallelism",
            "cpus",
            "freemem",
            "totalmem",
            "uptime",
            "release",
            "version",
            "machine",
            "userInfo",
            "networkInterfaces",
        ],
        "path" => &[
            "resolve",
            "join",
            "normalize",
            "dirname",
            "basename",
            "extname",
            "isAbsolute",
            "relative",
            "parse",
            "format",
            "toNamespacedPath",
            "sep",
            "delimiter",
            "win32",
            "posix",
        ],
        "perf_hooks" => &["performance"],
        "process" => &[
            "argv",
            "argv0",
            "execArgv",
            "execPath",
            "env",
            "platform",
            "arch",
            "version",
            "versions",
            "release",
            "cwd",
            "nextTick",
            "uptime",
            "hrtime",
            "memoryUsage",
            "cpuUsage",
            "resourceUsage",
            "emitWarning",
            "getBuiltinModule",
            "stdout",
            "stderr",
        ],
        "querystring" => &["parse", "stringify", "escape", "unescape"],
        "stream" => &[
            "Stream",
            "Readable",
            "Writable",
            "Duplex",
            "Transform",
            "PassThrough",
            "finished",
            "pipeline",
            "addAbortSignal",
            "getDefaultHighWaterMark",
            "setDefaultHighWaterMark",
            "isDestroyed",
            "isDisturbed",
            "isErrored",
            "isReadable",
            "isWritable",
            "_isUint8Array",
            "_uint8ArrayToBuffer",
            "promises",
        ],
        "stream/promises" => &["finished", "pipeline"],
        "stream/consumers" => &["arrayBuffer", "blob", "buffer", "json", "text"],
        "string_decoder" => &["StringDecoder"],
        "timers" => &[
            "setTimeout",
            "clearTimeout",
            "setInterval",
            "clearInterval",
            "setImmediate",
            "clearImmediate",
        ],
        "timers/promises" => &["setTimeout", "setInterval", "setImmediate", "scheduler"],
        "tty" => &["isatty", "ReadStream", "WriteStream"],
        "url" => &[
            "URL",
            "URLSearchParams",
            "parse",
            "format",
            "resolve",
            "fileURLToPath",
            "pathToFileURL",
            "domainToASCII",
            "domainToUnicode",
            "urlToHttpOptions",
        ],
        "worker_threads" => &[
            "isMainThread",
            "threadId",
            "threadName",
            "workerData",
            "parentPort",
            "resourceLimits",
            "SHARE_ENV",
            "Worker",
            "MessageChannel",
            "MessagePort",
            "BroadcastChannel",
            "getEnvironmentData",
            "setEnvironmentData",
            "receiveMessageOnPort",
            "markAsUntransferable",
            "markAsUncloneable",
            "isMarkedAsUntransferable",
            "moveMessagePortToContext",
            "postMessageToThread",
        ],
        "util" => &[
            "inherits",
            "deprecate",
            "debuglog",
            "format",
            "formatWithOptions",
            "inspect",
            "promisify",
            "callbackify",
            "stripVTControlCharacters",
            "TextEncoder",
            "TextDecoder",
            "types",
        ],
        "zlib" => &[
            "gzip",
            "gzipSync",
            "gunzip",
            "gunzipSync",
            "deflate",
            "deflateSync",
            "inflate",
            "inflateSync",
            "deflateRaw",
            "deflateRawSync",
            "inflateRaw",
            "inflateRawSync",
            "unzip",
            "unzipSync",
            "brotliCompress",
            "brotliCompressSync",
            "brotliDecompress",
            "brotliDecompressSync",
            "createGzip",
            "createGunzip",
            "createDeflate",
            "createInflate",
            "createDeflateRaw",
            "createInflateRaw",
            "createUnzip",
            "createBrotliCompress",
            "createBrotliDecompress",
            "Gzip",
            "Gunzip",
            "Deflate",
            "Inflate",
            "DeflateRaw",
            "InflateRaw",
            "Unzip",
            "BrotliCompress",
            "BrotliDecompress",
            "constants",
        ],
        _ => &[],
    };
    let exports = names
        .iter()
        .map(|name| format!("export const {name} = value.{name};"))
        .collect::<String>();
    Some(format!(
        "const value = {expression}; export default value; {exports}"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discovers_safe_commonjs_named_exports() {
        let source = "exports.first = 1; module.exports.second = 2; exports.default = 3;";

        assert_eq!(commonjs_named_exports(source), ["first", "second"]);
    }
}
