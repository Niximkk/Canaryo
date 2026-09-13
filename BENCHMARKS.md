# Benchmarks

Results collected on September 13, 2026 with Canaryo 0.1.0 after the async context, events, streams, Buffer, and HTTP header compatibility work.

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
| `node:http` | Node.js | 53.22 ms | 50.77 ms | 66.85 ms | |
| `node:http` | Canaryo | 25.47 ms | 24.20 ms | 27.00 ms | 52.1% lower |
| Express 5.2.1 | Node.js | 195.87 ms | 191.59 ms | 211.00 ms | |
| Express 5.2.1 | Canaryo | 168.53 ms | 157.42 ms | 173.25 ms | 14.0% lower |
| Fastify 5.12.3 | Node.js | 331.61 ms | 313.84 ms | 335.06 ms | |
| Fastify 5.12.3 | Canaryo | 199.67 ms | 196.87 ms | 203.57 ms | 39.8% lower |

The original Express startup result was 342.99 ms. Direct execution, cached CommonJS resolution, lexical path normalization, and subsequent bootstrap work reduced it by 50.9% in the current run.

## New connection per request

### Concurrency 1

| Application | Runtime | Requests/s | p50 | p95 | p99 | Errors | RSS | Canaryo throughput difference |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| `node:http` | Node.js | 2,402 | 0.40 ms | 0.53 ms | 0.61 ms | 0 | 37.4 MiB | |
| `node:http` | Canaryo | 2,540 | 0.38 ms | 0.49 ms | 0.61 ms | 0 | 12.7 MiB | 5.7% higher |
| Express 5.2.1 | Node.js | 1,849 | 0.52 ms | 0.70 ms | 0.87 ms | 0 | 59.6 MiB | |
| Express 5.2.1 | Canaryo | 1,746 | 0.54 ms | 0.70 ms | 0.86 ms | 0 | 12.9 MiB | 5.6% lower |
| Fastify 5.12.3 | Node.js | 2,368 | 0.40 ms | 0.55 ms | 0.63 ms | 0 | 52.7 MiB | |
| Fastify 5.12.3 | Canaryo | 2,153 | 0.44 ms | 0.58 ms | 0.68 ms | 0 | 15.3 MiB | 9.1% lower |

### Concurrency 16

| Application | Runtime | Requests/s | p50 | p95 | p99 | Errors | RSS | Canaryo throughput difference |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| `node:http` | Node.js | 6,998 | 2.27 ms | 2.93 ms | 3.37 ms | 0 | 40.3 MiB | |
| `node:http` | Canaryo | 8,069 | 1.92 ms | 2.57 ms | 2.95 ms | 0 | 8.8 MiB | 15.3% higher |
| Express 5.2.1 | Node.js | 3,665 | 4.29 ms | 6.02 ms | 7.43 ms | 0 | 67.5 MiB | |
| Express 5.2.1 | Canaryo | 3,242 | 4.66 ms | 6.73 ms | 7.42 ms | 0 | 12.7 MiB | 11.5% lower |
| Fastify 5.12.3 | Node.js | 5,968 | 2.60 ms | 3.81 ms | 4.54 ms | 0 | 53.2 MiB | |
| Fastify 5.12.3 | Canaryo | 6,040 | 2.53 ms | 3.17 ms | 5.95 ms | 0 | 15.9 MiB | 1.2% higher |

## Persistent connections

Each benchmark worker opens one HTTP/1.1 connection and reuses it for the complete sample.

### Concurrency 1

| Application | Runtime | Requests/s | p50 | p95 | p99 | Errors | RSS | Canaryo throughput difference |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| `node:http` | Node.js | 19,978 | 0.04 ms | 0.09 ms | 0.12 ms | 0 | 35.4 MiB | |
| `node:http` | Canaryo | 13,161 | 0.07 ms | 0.11 ms | 0.15 ms | 0 | 9.2 MiB | 34.1% lower |
| Express 5.2.1 | Node.js | 6,485 | 0.14 ms | 0.21 ms | 0.28 ms | 0 | 77.1 MiB | |
| Express 5.2.1 | Canaryo | 4,516 | 0.20 ms | 0.28 ms | 0.38 ms | 0 | 13.0 MiB | 30.4% lower |
| Fastify 5.12.3 | Node.js | 17,414 | 0.05 ms | 0.10 ms | 0.12 ms | 0 | 52.9 MiB | |
| Fastify 5.12.3 | Canaryo | 7,359 | 0.12 ms | 0.19 ms | 0.23 ms | 0 | 16.0 MiB | 57.7% lower |

### Concurrency 16

