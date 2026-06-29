const fs = require('fs');
const resultsPath = 'D:/Projects/servo/.understand-anything/tmp/ua-file-extract-results-7.json';
const data = JSON.parse(fs.readFileSync(resultsPath, 'utf8'));

function complexityClass(nel) {
  if (nel < 50) return 'simple';
  if (nel < 200) return 'moderate';
  return 'complex';
}

function complexityLines(n) {
  if (n < 10) return 'simple';
  if (n < 30) return 'moderate';
  return 'complex';
}

function isSigFunction(f, exportedNames) {
  const lc = f.endLine - f.startLine + 1;
  const isEx = exportedNames.has(f.name);
  if (f.name === 'fmt' && lc < 15) return false;
  if (f.name === 'from' && f.params.length === 1 && lc < 10) return false;
  return (isEx && lc >= 3) || lc >= 10;
}

function isSigClass(c, exportedNames) {
  const lc = c.endLine - c.startLine + 1;
  const isEx = exportedNames.has(c.name);
  return (c.methods && c.methods.length >= 2) || lc >= 20 || isEx;
}

let nodes = [];
let edges = [];
const fileOrder = data.results.map(r => r.path);

// Build all nodes and edges
data.results.forEach(r => {
  const fp = r.path;
  const nel = r.nonEmptyLines;
  const exp = r.exports || [];
  const exportedNames = new Set(exp.map(e => e.name));
  const funcs = r.functions || [];
  const classes = r.classes || [];
  const fileId = 'file:' + fp;
  const name = fp.split('/').pop();

  // File metadata
  let fileSummary, fileTags;

  if (fp === 'components/layout/fragment_tree/base_fragment.rs') {
    fileSummary = 'Defines the BaseFragment, FragmentStatus, BaseFragmentInfo, and Tag types that form the core fragment data structure, providing shared fields (rect, style, status, flags) and construction logic for all fragment types in the layout tree.';
    fileTags = ['fragment-tree', 'data-model', 'layout', 'core-types'];
  } else if (fp === 'components/layout/fragment_tree/box_fragment.rs') {
    fileSummary = 'Implements BoxFragment, the primary fragment type representing CSS boxes with children, containing block tracking, scrollable overflow calculation, baseline management, background modes, and resolved positioning insets.';
    fileTags = ['fragment-tree', 'box-model', 'layout', 'scrollable-overflow'];
  } else if (fp === 'components/layout/fragment_tree/containing_block.rs') {
    fileSummary = 'Provides ContainingBlockManager that tracks the containing block chain for fragments, supporting non-absolute, absolute, and fixed descendant containment strategies during layout.';
    fileTags = ['fragment-tree', 'containing-block', 'layout', 'positioning'];
  } else if (fp === 'components/layout/fragment_tree/fragment.rs') {
    fileSummary = 'Defines the Fragment enum (the top-level fragment abstraction) with variants for LayoutRoot, Box, Float, Positioning, Text, Image, and IFrame fragments, plus CollapsedBlockMargins/CollapsedMargin for margin collapsing and the ContainingBlockCalculation state machine.';
    fileTags = ['fragment-tree', 'data-model', 'layout', 'enum'];
  } else if (fp === 'components/layout/fragment_tree/fragment_tree.rs') {
    fileSummary = 'Implements FragmentTree, the top-level container that holds the root fragment tree for a layout document, providing scrolling area computation, fragment finding by position, and body fragment resolution.';
    fileTags = ['fragment-tree', 'tree-structure', 'layout', 'root'];
  } else if (fp === 'components/layout/fragment_tree/hoisted_shared_fragment.rs') {
    fileSummary = 'Defines HoistedSharedFragment, a lightweight wrapper for fragments hoisted out of the normal tree flow (e.g., for absolutely positioned elements), with an optional style override.';
    fileTags = ['fragment-tree', 'hoisted', 'absolute-positioning', 'layout'];
  } else if (fp === 'components/layout/fragment_tree/mod.rs') {
    fileSummary = 'Module barrel file re-exporting all fragment tree types for public use by other layout modules.';
    fileTags = ['fragment-tree', 'barrel', 'module', 're-exports'];
  } else {
    // ... continued in part 2
  }

  // Store for now
  r._fileSummary = fileSummary;
  r._fileTags = fileTags;
});

// Write preprocessed data
fs.writeFileSync('D:/Projects/servo/.understand-anything/tmp/gen-batch7-meta.json', JSON.stringify(data.results.map(r => ({
  path: r.path,
  nonEmptyLines: r.nonEmptyLines,
  functions: r.functions || [],
  classes: r.classes || [],
  exports: r.exports || [],
  _fileSummary: r._fileSummary,
  _fileTags: r._fileTags
})), null, 2));
