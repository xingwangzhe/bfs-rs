import { performance } from "node:perf_hooks";
import { bfsAllHistogram, createBfsGraph } from "./index.js";

const n = Number(process.env.BFS_BENCH_N ?? 2048);
const degree = Number(process.env.BFS_BENCH_DEGREE ?? 8);
const repetitions = Number(process.env.BFS_BENCH_REPS ?? 3);

function buildGraph(nodeCount, averageDegree) {
  const adjacency = Array.from({ length: nodeCount }, () => []);
  for (let u = 0; u < nodeCount; u++) {
    adjacency[u].push((u + 1) % nodeCount);
    for (let j = 1; j < averageDegree; j++) {
      adjacency[u].push((u * 1103515245 + j * 12345) % nodeCount);
    }
  }
  const adj = new Uint32Array(adjacency.flat());
  const offsets = new Uint32Array(nodeCount + 1);
  for (let i = 0; i < nodeCount; i++) offsets[i + 1] = offsets[i] + adjacency[i].length;
  return { adj, offsets };
}

const graphData = buildGraph(n, degree);
const preparedStart = performance.now();
const graph = createBfsGraph(graphData.adj, graphData.offsets, n);
const preparedMs = performance.now() - preparedStart;
graph.oneHistogram(0);

function measure(label, fn) {
  const samples = [];
  for (let i = 0; i < repetitions; i++) {
    const start = performance.now();
    fn();
    samples.push(performance.now() - start);
  }
  samples.sort((a, b) => a - b);
  return { label, p50Ms: samples[Math.floor(samples.length / 2)], samplesMs: samples };
}

const results = [
  measure("prepared.allHistogram", () => graph.allHistogram()),
  measure("compat.allHistogram", () => bfsAllHistogram(Array.from(graphData.adj), Array.from(graphData.offsets), n)),
];
console.log(JSON.stringify({ package: "bfs-rs", n, degree, edges: graphData.adj.length, preparedMs, results }, null, 2));
