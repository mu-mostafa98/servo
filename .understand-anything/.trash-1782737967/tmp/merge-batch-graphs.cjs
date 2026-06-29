#!/usr/bin/env node
// Merge batch graphs into assembled-graph.json (Node.js port of merge-batch-graphs.py)
const fs = require('fs');
const path = require('path');

const PROJECT_ROOT = process.argv[2] || 'd:\\Projects\\servo';
const INTER = path.join(PROJECT_ROOT, '.understand-anything', 'intermediate');

// Read all batch files
const allFiles = fs.readdirSync(INTER);
const batchFiles = allFiles.filter(f => /^batch-\d+(?:-part-\d+)?\.json$/.test(f));
console.error(`Merge: found ${batchFiles.length} batch files`);

// Parse all batches
const allNodes = [];
const allEdges = [];

for (const f of batchFiles) {
  const data = JSON.parse(fs.readFileSync(path.join(INTER, f), 'utf8'));
  const nodes = data.nodes || data;
  const edges = data.edges || [];
  if (Array.isArray(nodes)) {
    allNodes.push(...nodes);
  } else if (nodes && Array.isArray(nodes.nodes)) {
    allNodes.push(...nodes.nodes);
    allEdges.push(...(nodes.edges || []));
  }
  allEdges.push(...edges);
}

console.error(`Merge: read ${allNodes.length} raw nodes, ${allEdges.length} raw edges`);

// --- Normalize node IDs ---
// Strip double prefixes, project-name prefixes, add missing prefixes
const PROJECT_NAME = 'servo'; // from scan-result

function normalizeId(rawId) {
  if (!rawId) return rawId;
  let id = String(rawId);
  // Strip double prefixes like "file:file:"
  id = id.replace(/^([a-z]+):\1:/, '$1:');
  // Strip project-name prefixes like "servo-file:" -> "file:"
  id = id.replace(new RegExp(`^${PROJECT_NAME}-([a-z]+:)`), '$1');
  // If it's a raw file path without prefix, add file:
  if (!/^[a-z]+:/.test(id) && (id.includes('/') || id.includes('\\'))) {
    id = 'file:' + id;
  }
  return id;
}

// Normalize complexity values
const COMPLEXITY_MAP = {
  'low': 'simple',
  'very-low': 'simple',
  'medium': 'moderate',
  'high': 'complex',
  'very-high': 'very-complex',
  'very large': 'very-complex',
  'very-large': 'very-complex',
};

const PREFIXES = new Set(['file:', 'function:', 'class:', 'module:', 'concept:', 'config:', 'document:', 'service:', 'table:', 'endpoint:', 'pipeline:', 'schema:', 'resource:']);

function ensureFilePrefix(id) {
  if (!id) return id;
  const hasPrefix = [...PREFIXES].some(p => id.startsWith(p));
  if (!hasPrefix && (id.includes('/') || id.includes('\\'))) {
    return 'file:' + id;
  }
  return id;
}

// Apply normalization to all nodes
const normalizedNodes = new Map(); // id -> node (last occurrence wins)
const idCorrections = new Map(); // oldId -> correctedId

for (const node of allNodes) {
  const originalId = node.id;
  const correctedId = normalizeId(originalId);

  const newNode = { ...node, id: correctedId };

  // Normalize complexity
  if (newNode.complexity && COMPLEXITY_MAP[newNode.complexity]) {
    newNode.complexity = COMPLEXITY_MAP[newNode.complexity];
  }

  // Normalize filePath
  if (newNode.filePath) {
    newNode.filePath = newNode.filePath.replace(/\\/g, '/');
  }

  if (originalId !== correctedId) {
    idCorrections.set(originalId, correctedId);
  }

  normalizedNodes.set(correctedId, newNode);
}

console.error(`Merge: ${normalizedNodes.size} unique nodes after dedup`);

// Rewrite edges: apply id corrections, ensure file-prefix on source/target
const nodeIds = new Set(normalizedNodes.keys());
const correctedEdges = [];

for (const edge of allEdges) {
  let source = normalizeId(edge.source);
  let target = normalizeId(edge.target);

  // Apply id corrections
  if (idCorrections.has(source)) source = idCorrections.get(source);
  if (idCorrections.has(target)) target = idCorrections.get(target);

  // Ensure file prefix
  source = ensureFilePrefix(source);
  target = ensureFilePrefix(target);

  if (!nodeIds.has(source)) {
    console.error(`Merge: DROPPING edge — source '${source}' not in node set`);
    continue;
  }
  if (!nodeIds.has(target)) {
    console.error(`Merge: DROPPING edge — target '${target}' not in node set`);
    continue;
  }

  correctedEdges.push({
    source,
    target,
    type: edge.type || edge.relationship || 'related',
    weight: edge.weight || 0.5,
    ...(edge.metadata ? { metadata: edge.metadata } : {}),
    ...(edge.description ? { description: edge.description } : {}),
  });
}

