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

        let filename = serde_json::to_string(name)
            .map_err(|error| Error::new_loading_message(name, error.to_string()))?;
        Module::declare(
            context.clone(),
            name,
            format!(
                "const value = globalThis.__canaryoLoadCommonJS({filename}); export default value;"
            ),
        )
    }
}

fn is_builtin(name: &str) -> bool {
    matches!(
        name.strip_prefix("node:").unwrap_or(name),
        "assert"
            | "async_hooks"
            | "buffer"
            | "crypto"
            | "diagnostics_channel"
            | "dns"
            | "events"
            | "fs"
            | "fs/promises"
            | "http"
            | "net"
            | "os"
            | "path"
            | "perf_hooks"
            | "querystring"
            | "stream"
            | "string_decoder"
            | "timers"
            | "tty"
            | "url"
            | "util"
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
            "executionAsyncId",
            "triggerAsyncId",
        ],
        "http" => &[
            "createServer",
            "IncomingMessage",
            "ServerResponse",
            "METHODS",
            "STATUS_CODES",
        ],
        "buffer" => &["Buffer", "SlowBuffer", "INSPECT_MAX_BYTES", "kMaxLength"],
        "crypto" => &["createHash"],
        "diagnostics_channel" => &[
            "channel",
            "hasSubscribers",
            "subscribe",
            "unsubscribe",
            "tracingChannel",
        ],
        "dns" => &["lookup"],
        "events" => &["EventEmitter"],
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
        ],
        "net" => &["isIP", "isIPv4", "isIPv6"],
        "os" => &["networkInterfaces"],
        "path" => &[
            "resolve",
            "join",
            "normalize",
            "dirname",
            "basename",
            "extname",
            "isAbsolute",
            "relative",
            "sep",
            "delimiter",
            "win32",
            "posix",
        ],
        "perf_hooks" => &["performance"],
        "querystring" => &["parse", "stringify", "escape", "unescape"],
        "stream" => &["Readable", "Writable", "Duplex", "Transform", "PassThrough"],
        "string_decoder" => &["StringDecoder"],
        "timers" => &[
            "setTimeout",
            "clearTimeout",
            "setInterval",
            "clearInterval",
            "setImmediate",
            "clearImmediate",
        ],
        "tty" => &["isatty", "ReadStream", "WriteStream"],
        "url" => &["parse", "format", "resolve"],
        "util" => &[
            "inherits",
            "deprecate",
            "debuglog",
            "format",
            "formatWithOptions",
            "inspect",
            "types",
        ],
        "zlib" => &["constants"],
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
