const fs = require('fs');
const path = require('path');

const extResults = JSON.parse(fs.readFileSync(
  'd:/Projects/servo/.understand-anything/tmp/ua-file-extract-results-52.json', 'utf8'));

const nodes = [];
const edges = [];

// Helper to get a short name from path
function fileName(p) {
  return p.split('/').pop();
}

// Tag helpers
function isExportedFunc(funcName, exports, funcs) {
  return exports.some(e => e.name === funcName &&
    funcs.some(f => f.name === funcName));
}
function isExportedClass(className, exports, classes) {
  return exports.some(e => e.name === className &&
    classes.some(c => c.name === className));
}

function makeFuncId(filePath, funcName) {
  return `function:${filePath}:${funcName}`;
}
function makeClassId(filePath, className) {
  return `class:${filePath}:${className}`;
}
function makeFileId(filePath) {
  return `file:${filePath}`;
}

// Edge types with their fixed weights
const EDGE_WEIGHTS = {
  contains: 1.0,
  imports: 0.7,
  calls: 0.8,
  inherits: 0.9,
  implements: 0.9,
  exports: 0.8,
  depends_on: 0.6,
  tested_by: 0.5,
  configures: 0.6,
  documents: 0.5,
  deploys: 0.7,
  migrates: 0.7,
  triggers: 0.6,
  defines_schema: 0.8,
  serves: 0.7,
  provisions: 0.7,
  routes: 0.6,
  related: 0.5
};

function addEdge(source, target, type) {
  edges.push({
    source,
    target,
    type,
    direction: 'forward',
    weight: EDGE_WEIGHTS[type] || 0.5
  });
}

// File-level summaries and tags
const fileMetadata = {
  'components/script/dom/node/children_mutation.rs': {
    summary: 'Defines the ChildrenMutation enum encoding DOM child list mutation types (Append, Insert, Prepend, Replace, ReplaceAll, ChangeText) and associated helper functions for executing mutations and tracking modified edge elements.',
    tags: ['dom', 'mutation', 'children'],
    complexity: 'moderate'
  },
  'components/script/dom/node/context.rs': {
    summary: 'Defines context structs (BindContext, UnbindContext, MoveContext) that carry parent, sibling, index, and tree-connection state during DOM node bind/unbind/move operations.',
    tags: ['dom', 'context', 'tree-operations'],
    complexity: 'moderate'
  },
  'components/script/dom/node/focus.rs': {
    summary: 'Implements focus navigation scope management, including sequential focus navigation, focus delegation for shadow DOM, and the focusing steps algorithm as specified by the HTML standard.',
    tags: ['dom', 'focus', 'accessibility', 'navigation'],
    complexity: 'complex'
  },
  'components/script/dom/node/iterators.rs': {
    summary: 'Provides DOM tree traversal iterators for following, preceding, and depth-first tree navigation with shadow-including variants, supporting both rooted and unrooted (GC-safe) modes.',
    tags: ['dom', 'iterators', 'traversal', 'tree'],
    complexity: 'complex'
  },
  'components/script/dom/node/layout_dom.rs': {
    summary: 'Provides layout-related query methods on Node, exposing children, siblings, layout data, and element-type metadata needed by the Servo layout engine for rendering calculations.',
    tags: ['dom', 'layout', 'rendering'],
    complexity: 'complex'
  },
  'components/script/dom/node/mod.rs': {
    summary: 'Module barrel file that publicly re-exports all submodules of the DOM node package: children_mutation, context, focus, iterators, layout_dom, node, nodeiterator, nodelist, treewalker, and virtualmethods.',
    tags: ['entry-point', 'barrel', 'module'],
    complexity: 'simple',
    languageNotes: 'Rust module barrel using `pub mod` declarations to expose submodules.'
  },
  'components/script/dom/node/node.rs': {
    summary: 'Core Node implementation for Servo\'s DOM, providing parent/child/sibling tree management, mutation operations (insert, remove, replace, adopt, clone), tree traversal, query selector evaluation, DOM attribute accessors, and WebIDL-bound methods for all Node interface properties.',
    tags: ['dom', 'node', 'core', 'tree', 'api-handler'],
    complexity: 'complex',
    languageNotes: 'Extensive Rust DOM node implementation with 200+ methods covering the full Node WebIDL interface, tree traversal, mutation algorithms, and script-layout integration.'
  },
  'components/script/dom/node/nodeiterator.rs': {
    summary: 'Implements the DOM NodeIterator interface for traversing a subtree and filtering nodes by type and custom filter functions, with forward and backward iteration.',
    tags: ['dom', 'iterator', 'traversal', 'filter'],
    complexity: 'moderate'
  },
  'components/script/dom/node/nodelist.rs': {
    summary: 'Implements NodeList and its variants (ChildrenList, LabelsList, RadioList, ElementsByNameList) for DOM node collection management with live and static list types.',
    tags: ['dom', 'nodelist', 'collection', 'children'],
    complexity: 'moderate'
  },
  'components/script/dom/node/treewalker.rs': {
    summary: 'Implements the DOM TreeWalker interface for filtered depth-first traversal of a DOM subtree, providing parent, child, sibling, and sequential node navigation methods.',
    tags: ['dom', 'traversal', 'treewalker', 'filter'],
    complexity: 'complex'
  },
  'components/script/dom/node/virtualmethods.rs': {
    summary: 'Defines the VirtualMethods trait and its dispatch mechanism that enables DOM node subclasses to hook into lifecycle events (bind, unbind, children_changed, attribute_mutated, cloning, adopting) without requiring virtual dispatch on the Node base type.',
    tags: ['dom', 'virtual-methods', 'trait', 'lifecycle'],
    complexity: 'complex',
    languageNotes: 'Uses a vtable-based dynamic dispatch approach in Rust, matching node type IDs to trait method implementations for lifecycle hooks.'
  }
};

