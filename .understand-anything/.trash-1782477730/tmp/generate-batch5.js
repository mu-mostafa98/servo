const fs = require('fs');
const path = require('path');

// Read extraction results and batch input
const extractData = JSON.parse(fs.readFileSync('d:/Projects/servo/.understand-anything/tmp/ua-file-extract-results-5.json', 'utf8'));
const inputData = JSON.parse(fs.readFileSync('d:/Projects/servo/.understand-anything/tmp/ua-file-analyzer-input-5.json', 'utf8'));

const batchImportData = inputData.batchImportData;

// Build a map of file path -> result
const resultMap = {};
for (const r of extractData.results) {
  resultMap[r.path] = r;
}

// Check if a function is exported (by name proximity)
function isExportedFn(fn, exports) {
  for (const e of exports) {
    if (e.name === fn.name && Math.abs(e.line - fn.startLine) < 5) return true;
  }
  return false;
}

function isExportedClass(cls, exports) {
  for (const e of exports) {
    if (e.name === cls.name && Math.abs(e.line - cls.startLine) < 5) return true;
  }
  return false;
}

// Generate file-level summary based on path and data
function getFileSummary(path) {
  const summaries = {
    'accessibility_tree': 'Implements the accessibility tree (AccessKit) for layout, synchronizing DOM nodes with accessibility nodes for screen readers and assistive technologies.',
    'cell': 'Provides ArcRefCell and WeakRefCell smart pointer types extending Servo\'s cell-based threading primitives with atomic reference counting for layout data.',
    'construct_modern': 'Modern flow construction builder that creates block containers and handles inline/block-level element creation during layout tree construction.',
    'context': 'Defines the LayoutContext and ImageResolver, providing shared layout state including style context, font context, and image resolution with caching.',
    'background': 'Handles background display list painting including background layers, painting areas, clipping, and tiled background image rendering.',
    'clip': 'Manages display list clipping operations including clip paths, basic shapes (circle/ellipse), and the stacking context clip store.',
    'conversions': 'Provides trait implementations for converting Servo layout types into WebRender display item representations.',
    'gradient': 'Builds CSS gradient display items for WebRender, supporting linear, radial, and conic gradients with color stop interpolation.',
    'hit_test': 'Implements hit testing against the display list, determining which elements are at a given point including clip and rounded rect testing.',
    'paint_timing_handler': 'Tracks Largest Contentful Paint (LCP) candidates during display list painting for performance timing metrics.',
    'paint_traversal': 'Traverses the stacking context and fragment tree to drive display list painting, handling block-level, inline, and replaced content.',
    'stacking_context': 'Builds and manages the stacking context tree with scroll frames, sticky frames, reference frames, and transform/perspective matrix calculations.',
    'flexbox/geom': 'Flexbox geometry primitives including FlexRelativeVec2/Rect and FlexAxis for coordinate conversion between flex and flow-relative spaces.',
    'flexbox/layout': 'Core flexbox layout algorithm implementing item sizing, line breaking, alignment, cross-axis distribution, and flexible length resolution.',
    'flexbox/mod': 'Flexbox container construction and FlexLevelBox/FlexItemBox wrapper types for integrating flex items into the layout tree.',
    'flow/construct': 'Block flow construction module that builds flow tree fragments from DOM elements, handling block-level, inline-level, float, and absolutely positioned elements.',
    'flow/float': 'Float layout implementation using FloatContext, FloatBandTree, and SequentialLayoutState for placing floats and calculating inline clearance.',
    'flow/inline/construct': 'Inline formatting context construction, building inline items and text segments from DOM content with whitespace collapsing and text transformations.',
    'flow/inline/inline_box': 'Manages inline box data structures including InlineBox, InlineBoxes tree, and InlineBoxContainerState for inline formatting context layout.',
    'flow/inline/line': 'Line item layout engine that arranges inline content (text runs, atomics, floats, absolutes) into formatted lines with bidi reordering and whitespace trimming.',
    'flow/inline/line_breaker': 'Line breaking utility that identifies soft wrap opportunities in text based on Unicode line break properties.',
    'flow/inline/text_run': 'Text run shaping and segmentation for inline layout, handling font selection, script detection, and glyph generation for the inline formatter.',
    'flow/root': 'Root box tree construction and viewport overflow handling for the initial containing block layout.',
    'flow/same_formatting_context_block': 'Layout of block-level elements within the same formatting context, handling inline-size solving, margin resolution, and sequential block placement.',
    'formatting_contexts': 'Independent formatting context implementation (replaced, flow, flex, grid, table) with layout caching, aspect ratio handling, and content sizing.',
  };
  for (const [key, val] of Object.entries(summaries)) {
    if (path.includes(key)) return val;
  }
  if (path.includes('display_list/mod.rs')) return 'Core display list builder module that orchestrates display list construction from stacking contexts, handling text, images, borders, backgrounds, and shadows.';
  if (path.includes('dom.rs')) return 'Defines the LayoutBox enum and DOM layout data structures, providing the bridge between the DOM tree and the layout fragment tree.';
  if (path.includes('dom_traversal.rs')) return 'DOM traversal infrastructure for layout, handling pseudo-elements, content generation, and recursive element/child traversal.';
  if (path.includes('flow/mod.rs')) return 'Block formatting context and block container layout including block-level child placement, margin collapsing, float clearance, and inline content sizing.';
  if (path.includes('flow/inline/mod.rs')) return 'Core inline formatting context module managing InlineFormattingContext, inline item types, line construction, float placement, and inline content sizing.';
  return 'Rust source file in the Servo layout engine.';
}

