use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::visit::{Bfs, VisitMap};
use rayon::prelude::*;
use serde_json::{json, Value};
use std::time::Instant;

fn main() {
    let fixture_path = std::env::args().nth(1).expect("fixture JSON path");
    let fixture: Value = serde_json::from_slice(&std::fs::read(fixture_path).unwrap()).unwrap();
    let n = fixture["nodeCount"].as_u64().unwrap() as usize;
    let adj: Vec<u32> = fixture["adjFlat"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_u64().unwrap() as u32)
        .collect();
    let offsets: Vec<usize> = fixture["offsets"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_u64().unwrap() as usize)
        .collect();
    let create_started = Instant::now();
    let mut graph = DiGraph::<(), (), u32>::with_capacity(n, adj.len());
    for _ in 0..n {
        graph.add_node(());
    }
    for u in 0..n {
        for &v in &adj[offsets[u]..offsets[u + 1]] {
            graph.add_edge(NodeIndex::new(u), NodeIndex::new(v as usize), ());
        }
    }
    let create_ms = create_started.elapsed().as_secs_f64() * 1000.0;
    let started = Instant::now();
    let (histogram, max_distance) = (0..n)
        .into_par_iter()
        .fold(
            || (Bfs::new(&graph, NodeIndex::new(0)), vec![0u64; 128], 0usize),
            |(mut bfs, mut bins, mut max_distance), source| {
                // Reuse the crate's public queue and visited bitmap across sources.
                bfs.stack.clear();
                bfs.discovered.clear();
                let source = NodeIndex::<u32>::new(source);
                bfs.discovered.visit(source);
                bfs.stack.push_back(source);
                let mut depth = 0usize;
                while !bfs.stack.is_empty() {
                    let width = bfs.stack.len();
                    for _ in 0..width {
                        bfs.next(&graph);
                    }
                    let discovered = bfs.stack.len();
                    if discovered == 0 {
                        break;
                    }
                    if bins.len() == depth {
                        bins.push(0);
                    }
                    bins[depth] += discovered as u64;
                    depth += 1;
                }
                max_distance = max_distance.max(depth);
                (bfs, bins, max_distance)
            },
        )
        .map(|(_, bins, max_distance)| (bins, max_distance))
        .reduce(
            || (vec![0u64; 128], 0usize),
            |(mut left, lm), (right, rm)| {
                left.resize(left.len().max(right.len()), 0);
                for (a, b) in left.iter_mut().zip(right) {
                    *a += b;
                }
                (left, lm.max(rm))
            },
        );
    let elapsed_ms = started.elapsed().as_secs_f64() * 1000.0;
    println!(
        "{}",
        json!({
            "candidate": "petgraph 0.8.3 DiGraph + visit::Bfs + Rayon 1.12.0 source batches",
            "nodes": n,
            "arcs": adj.len(),
            "rayonThreads": rayon::current_num_threads(),
            "createMs": create_ms,
            "samplesMs": [elapsed_ms],
            "result": {"histogram": histogram, "maxDistance": max_distance},
        })
    );
}
