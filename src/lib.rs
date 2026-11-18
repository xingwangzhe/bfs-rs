#![deny(clippy::all)]

use napi::bindgen_prelude::Uint32Array;
use napi::{Error, Result, Status};
use napi_derive::napi;
use rayon::prelude::*;
use std::collections::{BTreeMap, HashMap, VecDeque};

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
    fn new(mut adj: Vec<u32>, offsets: Vec<u32>, n: usize) -> Result<Self> {
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
        // CSR may use a prefix of the provided adjacency storage. The reverse
        // graph must use that same prefix, rather than inventing incoming arcs
        // from the unused tail.
        adj.truncate(offsets[n] as usize);

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
        }
        self.frontier.clear();
        self.next.clear();
        self.epoch = self.epoch.wrapping_add(1);
        if self.epoch == 0 {
            self.stamp.fill(0);
            self.epoch = 1;
        }
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

    let mut histogram = Vec::new();
    let mut depth = 0u32;
    let mut visited = 1usize;
    let mut frontier_edges = (graph.offsets[source_i + 1] - graph.offsets[source_i]) as usize;

    while !scratch.frontier.is_empty() {
        scratch.next.clear();
        let use_pull = should_pull(frontier_edges, scratch.frontier.len(), n - visited, n);
        let mut next_edges = 0usize;

        if use_pull {
            scratch.frontier_bits.fill(0);
            for &u in &scratch.frontier {
                set_bit(&mut scratch.frontier_bits, u);
            }
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
                    next_edges +=
                        (graph.offsets[v as usize + 1] - graph.offsets[v as usize]) as usize;
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
                        next_edges +=
                            (graph.offsets[v as usize + 1] - graph.offsets[v as usize]) as usize;
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
        frontier_edges = next_edges;
        std::mem::swap(&mut scratch.frontier, &mut scratch.next);
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

fn merged_histogram_independent(graph: &Graph) -> MergedHistogram {
    let (bins, max_distance) = (0..graph.n as u32)
        .into_par_iter()
        .fold(
            || (vec![0u64; 128], 0u32),
            |(mut bins, max_distance), source| {
                let result = bfs_histogram_internal(graph, source);
                bins.resize(bins.len().max(result.histogram.len()), 0);
                for (distance, &count) in result.histogram.iter().enumerate() {
                    bins[distance] += count as u64;
                }
                (bins, max_distance.max(result.max_distance))
            },
        )
        .reduce(
            || (vec![0u64; 128], 0u32),
            |(mut left, left_max), (right, right_max)| {
                left.resize(left.len().max(right.len()), 0);
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

/// Each bit represents an independent source, not a union of their distances.
/// Level-synchronous propagation therefore computes the exact directed distance
/// of every source/vertex pair while sharing adjacency scans across 64 searches.
/// See Then et al., "The More the Merrier: Efficient Multi-Source Graph Traversal".
struct MultiScratch {
    seen: Vec<u64>,
    current: Vec<u64>,
    next: Vec<u64>,
    frontier: Vec<u32>,
    next_frontier: Vec<u32>,
}

impl MultiScratch {
    fn new(n: usize) -> Self {
        Self {
            seen: vec![0; n],
            current: vec![0; n],
            next: vec![0; n],
            frontier: Vec::with_capacity(n),
            next_frontier: Vec::with_capacity(n),
        }
    }

    fn run_dispatch<const WEIGHTED: bool>(
        &mut self,
        graph: &Graph,
        sources: &[u32],
        weights: &[u32],
        bins: &mut Vec<u64>,
    ) -> u32 {
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        if std::is_x86_feature_detected!("popcnt") {
            // SAFETY: runtime detection preserves the generic x86 CPU baseline.
            return unsafe { self.run_popcnt::<WEIGHTED>(graph, sources, weights, bins) };
        }
        self.run::<WEIGHTED>(graph, sources, weights, bins)
    }

    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    #[target_feature(enable = "popcnt")]
    unsafe fn run_popcnt<const WEIGHTED: bool>(
        &mut self,
        graph: &Graph,
        sources: &[u32],
        weights: &[u32],
        bins: &mut Vec<u64>,
    ) -> u32 {
        self.run::<WEIGHTED>(graph, sources, weights, bins)
    }

    #[inline(always)]
    fn run<const WEIGHTED: bool>(
        &mut self,
        graph: &Graph,
        sources: &[u32],
        weights: &[u32],
        bins: &mut Vec<u64>,
    ) -> u32 {
        self.seen.fill(0);
        self.current.fill(0);
        self.next.fill(0);
        self.frontier.clear();
        self.next_frontier.clear();
        for (lane, &source) in sources.iter().enumerate() {
            let source = source as usize;
            let bit = 1u64 << lane;
            self.seen[source] = bit;
            self.current[source] = bit;
            self.frontier.push(source as u32);
        }
        let mut depth = 0usize;
        let active_mask = u64::MAX >> (64 - sources.len());
        while !self.frontier.is_empty() {
            let mut count = 0u64;
            if self.frontier.len() > graph.n / 8 {
                for (v, row) in graph.rev_offsets.windows(2).enumerate() {
                    let missing = active_mask & !self.seen[v];
                    if missing == 0 {
                        continue;
                    }
                    let begin = row[0] as usize;
                    let end = row[1] as usize;
                    let mut reachable = 0u64;
                    for &u in &graph.rev_adj[begin..end] {
                        reachable |= self.current[u as usize];
                        if reachable & missing == missing {
                            break;
                        }
                    }
                    let fresh = reachable & missing;
                    if fresh != 0 {
                        self.next_frontier.push(v as u32);
                        self.seen[v] |= fresh;
                        self.next[v] = fresh;
                        let pairs = fresh.count_ones() as u64;
                        count += if WEIGHTED {
                            pairs * weights[v] as u64
                        } else {
                            pairs
                        };
                    }
                }
                for &u in &self.frontier {
                    self.current[u as usize] = 0;
                }
            } else {
                for &u in &self.frontier {
                    let bits = self.current[u as usize];
                    self.current[u as usize] = 0;
                    let begin = graph.offsets[u as usize] as usize;
                    let end = graph.offsets[u as usize + 1] as usize;
                    for &v in &graph.adj[begin..end] {
                        let v = v as usize;
                        let fresh = bits & !self.seen[v];
                        if fresh != 0 {
                            if self.next[v] == 0 {
                                self.next_frontier.push(v as u32);
                            }
                            self.seen[v] |= fresh;
                            self.next[v] |= fresh;
                            let pairs = fresh.count_ones() as u64;
                            count += if WEIGHTED {
                                pairs * weights[v] as u64
                            } else {
                                pairs
                            };
                        }
                    }
                }
            }
            if count == 0 {
                break;
            }
            if depth == bins.len() {
                bins.push(0);
            }
            bins[depth] += if WEIGHTED {
                count * weights[sources[0] as usize] as u64
            } else {
                count
            };
            depth += 1;
            std::mem::swap(&mut self.current, &mut self.next);
            std::mem::swap(&mut self.frontier, &mut self.next_frontier);
            self.next_frontier.clear();
        }
        depth as u32
    }
}

fn merged_histogram_internal(graph: &Graph) -> MergedHistogram {
    // Long paths/cycles offer little overlap between simultaneous frontiers.
    // Independent searches avoid the extra lane bookkeeping for this case.
    if graph.n < 64 || graph.offsets.windows(2).all(|row| row[1] - row[0] <= 2) {
        return merged_histogram_independent(graph);
    }
    if let Some((quotient, weights, internal_pairs)) = twin_quotient(graph) {
        return merged_histogram_weighted(&quotient, &weights, internal_pairs);
    }
    merged_histogram_multi(graph)
}

fn merged_histogram_multi(graph: &Graph) -> MergedHistogram {
    let (bins, max_distance) = (0..graph.n.div_ceil(64))
        .into_par_iter()
        .fold(
            || (MultiScratch::new(graph.n), vec![0u64; 128], 0u32),
            |(mut scratch, mut bins, max_distance), batch| {
                let sources: Vec<u32> = (batch * 64..((batch + 1) * 64).min(graph.n))
                    .map(|source| source as u32)
                    .collect();
                let depth = scratch.run_dispatch::<false>(graph, &sources, &[], &mut bins);
                (scratch, bins, max_distance.max(depth))
            },
        )
        .map(|(_, bins, max_distance)| (bins, max_distance))
        .reduce(
            || (vec![0u64; 128], 0u32),
            |(mut left, left_max), (right, right_max)| {
                left.resize(left.len().max(right.len()), 0);
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

/// Equal neighbor sets can be represented by one weighted vertex in an
/// undirected graph. Distances between distinct classes are unchanged. Pairs
/// inside a class have distance two (or one when the class has internal arcs).
/// Isolated twins remain unreachable from each other. Directed graphs must not
/// use this reduction: equal outgoing rows alone do not imply symmetry.
fn twin_quotient(graph: &Graph) -> Option<(Graph, Vec<u32>, [u64; 2])> {
    let mut rows = Vec::with_capacity(graph.n);
    for u in 0..graph.n {
        let mut row = graph.adj[graph.offsets[u] as usize..graph.offsets[u + 1] as usize].to_vec();
        row.sort_unstable();
        let reverse =
            &graph.rev_adj[graph.rev_offsets[u] as usize..graph.rev_offsets[u + 1] as usize];
        // The transpose is already sorted by the original source vertex.
        if row != reverse {
            return None;
        }
        row.dedup();
        rows.push(row);
    }
    let mut classes: HashMap<&[u32], u32> = HashMap::with_capacity(graph.n);
    let mut mapping = Vec::with_capacity(graph.n);
    let mut representatives = Vec::new();
    let mut weights = Vec::new();
    for (u, row) in rows.iter().enumerate() {
        let next_class = weights.len() as u32;
        let class = *classes.entry(row).or_insert_with(|| {
            representatives.push(u);
            weights.push(0u32);
            next_class
        });
        mapping.push(class);
        weights[class as usize] += 1;
    }
    // Building a quotient has fixed costs; require a meaningful reduction.
    if weights.len() * 10 > graph.n * 9 {
        return None;
    }
    let mut adj = Vec::new();
    let mut offsets = vec![0u32];
    let mut internal_pairs = [0u64; 2];
    for (class, &u) in representatives.iter().enumerate() {
        let mut row: Vec<u32> = rows[u].iter().map(|&v| mapping[v as usize]).collect();
        row.sort_unstable();
        row.dedup();
        let weight = weights[class] as u64;
        let internal_distance = if row.binary_search(&(class as u32)).is_ok() {
            1
        } else {
            2
        };
        if !row.is_empty() {
            internal_pairs[internal_distance - 1] += weight * (weight - 1);
        }
        adj.extend(row);
        offsets.push(adj.len() as u32);
    }
    Some((
        Graph::new(adj, offsets, weights.len()).expect("valid quotient CSR"),
        weights,
        internal_pairs,
    ))
}

fn merged_histogram_weighted(
    graph: &Graph,
    weights: &[u32],
    internal_pairs: [u64; 2],
) -> MergedHistogram {
    // Equal source weights make each fresh lane count a single popcount plus
    // a multiplication, rather than iterating over every source bit.
    let mut groups: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
    for (source, &weight) in weights.iter().enumerate() {
        groups.entry(weight).or_default().push(source as u32);
    }
    let batches: Vec<&[u32]> = groups
        .values()
        .flat_map(|sources| sources.chunks(64))
        .collect();
    let (mut bins, mut max_distance) = batches
        .into_par_iter()
        .fold(
            || (MultiScratch::new(graph.n), vec![0u64; 128], 0u32),
            |(mut scratch, mut bins, max_distance), sources| {
                let depth = scratch.run_dispatch::<true>(graph, sources, weights, &mut bins);
                (scratch, bins, max_distance.max(depth))
            },
        )
        .map(|(_, bins, max_distance)| (bins, max_distance))
        .reduce(
            || (vec![0u64; 128], 0u32),
            |(mut left, left_max), (right, right_max)| {
                left.resize(left.len().max(right.len()), 0);
                for (l, r) in left.iter_mut().zip(right) {
                    *l += r;
                }
                (left, left_max.max(right_max))
            },
        );
    for (index, count) in internal_pairs.into_iter().enumerate() {
        bins[index] += count;
        if count > 0 {
            max_distance = max_distance.max(index as u32 + 1);
        }
    }
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

#[cfg(feature = "pgo")]
#[napi]
pub fn flush_pgo_profile() -> Result<()> {
    unsafe extern "C" {
        fn __llvm_profile_write_file() -> i32;
    }
    let status = unsafe { __llvm_profile_write_file() };
    if status == 0 {
        Ok(())
    } else {
        Err(Error::new(
            Status::GenericFailure,
            "LLVM profile flush failed",
        ))
    }
}
