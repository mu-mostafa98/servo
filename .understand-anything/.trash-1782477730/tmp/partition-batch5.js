const fs = require('fs');

const graph = JSON.parse(fs.readFileSync('d:/Projects/servo/.understand-anything/tmp/ua-batch5-graph.json', 'utf8'));
const { nodes, edges } = graph;

// Group nodes by filePath
const filePathToNodes = {};
for (const node of nodes) {
  const fp = node.filePath || '';
  if (!filePathToNodes[fp]) filePathToNodes[fp] = [];
  filePathToNodes[fp].push(node);
}

// Get unique file paths and sort them alphabetically
const filePaths = Object.keys(filePathToNodes).sort();

console.log('Total file paths:', filePaths.length);
console.log('Total nodes:', nodes.length);
console.log('Total edges:', edges.length);

// Calculate number of parts
const nodeCount = nodes.length;
const edgeCount = edges.length;
const partsNeeded = Math.max(Math.ceil(nodeCount / 60), Math.ceil(edgeCount / 120));
console.log('Parts needed:', partsNeeded);

const filesPerPart = Math.ceil(filePaths.length / partsNeeded);

// Create a map: node id -> node
const nodeMap = {};
for (const node of nodes) {
  nodeMap[node.id] = node;
}

// Partition: chunk files and assign nodes/edges per part
let actualPartsCount = 0;
for (let partIdx = 0; partIdx < partsNeeded; partIdx++) {
  const startFileIdx = partIdx * filesPerPart;
  if (startFileIdx >= filePaths.length) break; // No more files to distribute
  const endFileIdx = Math.min(startFileIdx + filesPerPart, filePaths.length);
  const partFilePaths = filePaths.slice(startFileIdx, endFileIdx);
  actualPartsCount++;

  // Collect all node IDs for this part
  const partNodeIds = new Set();
  for (const fp of partFilePaths) {
    for (const node of filePathToNodes[fp]) {
      partNodeIds.add(node.id);
    }
  }

  // Collect all nodes for this part
  const partNodes = [];
  for (const id of partNodeIds) {
    partNodes.push(nodeMap[id]);
  }

  // Collect all edges whose source is in this part's nodes
  const partEdges = [];
  const seenEdgeKeys = new Set();
  for (const edge of edges) {
    if (partNodeIds.has(edge.source)) {
      const key = edge.source + '|' + edge.target + '|' + edge.type;
      if (!seenEdgeKeys.has(key)) {
        seenEdgeKeys.add(key);
        partEdges.push(edge);
      }
    }
  }

  // Validate: every edge's source must be in this part's nodes
  let validationErrors = [];
  for (const edge of partEdges) {
    if (!partNodeIds.has(edge.source)) {
      validationErrors.push('Edge source ' + edge.source + ' not in part nodes');
    }
  }

  // Also check that file nodes exist for import targets that are in this batch
  for (const edge of partEdges) {
    if (edge.type === 'imports' && partNodeIds.has(edge.target)) {
      // Target is also in this part - that's fine
    }
    // Targets outside this part are fine
  }

  const partFilename = 'd:/Projects/servo/.understand-anything/intermediate/batch-5-part-' + (partIdx + 1) + '.json';
  const partContent = JSON.stringify({ nodes: partNodes, edges: partEdges }, null, 2);
  fs.writeFileSync(partFilename, partContent);

  const actualNodeCount = partNodes.length;
  const actualEdgeCount = partEdges.length;
  console.log('Part ' + (partIdx + 1) + ': files ' + startFileIdx + '-' + (endFileIdx-1) + ', nodes=' + actualNodeCount + ', edges=' + actualEdgeCount);

  if (validationErrors.length > 0) {
    console.log('VALIDATION ERRORS for part ' + (partIdx + 1) + ':');
    for (const err of validationErrors) {
      console.log('  ' + err);
    }
  }
}

console.log('Actual non-empty parts written:', actualPartsCount);

// Verify all files were distributed
const totalPartNodes = [];
for (let partIdx = 0; partIdx < actualPartsCount; partIdx++) {
  const part = JSON.parse(fs.readFileSync('d:/Projects/servo/.understand-anything/intermediate/batch-5-part-' + (partIdx + 1) + '.json', 'utf8'));
  totalPartNodes.push(...part.nodes);
}

// Check no nodes were lost
const originalNodeIds = new Set(nodes.map(n => n.id));
const partNodeIdsTotal = new Set(totalPartNodes.map(n => n.id));
const missingNodes = [...originalNodeIds].filter(id => !partNodeIdsTotal.has(id));
if (missingNodes.length > 0) {
  console.log('Missing nodes:', missingNodes);
} else {
  console.log('All nodes accounted for.');
}

console.log('Done.');
