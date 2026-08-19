import test from "node:test";
import assert from "node:assert/strict";
import { bfsOne, bfsBatch, bfsAll, bfsPath, createBfsGraph } from "./index.js";
import { createRequire } from "node:module"; const require = createRequire(import.meta.url); const { bfsOneHistogram, bfsBatchHistogram, bfsAllHistogram } = require("./index.js");

const adj = [1, 2, 0, 2, 0, 1, 3, 2];
const offsets = [0, 2, 4, 7, 8];
const n = 4;

test("bfsOne from node 0", () => {
  const r = bfsOne(adj, offsets, n, 0);
  assert.deepEqual(r.distances, [0, 1, 1, 2]);
  assert.equal(r.maxDistance, 2);
  assert.deepEqual(r.histogram, [2, 1]);
});

test("bfsBatch from multiple sources", () => {
  const r = bfsBatch(adj, offsets, n, [0, 3]);
  assert.equal(r.processed, 2);
  assert.equal(r.results.length, 2);
});

test("bfsAll processes all nodes", () => {
  const r = bfsAll(adj, offsets, n);
  assert.equal(r.processed, 4);
});

test("bfsPath direct path", () => {
  const r = bfsPath(adj, offsets, n, 0, 3);
  assert.deepEqual(r.path, [0, 2, 3]);
  assert.equal(r.distance, 2);
});

test("bfsPath unreachable", () => {
  const r = bfsPath([], [0, 0, 0], 2, 0, 1);
  assert.deepEqual(r.path, []);
  assert.equal(r.distance, -1);
});

test("bfsOneHistogram returns histogram only", () => {
  const r = bfsOneHistogram(adj, offsets, n, 0);
  assert.equal(r.maxDistance, 2);
  assert.deepEqual(r.histogram, [2, 1]);
  assert.equal(r.distances, undefined); // 不应该有 distances
});

test("bfsBatchHistogram parallel histogram batch", () => {
  const r = bfsBatchHistogram(adj, offsets, n, [0, 3]);
  assert.equal(r.processed, 2);
  assert.equal(r.results.length, 2);
  for (const h of r.results) {
    assert.equal(h.distances, undefined);
    assert.ok(Array.isArray(h.histogram));
  }
});

test("bfsAllHistogram from all nodes", () => {
  const r = bfsAllHistogram(adj, offsets, n);
  assert.equal(r.processed, 4);
});

test("prepared Uint32Array graph keeps exact results", () => {
  const graph = createBfsGraph(new Uint32Array(adj), new Uint32Array(offsets), n);
  assert.deepEqual(graph.one(0).distances, [0, 1, 1, 2]);
  assert.deepEqual(graph.oneHistogram(0).histogram, [2, 1]);
  assert.deepEqual(graph.path(0, 3).path, [0, 2, 3]);
  assert.equal(graph.all().processed, n);
});

test("direction switching remains exact on a broad frontier", () => {
  const nodes = 128;
  const a = [];
  const o = [0];
  for (let u = 0; u < nodes; u++) {
    for (let v = 0; v < nodes; v++) {
      if (u !== v && ((u * 17 + v * 13) % 19) < 3) a.push(v);
    }
    o.push(a.length);
  }
  const result = bfsOne(a, o, nodes, 0);
  const dist = Array(nodes).fill(-1);
  const queue = [0];
  dist[0] = 0;
  for (let head = 0; head < queue.length; head++) {
    const u = queue[head];
    for (let i = o[u]; i < o[u + 1]; i++) {
      const v = a[i];
      if (dist[v] === -1) {
        dist[v] = dist[u] + 1;
        queue.push(v);
      }
    }
  }
  assert.deepEqual(result.distances, dist);
});
