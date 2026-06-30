#!/usr/bin/env node
/**
 * Architecture Structural Analysis Script (v2)
 * Improved directory grouping for multi-level project structures.
 */
const fs = require('fs');

const inputPath = process.argv[2];
const outputPath = process.argv[3];

if (!inputPath || !outputPath) {
  console.error('Usage: ua-arch-analyze.js <input.json> <output.json>');
  process.exit(1);
}

let raw;
try {
  raw = JSON.parse(fs.readFileSync(inputPath, 'utf8'));
} catch (e) {
  console.error('Failed to read input:', e.message);
  process.exit(1);
}

const { fileNodes, importEdges, allEdges } = raw;

if (!fileNodes || !Array.isArray(fileNodes)) {
  console.error('Input must contain fileNodes array');
  process.exit(1);
}

// ==========================================
// A. Directory Grouping (Improved)
// ==========================================

/**
 * Group files into meaningful architectural groups based on path patterns.
 *
 * For Servo project structure:
 *   components/<subcrate>/...  -> group "<subcrate>"
 *   ports/<port>/...           -> group "ports-<port>" or "ports"
 *   support/<tool>/...         -> group "support-<tool>" or "support"
 *   python/...                 -> group "python"
 *   etc/...                    -> group "etc"
 *   ffi/...                    -> group "ffi"
 *   Root-level files           -> group by type
 */

function getDirectoryGroup(filePath) {
  const normalized = filePath.replace(/\\/g, '/');
  const parts = normalized.split('/');

  // Paths starting with "components/<subcrate>/..." -> group by subcrate name
  if (parts[0] === 'components' && parts.length >= 2) {
    return parts[1]; // e.g., "script", "layout", "net", etc.
  }

  // Paths starting with "ports/<port>/..." -> group by port name
  if (parts[0] === 'ports' && parts.length >= 2) {
    const portName = parts[1];
    return `ports-${portName}`;
  }

  // Paths starting with "support/..." -> group by subdir
  if (parts[0] === 'support' && parts.length >= 2) {
    return `support-${parts[1]}`;
  }

  // Known top-level directories
  if (parts[0] === 'python') return 'python';
  if (parts[0] === 'etc') return 'etc';
  if (parts[0] === 'ffi') return 'ffi';
  if (parts[0] === 'docs') return 'docs';
  if (parts[0] === 'resources') return 'resources';
  if (parts[0] === '.github') return 'ci-cd';
  if (parts[0] === '.vscode') return 'vscode';
  if (parts[0] === '.devcontainer') return 'devcontainer';
  if (parts[0] === '.cargo') return 'cargo-config';

  // Root-level files: classify
  if (parts.length === 1) {
    return classifyRootFile(normalized);
  }

  // Catch-all for any other directory structure: use first segment
  return parts[0];
}

function classifyRootFile(filePath) {
  const name = filePath.split('/').pop();
  const lower = name.toLowerCase();

  // Documentation files
  if (['readme.md', 'contributing.md', 'license', 'license_whatwg_specs', 'security.md',
       'code_of_conduct.md', 'pull_request_template.md', 'changelog.md',
       'landscape.html', 'sunrise_valley.html'].includes(lower)) {
    return 'root-docs';
  }

  // Config files
  if (name.endsWith('.toml') || name.endsWith('.json') || name === '.gitignore' ||
      name.endsWith('.yaml') || name.endsWith('.yml') || name === '.gitattributes' ||
      name === '.mailmap' || name === '.servobuild' || name === '.python-version') {
    return 'root-config';
  }

  // Shell scripts / entry points
  if (name === 'mach' || name === 'mach.bat' || name === 'mach.ps1' || name === 'shell.nix') {
    return 'mach';
  }

  // Everything else
  return 'root-other';
}

const directoryGroups = {};
fileNodes.forEach(node => {
  const group = getDirectoryGroup(node.filePath);
  if (!directoryGroups[group]) {
    directoryGroups[group] = [];
  }
  directoryGroups[group].push(node.id);
});

