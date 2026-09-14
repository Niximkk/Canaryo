# Benchmarks

Results collected on September 13, 2026 with Canaryo 0.1.0 after the Web Streams and Fetch compatibility work. Node.js, Bun, and Canaryo execute the same application files under the same load generator.

## Environment

- Windows 11 build 26200, x86-64
- AMD Ryzen 5 5600X, 6 cores and 12 logical processors
- Node.js 22.15.1
- Bun 1.4.2
- Canaryo compiled with the Cargo release profile
- Three load samples of three seconds per result
- Seven startup samples per result
- 500 ms warm-up before each load sample

## Startup

Startup is measured from process creation until the first complete valid HTTP response. It includes module loading but does not run `canaryo check`.

| Application | Runtime | Median | Minimum | Maximum |
|---|---:|---:|---:|---:|
| `node:http` | Node.js | 50.90 ms | 48.97 ms | 60.98 ms |
| `node:http` | Bun | 51.30 ms | 48.21 ms | 52.95 ms |
| `node:http` | Canaryo | **25.24 ms** | **23.49 ms** | **39.86 ms** |
| Express 5.2.1 | Node.js | 218.48 ms | 204.18 ms | 223.70 ms |
| Express 5.2.1 | Bun | **171.33 ms** | **140.89 ms** | **221.97 ms** |
| Express 5.2.1 | Canaryo | 198.14 ms | 178.89 ms | 228.21 ms |
| Fastify 5.12.3 | Node.js | 372.25 ms | 340.49 ms | 415.49 ms |
| Fastify 5.12.3 | Bun | **195.19 ms** | **184.90 ms** | 254.56 ms |
| Fastify 5.12.3 | Canaryo | 213.90 ms | 212.30 ms | **246.95 ms** |

Canaryo starts the native HTTP fixture 50.8% faster than Bun. Bun starts Express 13.5% faster and Fastify 8.7% faster than Canaryo in this run. The original Canaryo Express startup result was 342.99 ms; direct execution, cached CommonJS resolution, lexical path normalization, and subsequent bootstrap work reduced it by 42.2%.

## New connection per request

### Concurrency 1

| Application | Runtime | Requests/s | p50 | p95 | p99 | Errors | RSS |
|---|---:|---:|---:|---:|---:|---:|---:|
| `node:http` | Node.js | 2,076 | 0.46 ms | 0.62 ms | 0.72 ms | 0 | 37.5 MiB |
| `node:http` | Bun | **2,403** | **0.39 ms** | **0.56 ms** | **0.65 ms** | 0 | 40.1 MiB |
| `node:http` | Canaryo | 2,282 | 0.41 ms | 0.58 ms | 0.73 ms | 0 | **12.2 MiB** |
| Express 5.2.1 | Node.js | 1,696 | 0.55 ms | 0.79 ms | 1.03 ms | 0 | 54.9 MiB |
| Express 5.2.1 | Bun | **2,169** | **0.43 ms** | **0.61 ms** | **0.76 ms** | 0 | 53.4 MiB |
| Express 5.2.1 | Canaryo | 1,647 | 0.56 ms | 0.80 ms | 1.08 ms | 0 | **12.6 MiB** |
| Fastify 5.12.3 | Node.js | 2,142 | 0.44 ms | 0.62 ms | 0.75 ms | 0 | 52.7 MiB |
| Fastify 5.12.3 | Bun | **2,489** | **0.38 ms** | **0.54 ms** | **0.65 ms** | 0 | 54.7 MiB |
| Fastify 5.12.3 | Canaryo | 2,159 | 0.44 ms | 0.61 ms | 0.71 ms | 0 | **16.0 MiB** |

### Concurrency 16

| Application | Runtime | Requests/s | p50 | p95 | p99 | Errors | RSS |
|---|---:|---:|---:|---:|---:|---:|---:|
| `node:http` | Node.js | 6,461 | 2.35 ms | 3.50 ms | 4.18 ms | 0 | 40.3 MiB |
| `node:http` | Bun | 7,183 | **2.11 ms** | 3.17 ms | 4.46 ms | 0 | 47.6 MiB |
| `node:http` | Canaryo | **7,365** | 2.16 ms | **2.89 ms** | **3.32 ms** | 0 | **9.8 MiB** |
| Express 5.2.1 | Node.js | 3,250 | 4.85 ms | 6.83 ms | 7.99 ms | 0 | 64.9 MiB |
| Express 5.2.1 | Bun | **6,253** | **2.47 ms** | **3.43 ms** | **5.02 ms** | 0 | 59.1 MiB |
| Express 5.2.1 | Canaryo | 3,630 | 4.15 ms | 6.38 ms | 7.43 ms | 0 | **12.9 MiB** |
| Fastify 5.12.3 | Node.js | 5,905 | 2.63 ms | 3.77 ms | **4.42 ms** | 0 | 53.4 MiB |
| Fastify 5.12.3 | Bun | **6,625** | **2.33 ms** | **3.15 ms** | 4.66 ms | 0 | 63.6 MiB |
| Fastify 5.12.3 | Canaryo | 5,269 | 2.82 ms | 4.09 ms | 6.56 ms | 0 | **16.3 MiB** |