// Determine complexity
function getFileComplexity(nonEmptyLines, fnCount, classCount) {
  if (nonEmptyLines < 50 && fnCount < 5) return 'simple';
  if (nonEmptyLines < 200 && fnCount < 15) return 'moderate';
  return 'complex';
}

// Get tags
function getFileTags(path) {
  const tags = ['rust', 'layout'];
  if (path.includes('display_list')) tags.push('display-list');
  if (path.includes('flexbox')) tags.push('flexbox');
  if (path.includes('flow/inline')) tags.push('inline-formatting');
  if (path.includes('flow')) tags.push('flow');
  if (path.includes('accessibility')) tags.push('accessibility');
  if (path.includes('dom')) tags.push('dom');
  if (path.includes('context')) tags.push('context');
  if (path.includes('float')) tags.push('float');
  if (path.includes('gradient')) tags.push('gradient');
  if (path.includes('hit_test')) tags.push('hit-test');
  if (path.includes('clip')) tags.push('clip');
  if (path.includes('stacking_context')) tags.push('stacking-context');
  if (path.includes('line_breaker')) tags.push('line-breaking');
  if (path.includes('construct_modern')) tags.push('construction');
  if (path.includes('paint_timing')) tags.push('paint-timing');
  if (path.includes('paint_traversal')) tags.push('paint');
  if (path.includes('background')) tags.push('background');
  if (path.includes('conversions')) tags.push('conversion');
  if (path.includes('text_run')) tags.push('text-shaping');
  if (path.includes('root.rs')) tags.push('root');
  if (path.includes('cell.rs')) tags.push('smart-pointer');
  if (path.includes('formatting_contexts')) tags.push('formatting-context');
  if (path.includes('geom')) tags.push('geometry');
  if (path.includes('same_formatting_context')) tags.push('block-formatting');
  return tags.slice(0, 5);
}

