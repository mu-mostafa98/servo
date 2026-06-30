#!/usr/bin/env node
/**
 * Phase 2 - Semantic Layer Assignment
 * Reads the structural analysis results and assigns every file node to exactly one layer.
 */
const fs = require('fs');
const path = require('path');

const resultsPath = path.join(__dirname, 'ua-arch-results.json');
const inputPath = path.join(__dirname, 'ua-arch-input.json');
const outputPath = path.resolve(__dirname, '..', 'intermediate', 'layers.json');

// Load data
const results = JSON.parse(fs.readFileSync(resultsPath, 'utf8'));
const input = JSON.parse(fs.readFileSync(inputPath, 'utf8'));

const { directoryGroups, patternMatches } = results;
const { fileNodes } = input;

// All file node IDs
const allIds = new Set(fileNodes.map(n => n.id));

// ==========================================
// Layer Mapping Definition
// ==========================================

/**
 * Map each directory group to one or more layers.
 * For 'script', we split by path into DOM and non-DOM.
 */
function getLayersForFile(id) {
  // Find which directory group this file belongs to
  let group = null;
  for (const [g, ids] of Object.entries(directoryGroups)) {
    if (ids.includes(id)) {
      group = g;
      break;
    }
  }

  if (!group) {
    // orphan file - shouldn't happen
    return 'layer:tooling';
  }

  switch (group) {
    // === DOM Layer ===
    case 'script':
      // Split: files with '/dom/' go to DOM, others go to script-engine
      if (id.includes('/dom/')) {
        return 'layer:dom';
      }
      return 'layer:script-engine';

    // === Layout Layer ===
    case 'layout':
    case 'paint':
    case 'fonts':
      return 'layer:layout';

    // === SVG Engine ===
    case 'svg_engine':
      return 'layer:svg-engine';

    // === Script Engine (non-DOM) ===
    case 'script_bindings':
    case 'devtools':
    case 'webdriver_server':
    case 'xpath':
      return 'layer:script-engine';

    // === Networking ===
    case 'net':
    case 'storage':
      return 'layer:networking';

    // === Media ===
    case 'media':
    case 'canvas':
    case 'webxr':
      return 'layer:media';

    // === Ports ===
    case 'ports-servoshell':
    case 'ffi':
    case 'support-android':
    case 'support-crown':
    case 'support-hitrace-bencher':
    case 'support-macos':
    case 'support-openharmony':
      return 'layer:ports';

    // === Shared Infrastructure ===
    case 'shared':
    case 'constellation':
    case 'servo':
    case 'config':
    case 'background_hang_monitor':
    case 'malloc_size_of':
      return 'layer:shared-infra';

    // === Tooling ===
    case 'python':
    case 'etc':
    case 'mach':
    case 'root-config':
    case 'root-docs':
    case 'root-other':
      return 'layer:tooling';

    default:
      // Unknown group - should not happen if all groups are mapped
      console.error('WARNING: Unmapped group:', group, 'for file:', id);
      return 'layer:shared-infra';
  }
}

// ==========================================
// Assign files to layers
// ==========================================

const layerFiles = {
  'layer:dom': [],
  'layer:layout': [],
  'layer:svg-engine': [],
  'layer:script-engine': [],
  'layer:networking': [],
  'layer:media': [],
  'layer:ports': [],
  'layer:shared-infra': [],
  'layer:tooling': [],
};

for (const id of allIds) {
  const layer = getLayersForFile(id);
  layerFiles[layer].push(id);
}

// ==========================================
// Verify: every file assigned exactly once
// ==========================================
const assigned = new Set();
let total = 0;
for (const [layer, ids] of Object.entries(layerFiles)) {
  for (const id of ids) {
    if (assigned.has(id)) {
      console.error('ERROR: Duplicate assignment:', id);
      process.exit(1);
    }
    assigned.add(id);
    total++;
  }
}

