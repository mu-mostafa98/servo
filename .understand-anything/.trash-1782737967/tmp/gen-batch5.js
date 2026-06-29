const fs = require('fs');

const results = JSON.parse(fs.readFileSync('.understand-anything/tmp/ua-file-extract-results-5.json', 'utf-8'));

const nodes = [];
const edges = [];

// File-level metadata
const fileMeta = {
  "components/layout/accessibility_tree.rs": {
    summary: "Manages the accessibility tree for assistive technologies, mapping DOM nodes to accesskit nodes with role and label information for screen readers and other AT tools.",
    tags: ["accessibility", "tree", "a11y", "dom-mapping"],
    complexity: "complex",
    languageNotes: "Uses accesskit crate for platform accessibility integration with bitflags for damage tracking."
  },
  "components/layout/cell.rs": {
    summary: "Provides custom thread-safe cell types (ArcRefCell, WeakRefCell, RefOrAtomicRef) combining Arc, AtomicRefCell, and atomic reference counting for concurrent layout operations.",
    tags: ["concurrency", "cell-types", "memory-management"],
    complexity: "simple",
    languageNotes: "Lightweight generic wrapper types combining Arc with AtomicRefCell for concurrent read/write access."
  },
  "components/layout/construct_modern.rs": {
    summary: "Implements the modern CSS layout construction algorithm for display: contents elements, building intermediate container representations for the fragment tree.",
    tags: ["layout-construction", "display-contents", "css"],
    complexity: "moderate",
    languageNotes: "Uses an intermediate job-based construction pattern to handle display: contents tree restructuring."
  },
  "components/layout/context.rs": {
    summary: "Provides the layout context including image resolution, caching, and SVG serialization services used throughout the layout engine.",
    tags: ["layout-context", "image-resolution", "caching", "svg"],
    complexity: "moderate",
  },
  "components/layout/display_list/background.rs": {
    summary: "Implements background painting layer calculations, including positioning areas, clipping, and 1D/2D background layout for CSS background rendering.",
    tags: ["background", "painting", "css", "display-list"],
    complexity: "moderate",
  },
  "components/layout/display_list/clip.rs": {
    summary: "Manages clip shapes and paths for the display list, including basic shapes, clip paths, and stacking context clip stores.",
    tags: ["clipping", "css-shapes", "display-list", "stacking-context"],
    complexity: "moderate",
  },
  "components/layout/display_list/conversions.rs": {
    summary: "Provides trait implementations for converting Servo internal display item types to WebRender-compatible types (ToWebRender, FilterToWebRender).",
    tags: ["conversion", "webrender", "display-list", "trait"],
    complexity: "simple",
  },
  "components/layout/display_list/gradient.rs": {
    summary: "Builds WebRender gradient display items from CSS gradient definitions, supporting linear, radial, and conic gradients with color stop interpolation.",
    tags: ["gradient", "css", "painting", "webrender"],
    complexity: "moderate",
  },
  "components/layout/display_list/hit_test.rs": {
    summary: "Performs hit-testing against the display list by traversing stacking contexts and testing containment against box and text fragments.",
    tags: ["hit-test", "event-handling", "stacking-context", "display-list"],
    complexity: "moderate",
  },
  "components/layout/display_list/mod.rs": {
    summary: "Core display list builder that constructs WebRender display items from the layout tree, handling backgrounds, borders, shadows, text, and stacking contexts.",
    tags: ["display-list", "painting", "webrender", "rendering"],
    complexity: "complex",
    languageNotes: "Massive builder pattern implementation with over 2000 non-empty lines, central to Servo's rendering pipeline."
  },
  "components/layout/display_list/paint_timing_handler.rs": {
    summary: "Tracks Largest Contentful Paint (LCP) candidates for performance measurement, computing intersection ratios and paint timing metrics.",
    tags: ["performance", "paint-timing", "lcp", "web-vitals"],
    complexity: "simple",
  },
  "components/layout/display_list/paint_traversal.rs": {
    summary: "Implements paint traversal through stacking contexts and fragment trees, dispatching to appropriate handlers for boxes, text, and replaced content.",
    tags: ["paint-traversal", "stacking-context", "fragment-tree", "rendering"],
    complexity: "complex",
  },
  "components/layout/display_list/stacking_context.rs": {
    summary: "Builds and manages the stacking context tree, handling reference frames, scroll frames, sticky positioning, clip frames, and overflow frames.",
    tags: ["stacking-context", "css-positioning", "scrolling", "clipping"],
    complexity: "complex",
    languageNotes: "Central to CSS stacking context management with over 1300 lines handling reference frames, transforms, and perspective."
  },
  "components/layout/dom.rs": {
    summary: "Defines the LayoutBox abstraction over DOM nodes with thread-safe weak/strong references, pseudo-element support, and fragment management.",
    tags: ["dom", "layout-box", "pseudo-elements", "fragment"],
    complexity: "complex",
    languageNotes: "Extensive use of RefCell and Arc for interior mutability in the parallel layout tree."
  },
  "components/layout/dom_traversal.rs": {
    summary: "Implements DOM tree traversal for layout construction, handling elements, pseudo-elements, replaced content, and CSS generated content.",
    tags: ["dom-traversal", "pseudo-elements", "generated-content", "layout-construction"],
    complexity: "moderate",
  },
  "components/layout/flexbox/geom.rs": {
    summary: "Defines flexbox-specific geometry types (FlexRelativeVec2, FlexRelativeRect, FlexAxis) and coordinate system conversions for flex layout.",
    tags: ["flexbox", "geometry", "coordinate-systems", "css"],
    complexity: "simple",
  },
  "components/layout/flexbox/layout.rs": {
    summary: "Implements the main Flexbox layout algorithm including flex line construction, flexible length resolution, cross-axis alignment, and auto margins.",
    tags: ["flexbox", "layout-algorithm", "css", "sizing"],
    complexity: "complex",
    languageNotes: "Over 2600 lines implementing the full CSS Flexbox specification layout algorithm with flexible length resolution."
  },
  "components/layout/flexbox/mod.rs": {
    summary: "Provides flex container and flex item box types with construction, repair, and sizing operations bridging the flexbox algorithm to the box tree.",
    tags: ["flexbox", "container", "item", "box-tree"],
    complexity: "moderate",
  },
  "components/layout/flow/construct.rs": {
    summary: "Implements flow tree construction from DOM nodes, creating block and inline formatting contexts with float and absolute positioning support.",
    tags: ["flow-construction", "block-formatting", "inline-formatting", "css"],
    complexity: "complex",
  },
  "components/layout/flow/float.rs": {
    summary: "Implements CSS float layout including float band trees, clearance calculations, sequential float placement, and block position tracking.",
    tags: ["floats", "css-layout", "float-bands", "clearance"],
    complexity: "complex",
    languageNotes: "Over 1100 lines implementing the complete CSS float specification with float band tree data structure."
  },
  "components/layout/flow/inline/construct.rs": {
    summary: "Constructs inline formatting contexts by processing text runs, atomic inlines, floats, and absolutely positioned elements within inline layout.",
    tags: ["inline-formatting", "text-layout", "white-space", "css"],
    complexity: "complex",
  },
  "components/layout/flow/inline/inline_box.rs": {
    summary: "Manages inline box structure including nesting, tree paths, and container state for inline formatting contexts.",
    tags: ["inline-box", "nesting", "css", "inline-formatting"],
    complexity: "moderate",
  },
  "components/layout/flow/inline/line.rs": {
    summary: "Implements line layout within inline formatting contexts, including line item layout, bidi reordering, whitespace trimming, and inline box positioning.",
    tags: ["line-layout", "bidi", "whitespace", "inline-formatting"],
    complexity: "complex",
  },
  "components/layout/flow/inline/line_breaker.rs": {
    summary: "Provides line breaking functionality using the Unicode line break algorithm to determine valid line break opportunities.",
    tags: ["line-break", "unicode", "text-layout", "inline"],
    complexity: "simple",
  },
  "components/layout/flow/inline/mod.rs": {
    summary: "Core inline formatting context implementation containing line construction, inline item management, white space processing, and float interaction.",
    tags: ["inline-formatting", "line-layout", "white-space", "floats"],
    complexity: "complex",
    languageNotes: "Over 3100 lines implementing the core inline layout algorithm with comprehensive float interaction and line breaking."
  },
  "components/layout/flow/inline/text_run.rs": {
    summary: "Handles text shaping, font selection, text segmentation, and layout-into-line-items for text runs within inline formatting contexts.",
    tags: ["text", "font", "shaping", "inline-formatting"],
    complexity: "moderate",
  },
  "components/layout/flow/mod.rs": {
    summary: "Implements block formatting context layout including block-level box placement, margin collapsing, parallel layout, and float/atomic inline placement.",
    tags: ["block-formatting", "margin-collapsing", "parallel-layout", "css"],
    complexity: "complex",
  },
  "components/layout/flow/root.rs": {
    summary: "Constructs the root box tree and viewport overflow handling for the initial containing block in layout.",
    tags: ["root", "box-tree", "viewport", "initial-containing-block"],
    complexity: "moderate",
  },
  "components/layout/flow/same_formatting_context_block.rs": {
    summary: "Implements layout for block-level boxes that remain in the same formatting context, handling non-replaced block-level element sizing and positioning.",
    tags: ["block-layout", "formatting-context", "css-sizing", "positioning"],
    complexity: "moderate",
  },
  "components/layout/formatting_contexts.rs": {
    summary: "Implements independent formatting contexts (replaced, flow, flex, grid, table) with layout caching, inline content sizing, and baseline management.",
    tags: ["formatting-context", "independent", "layout-caching", "css"],
    complexity: "moderate",
  }
};

