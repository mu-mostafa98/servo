const fs = require('fs');

const extResults = JSON.parse(fs.readFileSync(
  'd:/Projects/servo/.understand-anything/tmp/ua-file-extract-results-52.json', 'utf8'));

const nodes = [];
const edges = [];

function fileName(p) { return p.split('/').pop(); }
function makeFileId(p) { return 'file:' + p; }
function makeFuncId(p, f) { return 'function:' + p + ':' + f; }
function makeClassId(p, c) { return 'class:' + p + ':' + c; }

const EDGE_WEIGHTS = {
  contains: 1.0, imports: 0.7, calls: 0.8, inherits: 0.9,
  implements: 0.9, exports: 0.8, depends_on: 0.6, tested_by: 0.5,
  configures: 0.6, documents: 0.5, deploys: 0.7, migrates: 0.7,
  triggers: 0.6, defines_schema: 0.8, serves: 0.7, provisions: 0.7,
  routes: 0.6, related: 0.5
};

function addEdge(s, t, type) {
  edges.push({ source: s, target: t, type: type, direction: 'forward', weight: EDGE_WEIGHTS[type] || 0.5 });
}

const fileMeta = {
  'children_mutation.rs': {
    summary: 'Defines the ChildrenMutation enum encoding DOM child list mutation types (Append, Insert, Prepend, Replace, ReplaceAll, ChangeText) and associated helper functions for executing mutations and tracking modified edge elements.',
    tags: ['dom', 'mutation', 'children'], complexity: 'moderate'
  },
  'context.rs': {
    summary: 'Defines context structs (BindContext, UnbindContext, MoveContext) that carry parent, sibling, index, and tree-connection state during DOM node bind/unbind/move operations.',
    tags: ['dom', 'context', 'tree-operations'], complexity: 'moderate'
  },
  'focus.rs': {
    summary: 'Implements focus navigation scope management, including sequential focus navigation, focus delegation for shadow DOM, and the focusing steps algorithm as specified by the HTML standard.',
    tags: ['dom', 'focus', 'accessibility', 'navigation'], complexity: 'complex'
  },
  'iterators.rs': {
    summary: 'Provides DOM tree traversal iterators for following, preceding, and depth-first tree navigation with shadow-including variants, supporting both rooted and unrooted (GC-safe) modes.',
    tags: ['dom', 'iterators', 'traversal', 'tree'], complexity: 'complex'
  },
  'layout_dom.rs': {
    summary: 'Provides layout-related query methods on Node, exposing children, siblings, layout data, and element-type metadata needed by the Servo layout engine for rendering calculations.',
    tags: ['dom', 'layout', 'rendering'], complexity: 'complex'
  },
  'mod.rs': {
    summary: 'Module barrel file that publicly re-exports all submodules of the DOM node package, providing unified access to children_mutation, context, focus, iterators, layout_dom, node, nodeiterator, nodelist, treewalker, and virtualmethods.',
    tags: ['entry-point', 'barrel', 'module'], complexity: 'simple',
    languageNotes: 'Rust module barrel using pub mod declarations to expose submodules.'
  },
  'node.rs': {
    summary: 'Core Node implementation for Servo DOM, providing parent/child/sibling tree management, mutation operations (insert, remove, replace, adopt, clone), tree traversal, query selector evaluation, DOM attribute accessors, and WebIDL-bound methods for the full Node interface.',
    tags: ['dom', 'node', 'core', 'tree', 'api-handler'], complexity: 'complex',
    languageNotes: 'Extensive Rust DOM node implementation with 200+ methods covering tree traversal, mutation algorithms, and script-layout integration.'
  },
  'nodeiterator.rs': {
    summary: 'Implements the DOM NodeIterator interface for traversing a subtree and filtering nodes by type and custom filter functions, with forward and backward iteration.',
    tags: ['dom', 'iterator', 'traversal', 'filter'], complexity: 'moderate'
  },
  'nodelist.rs': {
    summary: 'Implements NodeList and its variants (ChildrenList, LabelsList, RadioList, ElementsByNameList) for DOM node collection management with live and static list types.',
    tags: ['dom', 'nodelist', 'collection', 'children'], complexity: 'moderate'
  },
  'treewalker.rs': {
    summary: 'Implements the DOM TreeWalker interface for filtered depth-first traversal of a DOM subtree, providing parent, child, sibling, and sequential node navigation methods.',
    tags: ['dom', 'traversal', 'treewalker', 'filter'], complexity: 'complex'
  },
  'virtualmethods.rs': {
    summary: 'Defines the VirtualMethods trait and its dispatch mechanism that enables DOM node subclasses to hook into lifecycle events (bind, unbind, children_changed, attribute_mutated, cloning, adopting) without requiring virtual dispatch on the Node base type.',
    tags: ['dom', 'virtual-methods', 'trait', 'lifecycle'], complexity: 'complex',
    languageNotes: 'Uses a vtable-based dynamic dispatch approach in Rust, matching node type IDs to trait method implementations for lifecycle hooks.'
  }
};

