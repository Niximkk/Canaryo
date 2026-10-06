# Benchmarks

Results collected on October 5, 2026 with Canaryo 0.2.0. The final POC is
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
response. Lower is better. Because startup varied by several milliseconds on
Windows, this table reports the median of the three independent run medians,
with seven process launches in each run.

| Application | Node.js | Bun | Canaryo |
|---|---:|---:|---:|
| Express 5.2.1 | 195.94 ms | 113.73 ms | **88.97 ms** |
| Express profile 5.2.1 | 241.44 ms | 129.35 ms | **120.00 ms** |

Canaryo starts the basic application 21.8% faster than Bun and the larger
profile 7.2% faster.

## New TCP connection per request

### Concurrency 1

| Application | Runtime | Requests/s | p50 | p95 | p99 | Errors | RSS |
|---|---|---:|---:|---:|---:|---:|---:|
| Express 5.2.1 | Node.js | 1,509 | 0.64 ms | 0.87 ms | 1.07 ms | 0 | 52.4 MiB |
| Express 5.2.1 | Bun | 2,187 | 0.43 ms | 0.60 ms | 0.75 ms | 0 | 54.2 MiB |
| Express 5.2.1 | Canaryo | **2,308** | **0.41 ms** | **0.57 ms** | **0.65 ms** | 0 | **14.8 MiB** |
| Express profile 5.2.1 | Node.js | 1,541 | 0.62 ms | 0.85 ms | 1.11 ms | 0 | 59.1 MiB |
| Express profile 5.2.1 | Bun | 2,175 | 0.43 ms | **0.58 ms** | 0.73 ms | 0 | 57.7 MiB |
| Express profile 5.2.1 | Canaryo | **2,259** | **0.41 ms** | **0.58 ms** | **0.67 ms** | 0 | **17.2 MiB** |

### Concurrency 16

| Application | Runtime | Requests/s | p50 | p95 | p99 | Errors | RSS |
|---|---|---:|---:|---:|---:|---:|---:|
| Express 5.2.1 | Node.js | 3,261 | 4.78 ms | 6.61 ms | 8.05 ms | 0 | 62.4 MiB |
| Express 5.2.1 | Bun | 6,318 | 2.44 ms | 3.22 ms | 4.94 ms | 0 | 60.3 MiB |
| Express 5.2.1 | Canaryo | **8,124** | **1.88 ms** | **2.61 ms** | **3.12 ms** | 0 | **20.5 MiB** |
| Express profile 5.2.1 | Node.js | 3,221 | 4.86 ms | 6.90 ms | 8.30 ms | 0 | 65.6 MiB |
| Express profile 5.2.1 | Bun | 6,104 | 2.53 ms | 3.46 ms | 4.97 ms | 0 | 67.2 MiB |
| Express profile 5.2.1 | Canaryo | **7,766** | **2.05 ms** | **2.61 ms** | **2.97 ms** | 0 | **23.4 MiB** |

## Persistent GET connections

Each worker opens one HTTP/1.1 connection and reuses it for the complete sample.

### Concurrency 1

| Application | Runtime | Requests/s | p50 | p95 | p99 | Errors | RSS |
|---|---|---:|---:|---:|---:|---:|---:|
| Express 5.2.1 | Node.js | 6,046 | 0.15 ms | 0.22 ms | 0.32 ms | 0 | 74.4 MiB |
| Express 5.2.1 | Bun | 14,967 | 0.06 ms | 0.10 ms | 0.13 ms | 0 | 61.0 MiB |
| Express 5.2.1 | Canaryo | **16,699** | **0.05 ms** | **0.09 ms** | **0.11 ms** | 0 | **26.9 MiB** |
| Express profile 5.2.1 | Node.js | 5,964 | 0.16 ms | 0.23 ms | 0.31 ms | 0 | 85.1 MiB |
| Express profile 5.2.1 | Bun | 13,266 | 0.07 ms | 0.11 ms | 0.14 ms | 0 | 67.7 MiB |
| Express profile 5.2.1 | Canaryo | **15,247** | **0.05 ms** | **0.10 ms** | **0.12 ms** | 0 | **28.2 MiB** |

