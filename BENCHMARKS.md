# Benchmarks

Results collected on September 13, 2026 with Canaryo 0.1.0. The final POC compares its two demonstrated application targets, `node:http` and Express 5.2.1, with Node.js and Bun.

## Environment

- Windows 11 build 26200, x86-64
- AMD Ryzen 5 5600X, 6 cores and 12 logical processors
- Node.js 22.15.1
- Bun 1.4.2
- Canaryo compiled with the Cargo release profile
- Three load samples of three seconds per result
- Seven startup samples per result
- 500 ms warm-up before each load sample

## Express profile baseline

This focused result was collected on September 20, 2026 after adding the
documented Express profile. The profile route runs global middleware, a
route-level middleware and a JSON response handler. It imports Express 5.2.1,
compression 1.8.2, cors 2.8.5, cookie-parser 1.4.7 and multer 2.0.2. These tables
use three two-second load samples and five startup samples; they are kept
separate from the longer fixture results below.

### Startup

| Runtime | Median | Minimum | Maximum |
|---|---:|---:|---:|
| Node.js | 242.41 ms | 238.17 ms | 249.56 ms |
| Bun | **147.16 ms** | **146.65 ms** | **155.81 ms** |
| Canaryo | 231.99 ms | 228.54 ms | 244.54 ms |

### Persistent connections

| Concurrency | Runtime | Requests/s | p50 | p95 | p99 | Errors | RSS |
|---:|---|---:|---:|---:|---:|---:|---:|
| 1 | Node.js | 6,019 | 0.15 ms | 0.23 ms | 0.32 ms | 0 | 86.0 MiB |
| 1 | Bun | **10,771** | **0.09 ms** | **0.12 ms** | **0.16 ms** | 0 | 59.6 MiB |
| 1 | Canaryo | 4,184 | 0.22 ms | 0.29 ms | 0.34 ms | 0 | **15.7 MiB** |
| 16 | Node.js | 6,524 | 2.20 ms | 3.73 ms | 10.76 ms | 0 | 97.2 MiB |
| 16 | Bun | **20,037** | **0.75 ms** | **1.00 ms** | **1.91 ms** | 0 | 59.6 MiB |
| 16 | Canaryo | 5,074 | 2.85 ms | 5.98 ms | 6.59 ms | 0 | **16.3 MiB** |

At concurrency 16, Canaryo uses 83.2% less resident memory than Node.js and
72.7% less than Bun. Its request rate is 22.2% below Node.js and 74.7% below
Bun. The native HTTP fixture is much faster than the Express profile under the
same runtime, so the remaining throughput cost is dominated by framework and
middleware JavaScript interpreted by QuickJS-NG. Express-specific optimization
must reduce that repeated JavaScript work while preserving the profile's
differential behavior.

Temporary phase instrumentation on the profile route measured approximately
178 microseconds per request inside the Express handler under QuickJS-NG. The
measured request/response object bridge, response extraction and HTTP
serialization together used about 20 microseconds. This confirms that further
HTTP parser micro-optimizations cannot close the framework throughput gap; a
large improvement would require reducing interpreted Express work or changing
the JavaScript engine. The instrumentation was removed after measurement so it
does not affect the published results.

Reproduce this focused measurement with:

```sh
cargo build --release
cargo bench --bench runtime -- --case "express profile" --bun --keep-alive --duration 2 --runs 3 --startup-runs 5
```

### Unreleased static-route fast path

A September 20 development build precomputed safe static Express route plans
and skipped idle native HTTP connection work. Five three-second samples produced
the following result. Dynamic routes, mounted routers and route stacks changed
after startup continue through Express's original dispatcher.

| Concurrency | Runtime | Requests/s | p50 | p95 | p99 | Errors | RSS |
|---:|---|---:|---:|---:|---:|---:|---:|
| 1 | Node.js | 5,716 | 0.16 ms | 0.24 ms | 0.31 ms | 0 | 87.6 MiB |
| 1 | Bun | **13,796** | **0.07 ms** | **0.11 ms** | **0.14 ms** | 0 | 61.5 MiB |
| 1 | Canaryo | 4,464 | 0.20 ms | 0.30 ms | 0.37 ms | 0 | **16.2 MiB** |
| 16 | Node.js | 5,771 | 2.67 ms | 3.88 ms | **5.27 ms** | 0 | 89.5 MiB |
| 16 | Bun | **16,662** | **0.87 ms** | **1.43 ms** | **2.03 ms** | 0 | 60.0 MiB |
| 16 | Canaryo | 5,323 | 2.84 ms | 3.86 ms | 7.27 ms | 0 | **16.6 MiB** |