## Persistent connections

Each benchmark worker opens one HTTP/1.1 connection and reuses it for the complete sample.

### Concurrency 1

| Application | Runtime | Requests/s | p50 | p95 | p99 | Errors | RSS |
|---|---:|---:|---:|---:|---:|---:|---:|
| `node:http` | Node.js | 16,958 | 0.05 ms | 0.10 ms | 0.14 ms | 0 | 35.5 MiB |
| `node:http` | Bun | **19,855** | **0.04 ms** | **0.09 ms** | **0.11 ms** | 0 | 42.0 MiB |
| `node:http` | Canaryo | 12,244 | 0.07 ms | 0.12 ms | 0.17 ms | 0 | **9.5 MiB** |
| Express 5.2.1 | Node.js | 6,028 | 0.16 ms | 0.23 ms | 0.30 ms | 0 | 76.7 MiB |
| Express 5.2.1 | Bun | **13,638** | **0.07 ms** | **0.12 ms** | **0.15 ms** | 0 | 60.1 MiB |
| Express 5.2.1 | Canaryo | 4,265 | 0.21 ms | 0.31 ms | 0.41 ms | 0 | **12.9 MiB** |
| Fastify 5.12.3 | Node.js | 14,443 | 0.07 ms | 0.11 ms | 0.14 ms | 0 | 53.8 MiB |
| Fastify 5.12.3 | Bun | **17,626** | **0.05 ms** | **0.09 ms** | **0.12 ms** | 0 | 57.8 MiB |
| Fastify 5.12.3 | Canaryo | 7,023 | 0.13 ms | 0.19 ms | 0.24 ms | 0 | **16.2 MiB** |

### Concurrency 16

| Application | Runtime | Requests/s | p50 | p95 | p99 | Errors | RSS |
|---|---:|---:|---:|---:|---:|---:|---:|
| `node:http` | Node.js | 20,220 | 0.66 ms | 1.72 ms | 2.35 ms | 0 | 38.8 MiB |
| `node:http` | Bun | **25,732** | **0.53 ms** | **0.99 ms** | **1.23 ms** | 0 | 47.3 MiB |
| `node:http` | Canaryo | 19,006 | 0.72 ms | 1.28 ms | 1.52 ms | 0 | **9.8 MiB** |
| Express 5.2.1 | Node.js | 6,019 | 2.49 ms | 3.77 ms | 5.66 ms | 0 | 76.2 MiB |
| Express 5.2.1 | Bun | **16,860** | **0.92 ms** | **1.37 ms** | **2.02 ms** | 0 | 54.7 MiB |
| Express 5.2.1 | Canaryo | 5,313 | 2.81 ms | 4.82 ms | 5.77 ms | 0 | **12.6 MiB** |
| Fastify 5.12.3 | Node.js | 17,587 | 0.84 ms | 1.40 ms | 1.75 ms | 0 | 54.8 MiB |
| Fastify 5.12.3 | Bun | **22,364** | **0.64 ms** | **1.09 ms** | **1.33 ms** | 0 | 58.5 MiB |
| Fastify 5.12.3 | Canaryo | 8,925 | 1.64 ms | 2.44 ms | 5.13 ms | 0 | **17.3 MiB** |

## Persistent connections with a 16 KiB JSON body

These requests use `POST /echo` and include a 16,384-byte JSON document. The response contains the parsed body, exercising request streaming, UTF-8 decoding, JSON parsing and serialization, hashing used by Express ETags, and binary response transfer.

### Concurrency 1