function getFunctionSummary(name) {
  const summaries = {
    'layout': 'Performs the main layout computation for this context type.',
    'new': 'Constructs a new instance with the provided parameters.',
    'construct': 'Builds the flow/fragment tree from DOM content during layout construction.',
    'finish': 'Finalizes construction or layout by post-processing accumulated state.',
    'build': 'Builds display list items or gradients for WebRender rendering.',
    'layout_into_line_items': 'Lays out content into formatted line items within the inline formatting context.',
    'traverse': 'Traverses the context tree driving layout or paint operations.',
    'repair_style': 'Updates layout state when the computed style changes for a node.',
    'attached_to_tree': 'Handles operations when a layout node is attached to the box tree.',
    'with_base': 'Provides access to the base fragment info via a callback.',
    'with_base_mut': 'Provides mutable access to the base fragment info via a callback.',
    'subtree_size': 'Computes the total subtree size for this layout node.',
    'push_text': 'Processes and pushes a text segment into the inline formatting context.',
    'place_float_fragment': 'Positions a float fragment accounting for clearance and margin collapsing.',
    'layout_in_flow_block_level': 'Lays out a block-level element in the normal flow with margin collapsing.',
    'build_background': 'Builds background display items for a box fragment.',
    'build_border': 'Builds border display items for a box fragment.',
    'build_box_shadow': 'Builds box shadow display items for a box fragment.',
    'build_outline': 'Builds outline display items for a box fragment.',
    'build_stacking_context_tree': 'Recursively builds the stacking context tree from fragments.',
    'calculate_transform_matrix': 'Computes the CSS transform matrix for a containing block.',
    'calculate_perspective_matrix': 'Computes the CSS perspective matrix for a containing block.',
    'push_reference_frame': 'Pushes a new reference frame spatial node into the scroll tree.',
    'define_scroll_frame': 'Defines a scroll frame spatial node with clipping and scroll sensitivity.',
    'define_sticky_frame': 'Defines a sticky-positioned frame with offset bounds.',
    'solve_margins': 'Solves for auto margins in block-level layout.',
    'find_block_margin_collapsing_with_parent': 'Calculates margin collapsing between a block and its parent.',
    'handle_element': 'Processes a DOM element during flow construction.',
    'handle_text': 'Processes a text node during flow construction.',
    'push_atomic': 'Pushes an atomic inline-level element (replaced content) into the inline builder.',
    'push_absolutely_positioned_box': 'Pushes an absolutely positioned box into the inline builder.',
    'push_float_box': 'Pushes a float box into the inline builder.',
    'start_inline_box': 'Begins a new inline box scope for nested inline formatting.',
    'end_inline_box': 'Ends the current inline box scope.',
    'get_or_request_image_or_meta': 'Fetches or requests image metadata for display list rendering.',
    'resolve_image': 'Resolves an image from the image cache or fetches it from the network.',
    'get_cached_image_for_url': 'Retrieves a cached image for the given URL, requesting it if not available.',
    'rasterize_vector_image': 'Rasterizes an SVG or vector image at the specified size.',
    'layout_layer': 'Lays out a single background layer including positioning and tiling.',
    'add_for_clip_path': 'Adds a clip chain entry for a CSS clip-path.',
    'add_for_basic_shape': 'Adds a clip chain entry for a CSS basic shape (circle, ellipse).',
    'compute_shape_radius_for_circle': 'Computes the radius for CSS circle() shape function.',
    'compute_shape_radius_for_ellipse': 'Computes the radii for CSS ellipse() shape function.',
    'run': 'Executes the main operation of this type.',
    'hit_test': 'Tests if a point hits any display item, returning the topmost element.',
    'rounded_rect_contains_point': 'Tests whether a point is inside a rounded rectangle.',
    'cursor': 'Determines the CSS cursor for a hit-tested element.',
    'check_bounding_rect': 'Checks if a bounding rect intersects the viewport for LCP.',
    'calculate_intersection_rect': 'Calculates the intersection rectangle between bounds and viewport clip.',
    'update_lcp_candidate': 'Updates the Largest Contentful Paint candidate with the given metrics.',
    'rebuild_box_tree_from_independent_formatting_context': 'Rebuilds the box tree for an independent formatting context.',
    'with_pseudo_element': 'Executes a callback with pseudo-element style information.',
    'traverse_children_of': 'Recursively traverses the children of a DOM element for layout.',
    'generate_pseudo_element_content': 'Generates rendered content for CSS pseudo-elements.',
    'collect_fragment': 'Collects and positions flex item fragments into flex lines.',
    'resolve_flexible_lengths': 'Resolves flex item lengths distributing free space according to flex factors.',
    'to_flex_item': 'Converts a layout box into a FlexItem with computed sizing properties.',
    'finish_with_final_cross_size': 'Completes flex layout by setting final cross sizes on all items.',
    'compute_inline_content_sizes': 'Calculates the min and max inline content sizes for a node.',
    'main_content_sizes': 'Computes main-axis content size contributions in flex layout.',
    'cross_content_sizes': 'Computes cross-axis content sizes for flex items.',
    'layout_block_level_children': 'Lays out block-level children within a block formatting context.',
    'layout_block_level_children_sequentially': 'Sequentially lays out block-level children with margin collapsing.',
    'layout_block_level_child': 'Lays out a single block-level child within a formatting context.',
    'solve_containing_block_padding_and_border_for_in_flow_box': 'Solves padding/border for in-flow block-level elements.',
    'place_fragment': 'Positions a fragment within its containing block accounting for float avoidance.',
    'build_linear': 'Builds a linear gradient display item for WebRender.',
    'build_radial': 'Builds a radial gradient display item for WebRender.',
    'build_conic': 'Builds a conic gradient display item for WebRender.',
    'fixup_stops': 'Normalizes gradient color stop positions to the [0, 1] range.',
    'interpolate_gradient_stop_colors': 'Interpolates gradient stops using the specified color interpolation method.',
    'create_webrender_stops': 'Converts gradient stops into WebRender stops with interpolation and extend mode.',
    'conic_gradient_items_to_color_stops': 'Converts conic gradient items into an ordered list of color stops.',
    'gradient_items_to_color_stops': 'Converts generic gradient items into color stops along the gradient line.',
    'layout_and_is_cached': 'Performs layout with caching if the input constraints match a cached result.',
    'layout_without_caching': 'Performs layout computation without checking the cache.',
    'tentative_block_content_size': 'Computes a tentative block content size from preferred aspect ratio.',
    'tentative_block_content_size_with_dependency': 'Computes tentative block size accounting for size dependencies.',
    'outer_inline_content_sizes': 'Calculates outer inline sizes including padding, border, and margins.',
    'preferred_aspect_ratio': 'Resolves the preferred aspect ratio for replaced elements.',
    'construct_contents': 'Constructs the inner contents of an independent formatting context.',
    'set_ceiling_from_non_floats': 'Sets the float ceiling from non-float elements in the formatting context.',
    'place_object': 'Places a float object into the float context, finding the optimal position.',
    'add_float': 'Adds a new float to the float context and adjusts band layout.',
    'calculate_clear_position': 'Calculates the block position accounting for CSS clear property.',
    'place_float_fragment': 'Places a float fragment within the sequential layout state.',
    'set_children': 'Sets children of an accessibility node, maintaining parent-child relationships.',
    'update_descendants_from_dom_node': 'Updates accessibility tree descendants from the corresponding DOM node subtree.',
  };
  return summaries[name] || null;
}

