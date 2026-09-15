# Compatibility Strategy

Canaryo targets source-compatible execution of existing Node.js applications. Compatibility work must be driven by measurable behavior rather than by the number of exported functions.

## Compatibility target

Node.js 22.15.1 is the initial compatibility baseline because it is already used by the benchmark suite. Bun is a useful implementation reference and performance comparator, but Node.js defines the expected behavior.

Compatibility is divided into three milestones:

1. **Production HTTP:** common Express and Fastify applications and their mainstream plugins run without source changes.
2. **Portable Node.js:** the supported platform-independent Node.js core test families pass at a high rate.
3. **Native ecosystem:** Node-API addons, isolated workers, child processes, IPC, and the remaining operating-system facilities work.

The first milestone makes Canaryo useful. The second makes compatibility broad and predictable. The third is a separate long-term program because it includes ABI compatibility and process isolation.

## Primary metric

Maintain a generated compatibility scorecard instead of estimating a percentage manually. Each module records:

- exported API surface matched;
- adapted Node.js tests passed, failed, skipped, or unsupported;
- real packages passed or failed;
- Windows and Linux results;
- known semantic differences;
- last verified Node.js and upstream test revisions.

The project-wide score is weighted by practical impact:

| Signal | Weight |
|---|---:|
| Behavioral Node.js tests | 50% |
| Real application and plugin corpus | 25% |
| API surface and descriptors | 20% |
| Cross-platform parity | 5% |

A missing method and a subtly incorrect method are both compatibility failures. Expected failures remain visible in a manifest with a reason and target milestone; they are not hidden as permanently ignored tests.

## Compatibility harness

Build a differential runner that executes the same fixture with Node.js, Bun, and Canaryo. It captures:

- exit code;
- stdout and stderr;
- uncaught error name, code, message, and selected properties;
- ordered observable events;
- filesystem output;
- HTTP status, headers, body, trailers, and connection behavior;
- duration and timeout status.

The runner normalizes temporary paths, ports, platform separators, timestamps, and unstable stack frames before comparing results. Node.js produces the expected result. Bun results identify useful precedents and implementation differences.

The harness should provide these commands:

```text
cargo xtask compat probe
cargo xtask compat run [module-or-pattern]
cargo xtask compat report
cargo xtask compat ecosystem [package-or-pattern]
```

`probe` compares exports, prototypes, property descriptors, constants, globals, and basic constructor behavior. `run` executes behavioral fixtures. `report` generates machine-readable JSON and the Markdown compatibility table. `ecosystem` installs and tests pinned real packages in disposable directories.

Store test metadata in a manifest containing the source revision, upstream path, license, feature tags, platform constraints, expected status, and any adaptation applied to the upstream test.

## Test sources

Use sources in this order:

1. Node.js core tests define behavior.
2. Web Platform Tests define Web APIs such as URL, Fetch, Web Streams, and Web Crypto.
3. Bun's Node compatibility tests provide focused cases and show which Node tests need adaptation for another engine.
4. Canaryo regression tests preserve every bug fixed in real projects.

Pin upstream commit hashes. Import or adapt small coherent test groups rather than copying entire repositories into Canaryo. Preserve the license and attribution for copied code. Bun itself is MIT-licensed, but its vendored dependencies have separate licenses and must be reviewed independently.

## Implementation order

Prioritize failures by dependency reach and by how early they prevent an application from starting.

### P0: application loading

- CommonJS and ESM resolution and interop
- package exports, imports, conditions, and self references
- process, errors, globals, timers, and async context
- Buffer, events, streams, path, URL, util, and filesystem fundamentals

### P1: HTTP production

- HTTP/1.1 server and client edge cases
- HTTPS, TLS options, DNS, and TCP backpressure
- request and response stream parity
- crypto, compression, diagnostics, and observability
- Express and Fastify plugin requirements

### P2: broader server applications

- worker threads with isolated runtimes
- child processes, signals, IPC, and UDP
- WebSocket and HTTP/2
- advanced filesystem and operating-system APIs

### P3: native ecosystem

- Node-API loading and ABI surface
- native addon lifecycle and threading
- V8-specific compatibility shims where feasible

Within a priority, fix the failure that unlocks the largest number of downstream tests or packages. Avoid completing obscure edge cases in one module while a missing foundational API blocks whole dependency trees.

## Real package corpus

Maintain pinned, minimal applications for:

- Express and Fastify core;
- static files, compression, CORS, multipart, cookies, sessions, authentication, validation, WebSocket, logging, metrics, and tracing;
- database clients for PostgreSQL, MySQL, Redis, and SQLite where native addons are not required;
- common configuration, CLI, templating, and serialization packages;
- at least one representative production-style open-source application for each framework.

Every corpus entry must execute the same application code under Node.js and Canaryo. Installation, generated files, ports, and databases live in temporary directories. A package failure becomes a reduced Canaryo regression fixture before implementation begins.