// Deduplicate edges by (source, target, type) - keep last occurrence
const edgeKey = e => `${e.source}|${e.target}|${e.type}`;
const edgeMap = new Map();
for (const e of correctedEdges) {
  edgeMap.set(edgeKey(e), e);
}
const dedupedEdges = [...edgeMap.values()];
console.error(`Merge: ${dedupedEdges.length} edges after dedup (${correctedEdges.length - dedupedEdges.length} duplicates removed)`);

// --- tested_by linker ---
// Pass 1: fix inversion (test -> production should be production -> test)
// Pass 2: supplement with path-convention pairings
const testPatterns = [/test/, /spec/, /_test\./, /\.test\./, /__tests__/];
const isTestPath = (nodeId) => testPatterns.some(p => p.test(nodeId));

const finalEdges = [];
const testedByEdges = [];

for (const e of dedupedEdges) {
  if (e.type === 'tested_by') {
    const sourceIsTest = isTestPath(e.source);
    const targetIsTest = isTestPath(e.target);

    if (sourceIsTest && !targetIsTest) {
      // Already correct: test -> production. Flip to production -> test
      finalEdges.push({ ...e, source: e.target, target: e.source });
      testedByEdges.push({ source: e.target, target: e.source });
      console.error(`Merge: FLIPPED tested_by edge ${e.source} -> ${e.target}`);
    } else if (!sourceIsTest && targetIsTest) {
      // Correct orientation: production -> test
      finalEdges.push(e);
      testedByEdges.push(e);
    } else {
      console.error(`Merge: DROPPED tested_by edge ${e.source} -> ${e.target} (both or neither are test paths)`);
    }
  } else {
    finalEdges.push(e);
  }
}

// Pass 2: supplement with path-convention pairings
// Look for production files with matching test files — be precise to avoid false matches
const prodNodes = [...normalizedNodes.values()].filter(n =>
  !isTestPath(n.id) && n.filePath && !n.filePath.startsWith('.')
);
const testNodes = [...normalizedNodes.values()].filter(n =>
  isTestPath(n.id) && n.filePath
);

for (const prod of prodNodes) {
  const prodBase = path.basename(prod.filePath || '', path.extname(prod.filePath || ''));
  const prodDir = path.dirname(prod.filePath || '');
  for (const test of testNodes) {
    const testBase = path.basename(test.filePath || '', path.extname(test.filePath || ''));
    const testDir = path.dirname(test.filePath || '');

    // foo.rs ↔ foo_test.rs or foo.test.rs (same directory, test suffix)
    const sameDirSuffix = prodDir === testDir &&
      (testBase === prodBase + '_test' || testBase === prodBase + '.test' || testBase === prodBase + '_spec');

    // foo/mod.rs ↔ foo/tests.rs or foo/test.rs (module-level test file)
    const modVsTest = prodBase === 'mod' && (testBase === 'tests' || testBase === 'test') && prodDir === testDir;

    // tests/foo.rs ↔ foo.rs (test in tests/test subdirectory)
    const testInSubdir = testBase === prodBase && (
      testDir === prodDir + '/tests' || testDir === prodDir + '/test' ||
      testDir.replace(/\/tests$/, '') === prodDir ||
      testDir.replace(/\/test$/, '') === prodDir
    );

    if ((sameDirSuffix || modVsTest || testInSubdir) && !testedByEdges.some(e => e.source === prod.id && e.target === test.id)) {
      finalEdges.push({
        source: prod.id,
        target: test.id,
        type: 'tested_by',
        weight: 0.5,
      });
      testedByEdges.push({ source: prod.id, target: test.id });
    }
  }
}

console.error(`Merge: ${finalEdges.length} total edges after tested_by linker`);

// Mark tested nodes
const testedNodeIds = new Set(testedByEdges.map(e => e.source));
for (const node of normalizedNodes.values()) {
  if (testedNodeIds.has(node.id)) {
    if (!node.tags) node.tags = [];
    if (!node.tags.includes('tested')) node.tags.push('tested');
  }
}

// Write assembled graph
const assembled = {
  nodes: [...normalizedNodes.values()],
  edges: finalEdges,
};

fs.writeFileSync(path.join(INTER, 'assembled-graph.json'), JSON.stringify(assembled, null, 2));
console.error(`Merge: written assembled-graph.json with ${assembled.nodes.length} nodes and ${assembled.edges.length} edges`);
console.log('Merge complete.');
process.exit(0);
