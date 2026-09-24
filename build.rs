use std::{env, error::Error, fs, path::PathBuf};

use rquickjs::{Context, Module, Runtime, WriteOptions};

const POLYFILL_PARTS: &[&str] = &[
    "src/polyfills/00_web_events.part.js",
    "src/polyfills/10_buffer_text.part.js",
    "src/polyfills/20_util_path_url.part.js",
    "src/polyfills/30_streams.part.js",
    "src/polyfills/40_fetch.part.js",
    "src/polyfills/50_async_process.part.js",
    "src/polyfills/60_performance_timers.part.js",
    "src/polyfills/70_filesystem.part.js",
    "src/polyfills/80_dns_workers_net.part.js",
    "src/polyfills/90_crypto_compression_registry.part.js",
];

fn main() -> Result<(), Box<dyn Error>> {
    println!("cargo:rerun-if-changed=src/runtime.rs");
    for path in POLYFILL_PARTS {
        println!("cargo:rerun-if-changed={path}");
    }

    let runtime_source = fs::read_to_string("src/runtime.rs")?;
    let bootstrap = extract_bootstrap(&runtime_source)?;
    let mut source = String::with_capacity(
        bootstrap.len()
            + POLYFILL_PARTS
                .iter()
                .map(|path| fs::metadata(path).map_or(0, |metadata| metadata.len() as usize))
                .sum::<usize>(),
    );
    source.push_str(bootstrap);
    for path in POLYFILL_PARTS {
        source.push('\n');
        source.push_str(&fs::read_to_string(path)?);
    }

    let runtime = Runtime::new()?;
    let context = Context::full(&runtime)?;
    let bytecode = context.with(|context| {
        Module::declare(context, "canaryo:runtime", source.as_bytes())?.write(WriteOptions {
            strip_source: true,
            strip_debug: true,
            ..WriteOptions::default()
        })
    })?;

    let output =
        PathBuf::from(env::var_os("OUT_DIR").ok_or("OUT_DIR is not set")?).join("runtime.qbc");
    fs::write(output, bytecode)?;
    Ok(())
}

fn extract_bootstrap(source: &str) -> Result<&str, Box<dyn Error>> {
    const START: &str = "const BOOTSTRAP: &str = r#\"";
    const END: &str = "\"#;";

    let start = source
        .find(START)
        .ok_or("BOOTSTRAP start marker not found")?
        + START.len();
    let end = source[start..]
        .find(END)
        .map(|offset| start + offset)
        .ok_or("BOOTSTRAP end marker not found")?;
    Ok(&source[start..end])
}
