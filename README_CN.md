[English](./README.md) | [中文](./README_CN.md)

# @xingwangzhe/bfs-rs

基于 Rust + Rayon 并行计算的高性能 BFS（广度优先搜索）库，使用 CSR（Compressed Sparse Row）压缩邻接表格式，适用于大规模图的最短路径计算。

## 安装

```bash
npm install @xingwangzhe/bfs-rs
# 或
yarn add @xingwangzhe/bfs-rs
```

## 数据格式

本库使用 **CSR（Compressed Sparse Row）** 格式表示图：

```
adj     = [1, 2, 0, 2, 0, 1, 3, 2]   // 所有邻居节点 ID 平铺
offsets = [0, 2, 4, 7, 8]            // 每个节点的邻接起始位置（长度 = n + 1）
```

以上数据表示的图结构：

| 节点 | 邻居 |
|------|------|
| 0    | 1, 2 |
| 1    | 0, 2 |
| 2    | 0, 1, 3 |
| 3    | 2    |

`offsets[i]` 到 `offsets[i+1]` 之间的 `adj` 元素即为节点 `i` 的所有邻居。

## API

### bfsOne(adj, offsets, n, source)

从**单个源节点**执行 BFS。

```ts
import { bfsOne } from '@xingwangzhe/bfs-rs';

const adj     = [1, 2, 0, 2, 0, 1, 3, 2];
const offsets = [0, 2, 4, 7, 8];
const n       = 4;  // 总节点数

const result = bfsOne(adj, offsets, n, 0);
// result.distances  → [0, 1, 1, 2]   // 节点 0 到各节点的最短距离
// result.maxDistance → 2               // 最大距离
```

**参数：**
| 参数    | 类型     | 说明                       |
|---------|---------|----------------------------|
| adj     | number[] | 平铺的邻接表数组             |
| offsets | number[] | 偏移数组，长度为 n + 1       |
| n       | number   | 总节点数                    |
| source  | number   | 源节点 ID（从 0 开始）       |

**返回值：** `BfsOneResult`
| 字段         | 类型     | 说明                          |
|-------------|---------|-------------------------------|
| distances   | number[] | 各节点到源的最短距离，-1 表示不可达 |
| maxDistance | number   | 所有可达节点中的最大距离          |

---

### bfsBatch(adj, offsets, n, sources)

从**多个源节点**并行执行 BFS（Rayon 多线程加速）。

```ts
import { bfsBatch } from '@xingwangzhe/bfs-rs';

const sources = [0, 3];
const result = bfsBatch(adj, offsets, n, sources);

result.processed;  // 2（成功处理的源节点数）
result.results;    // [BfsOneResult, BfsOneResult]
```

**参数：**
| 参数    | 类型     | 说明                  |
|---------|---------|----------------------|
| adj     | number[] | 平铺的邻接表数组        |
| offsets | number[] | 偏移数组，长度为 n + 1  |
| n       | number   | 总节点数               |
| sources | number[] | 源节点 ID 数组         |

**返回值：** `BfsBatchResult`
| 字段      | 类型           | 说明                |
|----------|---------------|---------------------|
| results  | BfsOneResult[] | 每个源节点的 BFS 结果  |
| processed| number         | 成功处理的源节点数      |

---

### bfsAll(adj, offsets, n)

从**所有节点**并行执行 BFS（等价于 `bfsBatch` 以所有节点为源）。

```ts
import { bfsAll } from '@xingwangzhe/bfs-rs';

const result = bfsAll(adj, offsets, n);
// 计算全源最短路径，并行处理所有 n 个节点
```

**返回值：** `BfsBatchResult`，其中 `results` 长度等于 `n`。

---

## 完整示例

```ts
import { bfsOne, bfsBatch, bfsAll } from '@xingwangzhe/bfs-rs';

// 构建图：5 个节点的无向图
// 0 -- 1 -- 2
// |         |
// 3 ------- 4
//
// CSR 格式：
const adj     = [1, 3, 0, 2, 1, 4, 0, 4, 2, 3];
const offsets = [0, 2, 4, 6, 8, 10];
const n       = 5;

// 1. 单源 BFS
const r1 = bfsOne(adj, offsets, n, 0);
console.log('从节点 0 出发:', r1.distances);
// [0, 1, 2, 1, 2]

// 2. 批量 BFS
const r2 = bfsBatch(adj, offsets, n, [0, 4]);
console.log('批量结果数:', r2.processed);  // 2
r2.results.forEach((res, i) => {
  console.log(`源节点 ${[0, 4][i]}:`, res.distances);
});

// 3. 全源 BFS
const r3 = bfsAll(adj, offsets, n);
console.log('全源结果数:', r3.processed);  // 5
```

## 性能

- 底层使用 **Rust** 编写，编译为原生机器码
- BFS 队列基于 [`parallel_frontier`](https://crates.io/crates/parallel_frontier) — 无锁、Cache-Line 对齐的前沿队列
- 批量/全源 BFS 使用 **Rayon** 并行调度，充分利用多核 CPU
- 内存布局紧凑，CSR 格式对大图友好

## 致谢

本包基于 [`parallel_frontier`](https://crates.io/crates/parallel_frontier)（Apache-2.0 OR LGPL-2.1-or-later）构建，这是一个专为并行 BFS 遍历设计的高性能并发队列。

## 开发

### 环境要求

- Node.js >= 18
- Rust 最新稳定版
- Yarn

### 本地构建

```bash
yarn install
yarn build
yarn test
```

## 协议

本项目采用双协议授权：[MIT](https://opensource.org/licenses/MIT) 或 [Apache-2.0](https://www.apache.org/licenses/LICENSE-2.0)，任选其一。

依赖 [`parallel_frontier`](https://crates.io/crates/parallel_frontier) 以其 Apache-2.0 选项使用。