// ==========================================
// B. Node Type Grouping
// ==========================================
const nodeTypeGroups = {};
fileNodes.forEach(node => {
  const t = node.type || 'file';
  if (!nodeTypeGroups[t]) {
    nodeTypeGroups[t] = [];
  }
  nodeTypeGroups[t].push(node.id);
});

// ==========================================
// C. Import Adjacency Matrix
// ==========================================
const fileIds = new Set(fileNodes.map(n => n.id));
const fileImportEdges = (importEdges || []).filter(e =>
  fileIds.has(e.source) && fileIds.has(e.target) && e.type === 'imports'
);

const importAdj = {};
fileImportEdges.forEach(e => {
  if (!importAdj[e.source]) importAdj[e.source] = [];
  importAdj[e.source].push(e.target);
});

// Fan-in / Fan-out
const fanOut = {};
const fanIn = {};
fileNodes.forEach(n => { fanOut[n.id] = 0; fanIn[n.id] = 0; });

Object.keys(importAdj).forEach(src => {
  fanOut[src] = importAdj[src].length;
  importAdj[src].forEach(tgt => {
    fanIn[tgt] = (fanIn[tgt] || 0) + 1;
  });
});

// Map node ID -> group
const nodeToGroup = {};
Object.keys(directoryGroups).forEach(g => {
  directoryGroups[g].forEach(id => { nodeToGroup[id] = g; });
});

// Group-level import tracking
const groupImportSet = {};
const groupImportedSet = {};
Object.keys(directoryGroups).forEach(g => {
  groupImportSet[g] = new Set();
  groupImportedSet[g] = new Set();
});

fileImportEdges.forEach(e => {
  const srcGroup = nodeToGroup[e.source];
  const tgtGroup = nodeToGroup[e.target];
  if (srcGroup && tgtGroup && srcGroup !== tgtGroup) {
    groupImportSet[srcGroup].add(tgtGroup);
    groupImportedSet[tgtGroup].add(srcGroup);
  }
});

// ==========================================
// D. Cross-Category Dependency Analysis
// ==========================================
const crossCategoryEdges = {};

(allEdges || []).forEach(e => {
  const srcNode = fileNodes.find(n => n.id === e.source);
  const tgtNode = fileNodes.find(n => n.id === e.target);
  if (srcNode && tgtNode && srcNode.type !== tgtNode.type) {
    const key = `${srcNode.type}->${tgtNode.type}`;
    if (!crossCategoryEdges[key]) {
      crossCategoryEdges[key] = { fromType: srcNode.type, toType: tgtNode.type, edgeType: e.type, count: 0 };
    }
    crossCategoryEdges[key].count++;
  }
});

// ==========================================
// E. Inter-Group Import Frequency
// ==========================================
const interGroupImports = {};
fileImportEdges.forEach(e => {
  const srcGroup = nodeToGroup[e.source];
  const tgtGroup = nodeToGroup[e.target];
  if (srcGroup && tgtGroup && srcGroup !== tgtGroup) {
    const key = `${srcGroup}->${tgtGroup}`;
    if (!interGroupImports[key]) {
      interGroupImports[key] = { from: srcGroup, to: tgtGroup, count: 0 };
    }
    interGroupImports[key].count++;
  }
});

// ==========================================
// F. Intra-Group Import Density
// ==========================================
const intraGroupDensity = {};
const groupInternalEdges = {};
const groupTotalEdges = {};

Object.keys(directoryGroups).forEach(g => {
  groupInternalEdges[g] = 0;
  groupTotalEdges[g] = 0;
});

fileImportEdges.forEach(e => {
  const srcGroup = nodeToGroup[e.source];
  const tgtGroup = nodeToGroup[e.target];
  if (srcGroup && tgtGroup) {
    groupTotalEdges[srcGroup] = (groupTotalEdges[srcGroup] || 0) + 1;
    if (srcGroup === tgtGroup) {
      groupInternalEdges[srcGroup] = (groupInternalEdges[srcGroup] || 0) + 1;
    }
  }
});

