[English](./README.md) | [中文](./README_CN.md)

# @xingwangzhe/bfs-rs

High-performance BFS (Breadth-First Search) library powered by Rust + Rayon parallelism, using CSR (Compressed Sparse Row) format for large-scale graph shortest-path computation.

## Installation

```bash
npm install @xingwangzhe/bfs-rs
# or
yarn add @xingwangzhe/bfs-rs
```

## Data Format

This library uses the **CSR (Compressed Sparse Row)** format to represent graphs:

```
adj     = [1, 2, 0, 2, 0, 1, 3, 2]   // all neighbor node IDs flattened
offsets = [0, 2, 4, 7, 8]            // start offset for each node (length = n + 1)
```

The data above represents:

| Node | Neighbors |
|------|-----------|
| 0    | 1, 2      |
| 1    | 0, 2      |
| 2    | 0, 1, 3   |
| 3    | 2         |

Elements in `adj` from `offsets[i]` to `offsets[i+1]` are the neighbors of node `i`.

## API

### bfsOne(adj, offsets, n, source)

Run BFS from a **single source node**.

```ts
import { bfsOne } from '@xingwangzhe/bfs-rs';

const adj     = [1, 2, 0, 2, 0, 1, 3, 2];
const offsets = [0, 2, 4, 7, 8];
const n       = 4;  // total nodes

const result = bfsOne(adj, offsets, n, 0);
// result.distances  → [0, 1, 1, 2]   // shortest distances from node 0
// result.maxDistance → 2               // maximum finite distance
// result.histogram   → [2, 1]          // 2 nodes at dist 1, 1 node at dist 2
```

**Parameters:**
| Param   | Type     | Description                   |
|---------|----------|-------------------------------|
| adj     | number[] | Flattened adjacency array     |
| offsets | number[] | Offset array, length = n + 1  |
| n       | number   | Total number of nodes         |
| source  | number   | Source node ID (0-based)      |

**Returns:** `BfsOneResult`
| Field       | Type     | Description                              |
|-------------|----------|------------------------------------------|
| distances   | number[] | Shortest distance to each node, -1 = unreachable |
| maxDistance | number   | Maximum distance among reachable nodes   |
| histogram   | number[] | histogram[d-1] = nodes at distance d     |

---

### bfsBatch(adj, offsets, n, sources)

Run BFS from **multiple source nodes** in parallel (Rayon multi-threaded).

```ts
import { bfsBatch } from '@xingwangzhe/bfs-rs';

const sources = [0, 3];
const result = bfsBatch(adj, offsets, n, sources);

result.processed;  // 2 (number of sources processed)
result.results;    // [BfsOneResult, BfsOneResult]
```

**Parameters:**
| Param   | Type     | Description                  |
|---------|----------|------------------------------|
| adj     | number[] | Flattened adjacency array    |
| offsets | number[] | Offset array, length = n + 1 |
| n       | number   | Total number of nodes        |
| sources | number[] | Source node IDs              |

**Returns:** `BfsBatchResult`
| Field     | Type           | Description                    |
|-----------|----------------|--------------------------------|
| results   | BfsOneResult[] | BFS result for each source     |
| processed | number         | Number of sources successfully processed |

---

### bfsAll(adj, offsets, n)

Run BFS from **all nodes** in parallel (equivalent to `bfsBatch` with all nodes as sources).

```ts
import { bfsAll } from '@xingwangzhe/bfs-rs';

const result = bfsAll(adj, offsets, n);
// computes all-pairs shortest paths, n nodes processed in parallel
```

**Returns:** `BfsBatchResult`, where `results.length === n`.

---

## Full Example

```ts
import { bfsOne, bfsBatch, bfsAll } from '@xingwangzhe/bfs-rs';

// Graph with 5 nodes (undirected)
// 0 -- 1 -- 2
// |         |
// 3 ------- 4
//
// CSR format:
const adj     = [1, 3, 0, 2, 1, 4, 0, 4, 2, 3];
const offsets = [0, 2, 4, 6, 8, 10];
const n       = 5;

// 1. Single-source BFS
const r1 = bfsOne(adj, offsets, n, 0);
console.log('From node 0:', r1.distances);
// [0, 1, 2, 1, 2]

// 2. Batch BFS
const r2 = bfsBatch(adj, offsets, n, [0, 4]);
console.log('Batch count:', r2.processed);  // 2
r2.results.forEach((res, i) => {
  console.log(`Source ${[0, 4][i]}:`, res.distances);
});

// 3. All-pairs BFS
const r3 = bfsAll(adj, offsets, n);
console.log('All-pairs count:', r3.processed);  // 5
```

## Performance

- Written in **Rust**, compiled to native machine code
- BFS queue powered by [`parallel_frontier`](https://crates.io/crates/parallel_frontier) — lock-free, cache-line-padded frontier
- Batch/all-pairs BFS uses **Rayon** for parallel scheduling across CPU cores
- Memory-efficient CSR format optimized for large graphs

## Credits

This package is built on top of [`parallel_frontier`](https://crates.io/crates/parallel_frontier) (Apache-2.0 OR LGPL-2.1-or-later), a high-performance concurrent queue designed for parallel BFS traversals.

## Development

### Requirements

- Node.js >= 18
- Rust stable toolchain
- Yarn

### Local Build

```bash
yarn install
yarn build
yarn test
```

## License

This project is dual-licensed under [MIT](https://opensource.org/licenses/MIT) OR [Apache-2.0](https://www.apache.org/licenses/LICENSE-2.0), at your option.

Dependency [`parallel_frontier`](https://crates.io/crates/parallel_frontier) is used under the Apache-2.0 option of its license.
