#!/usr/bin/env node
'use strict';

const fs = require('fs');

function main() {
  const inputPath = process.argv[2];
  const outputPath = process.argv[3];

  if (!inputPath || !outputPath) {
    console.error('Usage: ua-tour-analyze.js <input.json> <output.json>');
    process.exit(1);
  }

  const raw = JSON.parse(fs.readFileSync(inputPath, 'utf8'));
  const nodes = raw.nodes;
  const edges = raw.edges;
  const layers = raw.layers;

  // Build node lookup
  const nodeMap = {};
  nodes.forEach(n => { nodeMap[n.id] = n; });

  // ---------- Filter file-to-file edges for graph analysis ----------
  // We consider edges where source and target are file: nodes,
  // of types: imports, calls, depends_on, configures, related
  const fileEdgeTypes = new Set(['imports', 'calls', 'depends_on', 'configures', 'related']);
  const fileToFileEdges = edges.filter(e =>
    e.source.startsWith('file:') &&
    e.target.startsWith('file:') &&
    fileEdgeTypes.has(e.type)
  );

  // ---------- A. Fan-In Ranking ----------
  // Count unique sources pointing TO each node (any edge type, any node type)
  const fanIn = {};
  nodes.forEach(n => { fanIn[n.id] = new Set(); });
  edges.forEach(e => {
    if (fanIn[e.target]) {
      fanIn[e.target].add(e.source);
    }
  });
  const fanInRanking = nodes.map(n => ({
    id: n.id,
    fanIn: fanIn[n.id].size,
    name: n.name
  })).sort((a, b) => b.fanIn - a.fanIn).slice(0, 20);

  // ---------- B. Fan-Out Ranking ----------
  // Count unique targets that each node points TO (any edge type, any node type)
  const fanOut = {};
  nodes.forEach(n => { fanOut[n.id] = new Set(); });
  edges.forEach(e => {
    if (fanOut[e.source]) {
      fanOut[e.source].add(e.target);
    }
  });
  const fanOutRanking = nodes.map(n => ({
    id: n.id,
    fanOut: fanOut[n.id].size,
    name: n.name
  })).sort((a, b) => b.fanOut - a.fanOut).slice(0, 20);

  // ---------- C. Entry Point Candidates ----------
  // Score each node
  const entryPointPatterns = [
    'index.ts', 'index.js', 'main.ts', 'main.js', 'app.ts', 'app.js',
    'server.ts', 'server.js', 'mod.rs', 'main.go', 'main.py', 'main.rs',
    'manage.py', 'app.py', 'wsgi.py', 'asgi.py', 'run.py', '__main__.py',
    'Application.java', 'Main.java', 'Program.cs', 'config.ru', 'index.php',
    'App.swift', 'Application.kt', 'main.cpp', 'main.c', 'lib.rs'
  ];

  // Compute fanIn sizes array for percentile calculations
  const fanInSizes = nodes.map(n => fanIn[n.id].size).sort((a, b) => a - b);
  const fanOutSizes = nodes.map(n => fanOut[n.id].size).sort((a, b) => a - b);

  function percentile(arr, p) {
    const idx = Math.floor(arr.length * p);
    return arr[Math.min(idx, arr.length - 1)];
  }

  const top10PercentFanOut = percentile(fanOutSizes, 0.9);
  const bottom25PercentFanIn = percentile(fanInSizes, 0.25);

  const entryScores = nodes.map(n => {
    let score = 0;
    const name = n.name;
    const filePath = n.filePath || '';

    if (n.type === 'file') {
      // Filename match
      if (entryPointPatterns.includes(name)) {
        score += 3;
      }
      // Root or one-level deep
      const depth = filePath.split('/').length - 1; // account for no root dir
      if (depth <= 2) {
        score += 1;
      }
      // High fan-out (top 10%)
      if (fanOut[n.id].size >= top10PercentFanOut) {
        score += 1;
      }
      // Low fan-in (bottom 25%)
      if (fanIn[n.id].size <= bottom25PercentFanIn) {
        score += 1;
      }
    }

    if (n.type === 'document') {
      if (name === 'README.md' && !filePath.includes('/')) {
        score += 5;
      } else if (name.endsWith('.md') && !filePath.includes('/')) {
        score += 2;
      }
    }

    if (n.type === 'config') {
      if (name === 'Cargo.toml' && !filePath.includes('/')) {
        score += 1; // root Cargo.toml is important
      }
    }

    return { id: n.id, score, name, summary: n.summary || '' };
  });

  entryScores.sort((a, b) => b.score - a.score);
  const topEntryCandidates = entryScores.slice(0, 5);

  // ---------- D. BFS Traversal ----------
  // Find top code entry point (skip non-file nodes)
  const codeEntryCandidates = entryScores.filter(e => {
    const node = nodeMap[e.id];
    return node && node.type === 'file';
  }).sort((a, b) => b.score - a.score);

  let bfsStartNode = null;
  if (codeEntryCandidates.length > 0) {
    bfsStartNode = codeEntryCandidates[0].id;
  }

  // Build adjacency from file-to-file imports and calls edges
  const adjacency = {};
  nodes.forEach(n => { adjacency[n.id] = []; });
  fileToFileEdges.forEach(e => {
    if (adjacency[e.source]) {
      adjacency[e.source].push(e.target);
    }
  });

  let bfsOrder = [];
  let depthMap = {};
  let byDepth = {};

  if (bfsStartNode) {
    const visited = new Set();
    const queue = [{ id: bfsStartNode, depth: 0 }];
    visited.add(bfsStartNode);

    while (queue.length > 0) {
      const current = queue.shift();
      bfsOrder.push(current.id);
      depthMap[current.id] = current.depth;
      if (!byDepth[current.depth]) byDepth[current.depth] = [];
      byDepth[current.depth].push(current.id);

      const neighbors = adjacency[current.id] || [];
      for (const neighbor of neighbors) {
        if (!visited.has(neighbor)) {
          visited.add(neighbor);
          queue.push({ id: neighbor, depth: current.depth + 1 });
        }
      }
    }
  }

  // ---------- E. Non-Code File Inventory ----------
  const nonCodeFiles = {
    documentation: [],
    infrastructure: [],
    data: [],
    config: []
  };

  nodes.forEach(n => {
    if (n.type === 'document') {
      nonCodeFiles.documentation.push({
        id: n.id,
        name: n.name,
        type: n.type,
        summary: n.summary || ''
      });
    } else if (['service', 'pipeline', 'resource'].includes(n.type)) {
      nonCodeFiles.infrastructure.push({
        id: n.id,
        name: n.name,
        type: n.type,
        summary: n.summary || ''
      });
    } else if (['table', 'schema', 'endpoint'].includes(n.type)) {
      nonCodeFiles.data.push({
        id: n.id,
        name: n.name,
        type: n.type,
        summary: n.summary || ''
      });
    } else if (n.type === 'config') {
      nonCodeFiles.config.push({
        id: n.id,
        name: n.name,
        type: n.type,
        summary: n.summary || ''
      });
    }
  });

  // ---------- F. Tightly Coupled Clusters ----------
  // Find bidirectional relationships among file nodes
  const bidirMap = new Map(); // smaller id first -> bigger id
  const pairKey = (a, b) => a < b ? a + '||' + b : b + '||' + a;

  // Build adjacency matrix using file-to-file edges
  const fileEdgeSet = {};
  fileToFileEdges.forEach(e => {
    if (!fileEdgeSet[e.source]) fileEdgeSet[e.source] = new Set();
    fileEdgeSet[e.source].add(e.target);
  });

  // Find bidirectional pairs
  const bidirPairs = [];
  const fileNodes = nodes.filter(n => n.type === 'file');
  for (let i = 0; i < fileNodes.length; i++) {
    const a = fileNodes[i].id;
    const outA = fileEdgeSet[a] || new Set();
    for (let j = i + 1; j < fileNodes.length; j++) {
      const b = fileNodes[j].id;
      const outB = fileEdgeSet[b] || new Set();
      if (outA.has(b) && outB.has(a)) {
        bidirPairs.push([a, b]);
      }
    }
  }

  // Expand clusters from bidir pairs
  let clusters_pool = bidirPairs.map(pair => new Set(pair));

  // Try to merge overlapping or connected clusters, up to 5 nodes max
  let changed = true;
  while (changed) {
    changed = false;
    const newClusters = [];
    const used = new Set();

    for (let i = 0; i < clusters_pool.length; i++) {
      if (used.has(i)) continue;
      const cluster = clusters_pool[i];
      for (let j = i + 1; j < clusters_pool.length; j++) {
        if (used.has(j)) continue;
        const other = clusters_pool[j];
        // Check if they share at least 1 node or a node from one connects to 2+ of the other
        let shouldMerge = false;
        // if they share a node
        for (const node of other) {
          if (cluster.has(node)) {
            shouldMerge = true;
            break;
          }
        }
        if (!shouldMerge) {
          // Check if any node in other connects to 2+ nodes in cluster
          for (const node of other) {
            const outEdges = fileEdgeSet[node] || new Set();
            let count = 0;
            for (const cn of cluster) {
              if (outEdges.has(cn)) count++;
              if (count >= 2) break;
            }
            if (count >= 2) {
              // Also check reverse
              let revCount = 0;
              for (const cn of cluster) {
                const cnOut = fileEdgeSet[cn] || new Set();
                if (cnOut.has(node)) revCount++;
                if (revCount >= 2) break;
              }
              if (revCount >= 2) {
                shouldMerge = true;
                break;
              }
            }
          }
        }
        if (shouldMerge) {
          for (const node of other) cluster.add(node);
          used.add(j);
          changed = true;
        }
      }
      newClusters.push(cluster);
    }
    clusters_pool = newClusters;
  }

  // Filter to clusters of size 2-5 and compute edge counts
  const clusters = clusters_pool
    .filter(c => c.size >= 2 && c.size <= 5)
    .map(c => {
      const nodes_arr = Array.from(c);
      let edgeCount = 0;
      for (let i = 0; i < nodes_arr.length; i++) {
        const outEdges = fileEdgeSet[nodes_arr[i]] || new Set();
        for (let j = 0; j < nodes_arr.length; j++) {
          if (i !== j && outEdges.has(nodes_arr[j])) {
            edgeCount++;
          }
        }
      }
      return { nodes: nodes_arr, edgeCount };
    })
    .sort((a, b) => b.edgeCount - a.edgeCount)
    .slice(0, 10);

  // ---------- G. Layer List ----------
  const layerInfo = {
    count: layers.length,
    list: layers.map(l => ({
      id: l.id,
      name: l.name,
      description: l.description
    }))
  };

  // ---------- H. Node Summary Index ----------
  const nodeSummaryIndex = {};
  nodes.forEach(n => {
    nodeSummaryIndex[n.id] = {
      name: n.name,
      type: n.type,
      summary: n.summary || ''
    };
  });

  // ---------- Assemble Output ----------
  const result = {
    scriptCompleted: true,
    entryPointCandidates: topEntryCandidates,
    fanInRanking,
    fanOutRanking,
    bfsTraversal: {
      startNode: bfsStartNode,
      order: bfsOrder,
      depthMap,
      byDepth
    },
    nonCodeFiles,
    clusters,
    layers: layerInfo,
    nodeSummaryIndex,
    totalNodes: nodes.length,
    totalEdges: edges.length
  };

  fs.writeFileSync(outputPath, JSON.stringify(result, null, 2), 'utf8');
  console.log('Analysis complete. Output written to', outputPath);
  process.exit(0);
}

main();
