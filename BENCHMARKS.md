# Benchmarks

Results collected on September 10, 2026 with Canaryo 0.1.0.

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
| `node:http` | Node.js | 52.62 ms | 44.84 ms | 59.56 ms | |
| `node:http` | Canaryo | 10.47 ms | 9.28 ms | 15.04 ms | 80.1% lower |
| Express 5.2.1 | Node.js | 194.94 ms | 192.50 ms | 209.14 ms | |
| Express 5.2.1 | Canaryo | 153.39 ms | 151.03 ms | 166.75 ms | 21.3% lower |
| Fastify 5.12.3 | Node.js | 333.27 ms | 319.14 ms | 337.92 ms | |
| Fastify 5.12.3 | Canaryo | 199.03 ms | 196.10 ms | 202.91 ms | 40.3% lower |

The original Express startup result was 342.99 ms. Direct execution, cached CommonJS resolution, and lexical path normalization reduced it by 55.4%.

## New connection per request

### Concurrency 1

| Application | Runtime | Requests/s | p50 | p95 | p99 | Errors | RSS | Canaryo throughput difference |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| `node:http` | Node.js | 2,084 | 0.46 ms | 0.64 ms | 0.80 ms | 0 | 34.8 MiB | |
| `node:http` | Canaryo | 2,698 | 0.35 ms | 0.50 ms | 0.59 ms | 0 | 8.1 MiB | 29.5% higher |
| Express 5.2.1 | Node.js | 1,671 | 0.57 ms | 0.77 ms | 0.95 ms | 0 | 55.0 MiB | |
| Express 5.2.1 | Canaryo | 1,946 | 0.47 ms | 0.69 ms | 0.93 ms | 0 | 10.1 MiB | 16.5% higher |
| Fastify 5.12.3 | Node.js | 2,239 | 0.42 ms | 0.59 ms | 0.71 ms | 0 | 49.5 MiB | |
| Fastify 5.12.3 | Canaryo | 2,077 | 0.45 ms | 0.64 ms | 0.75 ms | 0 | 13.4 MiB | 7.2% lower |

### Concurrency 16

| Application | Runtime | Requests/s | p50 | p95 | p99 | Errors | RSS | Canaryo throughput difference |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| `node:http` | Node.js | 6,154 | 2.44 ms | 3.81 ms | 5.50 ms | 0 | 37.2 MiB | |
| `node:http` | Canaryo | 9,627 | 1.60 ms | 2.13 ms | 2.43 ms | 0 | 13.3 MiB | 56.4% higher |
| Express 5.2.1 | Node.js | 3,455 | 4.58 ms | 6.36 ms | 7.74 ms | 0 | 63.8 MiB | |
| Express 5.2.1 | Canaryo | 3,979 | 3.65 ms | 6.05 ms | 7.85 ms | 0 | 10.4 MiB | 15.2% higher |
| Fastify 5.12.3 | Node.js | 5,852 | 2.66 ms | 3.80 ms | 4.71 ms | 0 | 50.1 MiB | |
| Fastify 5.12.3 | Canaryo | 5,368 | 2.89 ms | 3.94 ms | 4.96 ms | 0 | 12.7 MiB | 8.3% lower |

## Persistent connections

Each benchmark worker opens one HTTP/1.1 connection and reuses it for the complete sample.

### Concurrency 1

| Application | Runtime | Requests/s | p50 | p95 | p99 | Errors | RSS | Canaryo throughput difference |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| `node:http` | Node.js | 17,572 | 0.05 ms | 0.10 ms | 0.14 ms | 0 | 35.7 MiB | |
| `node:http` | Canaryo | 15,555 | 0.06 ms | 0.10 ms | 0.13 ms | 0 | 7.1 MiB | 11.5% lower |
| Express 5.2.1 | Node.js | 6,136 | 0.15 ms | 0.22 ms | 0.29 ms | 0 | 74.4 MiB | |
| Express 5.2.1 | Canaryo | 4,711 | 0.19 ms | 0.29 ms | 0.39 ms | 0 | 10.6 MiB | 23.2% lower |
| Fastify 5.12.3 | Node.js | 14,065 | 0.07 ms | 0.11 ms | 0.15 ms | 0 | 50.6 MiB | |
| Fastify 5.12.3 | Canaryo | 6,833 | 0.14 ms | 0.21 ms | 0.26 ms | 0 | 13.3 MiB | 51.4% lower |

### Concurrency 16

| Application | Runtime | Requests/s | p50 | p95 | p99 | Errors | RSS | Canaryo throughput difference |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| `node:http` | Node.js | 20,446 | 0.66 ms | 1.81 ms | 2.82 ms | 0 | 37.8 MiB | |
| `node:http` | Canaryo | 25,458 | 0.55 ms | 0.97 ms | 1.12 ms | 0 | 7.1 MiB | 24.5% higher |
| Express 5.2.1 | Node.js | 6,081 | 2.44 ms | 3.83 ms | 5.97 ms | 0 | 87.7 MiB | |
| Express 5.2.1 | Canaryo | 5,941 | 2.51 ms | 4.13 ms | 4.99 ms | 0 | 10.4 MiB | 2.3% lower |
| Fastify 5.12.3 | Node.js | 17,401 | 0.83 ms | 1.44 ms | 1.94 ms | 0 | 51.4 MiB | |
| Fastify 5.12.3 | Canaryo | 9,197 | 1.65 ms | 2.19 ms | 2.81 ms | 0 | 12.9 MiB | 47.1% lower |

## Interpretation

Canaryo starts all three applications faster and uses substantially less resident memory. It leads `node:http` and Express when every request opens a connection, while Fastify remains 7% to 8% faster on Node.js in that mode.

Node.js remains faster on persistent framework connections. The largest gap is Fastify, where Node.js handles about twice as many requests per second. At that point networking overhead is small and the benchmark is dominated by framework JavaScript execution, where V8's optimizing JIT has an advantage over the lightweight QuickJS-NG engine. Improving this case requires engine-level work or safe native specialization rather than HTTP shortcuts.

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
```

Increase `--duration` and `--runs` for a longer comparison. Use `--startup-only` to skip the load tests while optimizing initialization.
