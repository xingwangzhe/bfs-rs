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
    /// 距离直方图：histogram[d] = 距离为 d 的节点数（不含源节点自身）
    pub histogram: Vec<u32>,
}

/// 批量 BFS 结果
#[napi(object)]
pub struct BfsBatchResult {
    /// 每个源节点的 BFS 结果
    pub results: Vec<BfsOneResult>,
    /// 成功处理的源节点数
    pub processed: u32,
}

/// 仅直方图结果（不含完整距离数组，节省内存）
#[napi(object)]
pub struct BfsHistogramResult {
    /// 距离直方图：histogram[d] = 距离为 d 的节点数（不含源节点自身）
    pub histogram: Vec<u32>,
    /// 最大有限距离
    pub max_distance: u32,
}

/// 批量直方图结果
#[napi(object)]
pub struct BfsHistogramBatchResult {
    /// 每个源节点的直方图结果
    pub results: Vec<BfsHistogramResult>,
    /// 成功处理的源节点数
    pub processed: u32,
}

/// 内部单源 BFS，仅返回直方图（不分配完整距离数组的副本）
fn bfs_one_histogram_internal(
    adj: &[u32],
    offsets: &[u32],
    source: u32,
    n: usize,
) -> BfsHistogramResult {
    let mut dist = vec![-1i32; n];

    BFS_POOL.install(|| {
        let mut curr = Frontier::with_threads(&BFS_POOL, Some(n));
        let mut next = Frontier::with_threads(&BFS_POOL, Some(n));

        dist[source as usize] = 0;
        curr.push(source);

        let mut max_dist = 0u32;
        let mut current_dist = 1i32;
        let mut histogram: Vec<u32> = Vec::new();

        while !curr.is_empty() {
            let mut level_count = 0u32;
            for &u in curr.iter() {
                let start = offsets[u as usize] as usize;
                let end = offsets[(u + 1) as usize] as usize;
                for &v in &adj[start..end] {
                    let vi = v as usize;
                    if dist[vi] == -1 {
                        dist[vi] = current_dist;
                        max_dist = current_dist as u32;
                        level_count += 1;
                        next.push(v);
                    }
                }
            }
            if level_count > 0 {
                histogram.push(level_count);
            }
            std::mem::swap(&mut curr, &mut next);
            next.clear();
            current_dist += 1;
        }

        // dist 在此作用域结束后自动释放，不 clone
        BfsHistogramResult {
            max_distance: max_dist,
            histogram,
        }
    })
}

/// 从单个源节点执行 BFS，仅返回距离直方图（节省内存）
#[napi]
pub fn bfs_one_histogram(
    adj: Vec<u32>,
    offsets: Vec<u32>,
    n: u32,
    source: u32,
) -> BfsHistogramResult {
    bfs_one_histogram_internal(&adj, &offsets, source, n as usize)
}

/// 从多个源节点并行执行 BFS，仅返回距离直方图（节省内存）
#[napi]
pub fn bfs_batch_histogram(
    adj: Vec<u32>,
    offsets: Vec<u32>,
    n: u32,
    sources: Vec<u32>,
) -> BfsHistogramBatchResult {
    let n_usize = n as usize;
    let total = sources.len();

    let results: Vec<BfsHistogramResult> = sources
        .par_iter()
        .map(|&src| bfs_one_histogram_internal(&adj, &offsets, src, n_usize))
        .collect();

    BfsHistogramBatchResult {
        processed: total as u32,
        results,
    }
}

/// 从所有节点并行执行 BFS（全量），仅返回直方图
#[napi]
pub fn bfs_all_histogram(adj: Vec<u32>, offsets: Vec<u32>, n: u32) -> BfsHistogramBatchResult {
    let sources: Vec<u32> = (0..n).collect();
    bfs_batch_histogram(adj, offsets, n, sources)
}
#[napi(object)]
pub struct BfsPathResult {
    /// 从 source 到 target 的最短路径节点序列（含两端），不可达时为空数组
    pub path: Vec<u32>,
    /// 路径长度（边数），不可达时为 -1
    pub distance: i32,
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
        let mut histogram: Vec<u32> = Vec::new(); // histogram[d] 在循环中动态增长

        while !curr.is_empty() {
            let mut level_count = 0u32;
            for &u in curr.iter() {
                let start = offsets[u as usize] as usize;
                let end = offsets[(u + 1) as usize] as usize;
                for &v in &adj[start..end] {
                    let vi = v as usize;
                    if dist[vi] == -1 {
                        dist[vi] = current_dist;
                        max_dist = current_dist as u32;
                        level_count += 1;
                        next.push(v);
                    }
                }
            }
            if level_count > 0 {
                histogram.push(level_count);
            }
            std::mem::swap(&mut curr, &mut next);
            next.clear();
            current_dist += 1;
        }

        BfsOneResult {
            distances: dist.clone(),
            max_distance: max_dist,
            histogram,
        }
    })
}

/// 计算从 source 到 target 的最短路径（BFS + 父节点回溯）
///
/// 找到 target 时立即终止，不遍历全图。
/// 不可达时 path 为空数组，distance 为 -1。
#[napi]
pub fn bfs_path(
    adj: Vec<u32>,
    offsets: Vec<u32>,
    n: u32,
    source: u32,
    target: u32,
) -> BfsPathResult {
    let n_usize = n as usize;
    let src = source as usize;
    let tgt = target as usize;

    if src == tgt {
        return BfsPathResult {
            path: vec![source],
            distance: 0,
        };
    }

    let mut parent = vec![-1i32; n_usize];
    let mut found = false;

    BFS_POOL.install(|| {
        let mut curr = Frontier::with_threads(&BFS_POOL, Some(n_usize));
        let mut next = Frontier::with_threads(&BFS_POOL, Some(n_usize));

        parent[src] = src as i32; // 标记源节点已访问
        curr.push(source);

        'outer: while !curr.is_empty() {
            for &u in curr.iter() {
                let start = offsets[u as usize] as usize;
                let end = offsets[(u + 1) as usize] as usize;
                for &v in &adj[start..end] {
                    let vi = v as usize;
                    if parent[vi] == -1 {
                        parent[vi] = u as i32;
                        if vi == tgt {
                            found = true;
                            break 'outer;
                        }
                        next.push(v);
                    }
                }
            }
            std::mem::swap(&mut curr, &mut next);
            next.clear();
        }
    });

    if !found {
        return BfsPathResult {
            path: vec![],
            distance: -1,
        };
    }

    // 回溯构建路径
    let mut path: Vec<u32> = Vec::new();
    let mut cur = tgt as i32;
    while cur != src as i32 {
        path.push(cur as u32);
        cur = parent[cur as usize];
    }
    path.push(source);
    path.reverse();

    BfsPathResult {
        distance: (path.len() - 1) as i32,
        path,
    }
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
