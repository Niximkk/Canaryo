# Benchmarks

Results collected on October 7, 2026 with Canaryo 0.2.1. The final POC is
focused on running existing Express applications, so these results compare an
Express 5.2.1 application and a larger Express profile against Node.js and Bun.

## Environment

- Windows 11 build 26200, x86-64
- AMD Ryzen 5 5600X, 6 cores and 12 logical processors
- Node.js 22.15.1
- Bun 1.4.2
- Canaryo compiled with the Cargo release profile
- Three load samples of three seconds per result
- Seven startup samples per run
- 500 ms warm-up before every load sample

The profile imports Express, compression, cors, cookie-parser and multer. Its
root route runs global middleware, route middleware and a JSON response handler.
All runtimes execute the same application files and every measured response is
validated by the load generator.

## Startup

Startup is measured from process creation until the first complete HTTP
response. Lower is better. This table reports the median of seven process
launches for each runtime and application.

| Application | Node.js | Bun | Canaryo |
|---|---:|---:|---:|
| Express 5.2.1 | 208.13 ms | 122.10 ms | **103.66 ms** |
| Express profile 5.2.1 | 268.60 ms | 143.17 ms | **137.98 ms** |

Canaryo starts the basic application 15.1% faster than Bun and the larger
profile 3.6% faster.

## New TCP connection per request

### Concurrency 1

| Application | Runtime | Requests/s | p50 | p95 | p99 | Errors | RSS |
|---|---|---:|---:|---:|---:|---:|---:|
| Express 5.2.1 | Node.js | 1,340 | 0.72 ms | 0.99 ms | 1.21 ms | 0 | 53.3 MiB |
| Express 5.2.1 | Bun | 2,097 | 0.45 ms | 0.62 ms | 0.77 ms | 0 | 55.8 MiB |
| Express 5.2.1 | Canaryo | **2,075** | **0.46 ms** | **0.64 ms** | **0.73 ms** | 0 | **18.6 MiB** |
| Express profile 5.2.1 | Node.js | 1,464 | 0.66 ms | 0.91 ms | 1.08 ms | 0 | 61.8 MiB |
| Express profile 5.2.1 | Bun | 1,843 | 0.51 ms | **0.74 ms** | 0.96 ms | 0 | 53.5 MiB |
| Express profile 5.2.1 | Canaryo | **2,016** | **0.44 ms** | **0.69 ms** | **0.85 ms** | 0 | **16.9 MiB** |

### Concurrency 16

| Application | Runtime | Requests/s | p50 | p95 | p99 | Errors | RSS |
|---|---|---:|---:|---:|---:|---:|---:|
| Express 5.2.1 | Node.js | 3,159 | 4.92 ms | 6.88 ms | 8.29 ms | 0 | 65.9 MiB |
| Express 5.2.1 | Bun | 6,370 | 2.38 ms | 3.37 ms | 4.85 ms | 0 | 60.9 MiB |
| Express 5.2.1 | Canaryo | **7,409** | **2.05 ms** | **2.97 ms** | **3.68 ms** | 0 | **20.0 MiB** |
| Express profile 5.2.1 | Node.js | 3,114 | 4.98 ms | 7.36 ms | 9.38 ms | 0 | 70.3 MiB |
| Express profile 5.2.1 | Bun | 6,264 | 2.43 ms | 3.35 ms | 4.79 ms | 0 | 64.7 MiB |
| Express profile 5.2.1 | Canaryo | **7,832** | **1.90 ms** | **2.68 ms** | **3.21 ms** | 0 | **23.5 MiB** |

## Persistent GET connections

Each worker opens one HTTP/1.1 connection and reuses it for the complete sample.

### Concurrency 1

| Application | Runtime | Requests/s | p50 | p95 | p99 | Errors | RSS |
|---|---|---:|---:|---:|---:|---:|---:|
| Express 5.2.1 | Node.js | 6,132 | 0.15 ms | 0.22 ms | 0.36 ms | 0 | 77.7 MiB |
| Express 5.2.1 | Bun | 12,535 | 0.07 ms | 0.13 ms | 0.20 ms | 0 | 54.6 MiB |
| Express 5.2.1 | Canaryo | **14,030** | **0.07 ms** | **0.11 ms** | **0.15 ms** | 0 | **25.8 MiB** |
| Express profile 5.2.1 | Node.js | 5,550 | 0.16 ms | 0.27 ms | 0.39 ms | 0 | 86.5 MiB |
| Express profile 5.2.1 | Bun | 12,973 | 0.07 ms | 0.12 ms | 0.16 ms | 0 | 64.0 MiB |
| Express profile 5.2.1 | Canaryo | **13,151** | **0.07 ms** | **0.12 ms** | **0.19 ms** | 0 | **28.8 MiB** |

### Concurrency 16

