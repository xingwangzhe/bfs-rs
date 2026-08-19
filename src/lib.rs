#![deny(clippy::all)]

use napi::bindgen_prelude::Uint32Array;
use napi::{Error, Result, Status};
use napi_derive::napi;
use rayon::prelude::*;
use std::collections::VecDeque;

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

struct Graph {
    adj: Vec<u32>,
    offsets: Vec<u32>,
    rev_adj: Vec<u32>,
    rev_offsets: Vec<u32>,
    n: usize,
}

impl Graph {
    fn new(adj: Vec<u32>, offsets: Vec<u32>, n: usize) -> Result<Self> {
        if offsets.len() != n.saturating_add(1) {
            return Err(Error::new(
                Status::InvalidArg,
                format!("offsets must contain n + 1 entries (expected {})", n + 1),
            ));
        }
        if offsets.first().copied().unwrap_or(0) != 0
            || offsets.windows(2).any(|w| w[0] > w[1])
            || offsets.last().copied().unwrap_or(0) as usize > adj.len()
        {
            return Err(Error::new(Status::InvalidArg, "invalid CSR offsets"));
        }
        if adj.iter().any(|&v| v as usize >= n) {
            return Err(Error::new(
                Status::InvalidArg,
                "adjacency contains an invalid vertex",
            ));
        }

        let mut rev_offsets = vec![0u32; n + 1];
        for &v in &adj {
            rev_offsets[v as usize + 1] += 1;
        }
        for i in 1..=n {
            rev_offsets[i] += rev_offsets[i - 1];
        }
        let mut rev_adj = vec![0u32; adj.len()];
        let mut cursor = rev_offsets[..n].to_vec();
        for u in 0..n {
            for &v in &adj[offsets[u] as usize..offsets[u + 1] as usize] {
                let at = cursor[v as usize] as usize;
                rev_adj[at] = u as u32;
                cursor[v as usize] += 1;
            }
        }

        Ok(Self {
            adj,
            offsets,
            rev_adj,
            rev_offsets,
            n,
        })
    }
}

#[derive(Default)]
struct Scratch {
    stamp: Vec<u32>,
    dist: Vec<i32>,
    frontier: Vec<u32>,
    next: Vec<u32>,
    frontier_bits: Vec<u64>,
    next_bits: Vec<u64>,
    epoch: u32,
}

impl Scratch {
    fn prepare(&mut self, n: usize) {
        if self.stamp.len() != n {
            self.stamp.resize(n, 0);
            self.dist.resize(n, -1);
        }
        let words = n.div_ceil(64);
        if self.frontier_bits.len() != words {
            self.frontier_bits.resize(words, 0);
            self.next_bits.resize(words, 0);
        }
        self.frontier.clear();
        self.next.clear();
        self.epoch = self.epoch.wrapping_add(1).max(1);
    }

    fn clear_bits(&mut self) {
        self.frontier_bits.fill(0);
        self.next_bits.fill(0);
    }
}

thread_local! {
    static SCRATCH: std::cell::RefCell<Scratch> = std::cell::RefCell::new(Scratch::default());
}

#[inline]
fn set_bit(bits: &mut [u64], v: u32) {
    bits[(v as usize) >> 6] |= 1u64 << (v & 63);
}

#[inline]
fn has_bit(bits: &[u64], v: u32) -> bool {
    (bits[(v as usize) >> 6] & (1u64 << (v & 63))) != 0
}

#[inline]
fn should_pull(frontier_edges: usize, frontier_len: usize, unvisited: usize, n: usize) -> bool {
    if frontier_len == 0 || unvisited == 0 {
        return false;
    }
    frontier_edges > unvisited.saturating_mul(15)
        || (frontier_len > n / 8 && frontier_edges > unvisited.saturating_mul(3))
}

