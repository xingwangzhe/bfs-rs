import { test, expect } from "bun:test";
import { bfsOne, bfsBatch, bfsAll } from "./index.js";

const adj = [1, 2, 0, 2, 0, 1, 3, 2];
const offsets = [0, 2, 4, 7, 8];
const n = 4;

test("bfsOne from node 0", () => {
  const r = bfsOne(adj, offsets, n, 0);
  expect(r.distances).toEqual([0, 1, 1, 2]);
  expect(r.maxDistance).toBe(2);
  // histogram: distance 1 has 2 nodes (1,2), distance 2 has 1 node (3)
  expect(r.histogram).toEqual([2, 1]);
});

test("bfsOne unreachable node", () => {
  // node 3 is reachable from 0 via 2
  const r = bfsOne(adj, offsets, n, 0);
  expect(r.distances[3]).toBe(2);
});

test("bfsBatch from multiple sources", () => {
  const r = bfsBatch(adj, offsets, n, [0, 3]);
  expect(r.processed).toBe(2);
  expect(r.results.length).toBe(2);
  expect(r.results[0].distances).toEqual([0, 1, 1, 2]);
  expect(r.results[1].distances).toEqual([2, 2, 1, 0]);
});

test("bfsAll processes all nodes", () => {
  const r = bfsAll(adj, offsets, n);
  expect(r.processed).toBe(4);
  expect(r.results.length).toBe(4);
});

test("single node graph", () => {
  const r = bfsOne([], [0, 0], 1, 0);
  expect(r.distances).toEqual([0]);
  expect(r.maxDistance).toBe(0);
});
