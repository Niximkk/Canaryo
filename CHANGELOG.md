# Changelog

## 0.2.0 - 2026-09-20

- Refocus development on a one-week Express 5.2.1 compatibility profile.
- Add differential and black-box coverage for route parameters, queries,
  mounted routers, built-in body parsers, middleware ordering, cookies,
  redirects and error handlers.
- Match Node's HTTP reason phrases and `set-cookie` response header shape.
- Support CommonJS CLI entry points with shebangs and the minimal `tls.Server`
  surface required by Express test tooling.
- Verify compression 1.8.2, cors 2.8.5, cookie-parser 1.4.7 and a
  memory-backed Multer 2.0.2 upload against Node.js.
- Add stream piping lifecycle and Buffer slice aliases required by multipart
  middleware.
- Report exact Express-profile package matches separately in `canaryo check`.
- Cover `sendFile`, downloads, ranges, cache validators and conditional
  responses in the differential Express profile.
- Add a representative Express middleware benchmark alongside the minimal
  Express fixture.
- Optimize release builds with thin LTO, one code generation unit and stripped
  symbols.
- Publish pre-release tags as GitHub pre-releases and reject tags that do not
  match the Cargo package version.

## 0.1.0

Canaryo 0.1.0 is the final proof-of-concept release.

- Runs focused CommonJS, `node:http`, and Express 5.2.1 applications through QuickJS-NG embedded in Rust.
- Supports Express routing, JSON bodies and responses, static files, and compression middleware in the published fixtures.
- Includes `canaryo check`, a differential compatibility harness, and reproducible Node.js/Bun benchmarks.
- Provides automated Windows x86-64 and Linux x86-64 release archives with SHA-256 checksums.
- Fixes Node.js 22.15.1 as the behavioral baseline.

The project is an educational experiment and does not promise production support or continued Node.js compatibility work.
