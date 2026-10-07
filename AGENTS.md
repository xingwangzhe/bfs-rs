# Engineering notes

## Architecture

`@xingwangzhe/bfs-rs` is a Rust + napi-rs addon with a CommonJS loader and TypeScript declarations. The main npm package contains all six supported native `.node` binaries; it does not currently declare platform `optionalDependencies`.

The graph engine uses CSR plus its transpose, epoch visit markers, bitmap frontiers, and direction-switching push/pull BFS. Rayon parallelizes source-node batches. `createBfsGraph(Uint32Array, Uint32Array, n)` owns a reusable graph; the ordinary-array compatibility functions remain available. Histogram APIs avoid allocating complete per-source distance results. The current dependencies do not include `parallel_frontier`.

## Development and performance

Use Bun for JavaScript dependencies and scripts, and Cargo for Rust. Preserve CommonJS semantics in `index.js`; do not add `type: module` without changing the generated loader.

The release profile uses O3, fat LTO, one code-generation unit, and no incremental compilation. `bun run build:pgo` performs profile generation, workload training, LLVM profile merging, and final profile-use compilation. Use toolchain-matched `llvm-tools-preview`, a matching native runner, and the deterministic corpus in `scripts/pgo-workload.mjs`. Linux musl training uses an Alpine Node container. See `scripts/PERFORMANCE.md`.

`bun run benchmark:pgo` compares unprofiled and PGO release builds on holdout inputs. Retain raw measurements and distinguish runtime measurements from compilation time. Never publish host-specific `target-cpu=native` builds under the existing generic target names.

## CI and release

The CI builds macOS x64/arm64, Windows x64, Linux GNU x64/arm64, and Linux musl x64. It tests the final bindings before aggregating them into the main npm package. Every target must pass; musl failures are not optional.

A main-branch commit whose message is a pure version number triggers npm publishing after the required jobs succeed. Non-version implementation commits run CI without publishing. CI uses npm Trusted Publishing (OIDC).

Verify the published registry version and tarball binaries, then load the actual published package in an isolated consumer. A successful push or CI run alone does not establish successful npm publication.

Project metadata currently declares MIT; preserve third-party license obligations independently.
