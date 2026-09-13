<div align="center">
  <img src="assets/canaryo-logo.png" width="220" alt="Canaryo logo">
</div>

<h1 align="center">Canaryo</h1>

<p align="center">
  <strong>A lightweight JavaScript runtime for existing Node.js HTTP applications.</strong>
</p>

<p align="center">
  <code>node server.js</code> → <code>canaryo server.js</code>
</p>

<p align="center">
  <img alt="Rust 2024" src="https://img.shields.io/badge/Rust-2024-dea584?logo=rust">
  <img alt="License MIT" src="https://img.shields.io/badge/license-MIT-2ea44f">
  <img alt="Status experimental" src="https://img.shields.io/badge/status-experimental-f0b429">
</p>

Canaryo embeds QuickJS-NG in a Rust executable and recreates the Node.js APIs needed by HTTP applications. Its goal is to run existing projects without source changes while reducing startup time and memory usage.

Canaryo 0.1.0 is an experimental runtime. Basic `node:http`, Express 5.2.1, and Fastify 5.12.3 applications work natively; broader Node.js compatibility is under active development.

## Why Canaryo?

- **Existing code first:** run supported CommonJS applications without rewriting them.
- **Small memory footprint:** the current Express fixture uses about 12.5 MiB of resident memory with 16 persistent connections.
- **Fast startup:** native HTTP starts in about 25 ms and the tested Express application starts faster than Node.js on the benchmark machine.
- **Compatibility report:** inspect an entry point and its dependencies before execution.
- **Explicit escape hatch:** delegate to the installed Node.js runtime when necessary.

## Quick start

Install Canaryo from this checkout:

```sh
cargo install --path .
```

Install your application's packages, inspect compatibility, and run it:

```sh
npm install
canaryo check server.js
canaryo server.js
```

A minimal server needs no Canaryo-specific code:

```js
const http = require("node:http");

http.createServer((_request, response) => {
    response.end("Hello from Canaryo!");
}).listen(3000);
```

The supported Fastify subset also uses the framework's regular API:

```js
const fastify = require("fastify")();

fastify.get("/", async () => ({ hello: "world" }));
fastify.listen({ port: 3000, host: "127.0.0.1" });
```

## CLI

| Command | Purpose |
|---|---|
| `canaryo <file> [args...]` | Run a JavaScript entry point with the native runtime. |
| `canaryo run <file> [args...]` | Explicit form of the native run command. |
| `canaryo check <file>` | Analyze local and package dependencies recursively. |
| `canaryo run --node <file> [args...]` | Run through the installed Node.js executable. |
| `canaryo --version` | Print the Canaryo version. |

Execution starts directly for low startup overhead. Use `canaryo check` in development or CI when you want the complete compatibility report.

## Performance

Results collected on Windows 11 with an AMD Ryzen 5 5600X, Node.js 22.15.1, and a release build of Canaryo 0.1.0. Every load result is the median of three 3-second samples after warm-up; all measured requests completed without errors.

### Startup latency

Lower is better.

| Application | Node.js | Canaryo | Canaryo improvement |
|---|---:|---:|---:|
| `node:http` | 53.22 ms | **25.47 ms** | **52.1% lower** |
| Express 5.2.1 | 195.87 ms | **168.53 ms** | **14.0% lower** |
| Fastify 5.12.3 | 331.61 ms | **199.67 ms** | **39.8% lower** |

### Throughput at concurrency 16

Higher is better. “New connection” opens a TCP connection for every request; “keep-alive” reuses one connection per worker.

| Application | New connection: Node.js | New connection: Canaryo | Difference | Keep-alive: Node.js | Keep-alive: Canaryo | Difference |
|---|---:|---:|---:|---:|---:|---:|
| `node:http` | 6,998 req/s | **8,069 req/s** | **+15.3%** | **23,635 req/s** | 21,175 req/s | **−10.4%** |
| Express 5.2.1 | **3,665 req/s** | 3,242 req/s | **−11.5%** | **6,651 req/s** | 4,262 req/s | **−35.9%** |
| Fastify 5.12.3 | 5,968 req/s | **6,040 req/s** | **+1.2%** | **20,509 req/s** | 10,496 req/s | **−48.8%** |

### Resident memory at concurrency 16

Lower is better. RSS is sampled from the runtime process during the selected throughput run.

| Application | New connection: Node.js | New connection: Canaryo | Reduction | Keep-alive: Node.js | Keep-alive: Canaryo | Reduction |
|---|---:|---:|---:|---:|---:|---:|
| `node:http` | 40.3 MiB | **8.8 MiB** | **78.2%** | 40.7 MiB | **9.3 MiB** | **77.1%** |
| Express 5.2.1 | 67.5 MiB | **12.7 MiB** | **81.2%** | 89.1 MiB | **12.5 MiB** | **86.0%** |
| Fastify 5.12.3 | 53.2 MiB | **15.9 MiB** | **70.1%** | 54.6 MiB | **15.2 MiB** | **72.2%** |

