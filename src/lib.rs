#![deny(clippy::all)]

use napi_derive::napi;
use rayon::prelude::*;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};

#[napi(object)]
pub struct BfsOneResult {
    pub distances: Vec<i32>,
    pub max_distance: u32,
    pub histogram: Vec<u32>,
}
#[napi(object)]
pub struct BfsBatchResult {
    pub results: Vec<BfsOneResult>,
    pub processed: u32,
}
#[napi(object)]
pub struct BfsPathResult {
    pub path: Vec<u32>,
    pub distance: i32,
}
#[napi(object)]
pub struct BfsHistogramResult {
    pub histogram: Vec<u32>,
    pub max_distance: u32,
}
#[napi(object)]
pub struct BfsHistogramBatchResult {
    pub results: Vec<BfsHistogramResult>,
    pub processed: u32,
}
#[napi(object)]
pub struct MergedHistogram {
    pub histogram: Vec<u32>,
    pub max_distance: u32,
}

thread_local! {
    static BUF_DIST: RefCell<Vec<i32>> = const { const { const { RefCell::new(Vec::new()) } } };
    static BUF_CURR: RefCell<Vec<u32>> = const { const { const { RefCell::new(Vec::new()) } } };
    static BUF_NEXT: RefCell<Vec<u32>> = const { const { const { RefCell::new(Vec::new()) } } };
}

fn bfs_internal(adj: &[u32], offsets: &[u32], source: u32, n: usize) -> BfsOneResult {
    let mut dist = vec![-1i32; n];
    let mut curr = Vec::with_capacity(n);
    let mut next = Vec::with_capacity(n);
    dist[source as usize] = 0;
    curr.push(source);
    let mut max_d = 0u32;
    let mut hist: Vec<u32> = Vec::new();
    while !curr.is_empty() {
        let mut cnt = 0u32;
        for &u in &curr {
            let nd = dist[u as usize] + 1;
            let s = offsets[u as usize] as usize;
            let e = offsets[(u + 1) as usize] as usize;
            for &v in &adj[s..e] {
                let vi = v as usize;
                if dist[vi] == -1 {
                    dist[vi] = nd;
                    if nd > max_d as i32 {
                        max_d = nd as u32;
                    }
                    cnt += 1;
                    next.push(v);
                }
            }
        }
        if cnt > 0 {
            hist.push(cnt);
        }
        std::mem::swap(&mut curr, &mut next);
        next.clear();
    }
    BfsOneResult {
        distances: dist,
        max_distance: max_d,
        histogram: hist,
    }
}

fn bfs_histogram_internal(
    adj: &[u32],
    offsets: &[u32],
    source: u32,
    n: usize,
) -> BfsHistogramResult {
    // Thread-local buffer reuse — massive savings on 57K allocations
    let mut dist = BUF_DIST.with(|c| c.take());
    if dist.len() < n {
        dist.resize(n, -1);
    } else {
        dist.fill(-1);
        dist[source as usize] = 0;
    }

    let mut curr = BUF_CURR.with(|c| c.take());
    let mut next = BUF_NEXT.with(|c| c.take());
    curr.clear();
    next.clear();
    dist[source as usize] = 0;
    curr.push(source);

    let mut max_d = 0u32;
    let mut hist: Vec<u32> = Vec::new();
    while !curr.is_empty() {
        let mut cnt = 0u32;
        for &u in &curr {
            let nd = dist[u as usize] + 1;
            let s = offsets[u as usize] as usize;
            let e = offsets[(u + 1) as usize] as usize;
            for &v in &adj[s..e] {
                let vi = v as usize;
                if dist[vi] == -1 {
                    dist[vi] = nd;
                    if nd > max_d as i32 {
                        max_d = nd as u32;
                    }
                    cnt += 1;
                    next.push(v);
                }
            }
        }
        if cnt > 0 {
            hist.push(cnt);
        }
        std::mem::swap(&mut curr, &mut next);
        next.clear();
    }

    let result = BfsHistogramResult {
        max_distance: max_d,
        histogram: hist,
    };
    // Return buffers
    BUF_DIST.with(|c| {
        let _ = c.replace(dist);
    });
    BUF_CURR.with(|c| {
        let _ = c.replace(curr);
    });
    BUF_NEXT.with(|c| {
        let _ = c.replace(next);
    });
    result
}

