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
| `node:http` | Node.js | 52.88 ms | 50.78 ms | 55.35 ms | |
| `node:http` | Canaryo | 9.99 ms | 9.39 ms | 23.42 ms | 81.1% lower |
| Express 5.2.1 | Node.js | 193.14 ms | 190.59 ms | 208.66 ms | |
| Express 5.2.1 | Canaryo | 152.82 ms | 151.62 ms | 166.56 ms | 20.9% lower |

The original Express startup result was 342.99 ms. Direct execution, cached CommonJS resolution, and lexical path normalization reduced it by 55.4%.

## New connection per request

### Concurrency 1

| Application | Runtime | Requests/s | p50 | p95 | p99 | Errors | RSS | Canaryo throughput difference |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| `node:http` | Node.js | 2,409 | 0.39 ms | 0.55 ms | 0.65 ms | 0 | 34.8 MiB | |
| `node:http` | Canaryo | 2,751 | 0.34 ms | 0.48 ms | 0.55 ms | 0 | 7.9 MiB | 14.2% higher |
| Express 5.2.1 | Node.js | 1,711 | 0.56 ms | 0.77 ms | 0.94 ms | 0 | 51.7 MiB | |
| Express 5.2.1 | Canaryo | 2,034 | 0.46 ms | 0.65 ms | 0.84 ms | 0 | 10.1 MiB | 18.9% higher |

### Concurrency 16

| Application | Runtime | Requests/s | p50 | p95 | p99 | Errors | RSS | Canaryo throughput difference |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| `node:http` | Node.js | 6,654 | 2.27 ms | 3.40 ms | 4.21 ms | 0 | 37.1 MiB | |
| `node:http` | Canaryo | 9,476 | 1.64 ms | 2.19 ms | 2.49 ms | 0 | 12.2 MiB | 42.4% higher |
| Express 5.2.1 | Node.js | 3,584 | 4.39 ms | 6.02 ms | 7.29 ms | 0 | 63.8 MiB | |
| Express 5.2.1 | Canaryo | 4,299 | 3.44 ms | 5.29 ms | 6.44 ms | 0 | 10.8 MiB | 20.0% higher |

## Persistent connections

Each benchmark worker opens one HTTP/1.1 connection and reuses it for the complete sample.

### Concurrency 1

| Application | Runtime | Requests/s | p50 | p95 | p99 | Errors | RSS | Canaryo throughput difference |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| `node:http` | Node.js | 17,669 | 0.05 ms | 0.09 ms | 0.13 ms | 0 | 35.6 MiB | |
| `node:http` | Canaryo | 16,059 | 0.05 ms | 0.09 ms | 0.12 ms | 0 | 7.0 MiB | 9.1% lower |
| Express 5.2.1 | Node.js | 6,371 | 0.15 ms | 0.21 ms | 0.26 ms | 0 | 75.3 MiB | |
| Express 5.2.1 | Canaryo | 5,256 | 0.17 ms | 0.24 ms | 0.32 ms | 0 | 10.1 MiB | 17.5% lower |

### Concurrency 16

| Application | Runtime | Requests/s | p50 | p95 | p99 | Errors | RSS | Canaryo throughput difference |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| `node:http` | Node.js | 22,056 | 0.63 ms | 1.54 ms | 2.79 ms | 0 | 37.6 MiB | |
| `node:http` | Canaryo | 27,205 | 0.51 ms | 0.91 ms | 1.07 ms | 0 | 7.0 MiB | 23.3% higher |
| Express 5.2.1 | Node.js | 6,359 | 2.33 ms | 3.68 ms | 11.11 ms | 0 | 88.4 MiB | |
| Express 5.2.1 | Canaryo | 6,584 | 2.20 ms | 3.48 ms | 4.15 ms | 0 | 10.6 MiB | 3.5% higher |

## Interpretation

Canaryo leads startup, memory, new-connection throughput, and persistent throughput at concurrency 16 in this test. Its event loop also produced a lower p99 latency in every concurrency-16 row.

Node.js remains faster with one persistent connection. At that point networking overhead is small and the benchmark is dominated by serial JavaScript execution, where V8's optimizing JIT has an advantage over the lightweight QuickJS-NG engine. Improving this case requires engine-level work or safe native specialization rather than HTTP shortcuts.

## Methodology

Both runtimes execute the same files in `fixtures/http-basic` and `fixtures/express-basic`. The load generator runs in a separate process and validates every HTTP status response. The table reports the sample with the median request rate; its latency percentiles and process resident memory are shown on the same row.

The benchmark is a synthetic loopback test of Canaryo's current compatibility surface. Canaryo implements fewer HTTP, stream, filesystem, event-loop, TLS, and debugging features than Node.js. Results will vary by operating system and hardware.

## Reproduce

```sh
npm ci --prefix fixtures/express-basic
cargo build --release
cargo bench --bench runtime -- --duration 3 --runs 3 --startup-runs 7
cargo bench --bench runtime -- --keep-alive --duration 3 --runs 3 --startup-runs 7
```

Increase `--duration` and `--runs` for a longer comparison. Use `--startup-only` to skip the load tests while optimizing initialization.