function ensureThreeTags(tags, isClass) {
  if (tags.length >= 3) return tags.slice(0, 5);
  const defaults = ['servo', 'layout', 'rust'];
  if (isClass) defaults.unshift('class');
  else defaults.unshift('function');
  for (const d of defaults) {
    if (tags.length >= 3) break;
    if (!tags.includes(d)) tags.push(d);
  }
  return tags.slice(0, 5);
}

function getClassSummary(name) {
  const summaries = {
    'AccessibilityTree': 'Manages the accessibility tree structure, synchronizing layout nodes with AccessKit for screen reader support.',
    'AccessibilityNode': 'Represents a single node in the accessibility tree with role, label, and state information.',
    'AccessibilityUpdate': 'Tracks accessibility tree changes during layout updates for batched processing.',
    'TreeChange': 'Enumerates the types of changes that can occur in the accessibility tree.',
    'InlineFormattingContext': 'Manages inline layout state including text runs, inline boxes, and float containment.',
    'InlineFormattingContextBuilder': 'Constructs inline formatting context items from DOM text and element content.',
    'InlineFormattingContextLayout': 'Drives the inline layout process managing lines, floats, and inline box nesting.',
    'InlineBox': 'Represents an inline box with shared styles, font info, and layout state.',
    'InlineBoxes': 'Manages the tree of nested inline boxes within a formatting context.',
    'InlineBoxContainerState': 'Tracks container-level inline state including font metrics and baseline offsets.',
    'LineItemLayout': 'Coordinates line item layout including bidi reordering and inline box handling.',
    'LineItem': 'Enumerates line content types: inline boxes, text runs, atomic elements, floats, and absolutes.',
    'TextRunLineItem': 'Represents a text run within a line with trimming and merging capabilities.',
    'LayoutContext': 'Central layout context holding style system, font context, image resolver, and parallelism configuration.',
    'ImageResolver': 'Manages image loading, caching, and resolution for display list rendering.',
    'DisplayListBuilder': 'Orchestrates WebRender display list construction from the stacking context tree and fragments.',
    'StackingContextTree': 'Manages the tree of stacking contexts with clip store and paint info.',
    'StackingContext': 'Represents a CSS stacking context with children, z-index ordering, and fragment association.',
    'ContainingBlock': 'Represents a containing block with scroll node and clip information for stacking context construction.',
    'PaintTraversalHandler': 'Implements fragment traversal callbacks for driving the display list paint process.',
    'TraversalState': 'Tracks spatial node, clip ID, and text decoration state during paint traversal.',
    'PaintTimingHandler': 'Tracks Largest Contentful Paint (LCP) performance metrics during display list painting.',
    'LayoutBox': 'Enum representing different types of layout boxes: block-level, inline-level, flex, table, and text.',
    'InnerDOMLayoutData': 'Stores layout data associated with a DOM node including box and pseudo-box storage.',
    'NodeExt': 'Extension trait on DOM nodes providing layout-specific operations and box tree management.',
    'FlexContainer': 'Represents a flex container managing flex items and configuration.',
    'FlexLevelBox': 'Wrapper for flex items and out-of-flow absolutely positioned boxes in flex layout.',
    'FlexItemBox': 'Represents an individual flex item within the flex container.',
    'FlexItem': 'Stores computed flex item properties including base size, hypothetical size, and alignment.',
    'FlexItemLayoutResult': 'Holds the result of flex item layout including fragments and baseline info.',
    'FlexAxis': 'Represents the flex axis direction (row or column) with coordinate conversion methods.',
    'BoxTree': 'The root of the layout box tree constructed from the DOM for the initial containing block.',
    'BlockFormattingContext': 'A block formatting context that lays out block-level children in the normal flow.',
    'BlockContainer': 'A block container that can hold either block-level boxes or an inline formatting context.',
    'BlockLevelBox': 'Enum representing block-level box types: independent, absolutely positioned, float, marker, and same-context.',
    'OutsideMarker': 'Handles layout of CSS list-style markers placed outside the list item box.',
    'SameFormattingContextBlock': 'A block-level element that shares the same formatting context as its parent.',
    'IndependentFormattingContext': 'An independent formatting context (replaced, flow, flex, grid, table) with layout caching.',
    'InlineItem': 'Enum representing inline-level items: inline boxes, text runs, floats, atomics, and absolutes.',
    'WhitespaceCollapse': 'Handles CSS white-space collapsing during text processing for inline layout.',
    'SequentialLayoutState': 'Tracks float and margin state during sequential block-level layout.',
    'FloatContext': 'Manages float placement within a formatting context using float band tracking.',
    'FloatBox': 'A box that participates in float layout with placement and clearance calculation.',
    'FloatBandTree': 'Tree structure tracking available space bands between floats in a formatting context.',
    'TraversalHandler': 'Trait defining callbacks for DOM traversal during layout construction.',
    'StackingContextTreeClipStore': 'Manages clip IDs and clip chains within the stacking context tree.',
    'Clip': 'Represents a clip region with radii and associated scroll node.',
    'BackgroundLayer': 'Enumerates background painting layers (background-color, background-image).',
    'BackgroundPainter': 'Paints CSS background layers including images, colors, and tiling.',
    'WebRenderGradient': 'Represents a WebRender gradient with stops and interpolation configuration.',
    'HitTest': 'Performs hit testing against the stacking context tree display items.',
    'WeakLayoutBox': 'A weak reference to a LayoutBox that can be upgraded to a strong reference.',
    'DOMLayoutData': 'Layout data stored per DOM node including box tree entries.',
    'BoxSlot': 'Enum representing layout box storage slots for different box types.',
    'FlexContainerConfig': 'Configuration for flex container layout behavior.',
    'BlockContainerBuilder': 'Assembles block containers from processed DOM content during flow construction.',
    'BlockLevelCreator': 'Creates block-level boxes from DOM elements during flow construction.',
    'MainStartCrossStart': 'Represents the main-start/cross-start pair for flex layout direction.',
    'FlexRelativeVec2': 'A 2D vector in flex-relative coordinate space.',
    'FlexRelativeRect': 'A rectangle in flex-relative coordinate space.',
    'PlacementAmongFloats': 'Represents the available inline space among floats at a given block position.',
    'PlacementInfo': 'Information about a float placement including size, clear, and margin.',
    'FloatBand': 'Represents a horizontal band of space between two vertical float intrusions.',
    'LineBreaker': 'Identifies line break opportunities in text using Unicode line break properties.',
    'LineMetrics': 'Metrics for a formatted line including advance, block size, and baseline offset.',
    'TextRunOffsets': 'Byte and character offsets for a text run within the inline content.',
    'AtomicLineItem': 'An atomic inline-level line item (replaced element or inline-block).',
    'AbsolutelyPositionedLineItem': 'An absolutely positioned element within a line.',
    'FloatLineItem': 'A float element within a line layout context.',
    'LineUnderConstruction': 'Tracks state while constructing a single line in the inline formatter.',
    'UnbreakableSegmentUnderConstruction': 'Tracks an unbreakable segment of inline content during line construction.',
    'BaselineRelativeSize': 'Represents ascent and descent sizes for baseline alignment.',
    'LineBlockSizes': 'Tracks line block sizes including line-height contributions from fonts.',
    'SharedInlineStyles': 'Shared style information for inline boxes and text runs.',
    'InlineContainerState': 'State tracking for an inline container including font metrics and baseline.',
    'ContentSizesComputation': 'Computes min and max content sizes for an inline formatting context.',
    'FontAndScriptInfo': 'Font and script metadata for text shaping and run segmentation.',
    'TextRunSegment': 'A segment of a text run with consistent font and script properties.',
    'TextRun': 'A shaped text run with glyph information and layout methods.',
    'TextRunItem': 'A text run item within the inline formatting context.',
    'BidiLevels': 'Bidirectional text levels for inline content reordering.',
    'CollapsibleWithParentStartMargin': 'Tracks whether a block\'s start margin can collapse with its parent.',
    'IndependentFloatOrAtomicLayoutResult': 'Layout result for an independent float or atomic inline element.',
    'Baselines': 'Tracks baseline information for alignment within a formatting context.',
    'IndependentFormattingContextContents': 'Contents of an independent formatting context (replaced, flow, flex, grid, table).',
    'ResolvedImage': 'Represents a resolved image ready for display list rendering.',
    'LayoutImageCacheResult': 'Result of an image cache lookup with loaded image data.',
  };
  return summaries[name] || null;
}

