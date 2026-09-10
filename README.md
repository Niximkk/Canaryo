# Canaryo

Canaryo is a Rust-built JavaScript runtime focused on running existing Node.js HTTP applications without application changes.

## Status

This repository contains the compatibility-first CLI foundation. `check` scans an entry file for a small initial set of Node.js APIs. `run` performs that check, then temporarily forwards execution to Node.js while the native runtime is built.

The forwarding behavior is intentional and explicit: Canaryo does not yet claim native Node.js compatibility.

## Commands

```sh
cargo run -- check server.js
cargo run -- run server.js
cargo run -- server.js
```

The intended release interface is:

```sh
canaryo check server.js
canaryo run server.js
canaryo server.js
```

## Compatibility levels

- **Compatible** — the currently scanned entry file uses no unsupported API.
- **Compatible with limitations** — it uses APIs queued for implementation.
- **Incompatible** — it uses an API that Canaryo cannot run at this stage.

The first native milestone is a small CommonJS HTTP server surface for basic Express-style applications. The next stages are module resolution, streams and buffers, filesystem support, then a larger package compatibility suite.