Compared with the Canaryo baseline above, throughput increased by 6.7% at
concurrency 1 and 4.9% at concurrency 16. In this run Canaryo reached 92.2% of
Node.js throughput at concurrency 16 while using 81.5% less resident memory.
The result still leaves a large gap to Bun because the selected middleware and
route handlers continue to execute in QuickJS-NG.

```sh
cargo bench --bench runtime -- --case "express profile" --bun --keep-alive --duration 3 --runs 5 --startup-runs 5
```

## Startup

Startup is measured from process creation until the first complete valid HTTP response. Lower is better.

| Application | Runtime | Median | Minimum | Maximum |
|---|---:|---:|---:|---:|
| `node:http` | Node.js | 50.90 ms | 48.97 ms | 60.98 ms |
| `node:http` | Bun | 51.30 ms | 48.21 ms | 52.95 ms |
| `node:http` | Canaryo | **25.24 ms** | **23.49 ms** | **39.86 ms** |
| Express 5.2.1 | Node.js | 218.48 ms | 204.18 ms | 223.70 ms |
| Express 5.2.1 | Bun | **171.33 ms** | **140.89 ms** | **221.97 ms** |
| Express 5.2.1 | Canaryo | 198.14 ms | 178.89 ms | 228.21 ms |

Canaryo starts the native HTTP fixture 50.8% faster than Bun. Bun starts the Express fixture 13.5% faster than Canaryo in this run.

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

### Concurrency 16

| Application | Runtime | Requests/s | p50 | p95 | p99 | Errors | RSS |
|---|---:|---:|---:|---:|---:|---:|---:|
| `node:http` | Node.js | 6,461 | 2.35 ms | 3.50 ms | 4.18 ms | 0 | 40.3 MiB |
| `node:http` | Bun | 7,183 | **2.11 ms** | 3.17 ms | 4.46 ms | 0 | 47.6 MiB |
| `node:http` | Canaryo | **7,365** | 2.16 ms | **2.89 ms** | **3.32 ms** | 0 | **9.8 MiB** |
| Express 5.2.1 | Node.js | 3,250 | 4.85 ms | 6.83 ms | 7.99 ms | 0 | 64.9 MiB |
| Express 5.2.1 | Bun | **6,253** | **2.47 ms** | **3.43 ms** | **5.02 ms** | 0 | 59.1 MiB |
| Express 5.2.1 | Canaryo | 3,630 | 4.15 ms | 6.38 ms | 7.43 ms | 0 | **12.9 MiB** |

## Persistent connections

Each worker opens one HTTP/1.1 connection and reuses it for the complete sample.

### Concurrency 1

| Application | Runtime | Requests/s | p50 | p95 | p99 | Errors | RSS |
|---|---:|---:|---:|---:|---:|---:|---:|
| `node:http` | Node.js | 16,958 | 0.05 ms | 0.10 ms | 0.14 ms | 0 | 35.5 MiB |
| `node:http` | Bun | **19,855** | **0.04 ms** | **0.09 ms** | **0.11 ms** | 0 | 42.0 MiB |
| `node:http` | Canaryo | 12,244 | 0.07 ms | 0.12 ms | 0.17 ms | 0 | **9.5 MiB** |
| Express 5.2.1 | Node.js | 6,028 | 0.16 ms | 0.23 ms | 0.30 ms | 0 | 76.7 MiB |
| Express 5.2.1 | Bun | **13,638** | **0.07 ms** | **0.12 ms** | **0.15 ms** | 0 | 60.1 MiB |
| Express 5.2.1 | Canaryo | 4,265 | 0.21 ms | 0.31 ms | 0.41 ms | 0 | **12.9 MiB** |

### Concurrency 16

