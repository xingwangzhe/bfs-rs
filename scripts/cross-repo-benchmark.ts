import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFileSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { resolve } from "node:path";
import { performance } from "node:perf_hooks";

const [bindingPath, fixturePath, outputPath] = process.argv.slice(2);
if (!bindingPath || !fixturePath) {
  throw new Error(
    "Usage: bun scripts/cross-repo-benchmark.ts <binding.node> <fixture.json> [output.json]",
  );
}
const native = createRequire(import.meta.url)(resolve(bindingPath));
const fixture = JSON.parse(readFileSync(fixturePath, "utf8"));
const adj = new Uint32Array(fixture.adjFlat);
const offsets = new Uint32Array(fixture.offsets);
const createStarted = performance.now();
const graph = native.createBfsGraph(adj, offsets, fixture.nodeCount);
const createMs = performance.now() - createStarted;
const samplesMs = [];
const warmupMs = [];
let result;
const repetitions = Number(process.env.BFS_BENCH_REPS ?? 3);
for (let sample = 0; sample < Number(process.env.BFS_BENCH_WARMUPS ?? 0); sample++) {
  const started = performance.now();
  result = graph.mergedHistogram();
  warmupMs.push(performance.now() - started);
}
for (let sample = 0; sample < repetitions; sample++) {
  const started = performance.now();
  const current = graph.mergedHistogram();
  samplesMs.push(performance.now() - started);
  if (result) assert.deepEqual(current, result);
  result = current;
}
const report = {
  binding: resolve(bindingPath),
  nodeCount: fixture.nodeCount,
  arcCount: adj.length,
  rayonThreads: process.env.RAYON_NUM_THREADS ?? "default",
  createMs,
  warmupMs,
  samplesMs,
  p50Ms: [...samplesMs].sort((a, b) => a - b)[Math.floor(samplesMs.length / 2)],
  result,
  resultSha256: createHash("sha256").update(JSON.stringify(result)).digest("hex"),
  maxRssKiB: process.resourceUsage().maxRSS,
  linuxVmHwmKiB:
    process.platform === "linux"
      ? Number(readFileSync("/proc/self/status", "utf8").match(/^VmHWM:\s+(\d+)/m)?.[1])
      : undefined,
};
if (outputPath) writeFileSync(outputPath, `${JSON.stringify(report, null, 2)}\n`);
console.log(JSON.stringify(report));
