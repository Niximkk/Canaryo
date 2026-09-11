# Benchmarks

Results collected on September 11, 2026 with Canaryo 0.1.0.

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
| `node:http` | Node.js | 54.83 ms | 48.41 ms | 74.50 ms | |
| `node:http` | Canaryo | 21.55 ms | 10.07 ms | 25.45 ms | 60.7% lower |
| Express 5.2.1 | Node.js | 218.57 ms | 206.50 ms | 224.74 ms | |
| Express 5.2.1 | Canaryo | 178.58 ms | 165.86 ms | 180.64 ms | 18.3% lower |
| Fastify 5.12.3 | Node.js | 361.39 ms | 350.06 ms | 365.71 ms | |
| Fastify 5.12.3 | Canaryo | 213.63 ms | 197.54 ms | 225.99 ms | 40.9% lower |

The original Express startup result was 342.99 ms. Direct execution, cached CommonJS resolution, and lexical path normalization reduced it by 47.9% in the current run.

## New connection per request

### Concurrency 1

| Application | Runtime | Requests/s | p50 | p95 | p99 | Errors | RSS | Canaryo throughput difference |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| `node:http` | Node.js | 2,135 | 0.45 ms | 0.62 ms | 0.72 ms | 0 | 38.1 MiB | |
| `node:http` | Canaryo | 2,484 | 0.38 ms | 0.54 ms | 0.64 ms | 0 | 7.8 MiB | 16.3% higher |
| Express 5.2.1 | Node.js | 1,582 | 0.60 ms | 0.84 ms | 1.07 ms | 0 | 54.3 MiB | |
| Express 5.2.1 | Canaryo | 1,832 | 0.50 ms | 0.74 ms | 1.06 ms | 0 | 10.7 MiB | 15.8% higher |
| Fastify 5.12.3 | Node.js | 2,056 | 0.47 ms | 0.64 ms | 0.78 ms | 0 | 51.5 MiB | |
| Fastify 5.12.3 | Canaryo | 2,182 | 0.43 ms | 0.61 ms | 0.78 ms | 0 | 13.7 MiB | 6.1% higher |

### Concurrency 16

| Application | Runtime | Requests/s | p50 | p95 | p99 | Errors | RSS | Canaryo throughput difference |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| `node:http` | Node.js | 4,971 | 3.00 ms | 5.29 ms | 7.15 ms | 0 | 40.2 MiB | |
| `node:http` | Canaryo | 7,569 | 2.01 ms | 2.86 ms | 4.20 ms | 0 | 11.7 MiB | 52.3% higher |
| Express 5.2.1 | Node.js | 2,279 | 6.49 ms | 11.43 ms | 16.02 ms | 0 | 62.2 MiB | |
| Express 5.2.1 | Canaryo | 3,211 | 4.86 ms | 7.44 ms | 9.07 ms | 0 | 10.6 MiB | 40.9% higher |
| Fastify 5.12.3 | Node.js | 4,166 | 3.53 ms | 6.01 ms | 11.60 ms | 0 | 51.9 MiB | |
| Fastify 5.12.3 | Canaryo | 5,119 | 3.01 ms | 4.23 ms | 6.13 ms | 0 | 12.9 MiB | 22.9% higher |

## Persistent connections

Each benchmark worker opens one HTTP/1.1 connection and reuses it for the complete sample.

### Concurrency 1

| Application | Runtime | Requests/s | p50 | p95 | p99 | Errors | RSS | Canaryo throughput difference |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| `node:http` | Node.js | 16,448 | 0.06 ms | 0.11 ms | 0.15 ms | 0 | 38.9 MiB | |
| `node:http` | Canaryo | 13,826 | 0.07 ms | 0.11 ms | 0.15 ms | 0 | 7.0 MiB | 15.9% lower |
| Express 5.2.1 | Node.js | 6,300 | 0.14 ms | 0.24 ms | 0.35 ms | 0 | 76.3 MiB | |
| Express 5.2.1 | Canaryo | 4,795 | 0.18 ms | 0.28 ms | 0.36 ms | 0 | 10.5 MiB | 23.9% lower |
| Fastify 5.12.3 | Node.js | 13,839 | 0.07 ms | 0.12 ms | 0.15 ms | 0 | 52.7 MiB | |
| Fastify 5.12.3 | Canaryo | 8,805 | 0.10 ms | 0.16 ms | 0.19 ms | 0 | 13.8 MiB | 36.4% lower |

### Concurrency 16

| Application | Runtime | Requests/s | p50 | p95 | p99 | Errors | RSS | Canaryo throughput difference |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| `node:http` | Node.js | 21,421 | 0.69 ms | 1.32 ms | 1.95 ms | 0 | 39.1 MiB | |
| `node:http` | Canaryo | 28,341 | 0.52 ms | 0.80 ms | 0.98 ms | 0 | 7.1 MiB | 32.3% higher |
| Express 5.2.1 | Node.js | 6,318 | 2.35 ms | 3.74 ms | 4.65 ms | 0 | 90.4 MiB | |
| Express 5.2.1 | Canaryo | 6,476 | 2.28 ms | 3.86 ms | 4.33 ms | 0 | 10.3 MiB | 2.5% higher |
| Fastify 5.12.3 | Node.js | 20,528 | 0.72 ms | 1.25 ms | 1.73 ms | 0 | 53.9 MiB | |
| Fastify 5.12.3 | Canaryo | 12,950 | 1.14 ms | 1.52 ms | 1.95 ms | 0 | 13.3 MiB | 36.9% lower |

## Interpretation

Canaryo starts all three applications faster and uses substantially less resident memory. It leads all three applications when every request opens a connection, including Fastify by 6.1% at concurrency 1 and 22.9% at concurrency 16.

With persistent connections, Canaryo leads `node:http` and narrowly leads Express at concurrency 16. Node.js remains faster on Fastify, where the gap is 36.9% at concurrency 16. Native UTF-8 byte counting removed a temporary `Buffer` allocation from each Fastify response; compared with the September 10 run, Canaryo's Fastify throughput rose from 6,833 to 8,805 requests/s at concurrency 1 and from 9,197 to 12,950 at concurrency 16. The remaining persistent Fastify workload is dominated by framework JavaScript execution, where V8's optimizing JIT has an advantage over QuickJS-NG.

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