| Application | Runtime | Requests/s | p50 | p95 | p99 | Errors | RSS |
|---|---:|---:|---:|---:|---:|---:|---:|
| `node:http` | Node.js | 20,220 | 0.66 ms | 1.72 ms | 2.35 ms | 0 | 38.8 MiB |
| `node:http` | Bun | **25,732** | **0.53 ms** | **0.99 ms** | **1.23 ms** | 0 | 47.3 MiB |
| `node:http` | Canaryo | 19,006 | 0.72 ms | 1.28 ms | 1.52 ms | 0 | **9.8 MiB** |
| Express 5.2.1 | Node.js | 6,019 | 2.49 ms | 3.77 ms | 5.66 ms | 0 | 76.2 MiB |
| Express 5.2.1 | Bun | **16,860** | **0.92 ms** | **1.37 ms** | **2.02 ms** | 0 | 54.7 MiB |
| Express 5.2.1 | Canaryo | 5,313 | 2.81 ms | 4.82 ms | 5.77 ms | 0 | **12.6 MiB** |

## Persistent connections with a 16 KiB JSON body

These requests use `POST /echo` and include a 16,384-byte JSON document.

### Concurrency 1

| Application | Runtime | Requests/s | p50 | p95 | p99 | Errors | RSS |
|---|---:|---:|---:|---:|---:|---:|---:|
| `node:http` | Node.js | 7,296 | 0.13 ms | 0.20 ms | 0.27 ms | 0 | 40.4 MiB |
| `node:http` | Bun | **9,056** | **0.10 ms** | **0.16 ms** | **0.26 ms** | 0 | 44.0 MiB |
| `node:http` | Canaryo | 6,895 | 0.13 ms | 0.27 ms | 0.37 ms | 0 | **9.6 MiB** |
| Express 5.2.1 | Node.js | 3,221 | 0.29 ms | 0.43 ms | **0.59 ms** | 0 | 78.3 MiB |
| Express 5.2.1 | Bun | **5,525** | **0.16 ms** | **0.24 ms** | 0.85 ms | 0 | 59.6 MiB |
| Express 5.2.1 | Canaryo | 1,748 | 0.51 ms | 0.78 ms | 1.92 ms | 0 | **14.1 MiB** |

### Concurrency 16

| Application | Runtime | Requests/s | p50 | p95 | p99 | Errors | RSS |
|---|---:|---:|---:|---:|---:|---:|---:|
| `node:http` | Node.js | 10,343 | 1.49 ms | **2.06 ms** | **2.29 ms** | 0 | 41.3 MiB |
| `node:http` | Bun | **10,643** | **1.34 ms** | 2.50 ms | 3.47 ms | 0 | 43.6 MiB |
| `node:http` | Canaryo | 7,999 | 1.88 ms | 3.01 ms | 4.46 ms | 0 | **9.2 MiB** |
| Express 5.2.1 | Node.js | 3,038 | 4.88 ms | 7.44 ms | 14.13 ms | 0 | 84.3 MiB |
| Express 5.2.1 | Bun | **6,493** | **2.28 ms** | **3.83 ms** | **5.12 ms** | 0 | 62.3 MiB |
| Express 5.2.1 | Canaryo | 1,797 | 8.75 ms | 11.27 ms | 12.53 ms | 0 | **13.0 MiB** |

## Interpretation

Canaryo's strongest result is resource efficiency. At concurrency 16 it uses 77.0% to 79.4% less resident memory than Bun across the GET workloads. It also starts the native HTTP fixture 50.8% faster and leads Bun's new-connection native HTTP throughput by 2.5%.

QuickJS-NG does not have the optimizing JIT used by V8 and JavaScriptCore. That cost is most visible in persistent Express requests and JSON parsing. These results support the POC's claim of fast startup and low memory; they do not claim overall performance leadership.

## Methodology

All runtimes execute the same files in `fixtures/http-basic` and `fixtures/express-basic`. The load generator runs in a separate process and validates every response. The table reports the sample with the median request rate and the latency and memory measurements from that sample.

This is a synthetic loopback benchmark on one machine. Results vary by operating system, hardware, runtime version, and workload.

## Reproduce

```sh
npm ci --prefix fixtures/express-basic
cargo build --release
cargo bench --bench runtime -- --duration 3 --runs 3 --startup-runs 7
cargo bench --bench runtime -- --keep-alive --duration 3 --runs 3 --startup-runs 7
cargo bench --bench runtime -- --keep-alive --body-size 16384 --duration 3 --runs 3 --startup-runs 7
cargo bench --bench runtime -- --bun --keep-alive --duration 3 --runs 3 --startup-runs 7
```

Use `--case express` or `--case node:http` to isolate one application, `--body-size` to generate the JSON workload, and `--startup-only` while investigating initialization.