Object.keys(directoryGroups).forEach(g => {
  const total = groupTotalEdges[g] || 0;
  const internal = groupInternalEdges[g] || 0;
  intraGroupDensity[g] = {
    internalEdges: internal,
    totalEdges: total,
    density: total > 0 ? Math.round((internal / total) * 100) / 100 : 0
  };
});

// ==========================================
// G. Directory Pattern Matching
// ==========================================
function matchDirectoryPattern(dirName) {
  const lower = dirName.toLowerCase();

  // Known Servo component subcrates
  const servoComponentPatterns = {
    'script': 'script-engine',
    'layout': 'layout-engine',
    'svg_engine': 'svg-engine',
    'net': 'networking',
    'paint': 'painting',
    'canvas': 'canvas',
    'webgl': 'web-graphics',
    'webgpu': 'web-graphics',
    'webxr': 'webxr',
    'fonts': 'fonts',
    'constellation': 'constellation',
    'devtools': 'devtools',
    'webdriver_server': 'webdriver',
    'xpath': 'xpath',
    'storage': 'storage',
    'media': 'media',
    'servo': 'servo-crate',
    'shared': 'shared-infra',
    'url': 'url',
    'profile': 'profiling',
    'metrics': 'profiling',
    'bluetooth': 'bluetooth',
    'timers': 'timers',
    'wakelock': 'wakelock',
    'allocator': 'allocator',
    'background_hang_monitor': 'hang-monitor',
    'script_bindings': 'script-bindings',
    'default-resources': 'resources',
    'deny_public_fields': 'macros',
    'dom_struct': 'macros',
    'jstraceable_derive': 'macros',
    'hyper_serde': 'macros',
    'malloc_size_of': 'macros',
    'pixels': 'macros',
    'geometry': 'geometry',
    'config': 'config-internal',
    'servo_tracing': 'tracing',
  };

  if (servoComponentPatterns[lower]) {
    return servoComponentPatterns[lower];
  }

  // Special ports
  if (lower.startsWith('ports-')) {
    return 'ports';
  }

  // Known patterns
  const genericPatterns = {
    'api': 'api',
    'routes': 'api',
    'controllers': 'api',
    'endpoints': 'api',
    'handlers': 'api',
    'services': 'service',
    'core': 'service',
    'domain': 'service',
    'logic': 'service',
    'internal': 'service',
    'models': 'data',
    'db': 'data',
    'data': 'data',
    'persistence': 'data',
    'repository': 'data',
    'entities': 'data',
    'entity': 'data',
    'migrations': 'data',
    'sql': 'data',
    'database': 'data',
    'schema': 'data',
    'utils': 'utility',
    'helpers': 'utility',
    'common': 'utility',
    'shared': 'utility',
    'tools': 'utility',
    'pkg': 'utility',
    'config': 'config',
    'constants': 'config',
    'env': 'config',
    'settings': 'config',
    'types': 'types',
    'interfaces': 'types',
    'dtos': 'types',
    'dto': 'types',
    '__tests__': 'test',
    'test': 'test',
    'tests': 'test',
    'spec': 'test',
    'docs': 'documentation',
    'documentation': 'documentation',
    'wiki': 'documentation',
    'deploy': 'infrastructure',
    'deployment': 'infrastructure',
    'infra': 'infrastructure',
    'infrastructure': 'infrastructure',
    'docker': 'infrastructure',
    'python': 'python-tooling',
    'etc': 'ci-cd',
    'ci-cd': 'ci-cd',
    'resources': 'resources',
    'components': 'ui',
    'views': 'ui',
    'pages': 'ui',
    'ui': 'ui',
    'layouts': 'ui',
    'screens': 'ui',
    'middleware': 'middleware',
    'plugins': 'middleware',
    'hooks': 'hooks',
    'store': 'state',
    'state': 'state',
    'reducers': 'state',
    'actions': 'state',
    'slices': 'state',
    'assets': 'assets',
    'static': 'assets',
    'public': 'assets',
    'cmd': 'entry',
    'bin': 'entry',
  };

  return genericPatterns[lower] || 'other';
}

