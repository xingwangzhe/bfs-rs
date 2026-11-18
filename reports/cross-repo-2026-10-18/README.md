# FriendLinks exact-histogram experiments, 2026-10-18

The consumer fixture contains 49,064 vertices and 215,054 directed CSR arcs,
representing 107,527 undirected edges. It includes 32,085 degree-one vertices and
27 isolated vertices. The input was exported by the FriendLinks application;
SHA-256 of the full JSON fixture is
`aa3f3db3eae6914ecb3651ecff3cdfeba7dc78fb1e4bf822e6936079b54c1e70`.
No graph is sampled for these measurements. Rust release builds retain the
generic CPU target and use Rust 1.99.0, O3, fat LTO, and one code-generation unit.
The host exposes 16 processors. Other agents' compilers and heavy benchmarks
were held during the reported runtime windows.

| Variant | Measurements in milliseconds | Decision |
| --- | --- | --- |
| Published npm 0.2.3 | 15,172, one standalone sample | Reference result and baseline; the complete consumer build measured its BFS stage separately. |
| Independent-source petgraph 0.8.3 control | 25,387, one sample | Rejected for this workload. Uses the public Bfs queue and visited bitmap, reused across sources, with Rayon across sources. |
| Generic 64-source push | 1,765 median of 3 | Large improvement, superseded by weighted twins. |
| Weighted twins, push | 623 median of 3; 654 median of 10 later | Correct, superseded by push/pull. |
| Weighted twins, push/pull | 387 and 446 medians of 5 in the two process orders | Selected traversal strategy. |
| Weighted twins, push/pull, dispatched POPCNT | 371 and 405 medians of 5 in the two process orders | Selected x86 path, with generic fallback. |

The last comparison ran A-B then B-A, each process with two discarded warmup
calls and five recorded samples. CPU frequency and temperature cause visible
variation; the raw arrays are retained. POPCNT improved both process medians
(4.3% and 9.1%) on this host. This is a workload-specific observation, not a
universal speed guarantee. The latest N-API dependency refresh was compiled
after these selection measurements; the consuming application also validates
the final installed native binary during its complete build.

Every candidate and the petgraph control returned exactly the same complete
128-bin histogram and `maxDistance: 14`. SHA-256 of the native result object,
serialized with `histogram` before `maxDistance`, is
`726dceb63c25d5f190e0a259bc201eb18905087e45072a498a4b6a98402e7680`.
Counts are exact ordered source/target pairs, as in the existing API.

The weighted-push thread sweep measured medians of 2,342 / 1,047 / 729 / 675 ms
for 1 / 4 / 8 / 16 Rayon threads respectively. That sweep preceded pull and
POPCNT selection; it does not claim to tune those later kernels independently.
The default Rayon thread selection remains configurable with
`RAYON_NUM_THREADS`.

A ten-call weighted-push process reached Linux `VmHWM` 66,604 KiB and the
enclosing `/usr/bin/time -v` measured 66,132 KiB, with no per-call growth trend.
Pull/POPCNT selection processes were around 65,000-66,000 KiB by Linux `VmHWM`. Some
early raw files contain an inherited `getrusage().maxRSS` of 773,600 KiB from a
shell `exec`; that field was rejected as an addon memory measurement. Later
reports include `/proc/self/status` `VmHWM`, and OS-level timing verified it.

Correctness checks cover 17 JavaScript tests: independent reference BFS on
directed sparse/dense/skewed graphs, duplicate edges, loops, disconnected and
isolated vertices; 63/64/65 and 127/128/129 lane boundaries; distance 320 through
both the independent and multi-source kernels; weighted undirected twins and
internal arcs; and unused CSR trailing storage. Strict all-target Clippy and
Rust formatting also pass. The six-platform CI and Trusted Publishing flow
remain required before publishing; local testing covers Linux x64 GNU.

`scripts/cross-repo-benchmark.ts` measures a native addon against an exported
fixture. `BFS_BENCH_REPS` controls measured samples, `BFS_BENCH_WARMUPS` controls
discarded warmup calls. The isolated crate control's complete source, manifest,
and lockfile are in `crate-control/`; its dependencies are not production
dependencies of the npm addon. Build it with:

```sh
CARGO_BUILD_JOBS=2 cargo build --release --manifest-path crate-control/Cargo.toml
RAYON_NUM_THREADS=16 crate-control/target/release/bfs-crate-control /path/to/fixture.json
```

See [algorithm details and primary references](../../scripts/PERFORMANCE.md).