| Application | Runtime | Requests/s | p50 | p95 | p99 | Errors | RSS |
|---|---:|---:|---:|---:|---:|---:|---:|
| `node:http` | Node.js | 7,296 | 0.13 ms | 0.20 ms | 0.27 ms | 0 | 40.4 MiB |
| `node:http` | Bun | **9,056** | **0.10 ms** | **0.16 ms** | **0.26 ms** | 0 | 44.0 MiB |
| `node:http` | Canaryo | 6,895 | 0.13 ms | 0.27 ms | 0.37 ms | 0 | **9.6 MiB** |
| Express 5.2.1 | Node.js | 3,221 | 0.29 ms | 0.43 ms | **0.59 ms** | 0 | 78.3 MiB |
| Express 5.2.1 | Bun | **5,525** | **0.16 ms** | **0.24 ms** | 0.85 ms | 0 | 59.6 MiB |
| Express 5.2.1 | Canaryo | 1,748 | 0.51 ms | 0.78 ms | 1.92 ms | 0 | **14.1 MiB** |
| Fastify 5.12.3 | Node.js | 4,779 | 0.19 ms | 0.28 ms | **0.51 ms** | 0 | 93.0 MiB |
| Fastify 5.12.3 | Bun | **6,397** | **0.14 ms** | **0.21 ms** | 0.72 ms | 0 | 61.2 MiB |
| Fastify 5.12.3 | Canaryo | 971 | 0.92 ms | 1.29 ms | 3.24 ms | 0 | **17.3 MiB** |

### Concurrency 16

| Application | Runtime | Requests/s | p50 | p95 | p99 | Errors | RSS |
|---|---:|---:|---:|---:|---:|---:|---:|
| `node:http` | Node.js | 10,343 | 1.49 ms | **2.06 ms** | **2.29 ms** | 0 | 41.3 MiB |
| `node:http` | Bun | **10,643** | **1.34 ms** | 2.50 ms | 3.47 ms | 0 | 43.6 MiB |
| `node:http` | Canaryo | 7,999 | 1.88 ms | 3.01 ms | 4.46 ms | 0 | **9.2 MiB** |
| Express 5.2.1 | Node.js | 3,038 | 4.88 ms | 7.44 ms | 14.13 ms | 0 | 84.3 MiB |
| Express 5.2.1 | Bun | **6,493** | **2.28 ms** | **3.83 ms** | **5.12 ms** | 0 | 62.3 MiB |
| Express 5.2.1 | Canaryo | 1,797 | 8.75 ms | 11.27 ms | 12.53 ms | 0 | **13.0 MiB** |
| Fastify 5.12.3 | Node.js | 4,919 | 3.05 ms | 4.75 ms | 5.84 ms | 0 | 97.5 MiB |
| Fastify 5.12.3 | Bun | **8,187** | **1.77 ms** | **3.26 ms** | **4.19 ms** | 0 | 72.4 MiB |
| Fastify 5.12.3 | Canaryo | 1,019 | 15.21 ms | 20.87 ms | 24.22 ms | 0 | **17.5 MiB** |

## Interpretation

Canaryo's clearest advantage over Bun is resource efficiency. It starts the native HTTP fixture 50.8% faster and uses 70.4% to 79.4% less resident memory at concurrency 16 across the GET workloads. With new connections at concurrency 16, Canaryo also leads Bun's native HTTP result by 2.5%, while trailing Bun by 41.9% on Express and 20.5% on Fastify.

Persistent connections expose the JavaScript engine gap. At concurrency 16, Canaryo trails Bun by 26.1% on `node:http`, 68.5% on Express, and 60.1% on Fastify. Bun's JavaScriptCore JIT benefits repeated framework code, while Canaryo's QuickJS-NG interpreter retains a much smaller memory footprint.

The 16 KiB body workload widens that gap: Canaryo trails Bun by 24.8% on `node:http`, 72.3% on Express, and 87.6% on Fastify at concurrency 16. Typed-array transfer keeps native HTTP competitive and memory below 10 MiB, but parsing, validation, and serialization inside large frameworks dominate the interpreted paths.

## Methodology

All three runtimes execute the same files in `fixtures/http-basic`, `fixtures/express-basic`, and `fixtures/fastify-basic`. The load generator runs in a separate process and validates every HTTP status response. The table reports the sample with the median request rate; its latency percentiles and process resident memory are shown on the same row.

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
cargo bench --bench runtime -- --bun --keep-alive --duration 3 --runs 3 --startup-runs 7
```

Increase `--duration` and `--runs` for a longer comparison. Use `--bun` to add Bun to every table, `--case` to run only matching applications, such as `fastify` or `express`, `--body-size` to benchmark `POST /echo` with a generated JSON body, and `--startup-only` to skip the load tests while optimizing initialization.
