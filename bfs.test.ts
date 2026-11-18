import test from "node:test";
import assert from "node:assert/strict";
import { bfsOne, bfsBatch, bfsAll, bfsPath, createBfsGraph } from "./index.js";
import {
  bfsOneHistogram,
  bfsBatchHistogram,
  bfsAllHistogram,
  bfsMergedHistogram,
} from "./index.js";

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
      if (u !== v && (u * 17 + v * 13) % 19 < 3) a.push(v);
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

function referenceDistances(adjacency, rowOffsets, nodeCount, source) {
  const distances = Array(nodeCount).fill(-1);
  if (source < 0 || source >= nodeCount) return distances;
  const queue = [source];
  distances[source] = 0;
  for (let head = 0; head < queue.length; head++) {
    const u = queue[head];
    for (let i = rowOffsets[u]; i < rowOffsets[u + 1]; i++) {
      const v = adjacency[i];
      if (distances[v] === -1) {
        distances[v] = distances[u] + 1;
        queue.push(v);
      }
    }
  }
  return distances;
}

test("distances match a simple reference across sparse, dense, skewed, and duplicate-edge graphs", () => {
  const cases = [
    { name: "empty", n: 0, edges: [] },
    { name: "sparse", n: 37, edges: Array.from({ length: 36 }, (_, i) => [i, i + 1]) },
    {
      name: "dense",
      n: 24,
      edges: Array.from({ length: 24 * 23 }, (_, i) => [
        Math.floor(i / 23),
        (Math.floor(i / 23) + 1 + (i % 23)) % 24,
      ]),
    },
    {
      name: "skewed",
      n: 41,
      edges: Array.from({ length: 40 }, (_, i) => (i === 0 ? [0, 1] : [1, i + 1])),
    },
    {
      name: "duplicates",
      n: 8,
      edges: [
        [0, 1],
        [0, 1],
        [1, 2],
        [1, 2],
        [4, 5],
      ],
    },
  ];

  for (const { name, n: nodeCount, edges } of cases) {
    const adjacency = Array.from({ length: nodeCount }, () => []);
    for (const [u, v] of edges) adjacency[u].push(v);
    const flat = adjacency.flat();
    const rowOffsets = [0];
    for (const row of adjacency) rowOffsets.push(rowOffsets.at(-1) + row.length);
    for (let source = 0; source < nodeCount; source++) {
      const expected = referenceDistances(flat, rowOffsets, nodeCount, source);
      assert.deepEqual(
        bfsOne(flat, rowOffsets, nodeCount, source).distances,
        expected,
        `${name}, source ${source}`,
      );
    }
    assert.equal(bfsAll(flat, rowOffsets, nodeCount).processed, nodeCount, name);
  }
});

test("path and histogram handle isolated vertices and invalid sources", () => {
  assert.deepEqual(bfsOne([], [0, 0, 0], 2, 1).distances, [-1, 0]);
  assert.deepEqual(bfsOne([], [0, 0, 0], 2, 4).distances, [-1, -1]);
  assert.deepEqual(bfsPath([], [0, 0, 0], 2, 1, 0), { path: [], distance: -1 });
  const histogram = bfsOneHistogram([], [0, 0, 0], 2, 1);
  assert.deepEqual(histogram.histogram, []);
  assert.equal(histogram.maxDistance, 0);
});

function csrFromEdges(nodeCount, edges) {
  const rows = Array.from({ length: nodeCount }, () => []);
  for (const [u, v] of edges) rows[u].push(v);
  const offsets = [0];
  for (const row of rows) offsets.push(offsets.at(-1) + row.length);
  return { adj: rows.flat(), offsets };
}

function referenceMerged(adjacency, rowOffsets, nodeCount) {
  const histogram = Array(128).fill(0);
  let maxDistance = 0;
  for (let source = 0; source < nodeCount; source++) {
    for (const distance of referenceDistances(adjacency, rowOffsets, nodeCount, source)) {
      if (distance <= 0) continue;
      while (histogram.length < distance) histogram.push(0);
      histogram[distance - 1]++;
      maxDistance = Math.max(maxDistance, distance);
    }
  }
  return { histogram, maxDistance };
}

