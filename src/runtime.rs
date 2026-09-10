use std::{env, fs, path::Path};

use rquickjs::{Array, CatchResultExt, Context, Function, Object, Runtime};

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

const httpModule = Object.freeze({
    createServer(requestListener) {
        if (typeof requestListener !== "function") {
            throw new TypeError("createServer requer uma função");
        }

        return {
            listen(port, hostOrCallback, callback) {
                const onListening = typeof hostOrCallback === "function"
                    ? hostOrCallback
                    : typeof callback === "function"
                        ? callback
                        : () => {};
                __canaryoListen(Number(port), requestListener, onListening);
                return this;
            }
        };
    }
});

const moduleCache = Object.create(null);

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
        throw error;
    }
}

function createRequire(parentFilename) {
    function require(name) {
        if (name === "http" || name === "node:http") return httpModule;
        return loadModule(__canaryoResolve(parentFilename, name));
    }

    require.resolve = name => __canaryoResolve(parentFilename, name);
    require.cache = moduleCache;
    return require;
}

globalThis.__canaryoRunMain = filename => loadModule(filename);
})();
"#;

pub fn execute(path: &str, arguments: &[String]) -> Result<(), String> {
    fs::metadata(path).map_err(|error| format!("não foi possível ler {path}: {error}"))?;
    let entry = Path::new(path)
        .canonicalize()
        .map_err(|error| format!("não foi possível resolver {path}: {error}"))?;
    let runtime = Runtime::new().map_err(|error| format!("erro ao criar runtime: {error}"))?;
    let context =
        Context::full(&runtime).map_err(|error| format!("erro ao criar contexto: {error}"))?;

    context.with(|context| {
        install_host_globals(&context, path, arguments)?;
        context
            .eval::<(), _>(BOOTSTRAP)
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
}