// Process each file
for (const result of extResults.results) {
  const fp = result.path;
  const fname = fileName(fp);
  const meta = fileMeta[fname];
  const funcs = result.functions || [];
  const classes = result.classes || [];
  const exports = result.exports || [];

  const exportedFuncNames = new Set();
  const exportedClassNames = new Set();
  for (const e of exports) {
    if (funcs.some(f => f.name === e.name)) exportedFuncNames.add(e.name);
    if (classes.some(c => c.name === e.name)) exportedClassNames.add(e.name);
  }

  // File node
  const fileNode = { id: makeFileId(fp), type: 'file', name: fname, filePath: fp, summary: meta.summary, tags: meta.tags, complexity: meta.complexity };
  if (meta.languageNotes) fileNode.languageNotes = meta.languageNotes;
  nodes.push(fileNode);

  // Track duplicate function names
  const funcCount = {};

  // Significant functions
  for (const func of funcs) {
    const lc = func.endLine - func.startLine + 1;
    const isExp = exportedFuncNames.has(func.name);
    if (!isExp && lc < 10) continue;

    funcCount[func.name] = (funcCount[func.name] || 0) + 1;
    let disp = func.name;
    if (funcCount[func.name] > 1) disp = func.name + '_' + funcCount[func.name];

    const fid = makeFuncId(fp, disp);
    nodes.push({
      id: fid, type: 'function', name: disp, filePath: fp, lineRange: [func.startLine, func.endLine],
      summary: func.name + ' in ' + fname + (func.params.length ? ' with params [' + func.params.join(', ') + ']' : ''),
      tags: ['dom', 'function'], complexity: lc < 20 ? 'simple' : lc < 100 ? 'moderate' : 'complex'
    });
    addEdge(makeFileId(fp), fid, 'contains');
    if (isExp) addEdge(makeFileId(fp), fid, 'exports');
  }

  // Significant classes
  for (const cls of classes) {
    const lc = cls.endLine - cls.startLine + 1;
    const mc = (cls.methods || []).length;
    const isExp = exportedClassNames.has(cls.name);
    if (!isExp && mc < 2 && lc < 20) continue;

    const cid = makeClassId(fp, cls.name);
    nodes.push({
      id: cid, type: 'class', name: cls.name, filePath: fp, lineRange: [cls.startLine, cls.endLine],
      summary: cls.name + ' in ' + fname + ' with ' + mc + ' methods' + ((cls.properties || []).length ? ' and properties [' + (cls.properties || []).join(', ') + ']' : ''),
      tags: ['dom', 'struct'], complexity: mc >= 10 || lc >= 100 ? 'complex' : mc >= 2 || lc >= 20 ? 'moderate' : 'simple'
    });
    addEdge(makeFileId(fp), cid, 'contains');
    if (isExp) addEdge(makeFileId(fp), cid, 'exports');
  }
}

// Import edges
const importData = {
  'components/script/dom/node/children_mutation.rs': [],
  'components/script/dom/node/context.rs': [],
  'components/script/dom/node/focus.rs': [],
  'components/script/dom/node/iterators.rs': [],
  'components/script/dom/node/layout_dom.rs': [],
  'components/script/dom/node/mod.rs': [
    'components/script/dom/node/children_mutation.rs',
    'components/script/dom/node/context.rs',
    'components/script/dom/node/focus.rs',
    'components/script/dom/node/iterators.rs',
    'components/script/dom/node/layout_dom.rs',
    'components/script/dom/node/node.rs',
    'components/script/dom/node/nodeiterator.rs',
    'components/script/dom/node/nodelist.rs',
    'components/script/dom/node/treewalker.rs',
    'components/script/dom/node/virtualmethods.rs'
  ],
  'components/script/dom/node/node.rs': [],
  'components/script/dom/node/nodeiterator.rs': [],
  'components/script/dom/node/nodelist.rs': [],
  'components/script/dom/node/treewalker.rs': [],
  'components/script/dom/node/virtualmethods.rs': []
};

