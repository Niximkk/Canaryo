# Benchmarks

Results collected on September 10, 2026 with Canaryo 0.1.0.

## Environment

- Windows 11 build 26200, x86-64
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
| `node:http` | Node.js | 51.86 ms | 43.46 ms | 53.38 ms | |
| `node:http` | Canaryo | 9.91 ms | 9.29 ms | 16.47 ms | 80.9% lower |
| Express 5.2.1 | Node.js | 206.60 ms | 193.95 ms | 221.86 ms | |
| Express 5.2.1 | Canaryo | 166.84 ms | 151.52 ms | 168.60 ms | 19.2% lower |

Canaryo starts the small HTTP fixture about 5.2 times faster. Express starts 19.2% faster after removing compatibility analysis from the execution path, caching CommonJS resolutions by directory, and replacing repeated filesystem canonicalization with lexical path normalization. Full compatibility analysis remains available through `canaryo check`.

## HTTP throughput, concurrency 1

| Application | Runtime | Requests/s | p50 | p95 | p99 | Errors | RSS | Canaryo throughput difference |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| `node:http` | Node.js | 2,329 | 0.42 ms | 0.54 ms | 0.65 ms | 0 | 34.6 MiB | |
| `node:http` | Canaryo | 2,720 | 0.35 ms | 0.47 ms | 0.56 ms | 0 | 5.8 MiB | 16.8% higher |
| Express 5.2.1 | Node.js | 1,638 | 0.59 ms | 0.77 ms | 0.94 ms | 0 | 52.5 MiB | |
| Express 5.2.1 | Canaryo | 2,029 | 0.46 ms | 0.61 ms | 0.77 ms | 0 | 10.7 MiB | 23.9% higher |

## HTTP throughput, concurrency 16

| Application | Runtime | Requests/s | p50 | p95 | p99 | Errors | RSS | Canaryo throughput difference |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| `node:http` | Node.js | 6,914 | 2.27 ms | 3.03 ms | 3.74 ms | 0 | 37.1 MiB | |
| `node:http` | Canaryo | 7,975 | 1.95 ms | 2.62 ms | 2.90 ms | 0 | 5.8 MiB | 15.3% higher |
| Express 5.2.1 | Node.js | 3,522 | 4.34 ms | 6.62 ms | 8.13 ms | 0 | 64.8 MiB | |
| Express 5.2.1 | Canaryo | 4,183 | 3.60 ms | 5.40 ms | 6.42 ms | 0 | 10.0 MiB | 18.8% higher |

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