Track package results separately as startup, basic operation, full package tests, and production scenario. A package that imports successfully but fails under load is not marked compatible.

## Architecture changes that increase development speed

Split `src/polyfills.js` into embedded modules organized by Node.js subsystem. Keep a small bootstrap responsible for globals and registration. This reduces merge conflicts, enables focused tests, and makes upstream attribution possible per module.

Define a stable Rust-to-JavaScript binding contract:

- binary inputs and outputs use typed arrays without intermediate strings;
- host errors carry Node-style `code`, `errno`, `syscall`, and path fields;
- blocking operations have one native implementation shared by sync, callback, and Promise wrappers;
- asynchronous operations use bounded queues and a common cancellation contract;
- resources use a common lifecycle for close, ref, unref, and finalization.

Centralize argument validation, error construction, encoding handling, abort handling, and callback scheduling. Node APIs repeat these rules; implementing them once prevents hundreds of inconsistent edge cases.

Port self-contained MIT-compatible JavaScript when it is genuinely reusable. Reimplement JavaScriptCore-specific Bun bindings against Canaryo's host contract rather than translating them line by line. Compatibility and performance changes remain separate so a correct behavior baseline exists before optimization.

## Development loop

For every compatibility batch:

1. Select one failing cluster from the generated report.
2. Reduce real-package failures to small differential fixtures.
3. Add the Node result and failing Canaryo result to the manifest.
4. Implement the smallest shared primitive that unlocks the cluster.
5. Run focused differential tests.
6. Run unit, integration, Express, and Fastify regression suites.
7. Update the generated scorecard.
8. Commit one coherent compatibility checkpoint.

Do not mix benchmark optimization into compatibility checkpoints unless performance prevents a test from completing. Run short benchmarks after changes to the event loop, buffers, streams, HTTP, crypto, or module loading to catch major regressions.

## CI structure

Use three validation levels:

| Level | Trigger | Contents | Target duration |
|---|---|---|---:|
| Smoke | Every commit | Rust checks, Canaryo regressions, basic Express/Fastify | Under 5 minutes |
| Compatibility | Pull requests and daily | Selected Node/WPT families, complete plugin corpus, Windows and Ubuntu | Under 30 minutes |
| Extended | Nightly or weekly | Broad upstream tests, ecosystem applications, sanitizers, long network tests | Time-boxed by shard |

Shard tests by subsystem and estimated duration. Cache pinned upstream repositories and npm packages. Upload JSON and Markdown reports as CI artifacts and fail when a previously passing test regresses.

## Definition of done

An API is marked supported only when:

- CommonJS, `node:` CommonJS, ESM default, and ESM named exports agree where applicable;
- sync, callback, and Promise variants share behavior where applicable;
- argument coercion and validation cover common and boundary cases;
- errors expose the expected class and stable Node properties;
- resource lifecycle and asynchronous ordering are tested;
- Windows and Linux behavior is accounted for;
- at least one upstream behavioral test passes;
- relevant real packages pass without Canaryo-specific application code.

A module is marked supported when its selected upstream suite reaches the documented threshold and every excluded test has an explicit reason. The README table is generated from this data.

## Execution plan

### Phase 1: measurement foundation

- [x] Add `cargo xtask compat` and the manifest format.
- [x] Add the Node/Canaryo API-surface probe.
- [x] Generate the first JSON and Markdown baseline.
- Convert current ignored framework tests into explicit compatibility cases.
- Pin Node.js, Bun, and upstream revisions.

### Phase 2: split and standardize

- [x] Split the polyfill monolith by subsystem without changing behavior.
- Centralize errors, validation, encodings, callbacks, aborts, and resource handles.
- [x] Add per-module test entry points and ownership boundaries.

### Phase 3: high-impact Node tests

- Import focused tests for path, querystring, string decoder, events, Buffer, timers, util, filesystem, streams, URL, and process.
- Fix failures in dependency-unlocking order.
- Require a compatibility report update in every checkpoint.

### Phase 4: HTTP ecosystem

- Expand Express and Fastify fixtures into the pinned plugin corpus.
- Add outbound clients, TLS configurations, streaming uploads, aborts, proxy behavior, and connection-pressure scenarios.
- Run representative applications and reduce every failure.

### Phase 5: process and native features

- Implement isolated workers, child processes, IPC, signals, UDP, WebSocket, and HTTP/2.
- Design Node-API support as its own compatibility layer and test program.

## Immediate backlog

The fastest next sequence is:

1. ~~Differential API probe and result schema.~~
2. ~~Compatibility manifest and expected-failure rules.~~
3. ~~`xtask` runner with timeout and output normalization.~~
4. ~~Initial differential cases for `path`, `querystring`, `string_decoder`, and `events`.~~
5. ~~Split `polyfills.js` after the harness protects behavior.~~
6. Import Buffer, filesystem, and stream clusters.
7. Build and continuously expand the Express/Fastify plugin corpus.

This order creates the measuring system before the large refactor and turns every later development session into a ranked queue of concrete failures.