#[napi]
pub fn bfs_one(adj: Vec<u32>, offsets: Vec<u32>, n: u32, source: u32) -> BfsOneResult {
    bfs_internal(&adj, &offsets, source, n as usize)
}
#[napi]
pub fn bfs_batch(adj: Vec<u32>, offsets: Vec<u32>, n: u32, sources: Vec<u32>) -> BfsBatchResult {
    let n_u = n as usize;
    let total = sources.len();
    let r: Vec<BfsOneResult> = sources
        .par_iter()
        .map(|&s| bfs_internal(&adj, &offsets, s, n_u))
        .collect();
    BfsBatchResult {
        processed: total as u32,
        results: r,
    }
}
#[napi]
pub fn bfs_all(adj: Vec<u32>, offsets: Vec<u32>, n: u32) -> BfsBatchResult {
    bfs_batch(adj, offsets, n, (0..n).collect())
}

#[napi]
pub fn bfs_path(
    adj: Vec<u32>,
    offsets: Vec<u32>,
    n: u32,
    source: u32,
    target: u32,
) -> BfsPathResult {
    let nu = n as usize;
    let src = source as usize;
    let tgt = target as usize;
    if src == tgt {
        return BfsPathResult {
            path: vec![source],
            distance: 0,
        };
    }
    let mut parent = vec![-1i32; nu];
    let mut q = VecDeque::new();
    parent[src] = src as i32;
    q.push_back(source);
    'outer: while let Some(u) = q.pop_front() {
        let s = offsets[u as usize] as usize;
        let e = offsets[(u + 1) as usize] as usize;
        for &v in &adj[s..e] {
            let vi = v as usize;
            if parent[vi] == -1 {
                parent[vi] = u as i32;
                if vi == tgt {
                    break 'outer;
                }
                q.push_back(v);
            }
        }
    }
    if parent[tgt] == -1 {
        return BfsPathResult {
            path: vec![],
            distance: -1,
        };
    }
    let mut path = vec![];
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

#[napi]
pub fn bfs_one_histogram(
    adj: Vec<u32>,
    offsets: Vec<u32>,
    n: u32,
    source: u32,
) -> BfsHistogramResult {
    bfs_histogram_internal(&adj, &offsets, source, n as usize)
}
#[napi]
pub fn bfs_batch_histogram(
    adj: Vec<u32>,
    offsets: Vec<u32>,
    n: u32,
    sources: Vec<u32>,
) -> BfsHistogramBatchResult {
    let n_u = n as usize;
    let total = sources.len();
    let r: Vec<BfsHistogramResult> = if rayon::current_num_threads() < 2 {
        sources
            .iter()
            .map(|&s| bfs_histogram_internal(&adj, &offsets, s, n_u))
            .collect()
    } else {
        sources
            .par_iter()
            .map(|&s| bfs_histogram_internal(&adj, &offsets, s, n_u))
            .collect()
    };
    BfsHistogramBatchResult {
        processed: total as u32,
        results: r,
    }
}
#[napi]
pub fn bfs_all_histogram(adj: Vec<u32>, offsets: Vec<u32>, n: u32) -> BfsHistogramBatchResult {
    bfs_batch_histogram(adj, offsets, n, (0..n).collect())
}

#[napi]
pub fn bfs_merged_histogram(adj: Vec<u32>, offsets: Vec<u32>, n: u32) -> MergedHistogram {
    let n_u = n as usize;
    let sources: Vec<u32> = (0..n).collect();
    let bins: Vec<AtomicU64> = (0..128).map(|_| AtomicU64::new(0)).collect();
    let max_d = AtomicU64::new(0);

    let run = |&s: &u32| {
        let r = bfs_histogram_internal(&adj, &offsets, s, n_u);
        for (d, &c) in r.histogram.iter().enumerate() {
            if d < bins.len() {
                bins[d].fetch_add(c as u64, Ordering::Relaxed);
            }
        }
        max_d.fetch_max(r.max_distance as u64, Ordering::Relaxed);
    };

    if rayon::current_num_threads() < 2 {
        sources.iter().for_each(run);
    } else {
        sources.par_iter().for_each(run);
    }

    let h: Vec<u32> = bins
        .iter()
        .map(|b| b.load(Ordering::Relaxed) as u32)
        .collect();
    MergedHistogram {
        max_distance: max_d.load(Ordering::Relaxed) as u32,
        histogram: h,
    }
}