// Process each file
for (const result of extResults.results) {
  const fp = result.path;
  const meta = fileMetadata[fp];
  const funcs = result.functions || [];
  const classes = result.classes || [];
  const exports = result.exports || [];
  const nonEmpty = result.nonEmptyLines;

  const exportedFuncNames = new Set();
  const exportedClassNames = new Set();
  for (const e of exports) {
    if (funcs.some(f => f.name === e.name)) exportedFuncNames.add(e.name);
    if (classes.some(c => c.name === e.name)) exportedClassNames.add(e.name);
  }

  // Create file node
  const isEntryPoint = fp.endsWith('/mod.rs');
  const fileNode = {
    id: makeFileId(fp),
    type: 'file',
    name: fileName(fp),
    filePath: fp,
    summary: meta.summary,
    tags: meta.tags,
    complexity: meta.complexity
  };
  if (meta.languageNotes) {
    fileNode.languageNotes = meta.languageNotes;
  }
  nodes.push(fileNode);

  // Disambiguation tracking for functions with repeated names (e.g. "new", "index", "next")
  const funcCount = {};

  // Create function nodes for significant functions
  for (const func of funcs) {
    const lineCount = func.endLine - func.startLine + 1;
    const isExported = exportedFuncNames.has(func.name);
    const isSignificant = isExported || lineCount >= 10;

    if (!isSignificant) continue;

    // Handle duplicate function names
    funcCount[func.name] = (funcCount[func.name] || 0) + 1;
    let disambiguatedName = func.name;
    if (funcCount[func.name] > 1) {
      disambiguatedName = func.name + '_' + funcCount[func.name];
    }

    const funcId = makeFuncId(fp, disambiguatedName);
    const isTrivial = lineCount < 10 && !isExported;
    if (isTrivial) continue;

    nodes.push({
      id: funcId,
      type: 'function',
      name: disambiguatedName,
      filePath: fp,
      lineRange: [func.startLine, func.endLine],
      summary: `${func.name} function in ${fileName(fp)} with parameters [${func.params.join(', ')}]`,
      tags: ['dom', 'function'],
      complexity: lineCount < 20 ? 'simple' : lineCount < 100 ? 'moderate' : 'complex'
    });

    // Contains edge
    addEdge(makeFileId(fp), funcId, 'contains');

    // Exports edge
    if (isExported) {
      addEdge(makeFileId(fp), funcId, 'exports');
    }
  }

  // Create class nodes for significant classes
  for (const cls of classes) {
    const lineCount = cls.endLine - cls.startLine + 1;
    const methodCount = (cls.methods || []).length;
    const isExported = exportedClassNames.has(cls.name);
    const isSignificant = isExported || methodCount >= 2 || lineCount >= 20;

    if (!isSignificant) continue;

    const clsId = makeClassId(fp, cls.name);

    nodes.push({
      id: clsId,
      type: 'class',
      name: cls.name,
      filePath: fp,
      lineRange: [cls.startLine, cls.endLine],
      summary: `${cls.name} ${lineCount < 10 ? 'enum/struct' : 'struct'} in ${fileName(fp)} with ${methodCount} methods and properties [${(cls.properties || []).join(', ')}]`,
      tags: ['dom', 'struct'],
      complexity: methodCount >= 10 || lineCount >= 100 ? 'complex' : methodCount >= 2 || lineCount >= 20 ? 'moderate' : 'simple'
    });

    // Contains edge
    addEdge(makeFileId(fp), clsId, 'contains');

    // Exports edge
    if (isExported) {
      addEdge(makeFileId(fp), clsId, 'exports');
    }
  }
}

