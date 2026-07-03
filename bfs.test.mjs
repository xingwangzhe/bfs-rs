import test from "node:test";
import assert from "node:assert/strict";
import { bfsOne, bfsBatch, bfsAll, bfsPath } from "./index.js";

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
