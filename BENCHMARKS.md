# Benchmarks

Results collected on September 10, 2026 with commit `cf6b99a` plus the benchmark harness added immediately afterward.

## Environment

- Windows 10 Home 10.0.26200.0, x86-64
- AMD Ryzen 5 5600X, 6 cores and 12 logical processors
- Node.js 22.15.1
- Canaryo 0.1.0, compiled with the Cargo release profile
- Three load samples of three seconds per result
- Seven startup samples per result
- 500 ms warm-up before each load sample

## Startup

Startup is measured from process creation until the first complete valid HTTP response. It includes Canaryo's compatibility analysis and module loading.

| Application | Runtime | Median | Minimum | Maximum | Canaryo difference |
|---|---:|---:|---:|---:|---:|
| `node:http` | Node.js | 53.01 ms | 50.02 ms | 54.28 ms | |
| `node:http` | Canaryo | 9.92 ms | 9.41 ms | 15.50 ms | 81.3% lower |
| Express 5.2.1 | Node.js | 207.38 ms | 191.75 ms | 209.75 ms | |
| Express 5.2.1 | Canaryo | 342.99 ms | 337.09 ms | 355.93 ms | 65.4% higher |

Canaryo starts the small HTTP fixture about 5.3 times faster. Express starts more slowly because Canaryo currently scans and loads its dependency tree synchronously before serving requests.

## HTTP throughput, concurrency 1

| Application | Runtime | Requests/s | p50 | p95 | p99 | Errors | RSS | Canaryo throughput difference |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| `node:http` | Node.js | 2,248 | 0.42 ms | 0.59 ms | 0.67 ms | 0 | 34.5 MiB | |
| `node:http` | Canaryo | 2,595 | 0.36 ms | 0.53 ms | 0.62 ms | 0 | 5.8 MiB | 15.4% higher |
| Express 5.2.1 | Node.js | 1,597 | 0.60 ms | 0.82 ms | 1.03 ms | 0 | 53.3 MiB | |
| Express 5.2.1 | Canaryo | 1,856 | 0.49 ms | 0.74 ms | 0.96 ms | 0 | 10.1 MiB | 16.2% higher |

## HTTP throughput, concurrency 16

| Application | Runtime | Requests/s | p50 | p95 | p99 | Errors | RSS | Canaryo throughput difference |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| `node:http` | Node.js | 6,620 | 2.33 ms | 3.24 ms | 3.82 ms | 0 | 37.1 MiB | |
| `node:http` | Canaryo | 7,581 | 2.06 ms | 2.69 ms | 2.99 ms | 0 | 5.9 MiB | 14.5% higher |
| Express 5.2.1 | Node.js | 3,180 | 4.85 ms | 7.51 ms | 9.37 ms | 0 | 58.7 MiB | |
| Express 5.2.1 | Canaryo | 3,873 | 3.98 ms | 5.86 ms | 6.69 ms | 0 | 10.4 MiB | 21.8% higher |

## Methodology

Both runtimes execute the same files in `fixtures/http-basic` and `fixtures/express-basic`. The load generator runs in a separate process and validates every HTTP status response. The table reports the sample with the median request rate; its latency percentiles and process resident memory are shown on the same row.

Every request opens a new TCP connection and sends `Connection: close`. This is the common behavior supported by both runtimes because Canaryo does not implement keep-alive yet. The test therefore does not measure Node.js's keep-alive performance.

The benchmark is a synthetic loopback test of Canaryo's current compatibility surface. Canaryo implements fewer HTTP, stream, filesystem, event-loop, TLS, and debugging features than Node.js, so these results do not establish broader runtime performance. Results will vary by operating system and hardware.

## Reproduce

Install the Express fixture once, then build and run the benchmark:

```sh
npm ci --prefix fixtures/express-basic
cargo build --release
cargo bench --bench runtime -- --duration 3 --runs 3 --startup-runs 7
```

Increase `--duration` and `--runs` for a longer comparison.