Canaryo currently leads startup and memory use in every tested application. With new connections at concurrency 16, it leads `node:http`, is close to Node.js on Fastify, and trails on Express. Persistent connections remain the main optimization target, especially for framework-heavy request paths that benefit from V8's optimizing JIT.

These synthetic loopback results cover the current compatibility surface on one machine. See [BENCHMARKS.md](BENCHMARKS.md) for latency percentiles, concurrency 1 results, methodology, limitations, and reproduction commands.

## How it works

```mermaid
flowchart LR
    CLI[Canaryo CLI] --> Run[Native execution]
    CLI --> Check[Compatibility check]
    Run --> QuickJS[QuickJS-NG]
    QuickJS --> CommonJS[CommonJS loader]
    QuickJS --> APIs[Node.js compatibility layer]
    APIs --> HTTP[Mio event loop]
    Check --> Graph[Recursive dependency graph]
```

- `src/runtime.rs` creates the JavaScript context, loads CommonJS, and caches resolutions.
- `src/modules.rs` implements Node-style file and package resolution.
- `src/http.rs` maps `node:http` calls to a non-blocking Rust event loop.
- `src/polyfills.js` provides the JavaScript-facing compatibility layer.
- `src/analyzer.rs` powers the recursive `check` command.

## Compatibility

| Area | Status | Current scope |
|---|---|---|
| CommonJS | Supported | Relative modules, JSON, package `main` and `exports`, conditional and wildcard exports, scoped packages, cache, upward `node_modules` lookup, common `node:module` helpers, and direct built-in subpaths such as `assert/strict`, `path/posix`, `path/win32`, and `util/types`. |
| `node:http` | Partial | Non-blocking servers, persistent HTTP/1.1 connections, pipelining, incrementally delivered request bodies with socket backpressure, chunk extensions and trailers, binary payloads, `HEAD`, response lifecycle, validated headers, header appending and bulk setting, connection admission limits, graceful close, idle/forced connection closing, configurable request limits, plus outbound `request` and `get`. Outbound uploads and responses stream through bounded queues and support pause/resume, timeouts, explicit aborts, and `AbortSignal`. `Agent` provides isolated pools, keep-alive, socket limits, `maxFreeSockets`, `agent: false`, `getName`, and pool destruction. Responses with redirects are delivered as 3xx, matching Node. Advanced socket creation and live pool introspection remain incomplete. |
| Express | Partial | Express 5.2.1 startup, basic routing, JSON request parsing, and JSON responses. |
| Fastify | Partial | Fastify 5.12.3 startup, parameterized routes, query strings, request/response hooks, JSON request parsing, async and timed handlers, JSON responses, and built-in Pino request logging. Plugin compatibility varies with the Node.js APIs each plugin uses. |
| Async context and timers | Partial | Promise jobs, `AsyncLocalStorage`, `AsyncResource`, resource lifecycle hooks and IDs, `process.nextTick`, `queueMicrotask`, callback timers, and `node:timers/promises` timeout, immediate, interval, and scheduler APIs. Async-local stores propagate through timers, ticks, Promise callbacks, and resource bindings. Referenced timers keep standalone scripts alive, while `unref()` permits exit. Precise Node scheduling phases and hooks for every internal resource remain incomplete. |
| Process and console | Partial | Importable `node:process` and `node:console`, arguments, environment, cwd, executable path, PID, platform/architecture/version metadata, process events, warnings, uptime, `hrtime`, resource-shape methods, built-in module lookup, the global console, and custom basic `Console` streams. Signals, IPC, privilege APIs, advanced console formatting, and exact resource accounting remain incomplete. |
| Events | Partial | Global `Event`, `EventTarget`, `AbortController`, and `AbortSignal`; listener ordering, one-time and prepended listeners, removal, introspection, monitored and unhandled errors, `captureRejections`, `EventEmitterAsyncResource`, `addAbortListener`, cancelable Promise-based `events.once`, and cancelable async iteration through `events.on`. Listener leak warnings remain incomplete. |
| Diagnostics | Partial | Named `node:diagnostics_channel` channels support publication, subscription, unsubscription, subscriber checks, `AsyncLocalStorage` store binding, and tracing of synchronous, Promise, and callback operations. Bounded channels and exact subscriber snapshot semantics remain incomplete. |
| URLs | Partial | Global and `node:url` `URL`/`URLSearchParams`, repeated query parameters, relative HTTP URLs, and basic file URL conversion. IDNA, complete percent-encoding rules, and every legacy URL edge case remain incomplete. |
| DNS and IP utilities | Partial | System-backed `dns.lookup`, IPv4/IPv6 filtering, all-results and Promise forms, basic `resolve4`/`resolve6`, result ordering, and `net.isIP` helpers. Full DNS record types and custom resolvers remain incomplete. |
| TCP networking | Partial | Non-blocking `net.createServer`, asynchronous `connect`/`createConnection`, concurrent duplex `Socket` streams, incremental reads and writes, half-close, byte counters, addresses, timeouts, connection lifecycle events, and graceful server close. IPC, dynamic socket options, connection admission limits, and socket-level backpressure remain incomplete. |
| Operating system APIs | Partial | Host platform, architecture, type, endianness, home/temp directories, hostname, CPU parallelism, user shape, EOL, and uptime. Exact CPU, memory, release, network-interface, and OS constants data remain incomplete. |
| Path APIs | Partial | Platform-correct default `node:path`, separate Win32/POSIX implementations, normalization, joining, resolution, relative paths, parsing, formatting, and basename/dirname/extension helpers. Namespace and uncommon drive-relative/UNC edge cases remain incomplete. |
| Worker threads | Partial | Main-thread metadata, environment data, and same-thread `MessageChannel`/`MessagePort` are available for libraries such as Pino. Isolated `Worker` execution and `BroadcastChannel` remain unsupported. |
| Utility APIs | Partial | Formatting, inspection, inheritance, deprecation/debug shims, type checks, text encoding/decoding, ANSI stripping, `promisify`, and `callbackify`. Advanced inspection, MIME, parsing, and transferable helpers remain incomplete. |
| Cryptography | Partial | SHA-1, SHA-256, SHA-384, and SHA-512 hashes and HMACs; OS-backed `randomBytes`, `randomFill`, `randomUUID`, Web Crypto random values, and constant-time equality. Ciphers, signatures, key objects, certificates, password derivation, and Web Crypto SubtleCrypto remain incomplete. |
| Compression | Partial | Synchronous, callback, and buffered stream forms of gzip, deflate, raw deflate, automatic unzip, and Brotli compression/decompression. Compression options, dictionaries, incremental native output, and advanced stream controls remain incomplete. |
| Buffers and streams | Partial | Buffer encodings and common binary operations; global and `node:buffer` Blob, File, `atob`, and `btoa`; functional `Readable`, `Writable`, `Duplex`, `Transform`, `PassThrough`, `pipe`, callback and Promise forms of `pipeline` and `finished`, `addAbortSignal`, `Readable.from`, async iteration, state inspection, configurable default high-water marks, and `node:stream/consumers` conversions; plus queued asynchronous writes, pause/resume, and drain-based backpressure. Web Streams and advanced Buffer and stream methods remain incomplete. |
| Filesystem APIs | Partial | Buffer-aware read, write, append, stat, exists, access, mkdir, readdir, unlink, rename, copy, recursive removal, realpath, and file streams through synchronous, callback, and `fs/promises` APIs. Watches, links, permissions, real file descriptors, and positional writes remain incomplete. |
| ESM | Partial | Native `.mjs` and `type: module` execution, relative imports, package `import` conditions, JSON loading, named imports from common built-ins, default CommonJS interop, and static detection of `exports.name`. Dynamic CJS exports and some Node resolution rules remain incomplete. |
| Keep-alive | Supported | Connections persist by default on HTTP/1.1 and honor `Connection: close`. |
| TLS | Partial | Asynchronous outbound `node:https` clients validate public certificates through rustls and WebPKI roots, stream request and response bodies, and support independent agents. Inbound `https.createServer` accepts PEM certificates and PKCS#1, PKCS#8, or SEC1 private keys, uses non-blocking TLS, and shuts sessions down cleanly. Certificate arrays, passphrases, SNI certificate selection, custom trust stores, and client certificates remain incomplete. |
| Native `.node` addons | Unsupported | Native Node.js ABI modules cannot be loaded. |

