# 工程原理 / Engineering Notes

> 2026-07-03 · @xingwangzhe/bfs-rs v0.1.2

## 架构概览

```
@xingwangzhe/bfs-rs (主包, ~10KB, 仅 JS)
  └── optionalDependencies (npm 自动按 os/cpu 选择安装)
        ├── @xingwangzhe/bfs-rs-linux-x64-gnu
        ├── @xingwangzhe/bfs-rs-linux-x64-musl
        ├── @xingwangzhe/bfs-rs-linux-arm64-gnu
        ├── @xingwangzhe/bfs-rs-darwin-x64
        ├── @xingwangzhe/bfs-rs-darwin-arm64
        └── @xingwangzhe/bfs-rs-win32-x64-msvc
```

## 技术栈

| 层 | 选型 | 理由 |
|----|------|------|
| BFS 算法 | [parallel_frontier](https://crates.io/crates/parallel_frontier) crate | 无锁并发队列，Cache-Line 对齐，专为并行 BFS 设计 |
| 并行调度 | Rayon | par_iter 按源节点并行，批处理多源 BFS |
| 图格式 | CSR (Compressed Sparse Row) | `adj[]` + `offsets[]`，内存紧凑，缓存友好 |
| FFI 框架 | napi-rs v3 | Rust → Node.js 原生扩展，平台子包自动分发 |
| 包管理器 | Bun | 比 yarn/npm 更快，CI 全流程 bun |
| CI/CD | GitHub Actions + OIDC | Trusted Publisher 免 Token 发布 |

## 发布机制

1. **commit message 匹配纯版本号**（如 `0.1.2`）→ 触发 publish
2. CI 先发布 6 个平台子包（`npm/*/`），再发布主包
3. 认证方式：OIDC Trusted Publisher（`id-token: write`），无需 `NPM_TOKEN`
4. 主包不含 `.node` 文件，`index.js` 运行时按平台 fallback 加载子包

## 支持的平台（6 个）

| Target | 构建环境 |
|--------|---------|
| `x86_64-apple-darwin` | macOS runner |
| `aarch64-apple-darwin` | macOS runner |
| `x86_64-pc-windows-msvc` | Windows runner |
| `x86_64-unknown-linux-gnu` | Ubuntu + napi-cross |
| `x86_64-unknown-linux-musl` | Ubuntu + zigbuild |
| `aarch64-unknown-linux-gnu` | Ubuntu + napi-cross |

> 从 napi-rs 支持的 30+ 平台精简到覆盖 99% 用户的 6 个桌面/服务器平台。

## API

```ts
bfsOne(adj, offsets, n, source)  → BfsOneResult   // 单源 BFS
bfsBatch(adj, offsets, n, sources) → BfsBatchResult // 多源并行 BFS
bfsAll(adj, offsets, n)           → BfsBatchResult  // 全源并行 BFS
```

输入为 CSR 格式：`adj` 平铺邻接表 + `offsets` 偏移数组（长度 n+1）。

## 协议

MIT OR Apache-2.0 双协议。依赖 `parallel_frontier` 按其 Apache-2.0 选项使用。

## 关键决策记录

1. **用 parallel_frontier 而非手写 BFS** → 减少维护成本，借用社区维护的高性能并发队列
2. **平台子包而非单包** → 避免用户下载 6×500KB 的全平台二进制，按需安装 ~500KB
3. **OIDC 而非 NPM_TOKEN** → 无需管理 secret，仓库级权限控制
4. **砍掉 Android/WASI/FreeBSD/i686/ARMv7** → 覆盖不足 1% 的场景不值得维护成本和 CI 耗时
5. **`"type": "module"` 必须去掉** → napi-rs 生成的 index.js 是 CJS，ESM 模式下 `module.exports` 失效