// Import edges from batchImportData
const importData = {
  "components/script/dom/node/children_mutation.rs": [],
  "components/script/dom/node/context.rs": [],
  "components/script/dom/node/focus.rs": [],
  "components/script/dom/node/iterators.rs": [],
  "components/script/dom/node/layout_dom.rs": [],
  "components/script/dom/node/mod.rs": ["components/script/dom/node/children_mutation.rs","components/script/dom/node/context.rs","components/script/dom/node/focus.rs","components/script/dom/node/iterators.rs","components/script/dom/node/layout_dom.rs","components/script/dom/node/node.rs","components/script/dom/node/nodeiterator.rs","components/script/dom/node/nodelist.rs","components/script/dom/node/treewalker.rs","components/script/dom/node/virtualmethods.rs"],
  "components/script/dom/node/node.rs": [],
  "components/script/dom/node/nodeiterator.rs": [],
  "components/script/dom/node/nodelist.rs": [],
  "components/script/dom/node/treewalker.rs": [],
  "components/script/dom/node/virtualmethods.rs": []
};

for (const [filePath, imports] of Object.entries(importData)) {
  for (const imp of imports) {
    addEdge(makeFileId(filePath), makeFileId(imp), 'imports');
  }
}

// Also create an imports edge from mod.rs to the external neighbor
// mod.rs -> components/script/dom/mod.rs (cross-batch)
addEdge(makeFileId('components/script/dom/node/mod.rs'), makeFileId('components/script/dom/mod.rs'), 'imports');

// Verify import edge count
let totalImports = 0;
for (const imps of Object.values(importData)) {
  totalImports += imps.length;
}
// Add one for the cross-batch import
totalImports += 1;

// Deduplicate edges (can happen if same edge added twice)
const edgeMap = new Map();
for (const edge of edges) {
  const key = edge.source + '->' + edge.target + '->' + edge.type;
  if (!edgeMap.has(key)) {
    edgeMap.set(key, edge);
  }
}
const dedupedEdges = Array.from(edgeMap.values());

// Node dedup
const nodeMap = new Map();
for (const node of nodes) {
  if (!nodeMap.has(node.id)) {
    nodeMap.set(node.id, node);
  }
}
const dedupedNodes = Array.from(nodeMap.values());

console.log(`Generated ${dedupedNodes.length} nodes and ${dedupedEdges.length} edges`);
console.log(`Import edges: ${totalImports} (expected), ${dedupedEdges.filter(e => e.type === 'imports').length} actual`);

// Compute split
const nodeCount = dedupedNodes.length;
const edgeCount = dedupedEdges.length;
const parts = Math.ceil(Math.max(nodeCount / 60, edgeCount / 120));
console.log(`Splitting into ${parts} parts (${nodeCount} nodes, ${edgeCount} edges)`);

if (parts <= 1) {
  const output = { nodes: dedupedNodes, edges: dedupedEdges };
  fs.writeFileSync('d:/Projects/servo/.understand-anything/intermediate/batch-52.json', JSON.stringify(output, null, 2));
  console.log('Written to batch-52.json');
} else {
  // Group files alphabetically for partitioning
  const filePaths = Object.keys(importData).sort();
  const chunkSize = Math.ceil(filePaths.length / parts);

  for (let k = 0; k < parts; k++) {
    const partFilePaths = filePaths.slice(k * chunkSize, (k + 1) * chunkSize);
    const partNodes = [];
    const partNodeIds = new Set();
    const partEdgeSet = new Map();

    // Add file nodes for this part's files
    for (const nd of dedupedNodes) {
      if (nd.filePath && partFilePaths.includes(nd.filePath)) {
        partNodes.push(nd);
        partNodeIds.add(nd.id);
      }
    }

    // Find all sub-file nodes (functions/classes) that belong to these files
    for (const nd of dedupedNodes) {
      if (nd.type === 'function' || nd.type === 'class') {
        if (nd.filePath && partFilePaths.includes(nd.filePath)) {
          partNodes.push(nd);
          partNodeIds.add(nd.id);
        }
      }
    }

    // Add edges where source is in this part
    for (const edge of dedupedEdges) {
      if (partNodeIds.has(edge.source)) {
        const key = edge.source + '->' + edge.target + '->' + edge.type;
        if (!partEdgeSet.has(key)) {
          partEdgeSet.set(key, edge);
          // Also ensure target node exists in part if it's in our batch
          if (edge.target.startsWith('file:') || edge.target.startsWith('function:') || edge.target.startsWith('class:')) {
            const targetInBatch = dedupedNodes.find(n => n.id === edge.target);
            if (targetInBatch && targetInBatch.filePath && partFilePaths.includes(targetInBatch.filePath)) {
              if (!partNodeIds.has(edge.target)) {
                partNodes.push(targetInBatch);
                partNodeIds.add(targetInBatch.id);
              }
            }
          }
        }
      }
    }

    const partEdges = Array.from(partEdgeSet.values());
    const partNum = k + 1;
    const filename = `d:/Projects/servo/.understand-anything/intermediate/batch-52-part-${partNum}.json`;
    fs.writeFileSync(filename, JSON.stringify({ nodes: partNodes, edges: partEdges }, null, 2));
    console.log(`Written part ${partNum}: ${filename} (${partNodes.length} nodes, ${partEdges.length} edges)`);
  }
}