// Build file nodes and their contained items
for (const file of results.results) {
  const path = file.path;
  const meta = fileMeta[path] || { summary: "No summary available.", tags: ["code", "layout"], complexity: "moderate" };

  const fileName = path.split('/').pop();
  const fileId = `file:${path}`;

  nodes.push({
    id: fileId,
    type: "file",
    name: fileName,
    filePath: path,
    summary: meta.summary,
    tags: meta.tags,
    complexity: meta.complexity,
    ...(meta.languageNotes ? { languageNotes: meta.languageNotes } : {})
  });

  // Create class nodes for significant classes
  for (const cls of file.classes) {
    const lineCount = cls.endLine - cls.startLine + 1;
    const isExported = file.exports.some(e => e.name === cls.name && e.line === cls.startLine);
    const isSignificant = isExported || cls.methods.length >= 2 || lineCount >= 20;

    if (!isSignificant) continue;

    const classId = `class:${path}:${cls.name}`;
    const classSummary = `${cls.name} ${cls.methods.length > 0 ? 'with ' + cls.methods.length + ' methods' : 'enum/struct'} for ${path.split('/').pop().replace('.rs', '')}.`;
    const classTags = [...new Set([...meta.tags, 'class'])].slice(0, 5);

    nodes.push({
      id: classId,
      type: "class",
      name: cls.name,
      filePath: path,
      lineRange: [cls.startLine, cls.endLine],
      summary: classSummary,
      tags: classTags,
      complexity: lineCount > 100 ? "complex" : lineCount > 30 ? "moderate" : "simple",
    });

    edges.push({
      source: fileId,
      target: classId,
      type: "contains",
      direction: "forward",
      weight: 1.0
    });

    if (isExported) {
      edges.push({
        source: fileId,
        target: classId,
        type: "exports",
        direction: "forward",
        weight: 0.8
      });
    }
  }

  // Create function nodes for exported functions or functions >= 10 lines
  const exportedFnKeys = new Set(
    file.exports
      .filter(e => file.functions.some(f => f.name === e.name && f.startLine === e.line))
      .map(e => `${e.name}:${e.line}`)
  );

  for (const fn of file.functions) {
    const lineCount = fn.endLine - fn.startLine + 1;
    const fnKey = `${fn.name}:${fn.startLine}`;
    const isExported = exportedFnKeys.has(fnKey);

    if (!isExported && lineCount < 10) continue;

    // Skip trivial getters/setters even if exported
    if (isExported && lineCount <= 5 && (
      fn.name === 'len' || fn.name === 'is_empty' ||
      fn.name.startsWith('get_') || fn.name.startsWith('set_') ||
      fn.name === 'role' || fn.name === 'label' || fn.name === 'value' ||
      fn.name === 'style' || fn.name === 'children'
    )) continue;

    const fnId = `function:${path}:${fn.name}`;

    // Deduplicate function names by appending line number if needed
    const dedupeKey = nodes.filter(n => n.id.startsWith(fnId)).length;
    const actualFnId = dedupeKey > 0 ? `${fnId}_${fn.startLine}` : fnId;

    const fnSummary = `${fn.name} handles ${fn.name.replace(/_/g, ' ')} in ${path.split('/').pop().replace('.rs', '')}.`;
    const fnTags = [...new Set([
      ...meta.tags,
      fn.name.startsWith('test_') ? 'test' : null,
      isExported ? 'public-api' : null,
      'function'
    ].filter(Boolean))].slice(0, 5);

    nodes.push({
      id: actualFnId,
      type: "function",
      name: fn.name,
      filePath: path,
      lineRange: [fn.startLine, fn.endLine],
      summary: fnSummary,
      tags: fnTags,
      complexity: lineCount > 100 ? "complex" : lineCount > 50 ? "moderate" : "simple",
    });

    edges.push({
      source: fileId,
      target: actualFnId,
      type: "contains",
      direction: "forward",
      weight: 1.0
    });

    if (isExported) {
      edges.push({
        source: fileId,
        target: actualFnId,
        type: "exports",
        direction: "forward",
        weight: 0.8
      });
    }
  }
}

