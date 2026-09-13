# Benchmarks

Results collected on September 11 and 13, 2026 with Canaryo 0.1.0. Startup and `node:http` load results were rerun on September 13 after the HTTP lifecycle and backpressure work. Express and Fastify load results remain from September 11.

## Environment

- Windows 11 build 26200, x86-64
- AMD Ryzen 5 5600X, 6 cores and 12 logical processors
- Node.js 22.15.1
- Canaryo compiled with the Cargo release profile
- Three load samples of three seconds per result
- Seven startup samples per result
- 500 ms warm-up before each load sample

## Startup

Startup is measured from process creation until the first complete valid HTTP response. It includes module loading but does not run `canaryo check`.

| Application | Runtime | Median | Minimum | Maximum | Canaryo difference |
|---|---:|---:|---:|---:|---:|
| `node:http` | Node.js | 51.35 ms | 40.67 ms | 52.81 ms | |
| `node:http` | Canaryo | 25.01 ms | 23.00 ms | 31.30 ms | 51.3% lower |
| Express 5.2.1 | Node.js | 191.96 ms | 186.99 ms | 206.28 ms | |
| Express 5.2.1 | Canaryo | 153.12 ms | 148.13 ms | 164.43 ms | 20.2% lower |
| Fastify 5.12.3 | Node.js | 319.57 ms | 315.74 ms | 321.35 ms | |
| Fastify 5.12.3 | Canaryo | 199.20 ms | 197.59 ms | 206.53 ms | 37.7% lower |

The original Express startup result was 342.99 ms. Direct execution, cached CommonJS resolution, lexical path normalization, and subsequent bootstrap work reduced it by 55.4% in the current run.

## New connection per request

### Concurrency 1

| Application | Runtime | Requests/s | p50 | p95 | p99 | Errors | RSS | Canaryo throughput difference |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| `node:http` | Node.js | 2,427 | 0.39 ms | 0.54 ms | 0.64 ms | 0 | 37.7 MiB | |
| `node:http` | Canaryo | 2,555 | 0.37 ms | 0.52 ms | 0.66 ms | 0 | 12.8 MiB | 5.3% higher |
| Express 5.2.1 | Node.js | 1,582 | 0.60 ms | 0.84 ms | 1.07 ms | 0 | 54.3 MiB | |
| Express 5.2.1 | Canaryo | 1,832 | 0.50 ms | 0.74 ms | 1.06 ms | 0 | 10.7 MiB | 15.8% higher |
| Fastify 5.12.3 | Node.js | 2,056 | 0.47 ms | 0.64 ms | 0.78 ms | 0 | 51.5 MiB | |
| Fastify 5.12.3 | Canaryo | 2,182 | 0.43 ms | 0.61 ms | 0.78 ms | 0 | 13.7 MiB | 6.1% higher |

### Concurrency 16

| Application | Runtime | Requests/s | p50 | p95 | p99 | Errors | RSS | Canaryo throughput difference |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| `node:http` | Node.js | 6,662 | 2.28 ms | 3.29 ms | 3.82 ms | 0 | 40.5 MiB | |
| `node:http` | Canaryo | 8,156 | 1.91 ms | 2.55 ms | 3.00 ms | 0 | 8.7 MiB | 22.4% higher |
| Express 5.2.1 | Node.js | 2,279 | 6.49 ms | 11.43 ms | 16.02 ms | 0 | 62.2 MiB | |
| Express 5.2.1 | Canaryo | 3,211 | 4.86 ms | 7.44 ms | 9.07 ms | 0 | 10.6 MiB | 40.9% higher |
| Fastify 5.12.3 | Node.js | 4,166 | 3.53 ms | 6.01 ms | 11.60 ms | 0 | 51.9 MiB | |
| Fastify 5.12.3 | Canaryo | 5,119 | 3.01 ms | 4.23 ms | 6.13 ms | 0 | 12.9 MiB | 22.9% higher |

## Persistent connections

Each benchmark worker opens one HTTP/1.1 connection and reuses it for the complete sample.

### Concurrency 1

