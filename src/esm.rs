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
        "_stream_duplex"
            | "_stream_passthrough"
            | "_stream_readable"
            | "_stream_transform"
            | "_stream_writable"
            | "assert"
            | "assert/strict"
            | "async_hooks"
            | "buffer"
            | "console"
            | "constants"
            | "crypto"
            | "diagnostics_channel"
            | "dns"
            | "dns/promises"
            | "events"
            | "fs"
            | "fs/promises"
            | "http"
            | "https"
            | "module"
            | "net"
            | "os"
            | "path"
            | "path/posix"
            | "path/win32"
            | "perf_hooks"
            | "process"
            | "querystring"
            | "stream"
            | "stream/consumers"
            | "stream/promises"
            | "stream/web"
            | "string_decoder"
            | "sys"
            | "timers"
            | "timers/promises"
            | "tty"
            | "url"
            | "util"
            | "util/types"
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
    } else if normalized == "assert/strict" {
        "globalThis.__canaryoBuiltins[\"assert/strict\"]".to_string()
    } else if normalized == "path/posix" {
        "globalThis.__canaryoBuiltins.path.posix".to_string()
    } else if normalized == "path/win32" {
        "globalThis.__canaryoBuiltins.path.win32".to_string()
    } else if normalized == "util/types" {
        "globalThis.__canaryoBuiltins.util.types".to_string()
    } else {
        format!(
            "globalThis.__canaryoBuiltins[{}]",
            serde_json::to_string(normalized).ok()?
        )
    };
    let names: &[&str] = match normalized {
        "_stream_duplex" => &["Duplex"],
        "_stream_passthrough" => &["PassThrough"],
        "_stream_readable" => &["Readable"],
        "_stream_transform" => &["Transform"],
        "_stream_writable" => &["Writable"],
        "assert" | "assert/strict" => &[
            "ok",
            "equal",
            "notEqual",
            "strictEqual",
            "notStrictEqual",
            "deepEqual",
            "notDeepEqual",
            "deepStrictEqual",
            "notDeepStrictEqual",
            "throws",
            "doesNotThrow",
            "rejects",
            "doesNotReject",
            "match",
            "doesNotMatch",
            "ifError",
            "fail",
            "AssertionError",
        ],
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
            "maxHeaderSize",
            "validateHeaderName",
            "validateHeaderValue",
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
            "maxHeaderSize",
            "validateHeaderName",
            "validateHeaderValue",
        ],
        "buffer" => &[
            "Buffer",
            "Blob",
            "File",
            "SlowBuffer",
            "atob",
            "btoa",
            "INSPECT_MAX_BYTES",
            "kMaxLength",
            "constants",
        ],
        "console" => &["Console", "log", "info", "debug", "warn", "error"],
        "constants" => &[
            "F_OK", "R_OK", "W_OK", "X_OK", "O_RDONLY", "O_WRONLY", "O_RDWR", "O_CREAT", "O_EXCL",
            "O_TRUNC", "O_APPEND",
        ],
        "crypto" => &[
            "createHash",
            "createHmac",
            "hash",
            "randomBytes",
            "randomFill",
            "randomFillSync",
            "randomInt",
            "randomFloat",
            "randomUUID",
            "pbkdf2",
            "pbkdf2Sync",
            "hkdf",
            "hkdfSync",
            "timingSafeEqual",
            "getHashes",
            "webcrypto",
            "subtle",
            "Crypto",
            "CryptoKey",
            "SubtleCrypto",
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
            "Resolver",
            "lookup",
            "resolve",
            "resolve4",
            "resolve6",
            "promises",
            "getServers",
            "setServers",
            "getDefaultResultOrder",
            "setDefaultResultOrder",
        ],
        "dns/promises" => &[
            "Resolver",
            "lookup",
            "resolve",
            "resolve4",
            "resolve6",
            "getServers",
            "setServers",
        ],
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
            "open",
            "openSync",
            "close",
            "closeSync",
            "read",
            "readSync",
            "readv",
            "readvSync",
            "write",
            "writeSync",
            "writev",
            "writevSync",
            "fstat",
            "fstatSync",
            "ftruncate",
            "ftruncateSync",
            "fsync",
            "fsyncSync",
            "fdatasync",
            "fdatasyncSync",
            "stat",
            "statSync",
            "lstat",
            "lstatSync",
            "existsSync",
            "accessSync",
            "mkdir",
            "mkdirSync",
            "readdir",
            "readdirSync",
            "opendir",
            "opendirSync",
            "unlink",
            "unlinkSync",
            "rename",
            "renameSync",
            "copyFile",
            "copyFileSync",
            "cp",
            "cpSync",
            "truncate",
            "truncateSync",
            "rm",
            "rmSync",
            "rmdir",
            "rmdirSync",
            "realpath",
            "realpathSync",
            "readlink",
            "readlinkSync",
            "link",
            "linkSync",
            "symlink",
            "symlinkSync",
            "chmod",
            "chmodSync",
            "fchmod",
            "fchmodSync",
            "utimes",
            "utimesSync",
            "futimes",
            "futimesSync",
            "mkdtemp",
            "mkdtempSync",
            "watch",
            "watchFile",
            "unwatchFile",
            "FSWatcher",
            "StatWatcher",
            "createReadStream",
            "createWriteStream",
            "ReadStream",
            "WriteStream",
            "promises",
            "constants",
            "Stats",
            "Dirent",
            "Dir",
        ],
        "fs/promises" => &[
            "constants",
            "open",
            "readFile",
            "writeFile",
            "appendFile",
            "stat",
            "lstat",
            "access",
            "mkdir",
            "readdir",
            "opendir",
            "unlink",
            "rename",
            "copyFile",
            "cp",
            "truncate",
            "rm",
            "rmdir",
            "realpath",
            "readlink",
            "link",
            "symlink",
            "chmod",
            "utimes",
            "mkdtemp",
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
        "path" | "path/posix" | "path/win32" => &[
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
            "_makeLong",
            "matchesGlob",
            "sep",
            "delimiter",
            "win32",
            "posix",
        ],
        "perf_hooks" => &[
            "performance",
            "PerformanceEntry",
            "PerformanceMark",
            "PerformanceMeasure",
            "PerformanceObserver",
            "PerformanceObserverEntryList",
            "monitorEventLoopDelay",
            "createHistogram",
            "constants",
        ],
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
            "exitCode",
            "exit",
            "reallyExit",
            "abort",
            "cwd",
            "nextTick",
            "uptime",
            "hrtime",
            "memoryUsage",
            "cpuUsage",
            "resourceUsage",
            "emitWarning",
            "getBuiltinModule",
            "stdin",
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
            "compose",
            "addAbortSignal",
            "destroy",
            "duplexPair",
            "getDefaultHighWaterMark",
            "setDefaultHighWaterMark",
            "isDestroyed",
            "isDisturbed",
            "isErrored",
            "isReadable",
            "isWritable",
            "_isArrayBufferView",
            "_isUint8Array",
            "_uint8ArrayToBuffer",
            "promises",
        ],
        "stream/promises" => &["finished", "pipeline"],
        "stream/consumers" => &["arrayBuffer", "blob", "buffer", "json", "text"],
        "stream/web" => &[
            "ReadableStream",
            "ReadableStreamDefaultReader",
            "ReadableStreamDefaultController",
            "WritableStream",
            "WritableStreamDefaultWriter",
            "WritableStreamDefaultController",
            "TransformStream",
            "TransformStreamDefaultController",
            "ByteLengthQueuingStrategy",
            "CountQueuingStrategy",
            "TextEncoderStream",
            "TextDecoderStream",
            "CompressionStream",
            "DecompressionStream",
        ],
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
        "util" | "sys" => &[
            "inherits",
            "deprecate",
            "debuglog",
            "format",
            "formatWithOptions",
            "inspect",
            "promisify",
            "callbackify",
            "parseArgs",
            "MIMEType",
            "MIMEParams",
            "stripVTControlCharacters",
            "TextEncoder",
            "TextDecoder",
            "types",
        ],
        "util/types" => &[
            "isDate",
            "isRegExp",
            "isNativeError",
            "isArrayBuffer",
            "isArrayBufferView",
            "isTypedArray",
            "isUint8Array",
            "isPromise",
            "isMap",
            "isSet",
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

    #[test]
    fn exposes_nested_node_builtins_to_esm() {
        assert!(builtin_source("node:dns/promises").is_some());
        assert!(builtin_source("node:module").is_some());
        assert!(
            builtin_source("node:_stream_readable")
                .unwrap()
                .contains("Readable")
        );
        assert!(
            builtin_source("node:constants")
                .unwrap()
                .contains("O_RDONLY")
        );
        assert!(builtin_source("sys").unwrap().contains("parseArgs"));
        assert!(
            builtin_source("node:stream/web")
                .unwrap()
                .contains("CompressionStream")
        );
    }
}