function getNodeTags(path, isClass, name) {
  const tags = [];
  if (isClass) tags.push('class');
  else tags.push('function');
  if (name.toLowerCase().includes('layout')) tags.push('layout');
  if (name.toLowerCase().includes('inline')) tags.push('inline');
  if (name.toLowerCase().includes('float')) tags.push('float');
  if (name.toLowerCase().includes('flex')) tags.push('flexbox');
  if (name.toLowerCase().includes('build')) tags.push('display-list');
  if (name.toLowerCase().includes('context')) tags.push('formatting-context');
  if (name.toLowerCase().includes('box')) tags.push('box-tree');
  if (name.toLowerCase().includes('style')) tags.push('style');
  if (name.toLowerCase().includes('margin')) tags.push('margin');
  if (name.toLowerCase().includes('clip')) tags.push('clip');
  if (name.toLowerCase().includes('text')) tags.push('text');
  if (name.toLowerCase().includes('image')) tags.push('image');
  if (name.toLowerCase().includes('frame')) tags.push('scroll');
  if (name.toLowerCase().includes('accessibility')) tags.push('accessibility');
  if (name.toLowerCase().includes('traversal')) tags.push('traversal');
  if (name.toLowerCase().includes('fragment')) tags.push('fragment');
  if (name.toLowerCase().includes('band')) tags.push('float');
  if (name.toLowerCase().includes('line')) tags.push('line-layout');
  if (name.toLowerCase().includes('gradient')) tags.push('gradient');
  if (name.toLowerCase().includes('paint')) tags.push('paint');
  if (name.toLowerCase().includes('render')) tags.push('webrender');
  if (name.toLowerCase().includes('bidi')) tags.push('bidi');
  if (name.toLowerCase().includes('font')) tags.push('font');
  if (name.toLowerCase().includes('tree')) tags.push('tree');
  if (name.toLowerCase().includes('store')) tags.push('clip');
  if (name.toLowerCase().includes('hit')) tags.push('hit-test');
  if (name.toLowerCase().includes('tree') && isClass) tags.push('tree');
  if (name.toLowerCase().includes('render') && !isClass) tags.push('webrender');
  if (name.toLowerCase().includes('band') && !isClass) tags.push('float');
  if (name.toLowerCase().includes('sign') && !isClass) tags.push('gradient');

  // Ensure we have at least 3 tags
  const defaults = ['servo', 'layout', 'rust'];
  for (const d of defaults) {
    if (tags.length >= 3) break;
    if (!tags.includes(d)) tags.push(d);
  }
  return tags.slice(0, 5);
}