// Write output
const output = { nodes, edges };

const nodeCount = nodes.length;
const edgeCount = edges.length;
console.log(`Total nodes: ${nodeCount}, Total edges: ${edgeCount}`);

// Decide split
if (nodeCount <= 60 && edgeCount <= 120) {
  fs.writeFileSync('.understand-anything/intermediate/batch-5.json', JSON.stringify(output, null, 2));
  console.log('Written as single file batch-5.json');
} else {
  const parts = Math.ceil(Math.max(nodeCount / 60, edgeCount / 120));
  console.log(`Splitting into ${parts} parts`);

  const filePaths = [...new Set(nodes.filter(n => n.type === 'file').map(n => n.filePath))];
  filePaths.sort();

  const filesPerPart = Math.ceil(filePaths.length / parts);
  let writtenParts = 0;
  for (let i = 0; i < parts; i++) {
    const partFilePaths = filePaths.slice(i * filesPerPart, (i + 1) * filesPerPart);
    if (partFilePaths.length === 0) continue;

    const partFileIds = new Set(partFilePaths.map(p => `file:${p}`));

    const partNodes = nodes.filter(n => {
      if (n.type === 'file') return partFileIds.has(n.id);
      return n.filePath && partFilePaths.includes(n.filePath);
    });

    if (partNodes.length === 0) continue;

    const partNodeIds = new Set(partNodes.map(n => n.id));
    const partEdges = edges.filter(e => partNodeIds.has(e.source));

    writtenParts++;
    const filename = `.understand-anything/intermediate/batch-5-part-${writtenParts}.json`;
    const part = { nodes: partNodes, edges: partEdges };
    fs.writeFileSync(filename, JSON.stringify(part, null, 2));
    console.log(`Part ${writtenParts}: ${partNodes.length} nodes, ${partEdges.length} edges -> ${filename}`);
  }
  console.log(`Total: ${writtenParts} parts written`);
}
