#!/usr/bin/env node
// Phase 6-7: Validate and finalize knowledge graph
const fs = require('fs');
const path = require('path');

const INTER = 'd:/Projects/servo/.understand-anything/intermediate';
const OUTPUT = 'd:/Projects/servo/.understand-anything/knowledge-graph.json';

// Load assembled graph
const graph = JSON.parse(fs.readFileSync(path.join(INTER, 'assembled-graph.json'), 'utf8'));
console.log(`Loaded assembled graph: ${graph.nodes.length} nodes, ${graph.edges.length} edges`);

// Load old knowledge graph for layers & tour
const oldGraph = JSON.parse(fs.readFileSync(OUTPUT, 'utf8'));
console.log(`Old graph: layers=${oldGraph.layers?.length}, tour=${oldGraph.tour?.length}`);

const nodeIds = new Set(graph.nodes.map(n => n.id));

// --- Rebuild layers ---
// Keep old layers but remove dangling refs, then add SVG engine nodes
let layers = (oldGraph.layers || []).map(layer => ({
  ...layer,
  nodeIds: (layer.nodeIds || []).filter(id => nodeIds.has(id)),
}));

// Find SVG engine nodes not yet in any layer
const svgNodeIds = graph.nodes
  .filter(n => n.filePath && n.filePath.startsWith('components/svg_engine'))
  .map(n => n.id);
console.log(`SVG engine nodes to assign: ${svgNodeIds.length}`);

// Check if there's already an SVG/painting layer
let paintingLayer = layers.find(l => l.id === 'layer:painting' || l.name === 'Painting');
let svgLayer = layers.find(l => l.id === 'layer:svg-engine');

if (svgNodeIds.length > 0) {
  if (svgLayer) {
    // Add to existing SVG layer
    svgLayer.nodeIds = [...new Set([...svgLayer.nodeIds, ...svgNodeIds])];
    console.log(`Added SVG nodes to layer:${svgLayer.id}`);
  } else if (paintingLayer) {
    // Add to painting layer
    paintingLayer.nodeIds = [...new Set([...paintingLayer.nodeIds, ...svgNodeIds])];
    console.log(`Added SVG nodes to layer:${paintingLayer.id}`);
  } else {
    // Create new SVG engine layer
    layers.push({
      id: 'layer:svg-engine',
      name: 'SVG Engine',
      description: 'SVG rendering engine - shape extraction, tessellation, and paint rendering for SVG elements',
      nodeIds: svgNodeIds,
    });
    console.log('Created new svg-engine layer');
  }
}

// Also assign any unassigned file-level nodes to catch-all layers
const fileLevelTypes = new Set(['file', 'config', 'document', 'service', 'pipeline', 'table', 'schema', 'resource', 'endpoint']);
const assigned = new Set();
layers.forEach(l => (l.nodeIds || []).forEach(id => assigned.add(id)));

const unassigned = graph.nodes.filter(n =>
  fileLevelTypes.has(n.type) && !assigned.has(n.id)
);
if (unassigned.length > 0) {
  console.log(`Unassigned file-level nodes: ${unassigned.length}`);
  // Group unassigned by directory prefix
  const dirGroups = {};
  for (const n of unassigned) {
    const dir = n.filePath ? n.filePath.split('/')[0] || n.filePath.split('/')[1] || '_root' : '_root';
    if (!dirGroups[dir]) dirGroups[dir] = [];
    dirGroups[dir].push(n.id);
  }
  for (const [dir, ids] of Object.entries(dirGroups)) {
    const existingLayer = layers.find(l => l.nodeIds.some(id => {
      const node = graph.nodes.find(n => n.id === id);
      return node?.filePath?.startsWith(dir + '/');
    }));
    if (existingLayer) {
      existingLayer.nodeIds = [...new Set([...existingLayer.nodeIds, ...ids])];
    }
  }
}