`canaryo check` reports one of three project-level outcomes:

- **Compatible:** no unsupported API was detected.
- **Compatible with limitations:** the application uses an API with partial support.
- **Incompatible:** the application requires functionality the runtime cannot execute.

## Development

Canaryo limits inbound request bodies to 1 MiB by default. Applications that need a different ceiling can pass the Canaryo extension `maxRequestSize` to `http.createServer({ maxRequestSize }, listener)`. Requests above it receive `413 Payload Too Large`.

Run the complete validation suite:

```sh
npm ci --prefix fixtures/express-basic
npm ci --prefix fixtures/fastify-basic
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo test --test runtime_smoke -- --ignored
```

Reproduce the performance comparison:

```sh
cargo build --release
cargo bench --bench runtime -- --duration 3 --runs 3 --startup-runs 7
cargo bench --bench runtime -- --keep-alive --duration 3 --runs 3 --startup-runs 7
```

Add `--case fastify`, `--case express`, or `--case node:http` to isolate one application while investigating performance.

Use `--startup-only` while working specifically on initialization:

```sh
cargo bench --bench runtime -- --startup-only --startup-runs 15
```

## Roadmap

- Additional asynchronous I/O sources.
- Wider Buffer, stream, filesystem, crypto, and networking support.
- Complete ESM/CommonJS interop and the remaining Node package-resolution rules.
- A larger Express, Fastify, and plugin compatibility suite.
- Advanced TLS options, isolated workers, diagnostics, and production observability.

## License

Canaryo is available under the [MIT License](LICENSE).
