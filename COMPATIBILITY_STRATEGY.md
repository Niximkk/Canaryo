# Canaryo Express Profile

Canaryo has one product goal:

```text
node server.js
canaryo server.js
```

The application must be an existing CommonJS Express 5.2.1 server and must not
contain Canaryo-specific code. Canaryo is an Express runtime, not an attempt to
reimplement every Node.js API or compete with Bun on unrelated workloads.

Version `v0.1.0` remains the finished initial POC. The next development cycle is
limited to one week and turns that POC into a documented Express compatibility
profile.

## Fixed baseline

- Node.js 22.15.1 is the behavioral reference.
- Express 5.2.1 is the only framework target.
- Windows 11 and Ubuntu are the supported platforms.
- CommonJS is the primary application format.
- The application runs without source changes.

The official Express 5.2.1 suite contains 1,238 passing tests on the fixed Node
baseline. Canaryo uses those tests as a behavior catalog. Its black-box tests
exercise real Canaryo servers because the upstream suite uses Supertest's
same-process ephemeral servers, while Canaryo owns the HTTP loop outside the
QuickJS context.

## One-week delivery profile

The release-blocking profile covers:

- application startup and shutdown;
- routing methods, route parameters, query strings, mounted routers and
  middleware ordering;
- synchronous and asynchronous handlers and Express error middleware;
- request headers and the common `req` helpers used by REST APIs;
- status, headers, JSON, text, redirects, cookies, downloads and common `res`
  helpers;
- `express.json`, `express.urlencoded`, `express.text`, `express.raw` and
  `express.static`;
- HTTP/1.1 request bodies, chunking, keep-alive and backpressure;
- CommonJS resolution through `node_modules`;
- `compression`, followed by a small explicitly documented middleware set;
- `canaryo check` diagnostics for APIs outside this profile;
- startup, memory and representative Express throughput benchmarks.

Passing the profile means every tracked black-box scenario produces the same
observable result under Node.js and Canaryo. It does not mean that arbitrary npm
packages are supported.

### Verified package versions

| Package | Version | Covered path |
|---|---:|---|
| Express | 5.2.1 | Application, routers, request/response helpers, body parsers, static files and errors |
| compression | 1.8.2 | Gzip response middleware |
| cors | 2.8.5 | Origin and credential response headers |
| cookie-parser | 1.4.7 | Request cookie parsing |
| multer | 2.0.2 | Memory-backed single-file multipart upload |

`canaryo check` reports whether the installed direct dependencies match these
versions. Other packages are outside the verified profile until a differential
case is added for them.

## Schedule

| Day | Delivery |
|---:|---|
| 1 | Freeze the profile, establish the 1,238-test Node baseline, and add black-box coverage for routers, body parsers, middleware, response helpers and errors. |
| 2 | Cover the remaining high-frequency Express request and response helpers and fix discovered runtime gaps. |
| 3 | Validate static files, downloads, ranges, cache validators, cookies and common error paths. |
| 4 | Validate `compression`, `cors`, `cookie-parser` and one multipart upload path; document every supported package version. |
| 5 | Make `canaryo check` report the Express profile and unsupported dependencies clearly; complete Windows and Ubuntu CI. |
| 6 | Profile the Express request path and move only measured HTTP, parsing, compression or serialization bottlenecks into Rust. |
| 7 | Run the full profile, benchmarks and release checks; publish the compatibility table and a release candidate. |

Compatibility takes priority over optimization. Performance work starts only
after the profile is green and must preserve the same behavior.

## Explicit exclusions

- complete Node.js API compatibility;
- Fastify and other web frameworks;
- Node-API or native `.node` addons;
- arbitrary database drivers;
- child processes, isolated workers, IPC, HTTP/2 and WebSockets;
- the complete Express middleware ecosystem;
- production support or compatibility with future Express and Node releases.

These exclusions are permanent for the one-week cycle. A dependency that needs
one of them is reported as outside the Express profile instead of expanding the
runtime.

## Optimization boundary

Rust owns connection handling, HTTP parsing and serialization, body transfer,
static files, compression and other measured infrastructure paths. QuickJS-NG
executes application handlers and custom middleware. Canaryo may cache module
resolution and precompute route-related data, but it must not change application
semantics or require a proprietary Express API.

QuickJS-NG has no optimizing JIT, so arbitrary JavaScript-heavy handlers may
remain slower than Bun. The performance claim is limited to measured Express
workloads where native HTTP work, startup time or memory use is material.

## Completion gates

- [x] Native Rust/QuickJS execution without forwarding to Node.js.
- [x] Basic Express routing, JSON, static files and compression.
- [x] Node.js 22.15.1 baseline for all 1,238 upstream Express tests.
- [x] Black-box coverage for params, queries, mounted routers, built-in body
      parsers, middleware order, cookies, redirects and error handlers.
- [x] High-frequency `req` and `res` helper matrix completed.
- [x] Selected middleware versions tested and documented.
- [x] Express-profile diagnostics implemented in `canaryo check`.
- [ ] Windows and Ubuntu CI green for the complete profile.
- [ ] Final profile benchmark and release candidate published.
