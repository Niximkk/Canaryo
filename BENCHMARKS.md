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
| `node:http` | Node.js | 20,102 | 0.04 ms | 0.09 ms | 0.12 ms | 0 | 39.1 MiB | |
| `node:http` | Canaryo | 13,852 | 0.07 ms | 0.11 ms | 0.14 ms | 0 | 9.2 MiB | 31.1% lower |
| Express 5.2.1 | Node.js | 6,357 | 0.14 ms | 0.22 ms | 0.26 ms | 0 | 76.9 MiB | |
| Express 5.2.1 | Canaryo | 3,748 | 0.24 ms | 0.34 ms | 0.42 ms | 0 | 12.5 MiB | 41.0% lower |
| Fastify 5.12.3 | Node.js | 17,869 | 0.05 ms | 0.10 ms | 0.12 ms | 0 | 53.0 MiB | |
| Fastify 5.12.3 | Canaryo | 7,901 | 0.12 ms | 0.18 ms | 0.21 ms | 0 | 16.2 MiB | 55.8% lower |

### Concurrency 16

| Application | Runtime | Requests/s | p50 | p95 | p99 | Errors | RSS | Canaryo throughput difference |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| `node:http` | Node.js | 23,635 | 0.66 ms | 1.22 ms | 1.83 ms | 0 | 40.7 MiB | |
| `node:http` | Canaryo | 21,175 | 0.69 ms | 1.04 ms | 1.25 ms | 0 | 9.3 MiB | 10.4% lower |
| Express 5.2.1 | Node.js | 6,651 | 2.21 ms | 3.52 ms | 4.56 ms | 0 | 89.1 MiB | |
| Express 5.2.1 | Canaryo | 4,262 | 3.52 ms | 5.29 ms | 6.17 ms | 0 | 12.5 MiB | 35.9% lower |
| Fastify 5.12.3 | Node.js | 20,509 | 0.73 ms | 1.13 ms | 1.58 ms | 0 | 54.6 MiB | |
| Fastify 5.12.3 | Canaryo | 10,496 | 1.42 ms | 1.81 ms | 4.55 ms | 0 | 15.2 MiB | 48.8% lower |

## Interpretation

Canaryo starts all three applications faster and uses substantially less resident memory. With a new connection per request, it leads `node:http` by 5.7% at concurrency 1 and 15.3% at concurrency 16. At concurrency 16, it trails Express by 11.5% and leads Fastify by 1.2%; the differences are small enough that repeated runs are necessary when evaluating optimization work.

Persistent connections expose the main throughput gap. At concurrency 16, Canaryo trails Node.js by 10.4% on `node:http`, 35.9% on Express, and 48.8% on Fastify. It uses between 9.3 and 15.2 MiB of resident memory in those samples, while Node.js uses between 40.7 and 89.1 MiB. Canaryo transfers text responses across the Rust/JavaScript boundary as one string and reuses the socket object for every request; framework-heavy request paths still pay for interpretation in QuickJS-NG while Node.js benefits from V8's optimizing JIT.

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