| Application | Runtime | Requests/s | p50 | p95 | p99 | Errors | RSS | Canaryo throughput difference |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| `node:http` | Node.js | 14,298 | 0.06 ms | 0.12 ms | 0.22 ms | 0 | 38.5 MiB | |
| `node:http` | Canaryo | 13,746 | 0.07 ms | 0.11 ms | 0.15 ms | 0 | 9.1 MiB | 3.9% lower |
| Express 5.2.1 | Node.js | 6,300 | 0.14 ms | 0.24 ms | 0.35 ms | 0 | 76.3 MiB | |
| Express 5.2.1 | Canaryo | 4,795 | 0.18 ms | 0.28 ms | 0.36 ms | 0 | 10.5 MiB | 23.9% lower |
| Fastify 5.12.3 | Node.js | 14,237 | 0.07 ms | 0.11 ms | 0.13 ms | 0 | 52.9 MiB | |
| Fastify 5.12.3 | Canaryo | 7,998 | 0.12 ms | 0.17 ms | 0.21 ms | 0 | 15.3 MiB | 43.8% lower |

### Concurrency 16

| Application | Runtime | Requests/s | p50 | p95 | p99 | Errors | RSS | Canaryo throughput difference |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| `node:http` | Node.js | 21,038 | 0.65 ms | 1.78 ms | 2.90 ms | 0 | 40.9 MiB | |
| `node:http` | Canaryo | 21,042 | 0.66 ms | 1.12 ms | 1.29 ms | 0 | 8.9 MiB | <0.1% higher |
| Express 5.2.1 | Node.js | 6,318 | 2.35 ms | 3.74 ms | 4.65 ms | 0 | 90.4 MiB | |
| Express 5.2.1 | Canaryo | 6,476 | 2.28 ms | 3.86 ms | 4.33 ms | 0 | 10.3 MiB | 2.5% higher |
| Fastify 5.12.3 | Node.js | 18,148 | 0.77 ms | 1.34 ms | 1.77 ms | 0 | 54.5 MiB | |
| Fastify 5.12.3 | Canaryo | 10,441 | 1.34 ms | 2.12 ms | 4.39 ms | 0 | 15.1 MiB | 42.5% lower |

## Interpretation

Canaryo starts all three applications faster and uses substantially less resident memory. In the September 13 `node:http` run with a new connection per request, it was 5.3% faster at concurrency 1 and 22.4% faster at concurrency 16 while delivering request bodies incrementally and enforcing socket-level backpressure. The September 11 Express and Fastify runs also favor Canaryo in this mode.

With persistent `node:http` connections, Node.js leads the single-connection run by 3.9%, down from the previous 22.6% gap. At concurrency 16 the runtimes are effectively tied at about 21,000 requests per second, while Canaryo uses 8.9 MiB of resident memory against Node.js's 40.9 MiB. In the September 11 framework samples, Canaryo narrowly leads Express at concurrency 16, while Node.js remains faster on Fastify by 42.5%. Canaryo transfers text responses across the Rust/JavaScript boundary as one string and reuses the socket object for every request on a persistent connection. The remaining Fastify workload is dominated by framework JavaScript execution, where V8's optimizing JIT has an advantage over QuickJS-NG.

## Methodology

Both runtimes execute the same files in `fixtures/http-basic`, `fixtures/express-basic`, and `fixtures/fastify-basic`. The load generator runs in a separate process and validates every HTTP status response. The table reports the sample with the median request rate; its latency percentiles and process resident memory are shown on the same row.

The benchmark is a synthetic loopback test of Canaryo's current compatibility surface. Canaryo implements fewer HTTP, stream, filesystem, event-loop, TLS, and debugging features than Node.js. Results will vary by operating system and hardware.

## Reproduce

```sh
npm ci --prefix fixtures/express-basic
npm ci --prefix fixtures/fastify-basic
cargo build --release
cargo bench --bench runtime -- --duration 3 --runs 3 --startup-runs 7
cargo bench --bench runtime -- --keep-alive --duration 3 --runs 3 --startup-runs 7
cargo bench --bench runtime -- --case fastify --keep-alive --duration 3 --runs 3 --startup-runs 7
```

Increase `--duration` and `--runs` for a longer comparison. Use `--case` to run only matching applications, such as `fastify` or `express`, and `--startup-only` to skip the load tests while optimizing initialization.
