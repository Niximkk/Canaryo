# JavaScript compatibility layer

The numbered `*.part.js` files form one JavaScript program. `runtime.rs` joins them with Rust's `concat!` macro before QuickJS-NG evaluates the result.

The files deliberately share one lexical scope. Their numeric prefixes preserve dependency order while keeping related APIs in focused files:

| Range | Subsystems |
|---|---|
| `00` | Web events, abort signals, and Node events |
| `10` | Buffer, blobs, text codecs, and query strings |
| `20` | Utilities, paths, and URLs |
| `30` | Node and Web streams |
| `40` | Fetch, headers, requests, responses, and form data |
| `50` | Async context, diagnostics, assertions, process, and TTY |
| `60` | Performance, timers, and stream helpers |
| `70` | Filesystem APIs |
| `80` | DNS, worker shims, TCP, and module helpers |
| `90` | Crypto, compression, globals, and builtin registration |

Keep the opening wrapper in `00_web_events.part.js` and the closing wrapper in `90_crypto_compression_registry.part.js`. Moving a declaration across files is safe when its initialization still occurs before its first use.