### Concurrency 16

| Application | Runtime | Requests/s | p50 | p95 | p99 | Errors | RSS |
|---|---|---:|---:|---:|---:|---:|---:|
| Express 5.2.1 | Node.js | 5,967 | 2.55 ms | 3.79 ms | 5.23 ms | 0 | 88.0 MiB |
| Express 5.2.1 | Bun | 18,153 | 0.79 ms | 1.32 ms | 2.03 ms | 0 | 59.4 MiB |
| Express 5.2.1 | Canaryo | **20,075** | **0.68 ms** | **1.15 ms** | **1.33 ms** | 0 | **29.6 MiB** |
| Express profile 5.2.1 | Node.js | 5,852 | 2.60 ms | 3.89 ms | 5.32 ms | 0 | 86.6 MiB |
| Express profile 5.2.1 | Bun | 16,593 | 0.85 ms | 1.49 ms | 2.40 ms | 0 | 70.3 MiB |
| Express profile 5.2.1 | Canaryo | **19,617** | **0.68 ms** | **1.14 ms** | **1.38 ms** | 0 | **34.8 MiB** |

## Persistent connections with a 16 KiB JSON body

These requests use `POST /echo` and return the parsed request body. This
exercises request streaming, JSON parsing, mutation checks, serialization,
Express ETags and binary transfer across the Rust/JavaScript boundary.

### Concurrency 1

| Application | Runtime | Requests/s | p50 | p95 | p99 | Errors | RSS |
|---|---|---:|---:|---:|---:|---:|---:|
| Express 5.2.1 | Node.js | 3,300 | 0.28 ms | 0.40 ms | 0.58 ms | 0 | 77.8 MiB |
| Express 5.2.1 | Bun | 5,728 | 0.15 ms | 0.23 ms | 0.88 ms | 0 | 62.4 MiB |
| Express 5.2.1 | Canaryo | **7,173** | **0.12 ms** | **0.19 ms** | **0.86 ms** | 0 | **15.5 MiB** |
| Express profile 5.2.1 | Node.js | 3,219 | 0.29 ms | 0.41 ms | 0.60 ms | 0 | 84.2 MiB |
| Express profile 5.2.1 | Bun | 5,830 | 0.15 ms | 0.22 ms | 0.80 ms | 0 | 61.7 MiB |
| Express profile 5.2.1 | Canaryo | **7,350** | **0.11 ms** | **0.18 ms** | **0.28 ms** | 0 | **17.0 MiB** |

### Concurrency 16

| Application | Runtime | Requests/s | p50 | p95 | p99 | Errors | RSS |
|---|---|---:|---:|---:|---:|---:|---:|
| Express 5.2.1 | Node.js | 3,658 | 4.12 ms | 6.04 ms | 7.12 ms | 0 | 83.7 MiB |
| Express 5.2.1 | Bun | 7,503 | 1.92 ms | 3.22 ms | 4.83 ms | 0 | 71.8 MiB |
| Express 5.2.1 | Canaryo | **8,922** | **1.58 ms** | **2.81 ms** | **3.64 ms** | 0 | **16.0 MiB** |
| Express profile 5.2.1 | Node.js | 3,574 | 4.26 ms | 5.94 ms | 6.87 ms | 0 | 86.7 MiB |
| Express profile 5.2.1 | Bun | 7,552 | 1.89 ms | **3.25 ms** | 4.89 ms | 0 | 71.8 MiB |
| Express profile 5.2.1 | Canaryo | **8,663** | **1.57 ms** | 3.33 ms | **4.03 ms** | 0 | **18.8 MiB** |

## Interpretation

Canaryo exceeds Bun's median request rate in all 12 measured Express workload
and concurrency combinations. The advantage ranges from 3.9% to 28.6%. At
concurrency 16, Canaryo uses 50.2% to 77.7% less resident memory than Bun.

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
runtime/application pair, it chooses the sample with the median request rate and
reports latency percentiles and resident memory from that same sample.

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
