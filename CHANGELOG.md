# Changelog

## 0.1.0

Canaryo 0.1.0 is the final proof-of-concept release.

- Runs focused CommonJS, `node:http`, and Express 5.2.1 applications through QuickJS-NG embedded in Rust.
- Supports Express routing, JSON bodies and responses, static files, and compression middleware in the published fixtures.
- Includes `canaryo check`, a differential compatibility harness, and reproducible Node.js/Bun benchmarks.
- Provides automated Windows x86-64 and Linux x86-64 release archives with SHA-256 checksums.
- Fixes Node.js 22.15.1 as the behavioral baseline.

The project is an educational experiment and does not promise production support or continued Node.js compatibility work.