if (total !== allIds.size) {
  console.error('ERROR: Total assigned (' + total + ') != total nodes (' + allIds.size + ')');
  // Find unassigned
  for (const id of allIds) {
    if (!assigned.has(id)) {
      console.error('  Unassigned:', id);
    }
  }
  process.exit(1);
}

console.error('All ' + total + ' file nodes assigned successfully.');

// ==========================================
// Layer definitions with descriptions
// ==========================================

const layers = [
  {
    id: 'layer:dom',
    name: 'DOM',
    description: 'DOM implementation and web-facing API bindings for the Servo browser engine in Rust',
    nodeIds: layerFiles['layer:dom']
  },
  {
    id: 'layer:layout',
    name: 'Layout',
    description: 'Layout engine implementing CSS formatting, painting, compositing, and font rendering for web content',
    nodeIds: layerFiles['layer:layout']
  },
  {
    id: 'layer:svg-engine',
    name: 'SVG Engine',
    description: 'SVG rendering pipeline handling shape construction, style resolution, tessellation, and SVG tree traversal',
    nodeIds: layerFiles['layer:svg-engine']
  },
  {
    id: 'layer:script-engine',
    name: 'Script Engine',
    description: 'Script engine core including JavaScript runtime bindings, IDL code generation, WebDriver, DevTools, and XPath evaluation',
    nodeIds: layerFiles['layer:script-engine']
  },
  {
    id: 'layer:networking',
    name: 'Networking',
    description: 'HTTP networking stack and persistent storage implementation for web resource loading and data persistence',
    nodeIds: layerFiles['layer:networking']
  },
  {
    id: 'layer:media',
    name: 'Media',
    description: 'Media playback, canvas rendering, WebGL/WebGPU graphics, and WebXR virtual reality pipeline',
    nodeIds: layerFiles['layer:media']
  },
  {
    id: 'layer:ports',
    name: 'Ports',
    description: 'Platform porting layer providing operating system integration, Android/OpenHarmony/macOS support, and C FFI bindings',
    nodeIds: layerFiles['layer:ports']
  },
  {
    id: 'layer:shared-infra',
    name: 'Shared Infra',
    description: 'Shared infrastructure including process management (Constellation), IPC channels, Servo binary entry point, configuration, memory utilities, and core types',
    nodeIds: layerFiles['layer:shared-infra']
  },
  {
    id: 'layer:tooling',
    name: 'Tooling',
    description: 'Build system tooling (Mach), Python-based development scripts, CI/CD pipeline configuration, project documentation, and root-level project configuration',
    nodeIds: layerFiles['layer:tooling']
  }
];

// ==========================================
// Final validation
// ==========================================
// Check no empty layers
for (const layer of layers) {
  if (layer.nodeIds.length === 0) {
    console.error('ERROR: Empty layer:', layer.id);
    process.exit(1);
  }
}

// Check layer count
if (layers.length < 3 || layers.length > 10) {
  console.error('ERROR: Layer count (' + layers.length + ') outside allowed range 3-10');
  process.exit(1);
}

// Verify total
const totalAssigned = layers.reduce((sum, l) => sum + l.nodeIds.length, 0);
if (totalAssigned !== allIds.size) {
  console.error('ERROR: Total assigned (' + totalAssigned + ') != total file nodes (' + allIds.size + ')');
  process.exit(1);
}

// Check for node IDs that don't exist in input
const allInputIds = new Set(fileNodes.map(n => n.id));
for (const layer of layers) {
  for (const id of layer.nodeIds) {
    if (!allInputIds.has(id)) {
      console.error('ERROR: Node ID in layers not in input:', id);
      process.exit(1);
    }
  }
}

console.error('Layer count: ' + layers.length);
for (const l of layers) {
  console.error('  ' + l.id + ': ' + l.nodeIds.length + ' files');
}

// Write output
try {
  fs.writeFileSync(outputPath, JSON.stringify(layers, null, 2), 'utf8');
  console.error('Layers written to', outputPath);
} catch (e) {
  console.error('Failed to write output:', e.message);
  process.exit(1);
}