| Application | Runtime | Requests/s | p50 | p95 | p99 | Errors | RSS | Canaryo throughput difference |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| `node:http` | Node.js | 23,385 | 0.64 ms | 1.14 ms | 1.85 ms | 0 | 36.3 MiB | |
| `node:http` | Canaryo | 21,411 | 0.68 ms | 1.02 ms | 1.21 ms | 0 | 9.3 MiB | 8.4% lower |
| Express 5.2.1 | Node.js | 6,569 | 2.22 ms | 3.42 ms | 4.70 ms | 0 | 90.7 MiB | |
| Express 5.2.1 | Canaryo | 5,848 | 2.50 ms | 4.20 ms | 4.93 ms | 0 | 12.5 MiB | 11.0% lower |
| Fastify 5.12.3 | Node.js | 20,286 | 0.73 ms | 1.23 ms | 1.76 ms | 0 | 54.8 MiB | |
| Fastify 5.12.3 | Canaryo | 10,377 | 1.43 ms | 1.87 ms | 4.55 ms | 0 | 15.4 MiB | 48.8% lower |

## Persistent connections with a 16 KiB JSON body

These requests use `POST /echo` and include a 16,384-byte JSON document. The response contains the parsed body, exercising request streaming, UTF-8 decoding, JSON parsing and serialization, hashing used by Express ETags, and binary response transfer.

### Concurrency 1

| Application | Runtime | Requests/s | p50 | p95 | p99 | Errors | RSS | Canaryo throughput difference |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| `node:http` | Node.js | 8,326 | 0.11 ms | 0.17 ms | 0.22 ms | 0 | 38.7 MiB | |
| `node:http` | Canaryo | 7,347 | 0.12 ms | 0.24 ms | 0.32 ms | 0 | 9.3 MiB | 11.8% lower |
| Express 5.2.1 | Node.js | 3,332 | 0.28 ms | 0.42 ms | 0.60 ms | 0 | 81.2 MiB | |
| Express 5.2.1 | Canaryo | 1,879 | 0.47 ms | 0.69 ms | 1.81 ms | 0 | 13.9 MiB | 43.6% lower |
| Fastify 5.12.3 | Node.js | 5,126 | 0.17 ms | 0.26 ms | 0.46 ms | 0 | 93.4 MiB | |
| Fastify 5.12.3 | Canaryo | 927 | 0.98 ms | 1.30 ms | 3.02 ms | 0 | 15.2 MiB | 81.9% lower |

### Concurrency 16

| Application | Runtime | Requests/s | p50 | p95 | p99 | Errors | RSS | Canaryo throughput difference |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| `node:http` | Node.js | 11,308 | 1.35 ms | 1.78 ms | 2.06 ms | 0 | 38.9 MiB | |
| `node:http` | Canaryo | 10,929 | 1.42 ms | 1.83 ms | 2.04 ms | 0 | 9.3 MiB | 3.4% lower |
| Express 5.2.1 | Node.js | 3,580 | 4.29 ms | 5.83 ms | 6.78 ms | 0 | 86.9 MiB | |
| Express 5.2.1 | Canaryo | 2,124 | 7.45 ms | 8.81 ms | 9.31 ms | 0 | 12.9 MiB | 40.7% lower |
| Fastify 5.12.3 | Node.js | 6,168 | 2.38 ms | 3.72 ms | 4.41 ms | 0 | 97.6 MiB | |
| Fastify 5.12.3 | Canaryo | 971 | 16.01 ms | 18.71 ms | 19.78 ms | 0 | 16.4 MiB | 84.3% lower |

## Interpretation

Canaryo starts all three applications faster and uses substantially less resident memory. With a new connection per request, it leads `node:http` by 5.7% at concurrency 1 and 15.3% at concurrency 16. At concurrency 16, it trails Express by 11.5% and leads Fastify by 1.2%; the differences are small enough that repeated runs are necessary when evaluating optimization work.

Persistent GET connections expose the remaining framework throughput gap. At concurrency 16, Canaryo trails Node.js by 8.4% on `node:http`, 11.0% on Express, and 48.8% on Fastify. Binary data now crosses the Rust/JavaScript boundary in typed-array chunks, and UTF-8 and crypto operations run directly on those bytes. This reduced the previous Express keep-alive gap from 35.9% to 11.0% while retaining a much smaller memory footprint.

The 16 KiB body test makes data conversion costs visible. Canaryo reaches 96.6% of Node.js `node:http` throughput at concurrency 16 and uses 76.1% less resident memory. Express and especially Fastify still pay for framework work interpreted by QuickJS-NG while Node.js benefits from V8's optimizing JIT.

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
cargo bench --bench runtime -- --keep-alive --body-size 16384 --duration 3 --runs 3 --startup-runs 7
cargo bench --bench runtime -- --case fastify --keep-alive --duration 3 --runs 3 --startup-runs 7
```

Increase `--duration` and `--runs` for a longer comparison. Use `--case` to run only matching applications, such as `fastify` or `express`, `--body-size` to benchmark `POST /echo` with a generated JSON body, and `--startup-only` to skip the load tests while optimizing initialization.