const patternMatches = {};
Object.keys(directoryGroups).forEach(g => {
  patternMatches[g] = matchDirectoryPattern(g);
});

// ==========================================
// H. Deployment Topology Detection
// ==========================================
const infraFiles = [];
const hasDockerfile = fileNodes.some(n => n.name.includes('Dockerfile'));
const hasCompose = fileNodes.some(n => n.name.includes('docker-compose'));
const hasK8s = fileNodes.some(n => n.filePath.includes('k8s') || n.filePath.includes('kubernetes') || n.filePath.includes('helm'));
const hasTerraform = fileNodes.some(n => n.name.endsWith('.tf') || n.name.endsWith('.tfvars'));
const hasCI = fileNodes.some(n => n.filePath.includes('.github/workflows') || n.filePath.includes('.gitlab-ci') || n.name === 'Jenkinsfile');

fileNodes.forEach(n => {
  if (n.name.includes('Dockerfile') || n.name.includes('docker-compose') ||
      n.filePath.includes('.github/workflows') || n.name.endsWith('.tf') ||
      n.name.endsWith('.tfvars') || n.name === 'Jenkinsfile' ||
      n.name.endsWith('.gitlab-ci.yml')) {
    infraFiles.push(n.filePath);
  }
});

// ==========================================
// I. Data Pipeline Detection
// ==========================================
const schemaFiles = [];
const migrationFiles = [];
const dataModelFiles = [];
const apiHandlerFiles = [];

fileNodes.forEach(n => {
  if (n.filePath.endsWith('.sql') || n.filePath.endsWith('.graphql') || n.filePath.endsWith('.proto')) {
    schemaFiles.push(n.filePath);
  }
  if (n.filePath.includes('migrations') || n.filePath.includes('migration')) {
    migrationFiles.push(n.filePath);
  }
  if (n.tags && (n.tags.includes('data-model') || n.tags.includes('orm'))) {
    dataModelFiles.push(n.filePath);
  }
  if (n.filePath.includes('/routes/') || n.filePath.includes('/api/') ||
      n.filePath.includes('/controllers/') || n.filePath.includes('/handlers/') ||
      (n.tags && n.tags.includes('api-handler'))) {
    apiHandlerFiles.push(n.filePath);
  }
});

// ==========================================
// J. Documentation Coverage
// ==========================================
const docFileSet = new Set();
fileNodes.forEach(n => {
  if (n.type === 'document' || n.filePath.endsWith('.md') || n.filePath.endsWith('.rst') ||
      (n.tags && n.tags.includes('documentation'))) {
    docFileSet.add(n.id);
  }
});

const docFilesByGroup = {};
Object.keys(directoryGroups).forEach(g => {
  docFilesByGroup[g] = directoryGroups[g].filter(id => docFileSet.has(id)).length;
});

const groupsWithDocs = Object.values(docFilesByGroup).filter(c => c > 0).length;
const totalGroups = Object.keys(directoryGroups).length;
const undocumentedGroups = Object.entries(docFilesByGroup)
  .filter(([_, c]) => c === 0)
  .map(([g, _]) => g);

// ==========================================
// K. Dependency Direction
// ==========================================
const groupImportCounts = {};
fileImportEdges.forEach(e => {
  const srcGroup = nodeToGroup[e.source];
  const tgtGroup = nodeToGroup[e.target];
  if (srcGroup && tgtGroup && srcGroup !== tgtGroup) {
    const key = `${srcGroup}||${tgtGroup}`;
    if (!groupImportCounts[key]) {
      groupImportCounts[key] = { a: srcGroup, b: tgtGroup, aToB: 0, bToA: 0 };
    }
    groupImportCounts[key].aToB++;
  }
});

