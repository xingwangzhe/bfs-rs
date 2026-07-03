#![deny(clippy::all)]

use napi_derive::napi;
use parallel_frontier::Frontier;
use rayon::prelude::*;
use std::sync::LazyLock;

/// 单线程池，用于单个 BFS 内使用 parallel_frontier 的 Frontier 队列
static BFS_POOL: LazyLock<rayon::ThreadPool> = LazyLock::new(|| {
    rayon::ThreadPoolBuilder::new()
        .num_threads(1)
        .build()
        .expect("failed to build BFS thread pool")
});

/// 单源 BFS 结果
#[napi(object)]
pub struct BfsOneResult {
    /// 源节点到每个节点的最短距离（-1 表示不可达）
    pub distances: Vec<i32>,
    /// 最大有限距离
    pub max_distance: u32,
}

/// 批量 BFS 结果
#[napi(object)]
pub struct BfsBatchResult {
    /// 每个源节点的 BFS 结果
    pub results: Vec<BfsOneResult>,
    /// 成功处理的源节点数
    pub processed: u32,
}

/// 内部单源 BFS，基于 parallel_frontier::Frontier 实现层级遍历
fn bfs_one_internal(adj: &[u32], offsets: &[u32], source: u32, n: usize) -> BfsOneResult {
    let mut dist = vec![-1i32; n];

    BFS_POOL.install(|| {
        let mut curr = Frontier::with_threads(&BFS_POOL, Some(n));
        let mut next = Frontier::with_threads(&BFS_POOL, Some(n));

        dist[source as usize] = 0;
        curr.push(source);

        let mut max_dist = 0u32;
        let mut current_dist = 1i32;

        while !curr.is_empty() {
            for &u in curr.iter() {
                let start = offsets[u as usize] as usize;
                let end = offsets[(u + 1) as usize] as usize;
                for &v in &adj[start..end] {
                    let vi = v as usize;
                    if dist[vi] == -1 {
                        dist[vi] = current_dist;
                        max_dist = current_dist as u32;
                        next.push(v);
                    }
                }
            }
            std::mem::swap(&mut curr, &mut next);
            next.clear();
            current_dist += 1;
        }

        BfsOneResult {
            distances: dist.clone(),
            max_distance: max_dist,
        }
    })
}

/// 从单个源节点执行 BFS，返回距离数组
#[napi]
pub fn bfs_one(adj: Vec<u32>, offsets: Vec<u32>, n: u32, source: u32) -> BfsOneResult {
    bfs_one_internal(&adj, &offsets, source, n as usize)
}

/// 从多个源节点并行执行 BFS
///
/// # 参数
/// * `adj` - 邻接表，所有邻居节点 ID 平铺
/// * `offsets` - 每个节点在 adj 中的起始偏移，长度为 n + 1，最后一项为 adj 的长度
/// * `n` - 总节点数
/// * `sources` - 需要执行 BFS 的源节点 ID 列表
#[napi]
pub fn bfs_batch(adj: Vec<u32>, offsets: Vec<u32>, n: u32, sources: Vec<u32>) -> BfsBatchResult {
    let n_usize = n as usize;
    let total = sources.len();

    let results: Vec<BfsOneResult> = sources
        .par_iter()
        .map(|&src| bfs_one_internal(&adj, &offsets, src, n_usize))
        .collect();

    BfsBatchResult {
        processed: total as u32,
        results,
    }
}

/// 从所有节点并行执行 BFS（全量）
#[napi]
pub fn bfs_all(adj: Vec<u32>, offsets: Vec<u32>, n: u32) -> BfsBatchResult {
    let sources: Vec<u32> = (0..n).collect();
    bfs_batch(adj, offsets, n, sources)
}
