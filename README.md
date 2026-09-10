# Canaryo

Canaryo is a Rust-built JavaScript runtime focused on running existing Node.js HTTP applications without application changes.

## Status

Canaryo embeds QuickJS-NG and runs JavaScript natively. It provides `console`, `process.argv`, `process.env`, `__filename`, `__dirname`, and an initial `node:http` implementation backed by Rust TCP sockets. `check` follows local and package dependencies recursively and reports the file that uses each unsupported API.

Node.js can be selected explicitly with `run --node` while native compatibility is expanded.

## Commands

```sh
cargo run -- examples/hello.js Canaryo
cargo run -- examples/commonjs.js
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

## Install from source

```sh
cargo install --path .
```

The `canaryo` binary will be installed in Cargo's binary directory and can run projects from any working directory.

## Validation

```sh
cargo fmt --check
cargo clippy -- -D warnings
cargo test
npm ci --prefix fixtures/express-basic
cargo test --test runtime_smoke -- --ignored
```

## Compatibility levels

- **Compatible** — the currently scanned entry file uses no unsupported API.
- **Compatible with limitations** — it uses APIs queued for implementation.
- **Incompatible** — it uses an API that Canaryo cannot run at this stage.

## Native API surface

The first `node:http` surface supports `createServer`, `listen`, request method, URL, HTTP version, headers and body, plus response `statusCode`, `setHeader`, `writeHead`, `write`, and `end`.

The CommonJS loader supports relative modules, `.js`, `.cjs`, `.json`, directory indexes, package `main` fields, scoped packages, module caching, `require.resolve`, and upward `node_modules` lookup.

Canaryo's compatibility layer currently boots Express 5.2.1 and serves basic routing and JSON responses natively. The fixture in `fixtures/express-basic` is pinned with a lockfile and used as the real-package smoke test.

Current limitations include one request per connection, no streaming request events, no TLS, no keep-alive, no ESM execution, and no native addons. Streams, buffers, filesystem APIs, package `exports`, and a larger package compatibility suite remain in development.