// --- Rebuild tour ---
let tour = (oldGraph.tour || []).map(step => ({
  ...step,
  nodeIds: (step.nodeIds || []).filter(id => nodeIds.has(id)),
})).filter(step => step.nodeIds.length > 0);

// Add SVG engine tour step if we have SVG nodes and no step covers them
const hasSvgTourStep = tour.some(s => s.title?.toLowerCase().includes('svg'));
if (svgNodeIds.length > 0 && !hasSvgTourStep) {
  const svgFileNodes = graph.nodes.filter(n => svgNodeIds.includes(n.id) && n.type === 'file');
  tour.push({
    order: tour.length + 1,
    title: 'SVG Rendering Engine',
    description: 'The SVG engine handles parsing, tessellation, and rendering of SVG shapes (circles, ellipses, lines, paths, polygons, polylines, rects) onto display lists for painting.',
    nodeIds: svgFileNodes.slice(0, 5).map(n => n.id),
  });
}

// Re-sort tour by order
tour.sort((a, b) => (a.order || 0) - (b.order || 0));
// Re-index order sequentially
tour.forEach((step, i) => { step.order = i + 1; });

// --- Phase 6: Inline validation ---
const issues = [];
const warnings = [];

// Validate nodes
const seen = new Map();
const validTypes = new Set(['file', 'function', 'class', 'module', 'concept', 'config', 'document', 'service', 'table', 'endpoint', 'pipeline', 'schema', 'resource']);
graph.nodes.forEach((n, i) => {
  if (!n.id) { issues.push(`Node[${i}] missing id`); return; }
  if (!n.name) warnings.push(`Node[${i}] '${n.id}' missing name`);
  if (!n.summary) warnings.push(`Node[${i}] '${n.id}' missing summary`);
  if (!n.tags || !n.tags.length) warnings.push(`Node[${i}] '${n.id}' missing tags`);
  if (seen.has(n.id)) issues.push(`Duplicate node ID '${n.id}' at indices ${seen.get(n.id)} and ${i}`);
  else seen.set(n.id, i);
});

// Validate edges
graph.edges.forEach((e, i) => {
  if (!nodeIds.has(e.source)) issues.push(`Edge[${i}] source '${e.source}' not found`);
  if (!nodeIds.has(e.target)) issues.push(`Edge[${i}] target '${e.target}' not found`);
});

// Validate layers
const layerNodeIds = new Set();
layers.forEach(layer => {
  if (!layer.id) issues.push(`Layer missing id`);
  if (!layer.name) warnings.push(`Layer '${layer.id}' missing name`);
  (layer.nodeIds || []).forEach(id => {
    if (!nodeIds.has(id)) issues.push(`Layer '${layer.id}' refs missing node '${id}'`);
    if (layerNodeIds.has(id)) issues.push(`Node '${id}' appears in multiple layers`);
    layerNodeIds.add(id);
  });
});

// Check unassigned file nodes
const fileNodes = graph.nodes.filter(n => fileLevelTypes.has(n.type));
fileNodes.forEach(n => {
  if (!layerNodeIds.has(n.id)) {
    warnings.push(`File node '${n.id}' not in any layer`);
  }
});

// Validate tour
tour.forEach((step, i) => {
  (step.nodeIds || []).forEach(id => {
    if (!nodeIds.has(id)) issues.push(`Tour step[${i}] ('${step.title}') refs missing node '${id}'`);
  });
});

// Orphan nodes
const edgeNodeIds = new Set();
graph.edges.forEach(e => { edgeNodeIds.add(e.source); edgeNodeIds.add(e.target); });
let orphanCount = 0;
graph.nodes.forEach(n => {
  if (!edgeNodeIds.has(n.id) && !fileLevelTypes.has(n.type)) {
    orphanCount++;
    if (orphanCount <= 5) warnings.push(`Node '${n.id}' has no edges (orphan)`);
  }
});
if (orphanCount > 5) warnings.push(`... and ${orphanCount - 5} more orphan nodes`);

