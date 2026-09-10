# Canaryo

Canaryo is a Rust-built JavaScript runtime focused on running existing Node.js HTTP applications without application changes.

## Status

Canaryo currently embeds QuickJS-NG and runs ordinary JavaScript natively. It provides `console`, `process.argv`, `process.env`, `__filename`, and `__dirname`. `check` scans an entry file for an initial set of Node.js APIs.

Node.js can be selected explicitly with `run --node` while native compatibility is expanded. Native HTTP and CommonJS module support are the next milestones.

## Commands

```sh
cargo run -- examples/hello.js Canaryo
cargo run -- check src/server.js
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

The next native milestone is a small CommonJS HTTP server surface. Later stages cover module resolution, streams and buffers, filesystem support, and a larger package compatibility suite.
