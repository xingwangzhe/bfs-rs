import { createRequire } from "node:module";
import { performance } from "node:perf_hooks";
const native = createRequire(import.meta.url)(process.env.PGO_BINDING_PATH);
const training = process.argv[2] === "train";
const results = {};
let sink = 0;
async function measure(name, fn, iterations = 1) {
  for (let i = 0; i < 2; i++) await fn();
  if (!training) {
    const started = performance.now();
    for (let i = 0; i < iterations; i++) await fn();
    const elapsed = Math.max(performance.now() - started, 0.001);
    iterations = Math.max(iterations, Math.ceil((iterations * 30) / elapsed));
  }
  const samples = [];
  for (let sample = 0; sample < (training ? 2 : 7); sample++) {
    const started = performance.now();
    for (let i = 0; i < iterations; i++) await fn();
    samples.push((performance.now() - started) / iterations);
  }
  results[name] = samples;
}
function graphData(n, shape) {
  const rows = Array.from({ length: n }, () => []);
  for (let u = 0; u < n; u++) {
    if (shape === "dense") {
      for (let v = 0; v < n; v++) if (u !== v) rows[u].push(v);
    } else if (shape === "path") {
      if (u + 1 < n) rows[u].push(u + 1);
    } else {
      rows[u].push((u + 1) % n);
      for (let j = 1; j < 8; j++)
        rows[u].push(
          shape === "skewed" && j < 4 ? 0 : (u * 1103515245 + j * 12345 + (training ? 7 : 31)) % n,
        );
    }
  }
  const offsets = new Uint32Array(n + 1);
  for (let i = 0; i < n; i++) offsets[i + 1] = offsets[i] + rows[i].length;
  return { adj: new Uint32Array(rows.flat()), offsets };
}
for (const shape of ["sparse", "dense", "skewed", "path"]) {
  const n = shape === "dense" ? (training ? 192 : 256) : training ? 1024 : 1536;
  const { adj, offsets } = graphData(n, shape);
  const graph = native.createBfsGraph(adj, offsets, n);
  const sources = Array.from({ length: 128 }, (_, i) => (i * 17) % n);
  await measure(
    `${shape}.preparedHistogram`,
    () => {
      sink += graph.batchHistogram(sources).processed;
    },
    3,
  );
  await measure(
    `${shape}.distances`,
    () => {
      sink += graph.batch(sources.slice(0, 16)).processed;
    },
    3,
  );
  await measure(
    `${shape}.path`,
    () => {
      sink += graph.path(0, n - 1).distance;
    },
    20,
  );
  if (training) {
    graph.allHistogram();
    graph.mergedHistogram();
    graph.one(0);
    graph.oneHistogram(0);
    native.bfsOne(Array.from(adj), Array.from(offsets), n, 0);
    native.bfsBatchHistogram(Array.from(adj), Array.from(offsets), n, sources);
  }
}
if (!Number.isFinite(sink)) throw new Error("Non-finite workload output");
console.log(JSON.stringify(results));