fn run_bfs(
    graph: &Graph,
    source: u32,
    want_distances: bool,
    scratch: &mut Scratch,
) -> (Vec<i32>, Vec<u32>, u32) {
    let n = graph.n;
    scratch.prepare(n);
    scratch.clear_bits();
    let epoch = scratch.epoch;
    let source_i = source as usize;
    if source_i >= n {
        return (
            if want_distances {
                vec![-1; n]
            } else {
                Vec::new()
            },
            Vec::new(),
            0,
        );
    }

    scratch.stamp[source_i] = epoch;
    if want_distances {
        scratch.dist.fill(-1);
        scratch.dist[source_i] = 0;
    }
    scratch.frontier.push(source);
    set_bit(&mut scratch.frontier_bits, source);

    let mut histogram = Vec::new();
    let mut depth = 0u32;
    let mut visited = 1usize;

    while !scratch.frontier.is_empty() {
        let frontier_edges = scratch
            .frontier
            .iter()
            .map(|&u| graph.offsets[u as usize + 1] as usize - graph.offsets[u as usize] as usize)
            .sum();
        scratch.next.clear();
        scratch.next_bits.fill(0);
        let use_pull = should_pull(frontier_edges, scratch.frontier.len(), n - visited, n);

        if use_pull {
            for v in 0..n as u32 {
                if scratch.stamp[v as usize] == epoch {
                    continue;
                }
                let begin = graph.rev_offsets[v as usize] as usize;
                let end = graph.rev_offsets[v as usize + 1] as usize;
                if graph.rev_adj[begin..end]
                    .iter()
                    .copied()
                    .any(|u| has_bit(&scratch.frontier_bits, u))
                {
                    scratch.stamp[v as usize] = epoch;
                    if want_distances {
                        scratch.dist[v as usize] = depth as i32 + 1;
                    }
                    scratch.next.push(v);
                    set_bit(&mut scratch.next_bits, v);
                }
            }
        } else {
            for &u in &scratch.frontier {
                let begin = graph.offsets[u as usize] as usize;
                let end = graph.offsets[u as usize + 1] as usize;
                for &v in &graph.adj[begin..end] {
                    if scratch.stamp[v as usize] != epoch {
                        scratch.stamp[v as usize] = epoch;
                        if want_distances {
                            scratch.dist[v as usize] = depth as i32 + 1;
                        }
                        scratch.next.push(v);
                        set_bit(&mut scratch.next_bits, v);
                    }
                }
            }
        }

        if scratch.next.is_empty() {
            break;
        }
        visited += scratch.next.len();
        histogram.push(scratch.next.len() as u32);
        depth += 1;
        std::mem::swap(&mut scratch.frontier, &mut scratch.next);
        std::mem::swap(&mut scratch.frontier_bits, &mut scratch.next_bits);
    }

    let distances = if want_distances {
        scratch.dist.clone()
    } else {
        Vec::new()
    };
    (distances, histogram, depth)
}

fn bfs_internal(graph: &Graph, source: u32) -> BfsOneResult {
    SCRATCH.with(|cell| {
        let mut scratch = cell.borrow_mut();
        let (distances, histogram, max_distance) = run_bfs(graph, source, true, &mut scratch);
        BfsOneResult {
            distances,
            max_distance,
            histogram,
        }
    })
}

fn bfs_histogram_internal(graph: &Graph, source: u32) -> BfsHistogramResult {
    SCRATCH.with(|cell| {
        let mut scratch = cell.borrow_mut();
        let (_, histogram, max_distance) = run_bfs(graph, source, false, &mut scratch);
        BfsHistogramResult {
            histogram,
            max_distance,
        }
    })
}

