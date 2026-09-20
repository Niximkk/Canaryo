# Canaryo POC Strategy

Canaryo is a finished proof of concept: a Rust executable embedding QuickJS-NG that can run a focused class of existing Express applications without source changes.

The project demonstrates this transition:

```text
node server.js
canaryo server.js
```

It is a side project and does not promise continued maintenance or compatibility with future Node.js releases.

## Fixed target

- Node.js 22.15.1 is the behavioral baseline.
- Express 5.2.1 is the only framework target.
- CommonJS applications using the demonstrated API subset are in scope.
- Windows 11 and Ubuntu CI protect the published snapshot.

Fastify is outside the POC. Supporting a second framework would expand the required surface into framework-specific logging, worker, Web Stream, Fetch, lifecycle, and plugin behavior without strengthening the original Express demonstration.

## Demonstrated scope

The final POC must keep these paths working:

- native `node:http` HTTP/1.1 servers;
- Express routing and JSON request/response handling;
- Express static files and `compression` middleware;
- CommonJS package resolution through `node_modules`;
- the `canaryo check` compatibility report;
- startup, throughput, latency, and memory benchmarks against Node.js and optionally Bun;
- differential behavior tests against the fixed Node.js baseline.

The existing generic implementations for streams, Buffer, filesystem, crypto, Fetch, TLS, and related Node APIs remain in the repository. They are supporting experiments rather than compatibility promises.

## Explicit non-goals

- complete Node.js API compatibility;
- Fastify or a general framework/plugin compatibility matrix;
- Node-API or native `.node` addons;
- isolated worker threads, child processes, IPC, HTTP/2, or WebSocket parity;
- compatibility with every Node.js release;
- production support, security maintenance, or long-term performance competition with Node.js and Bun.

## Completion checklist

- [x] Native Rust/QuickJS execution without forwarding to Node.js.
- [x] Basic `node:http` and Express applications run unchanged.
- [x] Express JSON, static-file, and compression scenarios pass.
- [x] Differential compatibility harness uses Node.js 22.15.1.
- [x] Windows and Ubuntu CI pass.
- [x] Reproducible Node.js/Bun/Canaryo benchmark harness.
- [x] Align the README and benchmark report with the Express-only scope.
- [x] Prepare automated Windows/Linux release archives and final 0.1.0 notes.

## Finalization policy

Only release-blocking defects in the demonstrated paths should receive further implementation work. Missing APIs outside this scope are documented limitations rather than backlog items. After the final release, the repository remains available as an educational experiment and portfolio project.