| Application | Runtime | Requests/s | p50 | p95 | p99 | Errors | RSS |
|---|---|---:|---:|---:|---:|---:|---:|
| Express 5.2.1 | Node.js | 5,584 | 2.46 ms | 4.89 ms | 15.27 ms | 0 | 89.4 MiB |
| Express 5.2.1 | Bun | 16,611 | 0.90 ms | 1.53 ms | 2.12 ms | 0 | 65.5 MiB |
| Express 5.2.1 | Canaryo | **18,443** | **0.73 ms** | **1.36 ms** | **2.99 ms** | 0 | **30.3 MiB** |
| Express profile 5.2.1 | Node.js | 5,224 | 2.75 ms | 4.83 ms | 14.81 ms | 0 | 88.7 MiB |
| Express profile 5.2.1 | Bun | 14,804 | 1.01 ms | 1.80 ms | 2.54 ms | 0 | 62.3 MiB |
| Express profile 5.2.1 | Canaryo | **16,333** | **0.82 ms** | **1.43 ms** | **2.09 ms** | 0 | **32.6 MiB** |

## Persistent connections with a 16 KiB JSON body

These requests use `POST /echo` and return the parsed request body. This
exercises request streaming, JSON parsing, mutation checks, serialization,
Express ETags and binary transfer across the Rust/JavaScript boundary.

### Concurrency 1

| Application | Runtime | Requests/s | p50 | p95 | p99 | Errors | RSS |
|---|---|---:|---:|---:|---:|---:|---:|
| Express 5.2.1 | Node.js | 2,634 | 0.35 ms | 0.57 ms | 1.12 ms | 0 | 78.3 MiB |
| Express 5.2.1 | Bun | 5,031 | 0.17 ms | 0.31 ms | 0.90 ms | 0 | 66.5 MiB |
| Express 5.2.1 | Canaryo | **6,052** | **0.13 ms** | **0.26 ms** | 1.22 ms | 0 | **14.7 MiB** |
| Express profile 5.2.1 | Node.js | 2,828 | 0.32 ms | 0.53 ms | 0.91 ms | 0 | 84.6 MiB |
| Express profile 5.2.1 | Bun | 5,479 | 0.16 ms | 0.25 ms | 0.85 ms | 0 | 61.0 MiB |
| Express profile 5.2.1 | Canaryo | **5,781** | **0.14 ms** | **0.29 ms** | **0.51 ms** | 0 | **17.6 MiB** |

### Concurrency 16

| Application | Runtime | Requests/s | p50 | p95 | p99 | Errors | RSS |
|---|---|---:|---:|---:|---:|---:|---:|
| Express 5.2.1 | Node.js | 2,823 | 5.52 ms | 7.30 ms | 8.44 ms | 0 | 84.7 MiB |
| Express 5.2.1 | Bun | 5,900 | 2.55 ms | 4.38 ms | 6.13 ms | 0 | 59.5 MiB |
| Express 5.2.1 | Canaryo | **7,615** | **1.86 ms** | **3.71 ms** | **4.81 ms** | 0 | **14.5 MiB** |
| Express profile 5.2.1 | Node.js | 3,333 | 4.71 ms | 6.49 ms | 7.11 ms | 0 | 89.8 MiB |
| Express profile 5.2.1 | Bun | 6,118 | 2.47 ms | 4.22 ms | 5.67 ms | 0 | 71.8 MiB |
| Express profile 5.2.1 | Canaryo | **7,262** | **1.90 ms** | **4.27 ms** | **5.33 ms** | 0 | **16.8 MiB** |

## Interpretation

Canaryo exceeds Bun's median request rate in 11 of the 12 measured Express
workload and concurrency combinations. The one exception is the basic
application with a new TCP connection per request, where Canaryo is 1.0%
below Bun. Across the other combinations, the advantage ranges from 1.4% to
29.1%. At concurrency 16, Canaryo uses 47.6% to 76.6% less resident memory
than Bun.

The latency distributions support the throughput result. Bun has a 0.08 ms
lower p95 in one profile body cell at concurrency 16; Canaryo is equal or lower
in the other 35 reported latency cells. Percentiles vary between repeated runs
on Windows, and the table keeps the values from each selected median-throughput
sample.

The result comes from conservative Express-specific fast paths: exact static
route plans can run through native dispatch, repeated response metadata remains
in Rust, default JSON parsing resumes the native plan directly, and a short
single-connection keep-alive polling window avoids scheduler latency. Dynamic
routes, mounted routers, asynchronous handlers, custom parsers and ambiguous
middleware chains continue through Express's regular implementations.

## Methodology and limits

The load generator runs in a separate process on the same machine. For every
runtime/application pair, it chooses the sample with the median request rate
from three load samples and reports latency percentiles and resident memory
from that same sample.

This is a synthetic loopback benchmark for the POC's verified compatibility
surface. The static-route and repeated-body workloads are favorable to the
bounded caches. Results will vary with hardware, operating system, application
code and response diversity. CPU use is not recorded; the concurrency-1
keep-alive path briefly polls after a response to reduce scheduler latency.

## Reproduce

```sh
npm ci --prefix fixtures/express-basic
cargo build --release
cargo bench --bench runtime -- --bun --case express --duration 3 --runs 3 --startup-runs 7
cargo bench --bench runtime -- --bun --case express --keep-alive --duration 3 --runs 3 --startup-runs 7
cargo bench --bench runtime -- --bun --case express --keep-alive --body-size 16384 --duration 3 --runs 3 --startup-runs 7
```

Increase `--duration` and `--runs` for longer comparisons. Use `--path` to
select another endpoint and `--startup-only` while investigating initialization.
﻿
