# Canaryo

Canaryo is a Rust-built JavaScript runtime focused on running existing Node.js HTTP applications without application changes.

## Status

Canaryo embeds QuickJS-NG and runs JavaScript natively. It provides `console`, `process.argv`, `process.env`, `__filename`, `__dirname`, and an initial `node:http` implementation backed by Rust TCP sockets. `check` scans an entry file for an initial set of Node.js APIs.

Node.js can be selected explicitly with `run --node` while native compatibility is expanded.

## Commands

```sh
cargo run -- examples/hello.js Canaryo
cargo run -- check src/server.js
cargo run -- src/server.js
cargo run -- run --node src/server.js
```

The intended release interface is:

```sh
canaryo check server.js
canaryo run server.js
canaryo run --node server.js
canaryo server.js
```

## Compatibility levels

- **Compatible** — the currently scanned entry file uses no unsupported API.
- **Compatible with limitations** — it uses APIs queued for implementation.
- **Incompatible** — it uses an API that Canaryo cannot run at this stage.

## Native API surface

The first `node:http` surface supports `createServer`, `listen`, request method, URL, HTTP version, headers and body, plus response `statusCode`, `setHeader`, `writeHead`, `write`, and `end`.

Current limitations include one request per connection, no streaming request events, no TLS, no keep-alive, and no native addons. Module resolution, streams, buffers, filesystem support, and a larger package compatibility suite remain in development.
