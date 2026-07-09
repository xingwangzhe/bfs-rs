[English](./README.md) | [中文](./README_CN.md)

# @xingwangzhe/bfs-rs

基于 Rust + Rayon 并行加速的大规模图 BFS，使用 CSR 压缩邻接表格式。

- **16 核**并行 `bfsAllHistogram`：57K 节点全量约 **~3s**
- **单核**自动降级为串行，零 Rayon 调度开销
- **直方图 API**：不返回完整距离数组，每源节点仅 ~40 字节

## 安装

```bash
npm install @xingwangzhe/bfs-rs
```

## 数据格式

使用 **CSR（Compressed Sparse Row）**：

```
adj     = [1, 2, 0, 2, 0, 1, 3, 2]   // 所有邻居 ID 平铺
offsets = [0, 2, 4, 7, 8]            // 每节点起止偏移（长度 n+1）
```

| 节点 | 邻居                  |
|------|----------------------|
| 0    | adj[0..2] = [1, 2]    |
| 1    | adj[2..4] = [0, 2]    |
| 2    | adj[4..7] = [0, 1, 3] |
| 3    | adj[7..8] = [2]       |

## API

### 完整距离 API — 需要每个节点的距离值

#### bfsOne(adj, offsets, n, source)

单源 BFS，返回距离数组。

```ts
import { bfsOne } from '@xingwangzhe/bfs-rs';
const r = bfsOne(adj, offsets, n, 0);
// r.distances → [0, 1, 1, 2]
// r.maxDistance → 2
// r.histogram → [2, 1]
```

#### bfsBatch(adj, offsets, n, sources)

多源并行 BFS。

```ts
import { bfsBatch } from '@xingwangzhe/bfs-rs';
const r = bfsBatch(adj, offsets, n, [0, 3]);
// r.processed → 2, r.results → [BfsOneResult, BfsOneResult]
```

#### bfsAll(adj, offsets, n)

全源 BFS（每个节点作为源）。

```ts
import { bfsAll } from '@xingwangzhe/bfs-rs';
const r = bfsAll(adj, offsets, n);
// r.results.length === n
```

#### bfsPath(adj, offsets, n, source, target)

两节点最短路径，找到目标立即终止。

```ts
import { bfsPath } from '@xingwangzhe/bfs-rs';
const r = bfsPath(adj, offsets, n, 0, 3);
// r.path → [0, 2, 3], r.distance → 2
```

### 直方图 API — 大图内存友好

只返回每源节点的**距离直方图**（不含完整距离数组），适用于六度分隔统计等仅需距离分布的 50K+ 节点大图场景。

#### bfsOneHistogram / bfsBatchHistogram / bfsAllHistogram

用法同上，返回类型为 `BfsHistogramResult`：

```ts
import { bfsAllHistogram } from '@xingwangzhe/bfs-rs';
const r = bfsAllHistogram(adj, offsets, n);
// r.results[i].histogram → [距离1的节点数, 距离2的节点数, ...]
// r.results[i].maxDistance → 最大距离
```

内存：每源节点约 (直径 × 4) 字节，而非 (n × 4) 字节。

## 性能

| 平台       | 57K 节点 × 179K 边 | 说明                       |
|------------|-------------------|---------------------------|
| 16 核       | **~3s**           | Rayon `par_iter` 16 线程并行 |
| 1 核        | ~70s               | `rayon::current_num_threads() < 2` 自动降级串行 |

所有 BFS 函数内部使用**双 Vec 交换**层级遍历，零每层分配开销。

## 完整示例

```ts
import { bfsOne, bfsBatch, bfsAll, bfsPath, bfsAllHistogram } from '@xingwangzhe/bfs-rs';

// 图结构: 0--1--2, 0--3--4--2
const adj     = [1, 3, 0, 2, 1, 4, 0, 4, 2, 3];
const offsets = [0, 2, 4, 6, 8, 10];
const n       = 5;

// 完整距离
const r1 = bfsOne(adj, offsets, n, 0);
console.log(r1.distances); // [0, 1, 2, 1, 2]

// 最短路径
const r2 = bfsPath(adj, offsets, n, 0, 4);
console.log(r2.path); // [0, 1, 2, 4] 或 [0, 3, 4, 2]

// 直方图模式（内存友好）
const r3 = bfsAllHistogram(adj, offsets, n);
// JS 侧聚合距离分布：
const degreeDist = {};
for (const h of r3.results) {
  for (let d = 0; d < h.histogram.length; d++) {
    degreeDist[d + 1] = (degreeDist[d + 1] || 0) + h.histogram[d];
  }
}
// degreeDist[1] 除以 2 即为无向图节点对数量
```

## 协议

MIT
