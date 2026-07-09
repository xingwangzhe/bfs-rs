[English](./README.md) | [中文](./README_CN.md)

# @xingwangzhe/bfs-rs

Fast BFS (Breadth-First Search) for large-scale graphs, written in Rust with Rayon parallelism. Uses CSR (Compressed Sparse Row) adjacency format.

- **16-core** parallel `bfsAllHistogram`: 57K nodes in **~3s**
- **Single-core** auto-fallback: sequential path with zero Rayon overhead
- **Histogram-only** API: no full distance arrays, O(histogram) memory per source

## Installation

```bash
npm install @xingwangzhe/bfs-rs
```

## Data Format

Uses **CSR (Compressed Sparse Row)**:

```
adj     = [1, 2, 0, 2, 0, 1, 3, 2]   // all neighbor IDs flattened
offsets = [0, 2, 4, 7, 8]            // node offset range (length = n + 1)
```

| Node | Neighbors             |
|------|-----------------------|
| 0    | adj[0..2] = [1, 2]    |
| 1    | adj[2..4] = [0, 2]    |
| 2    | adj[4..7] = [0, 1, 3] |
| 3    | adj[7..8] = [2]       |

## API

### Full Distance API — when you need per-node distances

#### bfsOne(adj, offsets, n, source)

Single-source BFS, returns distances array.

```ts
import { bfsOne } from '@xingwangzhe/bfs-rs';
const r = bfsOne(adj, offsets, n, 0);
// r.distances → [0, 1, 1, 2]
// r.maxDistance → 2
// r.histogram → [2, 1]
```

#### bfsBatch(adj, offsets, n, sources)

Parallel BFS from multiple sources.

```ts
import { bfsBatch } from '@xingwangzhe/bfs-rs';
const r = bfsBatch(adj, offsets, n, [0, 3]);
// r.processed → 2, r.results → [BfsOneResult, BfsOneResult]
```

#### bfsAll(adj, offsets, n)

All-pairs BFS (every node as source).

```ts
import { bfsAll } from '@xingwangzhe/bfs-rs';
const r = bfsAll(adj, offsets, n);
// r.results.length === n
```

#### bfsPath(adj, offsets, n, source, target)

Shortest path between two nodes. Stops early at target.

```ts
import { bfsPath } from '@xingwangzhe/bfs-rs';
const r = bfsPath(adj, offsets, n, 0, 3);
// r.path → [0, 2, 3], r.distance → 2
```

### Histogram-Only API — memory-efficient for large graphs

These return **only the distance histogram** per source (no full `distances` array), making them ideal for six-degree / diameter stats on graphs with 50K+ nodes.

#### bfsOneHistogram / bfsBatchHistogram / bfsAllHistogram

Same usage as above, but result type is `BfsHistogramResult`:

```ts
import { bfsAllHistogram } from '@xingwangzhe/bfs-rs';
const r = bfsAllHistogram(adj, offsets, n);
// r.results[i].histogram → [count_at_dist_1, count_at_dist_2, ...]
// r.results[i].maxDistance → number
```

Memory per source: ~(diameter × 4) bytes instead of ~(n × 4) bytes.

## Performance

| Platform   | 57K nodes × 179K edges | Notes                     |
|------------|----------------------|---------------------------|
| 16-core    | **~3s**              | Rayon `par_iter` across 16 threads |
| 1-core     | ~70s                 | auto-fallback to `iter` |

All BFS functions use **dual-`Vec` swap** level traversal with zero allocation per level.

## Full Example

```ts
import { bfsOne, bfsBatch, bfsAll, bfsPath, bfsAllHistogram } from '@xingwangzhe/bfs-rs';

// Graph: 0--1--2, 0--3--4--2
const adj     = [1, 3, 0, 2, 1, 4, 0, 4, 2, 3];
const offsets = [0, 2, 4, 6, 8, 10];
const n       = 5;

// Full distances
const r1 = bfsOne(adj, offsets, n, 0);
console.log(r1.distances); // [0, 1, 2, 1, 2]

// Shortest path
const r2 = bfsPath(adj, offsets, n, 0, 4);
console.log(r2.path); // [0, 1, 2, 4] or [0, 3, 4, 2]

// Histogram-only (memory efficient)
const r3 = bfsAllHistogram(adj, offsets, n);
// Aggregate in JS:
const degreeDist = {};
for (const h of r3.results) {
  for (let d = 0; d < h.histogram.length; d++) {
    degreeDist[d + 1] = (degreeDist[d + 1] || 0) + h.histogram[d];
  }
}
// degreeDist[1] = divide by 2 for undirected pair count
```

## License

MIT