// Need to merge both directions
const depDirectionMap = {};
Object.values(groupImportCounts).forEach(({ a, b, aToB }) => {
  const revKey = `${b}||${a}`;
  const revEntry = groupImportCounts[revKey];
  const actualBToA = revEntry ? revEntry.aToB : 0;

  const sortedKey = [a, b].sort().join('::');
  if (!depDirectionMap[sortedKey]) {
    depDirectionMap[sortedKey] = { a, b, aToB: 0, bToA: 0 };
  }
  if (aToB > 0) depDirectionMap[sortedKey].aToB += aToB;
  if (revEntry) depDirectionMap[sortedKey].bToA += revEntry.aToB;
});

const dependencyDirection = [];
Object.values(depDirectionMap).forEach(({ a, b, aToB, bToA }) => {
  if (aToB > bToA) {
    dependencyDirection.push({ dependent: a, dependsOn: b });
  } else if (bToA > aToB) {
    dependencyDirection.push({ dependent: b, dependsOn: a });
  }
});

// ==========================================
// Assemble Output
// ==========================================
const output = {
  scriptCompleted: true,
  directoryGroups: Object.fromEntries(
    Object.entries(directoryGroups).sort((a, b) => a[0].localeCompare(b[0]))
  ),
  nodeTypeGroups: Object.fromEntries(
    Object.entries(nodeTypeGroups).sort((a, b) => a[0].localeCompare(b[0]))
  ),
  crossCategoryEdges: Object.values(crossCategoryEdges).sort((a, b) => b.count - a.count),
  interGroupImports: Object.values(interGroupImports).filter(e => e.count > 0).sort((a, b) => b.count - a.count),
  intraGroupDensity,
  patternMatches,
  groupImportsFrom: Object.fromEntries(
    Object.keys(groupImportSet).map(g => [g, Array.from(groupImportSet[g])])
  ),
  groupImportedBy: Object.fromEntries(
    Object.keys(groupImportedSet).map(g => [g, Array.from(groupImportedSet[g])])
  ),
  deploymentTopology: {
    hasDockerfile,
    hasCompose,
    hasK8s,
    hasTerraform,
    hasCI,
    infraFiles
  },
  dataPipeline: {
    schemaFiles,
    migrationFiles,
    dataModelFiles,
    apiHandlerFiles
  },
  docCoverage: {
    groupsWithDocs,
    totalGroups,
    coverageRatio: totalGroups > 0 ? Math.round((groupsWithDocs / totalGroups) * 100) / 100 : 0,
    undocumentedGroups
  },
  dependencyDirection: dependencyDirection,
  fileStats: {
    totalFileNodes: fileNodes.length,
    filesPerGroup: Object.fromEntries(
      Object.entries(directoryGroups).map(([g, ids]) => [g, ids.length]).sort((a, b) => a[0].localeCompare(b[0]))
    ),
    nodeTypeCounts: Object.fromEntries(
      Object.entries(nodeTypeGroups).map(([t, ids]) => [t, ids.length])
    )
  },
  fileFanIn: Object.fromEntries(
    Object.entries(fanIn)
      .filter(([_, count]) => count > 0)
      .sort((a, b) => b[1] - a[1])
      .slice(0, 100)
  ),
  fileFanOut: Object.fromEntries(
    Object.entries(fanOut)
      .filter(([_, count]) => count > 0)
      .sort((a, b) => b[1] - a[1])
      .slice(0, 100)
  )
};

try {
  fs.writeFileSync(outputPath, JSON.stringify(output, null, 2), 'utf8');
  console.error('Analysis v2 complete. Output written to', outputPath);
  process.exit(0);
} catch (e) {
  console.error('Failed to write output:', e.message);
  process.exit(1);
}