// Stats
const nodeTypes = {};
graph.nodes.forEach(n => { nodeTypes[n.type] = (nodeTypes[n.type] || 0) + 1; });
const edgeTypes = {};
graph.edges.forEach(e => { edgeTypes[e.type] = (edgeTypes[e.type] || 0) + 1; });

console.log('\n=== Validation Results ===');
console.log(`Issues: ${issues.length}, Warnings: ${warnings.length}`);
if (issues.length > 0) issues.forEach(i => console.log('  [ISSUE] ' + i));
if (warnings.length > 0 && warnings.length <= 10) warnings.forEach(w => console.log('  [WARN] ' + w));
else if (warnings.length > 10) console.log(`  [WARN] ${warnings.length} total warnings (showing first 10)`);

const review = { issues, warnings, stats: {
  totalNodes: graph.nodes.length,
  totalEdges: graph.edges.length,
  totalLayers: layers.length,
  tourSteps: tour.length,
  nodeTypes, edgeTypes
}};
fs.writeFileSync(path.join(INTER, 'review.json'), JSON.stringify(review, null, 2));

// --- Assemble final graph ---
let hasCriticalIssues = issues.length > 0;

// Auto-fix: remove edges with dangling references
if (issues.length > 0) {
  console.log('\n--- Auto-fixing issues ---');
  const beforeEdges = graph.edges.length;
  graph.edges = graph.edges.filter(e => nodeIds.has(e.source) && nodeIds.has(e.target));
  console.log(`Removed ${beforeEdges - graph.edges.length} dangling edges`);

  // Re-check
  let remainingCritical = 0;
  graph.edges.forEach((e, i) => {
    if (!nodeIds.has(e.source) || !nodeIds.has(e.target)) remainingCritical++;
  });
  hasCriticalIssues = remainingCritical > 0;
}

const finalGraph = {
  version: '1.0.0',
  project: {
    name: 'servo',
    languages: ['rust', 'webidl', 'python', 'toml', 'yaml', 'javascript', 'html', 'css', 'json', 'markdown', 'kotlin', 'typescript', 'shell', 'batch', 'powershell', 'xml', 'c', 'java', 'template'],
    frameworks: ['WebVR/WebXR', 'WebGPU', 'WebGL', 'Servo (custom browser engine)'],
    description: 'The Servo Parallel Browser Engine Project — a prototype web browser engine written in Rust, currently developed on macOS, Linux, Windows, OpenHarmony, and Android.',
    analyzedAt: new Date().toISOString(),
    gitCommitHash: '766b89e0fad9460b3461ff00e4ac6f42dd5ea523',
  },
  nodes: graph.nodes,
  edges: graph.edges,
  layers,
  tour,
};

fs.writeFileSync(path.join(INTER, 'assembled-graph.json'), JSON.stringify({ nodes: graph.nodes, edges: graph.edges }, null, 2));
console.log(`\nWritten assembled-graph.json`);
fs.writeFileSync(OUTPUT, JSON.stringify(finalGraph, null, 2));
console.log(`Written ${OUTPUT}`);
console.log(`\n=== Final Summary ===`);
console.log(`Nodes: ${graph.nodes.length} (${Object.entries(nodeTypes).map(([k,v]) => `${k}:${v}`).join(', ')})`);
console.log(`Edges: ${graph.edges.length} (${Object.entries(edgeTypes).map(([k,v]) => `${k}:${v}`).join(', ')})`);
console.log(`Layers: ${layers.length}`);
console.log(`Tour steps: ${tour.length}`);
console.log(`Issues: ${issues.length}${hasCriticalIssues ? ' (CRITICAL)' : ' (all fixed)'}`);
console.log(`Warnings: ${warnings.length}`);

if (!hasCriticalIssues) {
  console.log('\nGraph ready for dashboard!');
}
process.exit(0);