test("merged histogram matches independent reference across lane boundaries and directed graphs", () => {
  let seed = 0x12345678;
  const next = () => {
    seed ^= seed << 13;
    seed ^= seed >>> 17;
    seed ^= seed << 5;
    return seed >>> 0;
  };
  for (const nodeCount of [0, 1, 2, 63, 64, 65, 127, 128, 129, 193]) {
    for (const degree of [0, 1, 4, 17]) {
      const edges = [];
      for (let u = 0; u < nodeCount; u++) {
        for (let edge = 0; edge < degree; edge++) {
          // Include directed edges, duplicate arcs, loops, and disconnected pieces.
          const componentSize = Math.ceil(nodeCount / 2);
          const begin = u < componentSize ? 0 : componentSize;
          edges.push([u, begin + (next() % Math.min(componentSize, nodeCount - begin))]);
        }
      }
      const { adj: adjacency, offsets: rowOffsets } = csrFromEdges(nodeCount, edges);
      const expected = referenceMerged(adjacency, rowOffsets, nodeCount);
      const graph = createBfsGraph(
        new Uint32Array(adjacency),
        new Uint32Array(rowOffsets),
        nodeCount,
      );
      assert.deepEqual(graph.mergedHistogram(), expected, `n=${nodeCount}, degree=${degree}`);
      assert.deepEqual(bfsMergedHistogram(adjacency, rowOffsets, nodeCount), expected);
    }
  }
});

test("merged histogram retains every bin beyond distance 128", () => {
  const nodeCount = 321;
  const { adj: adjacency, offsets: rowOffsets } = csrFromEdges(
    nodeCount,
    Array.from({ length: nodeCount - 1 }, (_, i) => [i, i + 1]),
  );
  const graph = createBfsGraph(new Uint32Array(adjacency), new Uint32Array(rowOffsets), nodeCount);
  const actual = graph.mergedHistogram();
  assert.deepEqual(actual, referenceMerged(adjacency, rowOffsets, nodeCount));
  assert.equal(actual.maxDistance, 320);
  assert.equal(actual.histogram[319], 1);
});

test("multi-source traversal retains long distances when a path has branches", () => {
  const nodeCount = 321;
  const edges = Array.from({ length: nodeCount - 1 }, (_, i) => [i, i + 1]);
  // The first vertex exceeds the independent-path fallback's maximum degree.
  edges.push([0, 0], [0, 0]);
  const { adj: adjacency, offsets: rowOffsets } = csrFromEdges(nodeCount, edges);
  const graph = createBfsGraph(new Uint32Array(adjacency), new Uint32Array(rowOffsets), nodeCount);
  assert.deepEqual(graph.mergedHistogram(), referenceMerged(adjacency, rowOffsets, nodeCount));
});

test("merged histogram preserves multiplicities of undirected twin vertices", () => {
  const cases = [];
  // A star has one large false-twin leaf group.
  cases.push({ n: 145, edges: Array.from({ length: 144 }, (_, i) => [0, i + 1]) });
  // Repeated neighbor sets with degree > 1, plus disconnected and isolated nodes.
  cases.push({
    n: 145,
    edges: Array.from({ length: 130 }, (_, i) => [i + 2, i % 3 === 0 ? 0 : 1]).concat(
      Array.from({ length: 130 }, (_, i) => [i + 2, 135]),
      [
        [135, 136],
        [140, 141],
      ],
    ),
  });
  // Equal rows that include self-loops have internal distance 1, not 2.
  cases.push({
    n: 80,
    edges: [
      [0, 0],
      [0, 1],
      [1, 1],
      [0, 2],
      [1, 2],
    ],
  });
  for (const { n: nodeCount, edges } of cases) {
    const directed = edges.flatMap(([u, v]) =>
      u === v
        ? [[u, v]]
        : [
            [u, v],
            [v, u],
          ],
    );
    const { adj: adjacency, offsets: rowOffsets } = csrFromEdges(nodeCount, directed);
    const graph = createBfsGraph(
      new Uint32Array(adjacency),
      new Uint32Array(rowOffsets),
      nodeCount,
    );
    assert.deepEqual(graph.mergedHistogram(), referenceMerged(adjacency, rowOffsets, nodeCount));
  }
});

test("unused CSR storage does not create incoming arcs during pull traversal", () => {
  const nodeCount = 80;
  const edges = [];
  for (let u = 0; u < nodeCount - 1; u++) {
    for (let v = 0; v < nodeCount - 1; v++) edges.push([u, v]);
  }
  const { adj: adjacency, offsets: rowOffsets } = csrFromEdges(nodeCount, edges);
  const padded = [...adjacency, ...Array(300).fill(nodeCount - 1)];
  const graph = createBfsGraph(new Uint32Array(padded), new Uint32Array(rowOffsets), nodeCount);
  assert.deepEqual(graph.one(1).distances, referenceDistances(adjacency, rowOffsets, nodeCount, 1));
  assert.deepEqual(graph.mergedHistogram(), referenceMerged(adjacency, rowOffsets, nodeCount));
});