function getFunctionComplexity(fn) {
  const lineCount = fn.endLine - fn.startLine + 1;
  if (lineCount < 20) return 'simple';
  if (lineCount < 80) return 'moderate';
  return 'complex';
}

function getClassComplexity(cls) {
  const lineCount = cls.endLine - cls.startLine + 1;
  if (lineCount < 30) return 'simple';
  if (lineCount < 80) return 'moderate';
  return 'complex';
}

// Now build nodes and edges
const nodes = [];
const edges = [];

// Helper to add a node if not duplicate
const nodeIds = new Set();

function addNode(node) {
  const key = node.id;
  if (nodeIds.has(key)) {
    console.error('Duplicate node ID:', key);
    return;
  }
  nodeIds.add(key);
  nodes.push(node);
}

// Helper to add an edge
const edgeKeys = new Set();

function addEdge(edge) {
  const key = edge.source + '|' + edge.target + '|' + edge.type;
  if (edgeKeys.has(key)) return;
  edgeKeys.add(key);
  edges.push(edge);
}

// Process each file
for (const r of extractData.results) {
  const path = r.path;
  const totalLines = r.totalLines;
  const nonEmptyLines = r.nonEmptyLines;
  const fileName = path.split('/').pop();
  const exports = r.exports || [];
  const functions = r.functions || [];
  const classes = r.classes || [];
  const metrics = r.metrics || {};

  // File node
  const fileSummary = getFileSummary(path);
  const fileTags = getFileTags(path);
  const fileComplexity = getFileComplexity(nonEmptyLines, functions.length, classes.length);

  const fileNode = {
    id: 'file:' + path,
    type: 'file',
    name: fileName,
    filePath: path,
    summary: fileSummary,
    tags: fileTags,
    complexity: fileComplexity
  };

  if (nonEmptyLines > 1000) {
    fileNode.languageNotes = 'Rust source with extensive use of enums, pattern matching, and trait-based polymorphism typical of the Servo layout engine.';
  }

  addNode(fileNode);

  // Check which functions are significant (>=10 lines OR exported)
  const significantFunctions = functions.filter(fn => {
    const lineCount = fn.endLine - fn.startLine + 1;
    if (lineCount >= 10) return true;
    return isExportedFn(fn, exports);
  });

  // Handle duplicate function names by tracking name usage
  const functionNameCounts = {};
  for (const fn of significantFunctions) {
    const count = (functionNameCounts[fn.name] || 0) + 1;
    functionNameCounts[fn.name] = count;
  }
  const functionNameIndex = {};

  // Create function nodes
  for (const fn of significantFunctions) {
    // Disambiguate duplicate function names using index
    const idx = functionNameIndex[fn.name] || 0;
    functionNameIndex[fn.name] = idx + 1;
    let fnId;
    if (functionNameCounts[fn.name] > 1) {
      fnId = 'function:' + path + ':' + fn.name + '_' + (idx + 1);
    } else {
      fnId = 'function:' + path + ':' + fn.name;
    }
    const lineCount = fn.endLine - fn.startLine + 1;

    let fnSummary = getFunctionSummary(fn.name);
    if (!fnSummary) {
      fnSummary = 'Handles ' + fn.name.replace(/_/g, ' ') + ' logic within ' + fileName.replace('.rs', '') + '.';
    }

    const fnNode = {
      id: fnId,
      type: 'function',
      name: fn.name,
      filePath: path,
      lineRange: [fn.startLine, fn.endLine],
      summary: fnSummary,
      tags: ensureThreeTags(getNodeTags(path, false, fn.name), false),
      complexity: getFunctionComplexity(fn)
    };

    if (lineCount > 100) {
      fnNode.languageNotes = 'Extensive Rust function leveraging pattern matching and iterator combinators for complex layout logic.';
    }

    addNode(fnNode);

    // Contains edge
    addEdge({
      source: 'file:' + path,
      target: fnId,
      type: 'contains',
      direction: 'forward',
      weight: 1.0
    });

    // Exports edge if exported
    if (isExportedFn(fn, exports)) {
      addEdge({
        source: 'file:' + path,
        target: fnId,
        type: 'exports',
        direction: 'forward',
        weight: 0.8
      });
    }
  }

  // Check which classes are significant (2+ methods OR 20+ lines OR exported)
  const significantClasses = classes.filter(cls => {
    const lineCount = cls.endLine - cls.startLine + 1;
    if (cls.methods && cls.methods.length >= 2) return true;
    if (lineCount >= 20) return true;
    return isExportedClass(cls, exports);
  });

  // Disambiguation for duplicate class names
  const classNameCounts = {};
  for (const cls of significantClasses) {
    const count = (classNameCounts[cls.name] || 0) + 1;
    classNameCounts[cls.name] = count;
  }
  const classNameIndex = {};

  // Create class nodes
  for (const cls of significantClasses) {
    const idx = classNameIndex[cls.name] || 0;
    classNameIndex[cls.name] = idx + 1;
    let clsId;
    if (classNameCounts[cls.name] > 1) {
      clsId = 'class:' + path + ':' + cls.name + '_' + (idx + 1);
    } else {
      clsId = 'class:' + path + ':' + cls.name;
    }
    const lineCount = cls.endLine - cls.startLine + 1;

    let clsSummary = getClassSummary(cls.name);
    if (!clsSummary) {
      const methods = cls.methods || [];
      const props = cls.properties || [];
      const detailParts = [];
      if (methods.length > 0) detailParts.push(methods.length + ' methods');
      if (props.length > 0) detailParts.push(props.length + ' properties');
      clsSummary = 'Structural type for ' + cls.name + ' in ' + fileName.replace('.rs', '') + ' with ' + detailParts.join(', ') + '.';
    }

    const methods = cls.methods || [];
    const properties = cls.properties || [];

    const clsNode = {
      id: clsId,
      type: 'class',
      name: cls.name,
      filePath: path,
      lineRange: [cls.startLine, cls.endLine],
      summary: clsSummary,
      tags: ensureThreeTags(getNodeTags(path, true, cls.name), true),
      complexity: getClassComplexity(cls)
    };

    if (methods.length > 5 || properties.length > 5) {
      clsNode.languageNotes = 'Rust struct with ' + methods.length + ' methods and ' + properties.length + ' fields, using Servo\'s fragment-oriented layout pattern.';
    }

    addNode(clsNode);

    // Contains edge
    addEdge({
      source: 'file:' + path,
      target: clsId,
      type: 'contains',
      direction: 'forward',
      weight: 1.0
    });

    // Exports edge if exported
    if (isExportedClass(cls, exports)) {
      addEdge({
        source: 'file:' + path,
        target: clsId,
        type: 'exports',
        direction: 'forward',
        weight: 0.8
      });
    }
  }
}

// Import edges from batchImportData
for (const [sourcePath, imports] of Object.entries(batchImportData)) {
  const sourceNodeId = 'file:' + sourcePath;
  for (const targetPath of imports) {
    const targetNodeId = 'file:' + targetPath;
    addEdge({
      source: sourceNodeId,
      target: targetNodeId,
      type: 'imports',
      direction: 'forward',
      weight: 0.7
    });
  }
}

// Validate: sum of imports edges must equal sum of batchImportData lengths
let expectedImportCount = 0;
for (const imports of Object.values(batchImportData)) {
  expectedImportCount += imports.length;
}
const actualImportEdges = edges.filter(e => e.type === 'imports').length;
console.log('Expected imports:', expectedImportCount, 'Actual:', actualImportEdges);

console.log('Total nodes:', nodes.length);
console.log('Total edges:', edges.length);

fs.writeFileSync('d:/Projects/servo/.understand-anything/tmp/ua-batch5-graph.json', JSON.stringify({ nodes, edges }, null, 2));
console.log('Written successfully');
