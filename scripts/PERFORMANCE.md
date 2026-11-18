# Runtime-focused native builds

Release builds use `opt-level = 3`, full (`fat`) LTO, one code-generation unit, and disabled incremental compilation. The CPU instruction baseline and panic-unwind behavior remain compatible with the existing package.

`bun run build:pgo` additionally builds an instrumented addon, executes deterministic representative workloads, merges its raw counters with the toolchain-matched `llvm-profdata`, then builds the final addon with `-Cprofile-use`. Install the tools with `rustup component add llvm-tools-preview` first. Training runs on a matching native host; Linux x64 musl training uses Node 24 in an Alpine container. For the font package GNU builds, use `--zig-glibc 2.28` to preserve the matching C/C++ sysroot and glibc baseline.

PGO applies to the Rust dependency graph. It does not apply to the font package's separately compiled C/C++ libraries. The release retains full LTO across Rust crates; no cross-language LTO or fast-math assumptions are introduced.

Use `bun run benchmark:pgo` (Node 24+, Bun, Rust, LLVM tools) to build an unprofiled baseline and the PGO candidate and compare them. Workload inputs differ between training and measurement. Reports use four Rayon threads, an ABBAAB process order, and the median of 21 warm samples per workload, with each sample calibrated to roughly 30 ms. They describe these synthetic workloads on the current machine, not a universal speedup. Profiles and binary sizes are not committed.

Build records and runtime reports are saved under `target/pgo/<target>/`. CI uploads the build record for each platform and tests the final binary before publishing. To train locally for an explicit target, add `--target <triple>` on a matching host. Do not use `target-cpu=native` for distributed npm binaries: that changes CPU compatibility to the build machine.

The `scripts/pgo-workload.ts` corpus should be updated alongside APIs or representative workloads; PGO is retrained from source for every release, using the same compiler for profile generation and use. Normal `bun run build` remains an unprofiled release build for development and comparison.

CI uses `bun run build:pgo --select`: it also builds the unprofiled release and measures both variants on each target. After two discarded warmup processes, it selects PGO only when the balanced workload geometric mean is at least 3% faster and no non-trivial workload (baseline at least 20 microseconds) regresses by more than 15%. Otherwise it publishes the unprofiled binary. This conservative sample-based selection is not a proof about all workloads; the per-target `build.json` records the selected variant and the comparison is uploaded as `benchmark.json`. The instrumented binary is never selected for publication.

Linux Zig builds use a small compiler-driver shim to forward rustc's PGO `-u SYMBOL` linker arguments as `-Wl,-u,SYMBOL`. All other arguments are preserved. This does not replace the Zig sysroot or alter the glibc baseline. The shim requires Python 3 (available on the CI Linux runners).

## Exact merged histograms

`mergedHistogram()` uses 64 independent source lanes in each `u64`, following the
MS-BFS approach described by [Then et al., VLDB 2014](https://www.vldb.org/pvldb/vol8/p449-then.pdf).
Each vertex stores the sources that have seen it, the sources arriving at the
current distance, and the sources arriving at the next distance. A source bit is
counted only at its first arrival, so the result preserves directed shortest-path
distances. This is exact traversal, with no sampling or approximate distances.
Rayon parallelizes independent 64-source batches, avoiding shared visitation
atomics and a synchronization barrier for every graph level. The implementation
uses portable Rust integer operations and the existing generic target baseline.
On x86, a runtime CPU feature check selects a POPCNT-enabled copy of the kernel
when supported; older CPUs retain the generic integer implementation. ARM
targets use their normal generic compilation. This does not change the package's
CPU instruction baseline or require a host-specific build.

The multi-source kernel switches to pull when more than one eighth of the
vertices are active. Pull reads incoming frontier masks, stops scanning a row
once all missing source lanes have been found, and skips vertices already seen
by every source in the batch. Otherwise it pushes only active outgoing rows.

For undirected graphs, vertices with identical neighbor sets can be represented
by one weighted vertex. The implementation verifies every CSR row against its
transpose before applying this reduction. Distances between different classes
are unchanged. A discovered class contributes `sourceWeight * targetWeight`
ordered pairs. Distinct vertices inside one class have distance two when their
neighbor set is nonempty, distance one when it contains internal arcs, and remain
unreachable when isolated; these pairs are added separately. Sources of equal
weight share a batch, allowing a population count instead of a loop over each
source bit. The quotient is used only when it removes at least 10% of vertices.
The original graph remains intact for paths, individual distances, and per-source
histograms.

Graphs with fewer than 64 vertices or a maximum outgoing degree at most two use
independent BFSs. Long paths and cycles have little overlap between simultaneous
frontiers. The independent traversal builds a frontier bitmap only when it uses
bottom-up pull; sparse levels no longer clear whole bitmaps. This removes the
previous diameter-dependent bitmap-clearing work. Histogram reduction retains
the legacy minimum of 128 bins and extends beyond 128 when needed, instead of
discarding long-distance counts.

The PGO training and holdout corpus includes directed merged histograms and an
undirected core with many shared-parent leaves. The two datasets use different
sizes and seeds. Generic graph packages were checked before implementing this
specialized kernel:

| Candidate | License | Relevant API and decision |
| --- | --- | --- |
| [petgraph 0.8.3](https://docs.rs/petgraph/0.8.3/petgraph/visit/struct.Bfs.html) | MIT OR Apache-2.0 | Queue-based single-source traversal; no shared-source exact histogram kernel. |
| [graph 0.3.2](https://docs.rs/graph/0.3.2/graph/) | MIT | CSR and parallel graph algorithms; weighted SSSP requires a different interface and does not provide the required histogram reduction. |
| [parallel_frontier 0.3.2](https://docs.rs/parallel_frontier/0.3.2/parallel_frontier/) | Apache-2.0 OR LGPL-2.1-or-later | Shards one traversal across workers; its parallel visit example needs atomic visitation. Independent source batches already provide parallel work here. |

These are interface and algorithm comparisons, not measured claims that one
library is universally slower. No third-party implementation code was copied.
Unused `serde` and `serde_json` dependencies were removed from the runtime graph
engine rather than introducing a new graph representation dependency.

For a cross-repository fixture exported by the consuming application:

```sh
RAYON_NUM_THREADS=16 BFS_BENCH_REPS=3 bun scripts/cross-repo-benchmark.ts \
  ./bfs-rs.linux-x64-gnu.node /path/to/fixture.json /path/to/result.json
```

The fixture supplies `nodeCount`, `adjFlat`, and `offsets`. Reports record every
sample, the exact full histogram and SHA-256, construction time, thread count,
and process memory. On Linux prefer `linuxVmHwmKiB` or an enclosing
`/usr/bin/time -v`: `getrusage().maxRSS` can include a peak inherited across an
earlier `exec`, which can overstate the addon's actual peak in shell chains.