for (const [fp, imps] of Object.entries(importData)) {
  for (const imp of imps) addEdge(makeFileId(fp), makeFileId(imp), 'imports');
}
// Cross-batch import from mod.rs to parent module
addEdge(makeFileId('components/script/dom/node/mod.rs'), makeFileId('components/script/dom/mod.rs'), 'imports');

// Deduplicate edges
const edgeMap = new Map();
for (const e of edges) {
  const k = e.source + '|' + e.target + '|' + e.type;
  if (!edgeMap.has(k)) edgeMap.set(k, e);
}
const dedupedEdges = Array.from(edgeMap.values());

// Deduplicate nodes
const nodeMap = new Map();
for (const n of nodes) { nodeMap.set(n.id, n); }
const dedupedNodes = Array.from(nodeMap.values());

console.log('Total: ' + dedupedNodes.length + ' nodes, ' + dedupedEdges.length + ' edges');

// Verify import count
const importEdges = dedupedEdges.filter(e => e.type === 'imports');
console.log('Import edges: ' + importEdges.length + ' (expected 11)');

// Import edge self-check
let totalExpectedImports = 0;
for (const imps of Object.values(importData)) totalExpectedImports += imps.length;
totalExpectedImports += 1; // cross-batch
console.log('Import edges: ' + importEdges.length + ' (expected ' + totalExpectedImports + ')');
if (importEdges.length !== totalExpectedImports) {
  console.error('ERROR: Import edge count mismatch!');
  process.exit(1);
}

// Partition: sort file paths alphabetically
const allFilePaths = Object.keys(importData).sort();
const parts = Math.ceil(Math.max(dedupedNodes.length / 60, dedupedEdges.length / 120));
console.log('Splitting into ' + parts + ' parts');

const chunkSize = Math.ceil(allFilePaths.length / parts);

for (let k = 0; k < parts; k++) {
  const partFiles = allFilePaths.slice(k * chunkSize, (k + 1) * chunkSize);
  const partNodeIds = new Set();
  const partNodes = [];

  // Add file nodes and all sub-file nodes for this part's files
  for (const n of dedupedNodes) {
    if (n.filePath && partFiles.includes(n.filePath)) {
      partNodeIds.add(n.id);
      partNodes.push(n);
    }
  }

  // Add edges whose source is in this part
  const partEdges = [];
  for (const e of dedupedEdges) {
    if (partNodeIds.has(e.source)) partEdges.push(e);
  }

  const partNum = k + 1;
  const fn = 'd:/Projects/servo/.understand-anything/intermediate/batch-52-part-' + partNum + '.json';
  fs.writeFileSync(fn, JSON.stringify({ nodes: partNodes, edges: partEdges }));

  // Validate
  const pnSet = new Set(partNodes.map(n => n.id));
  let badEdges = 0;
  let missingSources = [];
  for (const e of partEdges) {
    if (!pnSet.has(e.source)) {
      missingSources.push(e.source);
      badEdges++;
    }
  }

  // Check duplicates
  const idCounts = {};
  for (const n of partNodes) idCounts[n.id] = (idCounts[n.id] || 0) + 1;
  const dupes = Object.entries(idCounts).filter(([_, c]) => c > 1).length;

  if (badEdges > 0) {
    console.log('Part ' + partNum + ': ' + partNodes.length + ' nodes, ' + partEdges.length + ' edges, ' + dupes + ' dupes, ' + badEdges + ' bad edges');
    console.log('  Missing sources: ' + missingSources.slice(0, 5).join(', '));
  } else {
    console.log('Part ' + partNum + ': ' + partNodes.length + ' nodes, ' + partEdges.length + ' edges, ' + dupes + ' dupes, OK');
  }
}

console.log('Done.');