fn bfs_path_internal(graph: &Graph, source: u32, target: u32) -> BfsPathResult {
    let n = graph.n;
    let src = source as usize;
    let tgt = target as usize;
    if src >= n || tgt >= n {
        return BfsPathResult {
            path: vec![],
            distance: -1,
        };
    }
    if src == tgt {
        return BfsPathResult {
            path: vec![source],
            distance: 0,
        };
    }
    let mut parent = vec![-1i32; n];
    let mut q = VecDeque::new();
    parent[src] = src as i32;
    q.push_back(source);
    'outer: while let Some(u) = q.pop_front() {
        let begin = graph.offsets[u as usize] as usize;
        let end = graph.offsets[u as usize + 1] as usize;
        for &v in &graph.adj[begin..end] {
            if parent[v as usize] == -1 {
                parent[v as usize] = u as i32;
                if v as usize == tgt {
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
    let mut path = Vec::new();
    let mut cur = tgt as i32;
    while cur != src as i32 {
        path.push(cur as u32);
        cur = parent[cur as usize];
    }
    path.push(source);
    path.reverse();
    BfsPathResult {
        distance: path.len() as i32 - 1,
        path,
    }
}

fn batch_internal(graph: &Graph, sources: &[u32]) -> BfsBatchResult {
    let results = sources
        .par_iter()
        .map(|&s| bfs_internal(graph, s))
        .collect();
    BfsBatchResult {
        processed: sources.len() as u32,
        results,
    }
}

fn batch_histogram_internal(graph: &Graph, sources: &[u32]) -> BfsHistogramBatchResult {
    let results = sources
        .par_iter()
        .map(|&s| bfs_histogram_internal(graph, s))
        .collect();
    BfsHistogramBatchResult {
        processed: sources.len() as u32,
        results,
    }
}

fn merged_histogram_internal(graph: &Graph) -> MergedHistogram {
    let (bins, max_distance) = (0..graph.n as u32)
        .into_par_iter()
        .fold(
            || (vec![0u64; 128], 0u32),
            |(mut bins, max_distance), source| {
                let result = bfs_histogram_internal(graph, source);
                for (distance, &count) in result.histogram.iter().enumerate() {
                    if distance < bins.len() {
                        bins[distance] += count as u64;
                    }
                }
                (bins, max_distance.max(result.max_distance))
            },
        )
        .reduce(
            || (vec![0u64; 128], 0u32),
            |(mut left, left_max), (right, right_max)| {
                for (l, r) in left.iter_mut().zip(right) {
                    *l += r;
                }
                (left, left_max.max(right_max))
            },
        );
    MergedHistogram {
        histogram: bins.into_iter().map(|v| v as u32).collect(),
        max_distance,
    }
}

#[napi]
pub struct BfsGraph {
    graph: Graph,
}

#[napi]
impl BfsGraph {
    #[napi]
    pub fn one(&self, source: u32) -> BfsOneResult {
        bfs_internal(&self.graph, source)
    }

    #[napi]
    pub fn batch(&self, sources: Vec<u32>) -> BfsBatchResult {
        batch_internal(&self.graph, &sources)
    }

    #[napi]
    pub fn all(&self) -> BfsBatchResult {
        batch_internal(&self.graph, &(0..self.graph.n as u32).collect::<Vec<_>>())
    }

    #[napi]
    pub fn path(&self, source: u32, target: u32) -> BfsPathResult {
        bfs_path_internal(&self.graph, source, target)
    }

    #[napi]
    pub fn one_histogram(&self, source: u32) -> BfsHistogramResult {
        bfs_histogram_internal(&self.graph, source)
    }

    #[napi]
    pub fn batch_histogram(&self, sources: Vec<u32>) -> BfsHistogramBatchResult {
        batch_histogram_internal(&self.graph, &sources)
    }

    #[napi]
    pub fn all_histogram(&self) -> BfsHistogramBatchResult {
        batch_histogram_internal(&self.graph, &(0..self.graph.n as u32).collect::<Vec<_>>())
    }

    #[napi]
    pub fn merged_histogram(&self) -> MergedHistogram {
        merged_histogram_internal(&self.graph)
    }
}

#[napi]
pub fn create_bfs_graph(adj: Uint32Array, offsets: Uint32Array, n: u32) -> Result<BfsGraph> {
    Ok(BfsGraph {
        graph: Graph::new(adj.as_ref().to_vec(), offsets.as_ref().to_vec(), n as usize)?,
    })
}

#[napi]
pub fn bfs_one(adj: Vec<u32>, offsets: Vec<u32>, n: u32, source: u32) -> BfsOneResult {
    let graph = Graph::new(adj, offsets, n as usize).expect("invalid CSR graph");
    bfs_internal(&graph, source)
}

#[napi]
pub fn bfs_batch(adj: Vec<u32>, offsets: Vec<u32>, n: u32, sources: Vec<u32>) -> BfsBatchResult {
    let graph = Graph::new(adj, offsets, n as usize).expect("invalid CSR graph");
    batch_internal(&graph, &sources)
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
    let graph = Graph::new(adj, offsets, n as usize).expect("invalid CSR graph");
    bfs_path_internal(&graph, source, target)
}

#[napi]
pub fn bfs_one_histogram(
    adj: Vec<u32>,
    offsets: Vec<u32>,
    n: u32,
    source: u32,
) -> BfsHistogramResult {
    let graph = Graph::new(adj, offsets, n as usize).expect("invalid CSR graph");
    bfs_histogram_internal(&graph, source)
}

#[napi]
pub fn bfs_batch_histogram(
    adj: Vec<u32>,
    offsets: Vec<u32>,
    n: u32,
    sources: Vec<u32>,
) -> BfsHistogramBatchResult {
    let graph = Graph::new(adj, offsets, n as usize).expect("invalid CSR graph");
    batch_histogram_internal(&graph, &sources)
}

#[napi]
pub fn bfs_all_histogram(adj: Vec<u32>, offsets: Vec<u32>, n: u32) -> BfsHistogramBatchResult {
    bfs_batch_histogram(adj, offsets, n, (0..n).collect())
}

#[napi]
pub fn bfs_merged_histogram(adj: Vec<u32>, offsets: Vec<u32>, n: u32) -> MergedHistogram {
    let graph = Graph::new(adj, offsets, n as usize).expect("invalid CSR graph");
    merged_histogram_internal(&graph)
}
