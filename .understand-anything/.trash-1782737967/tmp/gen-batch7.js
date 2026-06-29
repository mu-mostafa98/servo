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
  if (f.name === 'from' && (f.params || []).length <= 1 && lc < 10) return false;
  return (isEx && lc >= 3) || lc >= 10;
}

function isSigClass(c, exportedNames) {
  const lc = c.endLine - c.startLine + 1;
  const isEx = exportedNames.has(c.name);
  return (c.methods && c.methods.length >= 2) || lc >= 20 || isEx;
}

// File metadata
const fileMeta = {};

const metaData = [
  ['components/layout/fragment_tree/base_fragment.rs', 'Defines the BaseFragment, FragmentStatus, BaseFragmentInfo, and Tag types that form the core fragment data structure, providing shared fields (rect, style, status, flags) and construction logic for all fragment types in the layout tree.', ['fragment-tree', 'data-model', 'layout', 'core-types']],
  ['components/layout/fragment_tree/box_fragment.rs', 'Implements BoxFragment, the primary fragment type representing CSS boxes with children, containing block tracking, scrollable overflow calculation, baseline management, background modes, and resolved positioning insets.', ['fragment-tree', 'box-model', 'layout', 'scrollable-overflow']],
  ['components/layout/fragment_tree/containing_block.rs', 'Provides ContainingBlockManager that tracks the containing block chain for fragments, supporting non-absolute, absolute, and fixed descendant containment strategies during layout.', ['fragment-tree', 'containing-block', 'layout', 'positioning']],
  ['components/layout/fragment_tree/fragment.rs', 'Defines the Fragment enum (the top-level fragment abstraction) with variants for LayoutRoot, Box, Float, Positioning, Text, Image, and IFrame fragments, plus CollapsedBlockMargins/CollapsedMargin for margin collapsing and the ContainingBlockCalculation state machine.', ['fragment-tree', 'data-model', 'layout', 'enum']],
  ['components/layout/fragment_tree/fragment_tree.rs', 'Implements FragmentTree, the top-level container that holds the root fragment tree for a layout document, providing scrolling area computation, fragment finding by position, and body fragment resolution.', ['fragment-tree', 'tree-structure', 'layout', 'root']],
  ['components/layout/fragment_tree/hoisted_shared_fragment.rs', 'Defines HoistedSharedFragment, a lightweight wrapper for fragments hoisted out of the normal tree flow (e.g., for absolutely positioned elements), with an optional style override.', ['fragment-tree', 'hoisted', 'absolute-positioning', 'layout']],
  ['components/layout/fragment_tree/mod.rs', 'Module barrel file re-exporting all fragment tree types for public use by other layout modules.', ['fragment-tree', 'barrel', 'module', 're-exports']],
  ['components/layout/fragment_tree/positioning_fragment.rs', 'Implements PositioningFragment, which wraps fragments needing special positioning treatment (e.g., floats), providing containing block offset, scrollable overflow, and print debugging support.', ['fragment-tree', 'positioning', 'float', 'layout']],
  ['components/layout/geom.rs', 'Defines logical geometry types (LogicalVec2, LogicalRect, LogicalSides, LogicalSides1D) for writing-mode-aware CSS layout, plus SyncPhysicalRectAu for thread-safe physical rect storage and conversion between physical and logical coordinate spaces.', ['geometry', 'logical-coordinates', 'writing-modes', 'layout']],
  ['components/layout/layout_box_base.rs', 'Implements LayoutBoxBase, the foundational structure for layout boxes, managing fragment storage, inline content sizing, caching of independent formatting context and same-formatting-context block layout results, and invalidation of layout caches.', ['layout-box', 'base', 'fragment-storage', 'caching']],
  ['components/layout/layout_impl.rs', 'Implements LayoutThread, the core layout engine loop: reflow orchestration, style resolution, font loading, display list building, accessibility tree updates, stacking context tree construction, and query handling for resolved styles, box areas, scroll containers, and font metrics.', ['layout-engine', 'reflow', 'display-list', 'styling', 'queries']],
  ['components/layout/layout_root.rs', 'Provides LayoutRoot, the initial containing block layout root that performs the top-level layout pass, determining viewport size and running the full box tree layout algorithm.', ['layout-root', 'initial-containing-block', 'viewport', 'layout']],
  ['components/layout/lib.rs', 'Module entry point for the layout crate, re-exporting core types (ConstraintSpace, ContainingBlock, PropagatedBoxTreeData) and establishing the crate public API surface.', ['crate-root', 'barrel', 'module', 'layout']],
  ['components/layout/lists.rs', 'Provides CSS list marker generation including ordered list counter representation, marker string formatting (decimal, roman, alpha styles), and marker box construction for list items.', ['lists', 'counters', 'markers', 'css']],
  ['components/layout/positioned.rs', 'Implements absolute and fixed positioning layout: PositioningContext management, hoisting of absolutely positioned boxes, static position adjustment, inset resolution, margin auto-solving, and layout of the initial containing block children.', ['positioned-layout', 'absolute', 'fixed', 'insets', 'hoisting']],
  ['components/layout/query.rs', 'Handles layout thread queries from script (DOM APIs): resolved styles, box areas, client rects, scroll containers, offset parents, font metrics, containing blocks, text indices, and element-from-point hit testing.', ['queries', 'dom-api', 'resolved-style', 'layout-query']],
  ['components/layout/quotes.rs', 'Provides CSS quotes data, mapping language codes to their corresponding open/close quote pairs for the quotes CSS property.', ['quotes', 'css', 'localization', 'typography']],
  ['components/layout/replaced.rs', 'Implements replaced element layout (images, iframes, svg, video, canvas): content sizing from natural dimensions, aspect ratio preservation, SVG render tree construction, and fragment creation for replaced content.', ['replaced-elements', 'images', 'svg', 'iframe', 'content-sizing']],
  ['components/layout/sizing.rs', 'Provides content sizing infrastructure: ContentSizes (min/max content), SizeConstraint, Sizes resolution, intrinsic sizing modes, inline content size computation, and preferred/max/min size resolution for CSS layout.', ['sizing', 'intrinsic-sizes', 'content-sizes', 'constraints']],
  ['components/layout/style_ext.rs', 'Extends Servo computed styles with layout-specific methods: writing mode queries, box size accessors, overflow handling, transform processing, stacking context establishment, containing block creation, padding/border/margin calculations, and aspect ratio resolution.', ['style-extensions', 'css-properties', 'layout-style', 'overflow']],
  ['components/layout/table/construct.rs', 'Implements anonymous table object construction: walks the DOM tree to build table structure with rows, cells, columns, and column groups, handling rowspan/colspan and creating anonymous wrappers as needed per the CSS table model.', ['table', 'anonymous-objects', 'construct', 'rowspan-colspan']],
  ['components/layout/table/layout.rs', 'Implements table layout algorithm: column width distribution, row height calculation, collapsed border resolution, cell layout, caption placement, and table-specific inline content sizing.', ['table', 'layout-algorithm', 'column-widths', 'border-collapse', 'row-heights']],
  ['components/layout/table/mod.rs', 'Module re-exporting table-level box types: Table, TableSlot, TableTrack, TableTrackGroup, TableCaption, collapsed borders, and related types for table layout.', ['table', 'barrel', 'module', 'layout']],
  ['components/layout/taffy/layout.rs', 'Implements Taffy-based layout (Flexbox/Grid) integration: computes child layouts using the Taffy library, resolves content sizes, handles inline content sizing, and provides layout entry points for flex/grid containers.', ['taffy', 'flexbox', 'grid', 'layout']],
  ['components/layout/taffy/mod.rs', 'Defines TaffyContainer and TaffyItemBox types wrapping the Taffy layout library flexbox/grid layout capabilities, with tree attachment and box repair operations.', ['taffy', 'flexbox', 'grid', 'module']],
  ['components/layout/taffy/stylo_taffy/convert.rs', 'Converts Servo/Stylo computed style values into Taffy layout library style representations, including length, dimension, margin, inset, position, overflow, alignment, gap, grid, and track size conversions.', ['taffy', 'style-conversion', 'stylo', 'bridge']],
  ['components/layout/taffy/stylo_taffy/mod.rs', 'Module barrel for the stylo-to-taffy bridge types, re-exporting StyleTaffyConverter, TaffyStyloStyle, and related types.', ['taffy', 'stylo', 'module', 'barrel']],
  ['components/layout/taffy/stylo_taffy/wrapper.rs', 'Wraps Servo computed values into Taffy-compatible style objects (TaffyStyloStyle), providing layout-specific access to computed style properties for inset, border, grid template, and line name data.', ['taffy', 'style-wrapper', 'stylo', 'grid-properties']],
  ['components/layout/traversal.rs', 'Implements style damage computation and box tree rebuilding: traverses the layout tree to compute element damage sets, propagates damage to children, rebuilds the box tree above/below dirty roots, and handles inline content size adjustment damage.', ['traversal', 'damage', 'box-tree', 'style-recalc']]
];

const classMeta = {
  'BaseFragment': ['Core fragment data structure providing shared fields (tag, flags, style, rect, status) and methods common to all fragment box types in the layout tree.', ['fragment', 'core', 'data-model']],
  'BoxFragment': ['Primary fragment type representing a CSS box, containing children, padding/border/margin, baselines, scrollable overflow, containing block tracking, and spatial tree node assignment.', ['fragment', 'box-model', 'children']],
  'Fragment': ['Top-level fragment enum dispatching to specific fragment types (LayoutRoot, Box, Float, Positioning, Text, Image, IFrame) for all fragment operations.', ['enum', 'fragment', 'dispatch']],
  'FragmentTree': ['Root container for a document fragment tree, manages scrollable overflow and provides methods to find fragments and access the body or root box fragments.', ['fragment-tree', 'root', 'container']],
  'PositioningFragment': ['Fragment wrapper for elements needing special positioning (floats), providing offset calculation, scrollable overflow, and tree printing support.', ['fragment', 'positioning', 'float']],
  'LayoutThread': ['Core layout engine thread that manages reflow, styling, font loading, display list construction, stacking context tree management, and DOM query resolution.', ['layout-engine', 'thread', 'reflow', 'queries']],
  'LayoutBoxBase': ['Foundation structure for all layout boxes, managing fragment storage, inline content sizing, layout result caching, and cache invalidation across formatting contexts.', ['layout-box', 'base', 'fragments', 'caching']],
  'ContainingBlockManager': ['Manages the containing block chain for fragments, providing strategies for non-absolute, absolute, and fixed descendants.', ['containing-block', 'positioning', 'chain']],
  'TextFragment': ['Fragment type for text content, storing glyphs, font metrics, selected style, justification data, and providing character offset computation for hit testing.', ['fragment', 'text', 'glyphs', 'hit-test']],
  'ImageFragment': ['Fragment type for replaced image content with clip region, image key, broken image handling, and SVG render tree support.', ['fragment', 'image', 'replaced']],
  'IFrameFragment': ['Fragment type for embedded iframe content, storing the sub-pipeline identifier.', ['fragment', 'iframe', 'embedded']],
  'LayoutRoot': ['Top-level layout root representing the initial containing block, performing viewport-sized layout passes.', ['layout-root', 'viewport', 'initial-containing-block']],
  'ConstraintSpace': ['Represents the available space constraints for layout, including inline and block sizes.', ['constraint', 'space', 'layout-input']],
  'ContainingBlock': ['Represents the containing block size and writing mode for layout computations.', ['containing-block', 'size', 'writing-mode']],
  'ReplacedContents': ['Handles replaced element content (images, SVG, iframes, video, canvas), computing natural sizes and creating appropriate display fragments.', ['replaced', 'content', 'images', 'svg']],
  'ContentSizes': ['Tracks min-content and max-content inline sizes with union/max/shrink-to-fit operations for CSS intrinsic sizing.', ['content-sizing', 'intrinsic', 'min-max']],
  'ComputedValuesExt': ['Extension trait on Servo ComputedValues providing layout-specific methods for accessing box model properties, writing mode, overflow, transforms, and stacking context establishment.', ['style', 'extension-trait', 'computed-values']],
  'TableBuilder': ['Builds the table structure from DOM elements, creating anonymous table objects as needed, managing row groups, cells, columns, and spanning.', ['table', 'builder', 'anonymous']],
  'TableLayout': ['Implements the full CSS table layout algorithm including column width distribution, row height calculation, collapsed borders, and cell positioning.', ['table', 'layout-algorithm', 'columns', 'rows']],
  'SizeConstraint': ['Represents CSS sizing constraints (preferred/min/max) with resolution logic against containing block size and intrinsic dimensions.', ['sizing', 'constraints', 'resolution']],
  'TaffyContainer': ['Wraps a flex/grid container for layout via the Taffy library, managing the taffy tree node and style representation.', ['taffy', 'flexbox', 'grid', 'wrapper']],
  'TaffyItemBox': ['Wraps a flex/grid item for layout via the Taffy library, providing layout entry points and style resolution.', ['taffy', 'flexbox', 'grid', 'item']],
  'TaffyStyloStyle': ['Wraps Servo computed values to provide Taffy-compatible style properties for flex/grid layout.', ['taffy', 'style', 'wrapper', 'bridge']],
  'AbsoluteAxisSolver': ['Solves absolute positioning along a single axis, computing resolved insets, margin auto-distribution, and final fragment position.', ['absolute', 'positioning', 'axis-solver', 'insets']],
  'PositioningContext': ['Manages a positioning context for absolutely/fixed positioned elements, including box collection, hoisting, and layout coordination.', ['positioning', 'context', 'hoisting']],
  'LayoutRootLayoutInputs': ['Input parameters for the initial containing block layout pass, including viewport size and optional parent layout data.', ['layout-inputs', 'root', 'viewport']],
  'ElementDamageSet': ['Tracks damage (style change, reflow required, repaint needed) for elements during incremental layout updates.', ['damage', 'incremental', 'style-change']],
  'LayoutStyle': ['Represents the computed style for a layout box, providing access to layout-relevant style properties.', ['style', 'layout', 'computed-values']],
  'QuotePair': ['Represents a pair of open/close quote characters for a specific language.', ['quotes', 'typography', 'pair']],
  'HoistedSharedFragment': ['Lightweight wrapper for fragments hoisted out of normal tree flow (e.g., absolutely positioned elements), with optional style override.', ['hoisted', 'fragment', 'wrapper']],
  'FragmentStatus': ['Enum tracking the layout status of a fragment (New, StyleChanged, OnlyDescendantsChanged, Clean) for incremental update optimization.', ['status', 'incremental', 'dirty-tracking']],
  'BaseFragmentInfo': ['Provides construction information for creating a BaseFragment, wrapping tag and flags data.', ['construction', 'fragment-info']],
  'Tag': ['Fragment tag identifying the originating DOM node and pseudo-element chain for mapping fragments back to their style sources.', ['tag', 'node-mapping', 'pseudo-element']],
  'LayoutFactoryImpl': ['Factory implementation creating LayoutThread instances for the layout engine.', ['factory', 'layout-thread']],
  'AbsolutelyPositionedBox': ['Represents a single absolutely positioned box within a positioning context, tracking its fragment and hoisting status.', ['absolute', 'positioned-box', 'hoisted']],
  'HoistedAbsolutelyPositionedBox': ['An absolutely positioned box that has been hoisted to a higher positioning context for layout.', ['absolute', 'hoisted', 'positioned-box']],
  'ContainingBlockCalculation': ['State machine for lazy containing block calculation with stacking context tree awareness.', ['containing-block', 'lazy-calculation', 'state-machine']],
  'CollapsedBlockMargins': ['Tracks the collapsed through, start, and end margin states for CSS margin collapsing.', ['margin', 'collapsing', 'block']],
  'CollapsedMargin': ['Resolves CSS margin collapsing by tracking max positive and min negative values through adjacent margins.', ['margin', 'collapsing', 'resolution']],
  'LayoutRootFragment': ['Wrapper fragment for accessing the inner layout root fragment and its box fragment.', ['layout-root', 'fragment', 'wrapper']],
  'PropagatedBoxTreeData': ['Tracks box tree propagation data during layout for handling special formatting context interactions.', ['box-tree', 'propagation', 'layout-data']],
  'NaturalSizes': ['Represents the natural (intrinsic) dimensions of a replaced element such as an image or video.', ['intrinsic', 'natural-sizes', 'replaced']],
  'IntrinsicSizingMode': ['Enum representing intrinsic sizing mode (match-legacy or modern) for CSS sizing.', ['sizing', 'intrinsic-mode']],
  'InlineContentSizesResult': ['Result type for inline content size computation, containing min and max content sizes.', ['content-sizing', 'result-type']],
  'ComputeInlineContentSizes': ['Trait for computing inline content sizes of DOM elements.', ['trait', 'content-sizing']],
  'TableSlotCell': ['Represents a cell within the CSS table grid, tracking its row/column placement and spanning.', ['table', 'cell', 'grid-placement']],
  'Table': ['Represents a CSS table-level box in the box tree, with repair, sizing, and testing support.', ['table', 'box-tree']],
  'TableSlot': ['Represents a slot (cell position) in the CSS table grid structure.', ['table', 'slot', 'grid']],
  'TableTrack': ['Represents a track (row or column) in the CSS table grid structure.', ['table', 'track']],
  'TableTrackGroup': ['Represents a group of table tracks (rows or columns).', ['table', 'track-group']],
  'TableTrackGroupType': ['Enum for table track group type (row group, column group).', ['table', 'track-group-type']],
  'TableCaption': ['Represents a table caption box in the table layout model.', ['table', 'caption']],
  'CollapsedBorder': ['Represents a collapsed border between table cells with resolved style and width.', ['table', 'collapsed-border', 'border']],
  'SpecificTableGridInfo': ['Stores the computed grid structure for a table, including column measures and row data.', ['table', 'grid-info']],
  'TableLayoutStyle': ['Enum representing the table layout style (auto or fixed).', ['table', 'layout-style']],
  'TableLevelBox': ['Base type for table-level boxes in the box tree, providing tree attachment and style management.', ['table', 'table-level-box', 'base']],
  'WeakTableLevelBox': ['Weak reference to a TableLevelBox for non-owning access.', ['table', 'weak-reference']],
  'SpecificTaffyGridInfo': ['Stores detailed grid layout information from the Taffy library.', ['taffy', 'grid-info']],
  'RecalcStyle': ['Traversal type for the style recalculation pass over the layout tree.', ['traversal', 'style-recalc']],
  'PaddingBorderMargin': ['Represents padding, border, and margin values for a box in logical coordinates.', ['padding', 'border', 'margin', 'logical']],
  'AspectRatio': ['Represents a preferred aspect ratio for sizing, combining CSS aspect-ratio property with intrinsic ratio.', ['aspect-ratio', 'sizing']],
  'ContentBoxSizesAndPBM': ['Aggregates content box sizes and padding/border/margin for a layout box.', ['sizing', 'box-model', 'content-box']],
  'BorderStyleColor': ['Represents a border side with its style and color for table border rendering.', ['border', 'style', 'color']],
  'OverflowDirection': ['Represents the overflow direction (horizontal/vertical/both/none) for scrollable containers.', ['overflow', 'direction']],
  'Clamp': ['Utility for clamping values within min/max constraints during size resolution.', ['utility', 'clamping']],
  'TransformExt': ['Extension trait for transform matrix calculations on layout boxes.', ['transform', 'extension-trait']],
  'BoxFragmentRareData': ['Optional extended data for a BoxFragment, including sticky insets and generated clip/scroll node IDs.', ['box-fragment', 'rare-data']],
  'BoxFragmentWithStyle': ['Combines a BoxFragment with its style for layout operations that need both.', ['box-fragment', 'style', 'wrapper']],
  'BackgroundMode': ['Enum controlling background rendering mode (Normal, Extra, None) for fragment painting.', ['background', 'rendering-mode']],
  'ExtraBackground': ['Additional background style and rect for fragments needing layered backgrounds.', ['background', 'extra']],
  'SpecificLayoutInfo': ['Enum for layout-specific information (Grid, TableCell, TableGrid, TableWrapper) for specialized layout handling.', ['layout', 'specific-info']],
  'BlockLevelLayoutInfo': ['Block-level layout metadata including clearance and collapsed margin state.', ['block-level', 'clearance', 'collapsed-margins']],
  'Display': ['Display type classification for box generation rules.', ['display', 'box-generation']],
  'DisplayGeneratingBox': ['Display value classification for whether a box generates a principal box, no box, or contents.', ['display', 'box-generation']],
  'DisplayOutside': ['Display outside value (block, inline, run-in) for CSS display property.', ['display', 'outside']],
  'DisplayInside': ['Display inside value (flow, flex, grid, table, ruby) for CSS display property.', ['display', 'inside']],
  'DisplayLayoutInternal': ['Display layout-internal values for table parts and ruby.', ['display', 'layout-internal', 'table']],
  'IndefiniteContainingBlock': ['Represents an indefinite containing block measurement along one axis.', ['containing-block', 'indefinite']],
  'ContainingBlockSize': ['Represents the containing block size for layout with computed values.', ['containing-block', 'size']],
  'Sizes': ['Container for preferred, min, and max sizing constraints with resolution methods.', ['sizing', 'constraints']],
  'LazySize': ['Lazily-computed size value for deferred resolution.', ['sizing', 'lazy']],
  'LogicalVec2': ['A 2D vector in logical (inline/block) coordinate space for writing-mode-aware layout.', ['geometry', 'logical', 'vector']],
  'LogicalRect': ['A rectangle in logical (inline/block) coordinate space.', ['geometry', 'logical', 'rect']],
  'LogicalSides': ['Logical-side inset values (inline-start, inline-end, block-start, block-end).', ['geometry', 'logical', 'sides']],
  'LogicalSides1D': ['Logical-side inset values along one axis (start/end).', ['geometry', 'logical', 'sides']],
  'ToLogical': ['Trait for converting physical geometry values to logical coordinates.', ['geometry', 'conversion', 'trait']],
  'ToLogicalWithContainingBlock': ['Trait for converting physical geometry to logical coordinates using containing block data.', ['geometry', 'conversion', 'trait']],
  'SyncPhysicalRectAu': ['Thread-safe wrapper around a physical rect using atomic operations for concurrent access.', ['geometry', 'atomic', 'sync']],
  'CellLayout': ['Per-cell layout tracking during table layout, including column mapping and measure information.', ['table', 'cell', 'layout']],
  'CellOrTrackMeasure': ['Measure data for a table cell or track during layout computation.', ['table', 'measure']],
  'RowGroupFragmentLayout': ['Row group fragment layout data including position and border information.', ['table', 'row-group', 'layout']],
  'TableAndTrackDimensions': ['Aggregated table and track dimension data after layout computation.', ['table', 'dimensions']],
  'ColspanToDistribute': ['Tracks colspan cell distribution across table columns for intrinsic sizing.', ['table', 'colspan', 'distribution']],
  'LayoutResultAndInputs': ['Aggregates layout results with their input parameters for cache management.', ['layout', 'caching', 'results']],
  'IndependentFormattingContextLayoutResult': ['Layout result for an independent formatting context.', ['layout', 'ifc', 'results']],
  'IndependentFormattingContextLayoutResultAndInputs': ['Layout result and inputs for independent formatting contexts.', ['layout', 'ifc', 'caching']],
  'SameFormattingContextBlockLayoutResult': ['Layout result for block-level layout in the same formatting context.', ['layout', 'block', 'results']],
  'SameFormattingContextBlockLayoutResultAndInputs': ['Layout result and inputs for same-formatting-context block layout.', ['layout', 'block', 'caching']],
  'StyloLineNameIter': ['Iterator over CSS grid line names from Stylo computed values.', ['grid', 'line-names', 'stylo']],
  'RepetitionWrapper': ['Wraps grid track repetition data for Taffy compatibility.', ['grid', 'repetition', 'taffy']],
  'AnonymousTableContent': ['Tracks anonymous table content during table structure construction.', ['table', 'anonymous', 'construction']],
  'ResolvedSlotAndLocation': ['Resolved grid slot position and location during table construction.', ['table', 'slot', 'construction']],
  'TableBuilderTraversal': ['DOM traversal state for building table structure from child elements.', ['table', 'dom-traversal', 'construction']],
  'TableColumnGroupBuilder': ['Builds column group and column definitions during table construction.', ['table', 'column-group', 'construction']],
  'Size': ['A size classification enum (preferred/min/max) for constraint resolution.', ['sizing', 'size-type', 'enum']],
  'UserAgentStylesheets': ['Container for user agent stylesheet references used by the layout thread.', ['styling', 'user-agent', 'stylesheets']],
  'RegisteredPainterImpl': ['Implements registered CSS paint worklets for custom painting.', ['paint', 'worklet', 'css-paint']],
  'LayoutFontMetricsProvider': ['Provides font metrics for layout calculations, wrapping Servo font data.', ['fonts', 'metrics', 'provider']],
  'PositioningContextLength': ['Wrapper around positioning context length for indexed access.', ['positioning', 'length']],
  'IFrameInfo': ['Information about an iframe replaced element, tracking its pipeline and size.', ['iframe', 'replaced', 'info']],
  'ImageInfo': ['Information about an image replaced element, including URL and load state.', ['image', 'replaced', 'info']],
  'VideoInfo': ['Information about a video replaced element.', ['video', 'replaced', 'info']],
  'CanvasInfo': ['Information about a canvas replaced element.', ['canvas', 'replaced', 'info']],
  'ReplacedContentKind': ['Enum categorizing the kind of replaced content (Image, IFrame, SVG, Video, Canvas).', ['replaced', 'content-kind', 'enum']],
  'TaffyItemBoxInner': ['Inner type for a TaffyItemBox, wrapping the actual fragment or box.', ['taffy', 'item', 'inner']],
  'SpecificTaffyGridTrackInfo': ['Track-level grid information from the Taffy library.', ['taffy', 'grid', 'track-info']]
};

// Build nodes and edges
let nodes = [];
let edges = [];
const fileOrder = data.results.map(r => r.path);

for (const r of data.results) {
  const fp = r.path;
  const nel = r.nonEmptyLines;
  const exp = r.exports || [];
  const exportedNames = new Set(exp.map(e => e.name));
  const funcs = r.functions || [];
  const classes = r.classes || [];
  const fileId = 'file:' + fp;
  const name = fp.split('/').pop();

  // File node
  const fm = metaData.find(m => m[0] === fp);
  const fileSummary = fm ? fm[1] : name + ' module.';
  const fileTags = fm ? fm[2] : ['layout'];

  nodes.push({
    id: fileId,
    type: 'file',
    name: name,
    filePath: fp,
    summary: fileSummary,
    tags: fileTags,
    complexity: complexityClass(nel)
  });

  // Functions
  for (const f of funcs) {
    if (!isSigFunction(f, exportedNames)) continue;
    const lc = f.endLine - f.startLine + 1;
    const isEx = exportedNames.has(f.name);
    const funcId = 'function:' + fp + ':' + f.name;
    const fn = f.name;

    let funcSummary, funcTags;

    if (['new', 'new_for_testing', 'new_empty', 'new_anonymous', 'new_with_base_fragment_info'].includes(fn)) {
      funcSummary = 'Constructs a new instance with the provided parameters.';
      funcTags = ['function', 'constructor'];
    } else if (fn.startsWith('compute_') || fn.startsWith('calculate_')) {
      funcSummary = 'Computes ' + fn.replace(/^(compute_|calculate_)/, '').replace(/_/g, ' ') + ' from the current layout state.';
      funcTags = ['function', 'computation'];
    } else if (fn.startsWith('resolve_')) {
      funcSummary = 'Resolves ' + fn.replace('resolve_', '').replace(/_/g, ' ') + ' values based on available space and style constraints.';
      funcTags = ['function', 'resolution'];
    } else if (fn === 'layout' || fn.startsWith('layout_')) {
      funcSummary = 'Performs layout for this element or its children, computing sizes and positions.';
      funcTags = ['function', 'layout'];
    } else if (fn === 'print') {
      funcSummary = 'Debug-prints the fragment tree structure for inspection.';
      funcTags = ['function', 'debug'];
    } else if (fn.includes('scrollable')) {
      funcSummary = 'Calculates or retrieves the scrollable overflow area for this fragment.';
      funcTags = ['function', 'scrollable-overflow'];
    } else if (fn.includes('containing_block') && fn !== 'ensure_containing_block_calculation') {
      funcSummary = 'Manages the containing block relationship for this fragment.';
      funcTags = ['function', 'containing-block'];
    } else if (fn.startsWith('process_')) {
      funcSummary = 'Processes a ' + fn.replace('process_', '').replace(/_/g, ' ') + ' from the script/layout thread.';
      funcTags = ['function', 'query-processing'];
    } else if (fn === 'handle_reflow') {
      funcSummary = 'Main reflow handler: performs style resolution, box construction, layout, and display list building for the document.';
      funcTags = ['reflow', 'core', 'layout-engine'];
    } else if (fn === 'restyle_and_build_trees') {
      funcSummary = 'Restyles the document and rebuilds the element/style/box trees, handling incremental style changes.';
      funcTags = ['function', 'styling', 'tree-building'];
    } else if (fn === 'build_display_list') {
      funcSummary = 'Builds the display list from the stacking context tree for rendering.';
      funcTags = ['function', 'display-list', 'rendering'];
    } else if (fn.includes('content_sizes') || fn.includes('inline_content')) {
      funcSummary = 'Computes the min/max inline content sizes for this element.';
      funcTags = ['function', 'content-sizing', 'intrinsic-sizes'];
    } else if (fn.includes('cache')) {
      funcSummary = 'Manages cached layout results to avoid redundant computation.';
      funcTags = ['function', 'caching'];
    } else if (fn.includes('damage') || fn.includes('repair') || fn.includes('Damage')) {
      funcSummary = 'Computes or applies style/layout damage for incremental update tracking.';
      funcTags = ['function', 'damage', 'incremental'];
    } else if (fn === 'find') {
      funcSummary = 'Searches the fragment tree for fragments matching a processing function at a given level.';
      funcTags = ['function', 'search', 'traversal'];
    } else if (fn === 'construct') {
      funcSummary = 'Constructs the box tree or table structure from DOM elements.';
      funcTags = ['function', 'construction'];
    } else if (fn.includes('offset') || fn.includes('translate')) {
      funcSummary = 'Applies a positional offset or translation to the fragment geometry.';
      funcTags = ['function', 'positioning'];
    } else if (fn.includes('marker') || fn.includes('counter')) {
      funcSummary = 'Generates CSS list marker content or counter representation.';
      funcTags = ['function', 'lists', 'counters'];
    } else if (fn.startsWith('from_') || fn === 'from') {
      funcSummary = 'Converts from another type or constructs from source parameters.';
      funcTags = ['function', 'conversion'];
    } else if (fn === 'collect_reports') {
      funcSummary = 'Collects profiling and timing reports from the layout thread.';
      funcTags = ['function', 'profiling'];
    } else if (fn.startsWith('query_') && fp === 'components/layout/query.rs') {
      funcSummary = 'Handles the ' + fn.replace('query_', '').replace(/_/g, ' ') + ' DOM query from script.';
      funcTags = ['function', 'dom-query', 'layout-query'];
    } else if (fn === 'make_marker') {
      funcSummary = 'Builds a marker fragment for list items.';
      funcTags = ['function', 'lists', 'markers'];
    } else if (fn === 'generate_counter_representation') {
      funcSummary = 'Generates the string representation for a CSS counter value.';
      funcTags = ['function', 'lists', 'counters'];
    } else if (fn === 'marker_string') {
      funcSummary = 'Formats the marker string using the specified list style type.';
      funcTags = ['function', 'lists', 'markers'];
    } else if (fn === 'try_from' || fn === 'try_layout') {
      funcSummary = 'Performs the top-level layout pass for the initial containing block.';
      funcTags = ['function', 'layout-root'];
    } else if (fn === 'for_element') {
      funcSummary = 'Creates replaced contents for a given DOM element, detecting its type.';
      funcTags = ['function', 'replaced', 'element-detection'];
    } else if (fn === 'make_fragments') {
      funcSummary = 'Creates display fragments for replaced content elements.';
      funcTags = ['function', 'fragment-creation', 'replaced'];
    } else if (['set_theme', 'set_viewport_details'].includes(fn)) {
      funcSummary = 'Configures layout thread state with theme/viewport information.';
      funcTags = ['function', 'configuration'];
    } else if (fn === 'build_stacking_context_tree') {
      funcSummary = 'Builds the stacking context tree from the fragment tree for display list construction.';
      funcTags = ['function', 'stacking-context'];
    } else if (fn.includes('accessibility')) {
      funcSummary = 'Updates the accessibility tree based on layout changes.';
      funcTags = ['function', 'accessibility'];
    } else if (fn.includes('svg')) {
      funcSummary = 'Builds SVG render tree nodes from the SVG DOM for rendering.';
      funcTags = ['function', 'svg'];
    } else if (['quotes_for_lang', 'quotes_data_for_lang', 'create_quotes_map'].includes(fn)) {
      funcSummary = 'Retrieves CSS quote pairs for the specified language.';
      funcTags = ['function', 'quotes', 'localization'];
    } else if (['display_inside', 'used_value_for_contents'].includes(fn)) {
      funcSummary = 'Determines the display type and box generation rules for this element.';
      funcTags = ['function', 'display', 'box-generation'];
    } else if (fn === 'establishes_stacking_context') {
      funcSummary = 'Determines whether this element establishes a new stacking context.';
      funcTags = ['function', 'stacking-context'];
    } else if (fn === 'establishes_block_formatting_context') {
      funcSummary = 'Determines whether this element establishes a new block formatting context.';
      funcTags = ['function', 'bfc', 'formatting-context'];
    } else if (fn === 'effective_overflow') {
      funcSummary = 'Computes the effective overflow behavior for this element.';
      funcTags = ['function', 'overflow'];
    } else if (fn === 'compute_scrollable_overflow' || fn === 'calculate_scrollable_overflow') {
      funcSummary = 'Calculates the full scrollable overflow area, aggregating child contributions and applying transforms.';
      funcTags = ['function', 'scrollable-overflow', 'computation'];
    } else if (fn === 'layout_many') {
      funcSummary = 'Lays out all collected absolutely positioned boxes within the positioning context.';
      funcTags = ['function', 'positioned', 'absolute'];
    } else if (fn === 'layout_as_absolute') {
      funcSummary = 'Performs full absolute positioning layout including inset resolution and margin solving.';
      funcTags = ['function', 'absolute', 'positioning', 'layout'];
    } else if (['compute_track_constrainedness_and_has_originating_cells', 'compute_column_measures', 'compute_grid_min_max'].includes(fn)) {
      funcSummary = 'Computes ' + fn.replace('compute_', '').replace(/_/g, ' ') + ' for the table layout algorithm.';
      funcTags = ['function', 'table', 'computation'];
    } else if (['distribute_width_to_columns', 'distribute_extra_width_to_columns'].includes(fn)) {
      funcSummary = 'Distributes available width among table columns according to CSS table width distribution rules.';
      funcTags = ['function', 'table', 'column-widths'];
    } else if (['layout_grid', 'do_first_row_layout', 'layout_cells_in_row'].includes(fn)) {
      funcSummary = 'Performs a phase of the table layout algorithm.';
      funcTags = ['function', 'table', 'layout'];
    } else if (fn === 'layout_initial_containing_block_children') {
      funcSummary = 'Lays out absolutely positioned children of the initial containing block.';
      funcTags = ['function', 'absolute', 'initial-containing-block'];
    } else if (fn === 'compute_inline_content_sizes') {
      funcSummary = 'Computes the inline content sizing (min/max) for this element.';
      funcTags = ['function', 'content-sizing'];
    } else if (fn === 'compute_damage_and_rebuild_box_tree' || fn.startsWith('compute_damage_and_rebuild_box_tree_')) {
      funcSummary = 'Computes element damage and rebuilds the box tree for incremental layout.';
      funcTags = ['function', 'damage', 'box-tree', 'rebuild'];
    } else if (fn === 'process_preorder') {
      funcSummary = 'Processes elements in preorder during tree traversal for style damage computation.';
      funcTags = ['function', 'traversal', 'damage'];
    } else if (fn === 'apply_damage') {
      funcSummary = 'Applies the computed damage set to elements, triggering appropriate style recalc and reflow.';
      funcTags = ['function', 'damage', 'style-recalc'];
    } else if (fn === 'compute_child_layout') {
      funcSummary = 'Computes the layout for a single flex/grid child using the Taffy library.';
      funcTags = ['function', 'taffy', 'child-layout'];
    } else if (['set_scroll_offset_from_script', 'set_scroll_offsets_from_renderer'].includes(fn)) {
      funcSummary = 'Updates scroll offsets from script or renderer input.';
      funcTags = ['function', 'scrolling'];
    } else if (fn === 'ensure_containing_block_calculation') {
      funcSummary = 'Ensures containing block calculation is performed for a fragment.';
      funcTags = ['function', 'containing-block'];
    } else if (fn === 'ensure_stacking_context_tree') {
      funcSummary = 'Ensures the stacking context tree is built and up to date.';
      funcTags = ['function', 'stacking-context'];
    } else if (fn === 'clear_layout_trees_and_send_empty_display_list') {
      funcSummary = 'Clears layout trees and sends an empty display list for hidden or unloaded pages.';
      funcTags = ['function', 'cleanup', 'display-list'];
    } else if (fn === 'layout_maybe_position_relative_fragment') {
      funcSummary = 'Performs layout for a possibly position:relative fragment.';
      funcTags = ['function', 'positioned', 'relative'];
    } else if (fn === 'solve_margins') {
      funcSummary = 'Solves margin auto values for absolute positioning along a single axis.';
      funcTags = ['function', 'margins', 'absolute'];
    } else if (fn === 'origin_for_margin_box') {
      funcSummary = 'Computes the origin position for the margin box of an absolutely positioned element.';
      funcTags = ['function', 'origin', 'absolute'];
    } else if (fn === 'relative_adjustement') {
      funcSummary = 'Applies relative positioning adjustment to a fragment.';
      funcTags = ['function', 'relative', 'positioning'];
    } else if (fn === 'compute_border_collapse') {
      funcSummary = 'Computes collapsed border styles and widths for adjacent table cells.';
      funcTags = ['function', 'table', 'border-collapse'];
    } else if (fn === 'distribute_colspanned_cell_to_columns') {
      funcSummary = 'Distributes a colspan cell intrinsic sizes across the columns it spans.';
      funcTags = ['function', 'table', 'colspan'];
    } else if (fn === 'compute_table_width') {
      funcSummary = 'Computes the final table width based on column measures and available space.';
      funcTags = ['function', 'table', 'width'];
    } else if (fn === 'compute_table_height_and_final_row_heights') {
      funcSummary = 'Computes table height and assigns final row heights.';
      funcTags = ['function', 'table', 'height'];
    } else if (fn === 'layout_caption') {
      funcSummary = 'Lays out the table caption element.';
      funcTags = ['function', 'table', 'caption'];
    } else if (['build_svg_render_tree', 'build_svg_render_node'].includes(fn)) {
      funcSummary = 'Builds SVG render trees and nodes from the SVG DOM for rendering.';
      funcTags = ['function', 'svg', 'render-tree'];
    } else if (fn === 'content_size') {
      funcSummary = 'Computes the content size for replaced elements based on available space.';
      funcTags = ['function', 'replaced', 'content-size'];
    } else if (fn === 'calculate_fragment_rect') {
      funcSummary = 'Calculates the fragment rectangle for a replaced element based on content size and alignment.';
      funcTags = ['function', 'replaced', 'fragment-rect'];
    } else if (fn === 'preferred_aspect_ratio') {
      funcSummary = 'Computes the preferred aspect ratio for this element based on CSS properties and natural dimensions.';
      funcTags = ['function', 'aspect-ratio', 'sizing'];
    } else if (fn === 'shrink_to_fit') {
      funcSummary = 'Computes the shrink-to-fit inline size from available space.';
      funcTags = ['function', 'sizing', 'shrink-to-fit'];
    } else if (['adjoin', 'adjoin_assign'].includes(fn)) {
      funcSummary = 'Adjoins another collapsed margin for margin collapsing resolution.';
      funcTags = ['function', 'margins', 'collapsing'];
    } else if (fn === 'solve') {
      funcSummary = 'Resolves the collapsed margin to its final value.';
      funcTags = ['function', 'margins', 'collapsing'];
    } else if (fn === 'ensure') {
      funcSummary = 'Ensures the containing block calculation state is resolved.';
      funcTags = ['function', 'containing-block', 'lazy'];
    } else if (fn === 'new_for_layout_box_base') {
      funcSummary = 'Creates a new PositioningContext for a LayoutBoxBase.';
      funcTags = ['function', 'positioning', 'constructor'];
    } else if (fn === 'new_for_style_and_fragment_flags') {
      funcSummary = 'Creates a new PositioningContext from style and fragment flags.';
      funcTags = ['function', 'positioning', 'constructor'];
    } else if (['adjust_static_position_of_hoisted_fragments', 'adjust_static_position_of_hoisted_fragments_with_offset'].includes(fn)) {
      funcSummary = 'Adjusts the static position of hoisted absolutely positioned fragments.';
      funcTags = ['function', 'positioned', 'static-position'];
    } else if (fn === 'forget_unhoisted_boxes' || fn === 'take_boxes_for_fragment') {
      funcSummary = 'Manages the collection of absolutely positioned boxes for layout.';
      funcTags = ['function', 'positioned', 'box-collection'];
    } else if (fn === 'ensure_containing_block_calculation') {
      funcSummary = 'Ensures containing block calculation is performed for a fragment.';
      funcTags = ['function', 'containing-block'];
    } else if (fn === 'get_ua_stylesheets') {
      funcSummary = 'Retrieves user agent stylesheets for the layout thread.';
      funcTags = ['function', 'styling', 'user-agent'];
    } else if (fn === 'draw_a_paint_image') {
      funcSummary = 'Draws a paint image for CSS paint worklets.';
      funcTags = ['function', 'paint', 'worklet'];
    } else if (fn === 'query_font_metrics') {
      funcSummary = 'Queries font metrics for a given font from the layout thread.';
      funcTags = ['function', 'font-metrics', 'query'];
    } else if (fn === 'load_web_fonts_from_stylesheet' || fn === 'load_all_web_fonts_from_stylesheet_with_guard') {
      funcSummary = 'Loads web fonts referenced in stylesheets for rendering.';
      funcTags = ['function', 'fonts', 'loading'];
    } else if (fn === 'add_stylesheet') {
      funcSummary = 'Adds a stylesheet to the layout thread for style resolution.';
      funcTags = ['function', 'styling', 'stylesheet'];
    } else if (fn === 'can_skip_reflow_request_entirely') {
      funcSummary = 'Determines if a reflow request can be skipped entirely based on damage and viewport state.';
      funcTags = ['function', 'reflow', 'optimization'];
    } else if (fn === 'maybe_print_reflow_event') {
      funcSummary = 'Optionally prints reflow event debugging information.';
      funcTags = ['function', 'debug', 'reflow'];
    } else if (fn === 'handle_accessibility_tree_update') {
      funcSummary = 'Updates the accessibility tree based on layout changes.';
      funcTags = ['function', 'accessibility', 'update'];
    } else if (fn === 'prepare_stylist_for_reflow') {
      funcSummary = 'Prepares the stylist (style resolution engine) for a reflow pass.';
      funcTags = ['function', 'styling', 'reflow'];
    } else if (fn === 'build_stacking_context_tree_for_reflow') {
      funcSummary = 'Builds the stacking context tree as part of the reflow process.';
      funcTags = ['function', 'stacking-context', 'reflow'];
    } else if (fn === 'profiler_metadata') {
      funcSummary = 'Returns profiler metadata for the layout thread.';
      funcTags = ['function', 'profiling', 'metadata'];
    } else if (fn === 'resolve_content_size') {
      funcSummary = 'Resolves a content size from style constraints for flex/grid layout.';
      funcTags = ['function', 'taffy', 'content-size'];
    } else if (fn === 'with_independent_formatting_context') {
      funcSummary = 'Runs a layout computation within an independent formatting context.';
      funcTags = ['function', 'taffy', 'formatting-context'];
    } else if (fn === 'resolve_calc_value') {
      funcSummary = 'Resolves a calc() style value to an absolute length.';
      funcTags = ['function', 'taffy', 'calc'];
    } else if (fn === 'get_grid_child_style') {
      funcSummary = 'Retrieves the grid layout style for a child element.';
      funcTags = ['function', 'taffy', 'grid'];
    } else if (fn === 'length_percentage') {
      funcSummary = 'Converts a length-percentage style value to a Taffy dimension.';
      funcTags = ['function', 'taffy', 'conversion'];
    } else if (fn === 'dimension') {
      funcSummary = 'Converts a dimension style value to Taffy representation.';
      funcTags = ['function', 'taffy', 'conversion'];
    } else if (fn === 'max_size_dimension') {
      funcSummary = 'Converts a max-size dimension to Taffy representation.';
      funcTags = ['function', 'taffy', 'conversion'];
    } else if (fn === 'margin') {
      funcSummary = 'Converts a margin style value to Taffy representation.';
      funcTags = ['function', 'taffy', 'conversion'];
    } else if (fn === 'inset') {
      funcSummary = 'Converts an inset style value to Taffy representation.';
      funcTags = ['function', 'taffy', 'conversion'];
    } else if (fn === 'content_alignment') {
      funcSummary = 'Converts a content alignment style value to Taffy representation.';
      funcTags = ['function', 'taffy', 'conversion'];
    } else if (fn === 'item_alignment') {
      funcSummary = 'Converts an item alignment style value to Taffy representation.';
      funcTags = ['function', 'taffy', 'conversion'];
    } else if (fn === 'gap') {
      funcSummary = 'Converts a gap style value to Taffy representation.';
      funcTags = ['function', 'taffy', 'conversion'];
    } else if (fn === 'grid_auto_flow') {
      funcSummary = 'Converts a grid auto-flow style value to Taffy representation.';
      funcTags = ['function', 'taffy', 'conversion'];
    } else if (fn === 'grid_line') {
      funcSummary = 'Converts a grid line style value to Taffy representation.';
      funcTags = ['function', 'taffy', 'conversion'];
    } else if (fn === 'track_repeat' || fn === 'track_size' || fn === 'min_track' || fn === 'max_track') {
      funcSummary = 'Converts grid track sizing data to Taffy representation.';
      funcTags = ['function', 'taffy', 'conversion'];
    } else if (fn === 'position' && fp.includes('stylo_taffy')) {
      funcSummary = 'Converts a position style value to Taffy representation.';
      funcTags = ['function', 'taffy', 'conversion'];
    } else if (fn === 'overflow' && fp.includes('stylo_taffy')) {
      funcSummary = 'Converts an overflow style value to Taffy representation.';
      funcTags = ['function', 'taffy', 'conversion'];
    } else if (fn === 'aspect_ratio' && fp.includes('stylo_taffy')) {
      funcSummary = 'Converts aspect ratio to Taffy representation.';
      funcTags = ['function', 'taffy', 'conversion'];
    } else if (fn === 'direction' && fp.includes('stylo_taffy')) {
      funcSummary = 'Converts a direction style value to Taffy representation.';
      funcTags = ['function', 'taffy', 'conversion'];
    } else if (fn === 'box_sizing' && fp.includes('stylo_taffy')) {
      funcSummary = 'Converts box-sizing style value to Taffy representation.';
      funcTags = ['function', 'taffy', 'conversion'];
    } else if (fn === 'box_generation_mode') {
      funcSummary = 'Converts box generation mode to Taffy representation.';
      funcTags = ['function', 'taffy', 'conversion'];
    } else if (fn === 'is_block' && fp.includes('stylo_taffy')) {
      funcSummary = 'Determines if the element is block-level for Taffy layout.';
      funcTags = ['function', 'taffy', 'conversion'];
    } else if (fn === 'new' && fp === 'components/layout/query.rs') {
      funcSummary = 'Creates a new layout query processor.';
      funcTags = ['function', 'constructor'];
    } else if (fn === 'necessary') {
      funcSummary = 'Determines if a reflow is necessary based on current state.';
      funcTags = ['function', 'reflow', 'optimization'];
    } else if (fn === 'node_rendering_type') {
      funcSummary = 'Determines the rendering type for a DOM node during layout.';
      funcTags = ['function', 'rendering', 'node-type'];
    } else if (fn === 'layout_style') {
      funcSummary = 'Returns the layout-specific style for this element.';
      funcTags = ['function', 'style', 'layout'];
    } else if (fn === 'attached_to_tree') {
      funcSummary = 'Checks whether this box is attached to the layout tree.';
      funcTags = ['function', 'tree', 'attachment'];
    } else if (fn === 'subtree_size') {
      funcSummary = 'Returns the subtree size of this element for layout traversal.';
      funcTags = ['function', 'tree', 'subtree'];
    } else if (fn === 'add_cell') {
      funcSummary = 'Adds a cell to the table builder with spanning and slot assignment.';
      funcTags = ['function', 'table', 'cell-placement'];
    } else if (fn === 'add_column') {
      funcSummary = 'Adds a column to the table column group builder.';
      funcTags = ['function', 'table', 'column'];
    } else if (fn === 'handle_element' && fp.includes('table/construct')) {
      funcSummary = 'Processes a DOM element during table structure construction.';
      funcTags = ['function', 'table', 'construction'];
    } else if (fn === 'resolve_slot_at') {
      funcSummary = 'Resolves a table grid slot at the given row/column position.';
      funcTags = ['function', 'table', 'slot-resolution'];
    } else if (fn === 'push_spanned') {
      funcSummary = 'Pushes a spanned cell into the table grid for rowspan/colspan handling.';
      funcTags = ['function', 'table', 'spanning'];
    } else if (fn === 'start_row' || fn === 'end_row') {
      funcSummary = 'Gets the start or end row index for this slot in the table grid.';
      funcTags = ['function', 'table', 'row'];
    } else if (fn === 'compute_cell_measures') {
      funcSummary = 'Computes and stores intrinsic size measures for each cell in the table.';
      funcTags = ['function', 'table', 'cell-measures'];
    } else if (fn === 'distribute_extra_size_to_rows') {
      funcSummary = 'Distributes extra height to table rows after the initial layout pass.';
      funcTags = ['function', 'table', 'row-height'];
    } else if (fn === 'do_final_cell_layout') {
      funcSummary = 'Performs the final layout pass for each table cell.';
      funcTags = ['function', 'table', 'cell-layout'];
    } else if (fn === 'make_fragments_for_columns_and_column_groups') {
      funcSummary = 'Creates display fragments for table column and column group elements.';
      funcTags = ['function', 'table', 'column-fragments'];
    } else if (fn === 'specific_layout_info_for_grid') {
      funcSummary = 'Creates SpecificLayoutInfo for a table grid with collapsed borders.';
      funcTags = ['function', 'table', 'layout-info'];
    } else if (fn === 'is_row_collapsed' || fn === 'is_column_collapsed') {
      funcSummary = 'Checks whether a specific table row or column is collapsed.';
      funcTags = ['function', 'table', 'collapse'];
    } else if (fn === 'get_collapsed_border_widths_for_area') {
      funcSummary = 'Gets the collapsed border widths for a specific table area.';
      funcTags = ['function', 'table', 'border-widths'];
    } else if (fn === 'collapse_borders') {
      funcSummary = 'Checks whether the table uses collapsed borders model.';
      funcTags = ['function', 'table', 'border-collapse'];
    } else if (['halved_collapsed_border_widths', 'create_fragment'].includes(fn)) {
      funcSummary = 'Creates table fragments for rendering collapsed borders.';
      funcTags = ['function', 'table', 'border'];
    } else if (fn === 'get_size_percentage_contribution') {
      funcSummary = 'Computes the percentage-based size contribution for a table element.';
      funcTags = ['function', 'table', 'sizing'];
    } else if (fn === 'border' && fp.includes('wrapper')) {
      funcSummary = 'Converts border style values to Taffy-compatible representation.';
      funcTags = ['function', 'taffy', 'conversion'];
    } else if (fn === 'grid_template_rows' || fn === 'grid_template_columns' || fn === 'grid_template_areas' || fn === 'grid_template_column_names' || fn === 'grid_template_row_names') {
      funcSummary = 'Converts CSS grid template data to Taffy-compatible representation.';
      funcTags = ['function', 'taffy', 'conversion', 'grid'];
    } else if (fn === 'from_detailed_grid_layout') {
      funcSummary = 'Constructs SpecificTaffyGridInfo from detailed Taffy grid layout output.';
      funcTags = ['function', 'taffy', 'grid', 'conversion'];
    } else if (['construct_anonymous', 'push_new_slot_to_last_row', 'remove_whitespace_only'].includes(fn)) {
      funcSummary = 'Manages anonymous table object construction and whitespace handling.';
      funcTags = ['function', 'table', 'anonymous'];
    } else if (['reorder_first_thead_and_tfoot', 'regenerate_track_ranges', 'move_row_group_to_front', 'move_row_group_to_end', 'do_final_rowspan_calculation'].includes(fn)) {
      funcSummary = 'Manages table row group ordering and track range regeneration.';
      funcTags = ['function', 'table', 'row-group'];
    } else if (['create_spanned_slot_based_on_cell_above', 'create_slots_for_cells_above_with_rowspan'].includes(fn)) {
      funcSummary = 'Creates table grid slots for spanned cells during construction.';
      funcTags = ['function', 'table', 'spanning'];
    } else if (['finish_anonymous_row_if_needed', 'finish_current_anonymous_cell_if_needed'].includes(fn)) {
      funcSummary = 'Finalizes anonymous table rows or cells during construction.';
      funcTags = ['function', 'table', 'anonymous'];
    } else if (fn === 'push' || fn === 'append') {
      funcSummary = 'Adds an absolutely positioned box to this positioning context.';
      funcTags = ['function', 'positioned', 'box-collection'];
    } else if (fn === 'len' || fn === 'truncate') {
      funcSummary = 'Gets the length or truncates the positioning context box list.';
      funcTags = ['function', 'positioned', 'utility'];
    } else if (fn === 'position') {
      funcSummary = 'Computes the position of boxes within a positioning context.';
      funcTags = ['function', 'positioned', 'positioning'];
    } else if (fn === 'get_axis') {
      funcSummary = 'Gets the axis solver for a given CSS axis (inline/block).';
      funcTags = ['function', 'positioned', 'axis'];
    } else if (fn === 'inset_sum') {
      funcSummary = 'Sums the resolved inset values for an absolutely positioned element.';
      funcTags = ['function', 'positioned', 'insets'];
    } else if (fn === 'root_transform_for_layout_node') {
      funcSummary = 'Computes the root transform matrix for a layout node.';
      funcTags = ['function', 'transform', 'layout'];
    } else if (fn === 'containing_block_for_node') {
      funcSummary = 'Finds the containing block for a given DOM node.';
      funcTags = ['function', 'containing-block', 'lookup'];
    } else if (fn === 'is_containing_block_for_position') {
      funcSummary = 'Checks if an element is a containing block for a given positioning scheme.';
      funcTags = ['function', 'containing-block', 'check'];
    } else if (fn === 'transform_au_rectangle') {
      funcSummary = 'Transforms an Au rectangle by a given matrix.';
      funcTags = ['function', 'transform', 'geometry'];
    } else if (fn === 'process_effective_overflow_query') {
      funcSummary = 'Processes an effective overflow query from script.';
      funcTags = ['function', 'overflow', 'query'];
    } else if (fn === 'find_character_offset_in_fragment_descendants') {
      funcSummary = 'Finds the character offset for a given point in fragment descendants.';
      funcTags = ['function', 'text', 'hit-test'];
    } else if (fn === 'default') {
      funcSummary = 'Provides default rendering steps for text collection.';
      funcTags = ['function', 'text', 'rendering'];
    } else if (fn === 'rendered_text_collection_steps') {
      funcSummary = 'Collects text rendering steps for accessibility or selection.';
      funcTags = ['function', 'text', 'accessibility'];
    } else if (fn === 'get_the_text_steps') {
      funcSummary = 'Gets the text rendering steps for a given DOM node and range.';
      funcTags = ['function', 'text', 'rendering'];
    } else if (fn === 'offset_parent_fragments') {
      funcSummary = 'Finds the offset parent fragments for a DOM node.';
      funcTags = ['function', 'offset-parent', 'positioning'];
    } else if (fn === 'process_offset_parent_query') {
      funcSummary = 'Processes the offsetParent DOM query for a node.';
      funcTags = ['function', 'offset-parent', 'query'];
    } else if (fn === 'process_scroll_container_query') {
      funcSummary = 'Processes a scroll container query from script.';
      funcTags = ['function', 'scroll', 'query'];
    } else if (fn === 'shorthand_to_css_string') {
      funcSummary = 'Converts shorthand CSS property values to strings.';
      funcTags = ['function', 'style', 'serialization'];
    } else if (fn === 'resolved_size_should_be_used_value' || fn === 'should_honor_min_size_auto') {
      funcSummary = 'Determines size resolution behavior for resolved styles.';
      funcTags = ['function', 'style', 'resolution'];
    } else if (fn === 'resolve_grid_template') {
      funcSummary = 'Resolves grid-template shorthand values to CSS strings.';
      funcTags = ['function', 'grid', 'serialization'];
    } else if (fn === 'process_resolved_style_request_for_unstyled_node') {
      funcSummary = 'Processes resolved style requests for non-styled nodes like text.';
      funcTags = ['function', 'style', 'query'];
    } else if (fn === 'process_resolved_style_request') {
      funcSummary = 'Processes a resolved style query from script for a DOM node.';
      funcTags = ['function', 'style', 'query'];
    } else if (fn === 'compute_caption_minimum_inline_size') {
      funcSummary = 'Computes the minimum inline size for table captions.';
      funcTags = ['function', 'table', 'caption'];
    } else if (fn === 'calculate_row_sizes_after_first_layout') {
      funcSummary = 'Calculates row sizes after the first layout pass in table layout.';
      funcTags = ['function', 'table', 'row-heights'];
    } else if (fn === 'border_spacing' || fn === 'total_border_spacing') {
      funcSummary = 'Calculates border spacing values for separated-border table model.';
      funcTags = ['function', 'table', 'border-spacing'];
    } else if (fn === 'get_column_measure_for_column_at_index' || fn === 'get_row_measure_for_row_at_index') {
      funcSummary = 'Retrieves column or row measure data for a given index in the table grid.';
      funcTags = ['function', 'table', 'measure'];
    } else if (fn === 'get_row_group_rect' || fn === 'get_column_group_rect' || fn === 'get_cell_rect') {
      funcSummary = 'Gets the rectangle for a table row group, column group, or cell.';
      funcTags = ['function', 'table', 'geometry'];
    } else if (fn === 'compute_inline_content_sizes' && fp.includes('taffy/layout')) {
      funcSummary = 'Computes inline content sizes for flex/grid containers.';
      funcTags = ['function', 'taffy', 'content-sizing'];
    } else if (fn === 'with_base' || fn === 'with_base_mut') {
      funcSummary = 'Accesses the base data of a table-level box.';
      funcTags = ['function', 'table', 'base-access'];
    } else if (fn === 'downgrade' || fn === 'upgrade') {
      funcSummary = 'Converts between strong and weak references for table-level boxes.';
      funcTags = ['function', 'table', 'reference'];
    } else if (fn === 'resolve_first_cell_coords' || fn === 'resolve_first_cell') {
      funcSummary = 'Resolves the coordinates of the first cell for table grid placement.';
      funcTags = ['function', 'table', 'grid-placement'];
    } else if (fn === 'mock_for_testing') {
      funcSummary = 'Creates a mock table instance for testing purposes.';
      funcTags = ['function', 'table', 'testing'];
    } else if (fn === 'node_id') {
      funcSummary = 'Returns the DOM node ID for this table-level box.';
      funcTags = ['function', 'table', 'node'];
    } else if (fn === 'is_empty') {
      funcSummary = 'Checks whether the table track is empty.';
      funcTags = ['function', 'table', 'utility'];
    } else if (fn === 'is_line_box') {
      funcSummary = 'Checks whether the fragment is a line box.';
      funcTags = ['function', 'fragment', 'line-box'];
    } else if (fn === 'is_inline_box') {
      funcSummary = 'Checks whether the fragment is an inline box.';
      funcTags = ['function', 'fragment', 'inline-box'];
    } else if (fn === 'is_atomic_inline_level') {
      funcSummary = 'Checks whether the fragment is atomic inline-level.';
      funcTags = ['function', 'fragment', 'atomic-inline'];
    } else if (fn === 'has_collapsed_borders') {
      funcSummary = 'Checks whether the fragment has collapsed table borders.';
      funcTags = ['function', 'fragment', 'table-borders'];
    } else if (fn === 'has_outline') {
      funcSummary = 'Checks whether the fragment has an outline.';
      funcTags = ['function', 'fragment', 'outline'];
    } else if (fn === 'is_flex_or_grid_item') {
      funcSummary = 'Checks whether the fragment is a flex or grid item.';
      funcTags = ['function', 'fragment', 'flex-grid'];
    } else if (fn === 'is_replaced') {
      funcSummary = 'Checks whether the fragment is a replaced element.';
      funcTags = ['function', 'fragment', 'replaced'];
    } else if (fn === 'is_table_wrapper') {
      funcSummary = 'Checks whether the fragment wraps a table element.';
      funcTags = ['function', 'fragment', 'table'];
    } else if (fn === 'is_table_grid_with_collapsed_borders') {
      funcSummary = 'Checks whether the fragment is a table grid with collapsed borders.';
      funcTags = ['function', 'fragment', 'table-borders'];
    } else if (fn === 'spatial_tree_node') {
      funcSummary = 'Gets the spatial tree node for this fragment.';
      funcTags = ['function', 'spatial', 'node'];
    } else if (fn === 'is_root_element') {
      funcSummary = 'Checks whether the fragment is the root html element.';
      funcTags = ['function', 'fragment', 'root'];
    } else if (fn === 'is_body_element_of_html_element_root') {
      funcSummary = 'Checks whether the fragment is the body element of the root html.';
      funcTags = ['function', 'fragment', 'body'];
    } else if (fn === 'character_offset') {
      funcSummary = 'Computes the character offset within a text fragment at a given point.';
      funcTags = ['function', 'text', 'hit-test'];
    } else if (fn === 'point_is_within_vertical_boundaries') {
      funcSummary = 'Checks if a point is within the vertical boundaries of a text fragment.';
      funcTags = ['function', 'text', 'hit-test'];
    } else if (fn === 'distance_to_point_for_glyph_offset') {
      funcSummary = 'Computes the distance to a point for glyph offset determination.';
      funcTags = ['function', 'text', 'hit-test'];
    } else if (fn === 'client_rect') {
      funcSummary = 'Computes the client rect (CSSOM getClientRects) for a fragment.';
      funcTags = ['function', 'geometry', 'dom-api'];
    } else if (fn === 'cumulative_box_area_rect') {
      funcSummary = 'Computes the cumulative box area rect for a specific box area type.';
      funcTags = ['function', 'geometry', 'box-area'];
    } else if (fn === 'scrolling_area') {
      funcSummary = 'Computes the scrolling area rect for a fragment.';
      funcTags = ['function', 'scrolling', 'geometry'];
    } else if (fn === 'children') {
      funcSummary = 'Returns the child fragments of this fragment as an iterator.';
      funcTags = ['function', 'fragment', 'children'];
    } else if (fn === 'retrieve_box_fragment') {
      funcSummary = 'Retrieves the inner BoxFragment from a fragment variant.';
      funcTags = ['function', 'fragment', 'accessor'];
    } else if (fn === 'calculate_resolved_insets_if_positioned') {
      funcSummary = 'Calculates resolved inset values for positioned elements.';
      funcTags = ['function', 'positioned', 'insets', 'resolution'];
    } else if (fn === 'clip_wholly_unreachable_scrollable_overflow') {
      funcSummary = 'Clips scrollable overflow that is unreachable due to overflow clipping.';
      funcTags = ['function', 'overflow', 'clipping'];
    } else if (fn === 'scrollable_overflow_for_parent') {
      funcSummary = 'Returns the overflow area for the parent fragment to use.';
      funcTags = ['function', 'overflow', 'parent'];
    } else if (fn === 'scrollable_overflow_padding_contribution_for_parent') {
      funcSummary = 'Returns the padding box overflow contribution for the parent.';
      funcTags = ['function', 'overflow', 'padding'];
    } else if (fn === 'clear_scrollable_overflow') {
      funcSummary = 'Clears the cached scrollable overflow, forcing recalculation.';
      funcTags = ['function', 'overflow', 'cache'];
    } else if (fn === 'set_containing_block') {
      funcSummary = 'Sets the containing block rectangle for this fragment.';
      funcTags = ['function', 'containing-block', 'setter'];
    } else if (fn === 'cumulative_content_box_rect') {
      funcSummary = 'Computes the cumulative content box rect offset by containing block.';
      funcTags = ['function', 'geometry', 'content-box'];
    } else if (fn === 'cumulative_padding_box_rect') {
      funcSummary = 'Computes the cumulative padding box rect offset by containing block.';
      funcTags = ['function', 'geometry', 'padding-box'];
    } else if (fn === 'cumulative_border_box_rect') {
      funcSummary = 'Computes the cumulative border box rect offset by containing block.';
      funcTags = ['function', 'geometry', 'border-box'];
    } else if (fn === 'padding_rect' || fn === 'border_rect' || fn === 'margin_rect' || fn === 'padding_border_margin') {
      funcSummary = 'Computes the ' + fn.replace('_', ' ') + ' for this fragment.';
      funcTags = ['function', 'geometry', fn];
    } else if (fn === 'content_rect') {
      funcSummary = 'Returns the content rect (base rect) of this fragment.';
      funcTags = ['function', 'geometry', 'content-rect'];
    } else if (fn === 'offset_by_containing_block') {
      funcSummary = 'Offsets a rect by the cumulative containing block origin.';
      funcTags = ['function', 'geometry', 'containing-block'];
    } else if (fn === 'with_style') {
      funcSummary = 'Wraps the fragment with its style for layout operations.';
      funcTags = ['function', 'style', 'wrapper'];
    } else if (fn === 'baselines') {
      funcSummary = 'Computes baseline information for this fragment.';
      funcTags = ['function', 'baselines', 'typography'];
    } else if (fn === 'add_extra_background') {
      funcSummary = 'Adds an extra background layer to the fragment.';
      funcTags = ['function', 'background', 'styling'];
    } else if (fn === 'set_does_not_paint_background') {
      funcSummary = 'Marks the fragment as not painting a background.';
      funcTags = ['function', 'background', 'optimization'];
    } else if (fn === 'ensure_rare_data') {
      funcSummary = 'Ensures the rare data field is initialized for this BoxFragment.';
      funcTags = ['function', 'initialization', 'rare-data'];
    } else if (fn === 'specific_layout_info') {
      funcSummary = 'Returns the specific layout info (grid, table) for this fragment.';
      funcTags = ['function', 'layout', 'specific-info'];
    } else if (fn === 'resolved_sticky_insets' || fn === 'set_resolved_sticky_insets') {
      funcSummary = 'Manages resolved sticky positioning inset values.';
      funcTags = ['function', 'sticky', 'insets'];
    } else if (fn === 'generated_clip_id' || fn === 'set_generated_clip_id') {
      funcSummary = 'Manages the generated clip ID for this fragment.';
      funcTags = ['function', 'clip'];
    } else if (fn === 'generated_scroll_tree_node_id' || fn === 'set_generated_scroll_tree_node_id') {
      funcSummary = 'Manages the generated scroll tree node ID.';
      funcTags = ['function', 'scroll', 'tree-node'];
    } else if (fn === 'with_block_level_layout_info') {
      funcSummary = 'Adds block-level layout info (clearance, collapsed margins) to the fragment.';
      funcTags = ['function', 'block-level', 'layout-info'];
    } else if (fn === 'to_hoisted') {
      funcSummary = 'Converts an AbsolutelyPositionedBox to a HoistedAbsolutelyPositionedBox.';
      funcTags = ['function', 'positioned', 'hoisting'];
    } else if (fn === 'new_for_layout_box_base') {
      funcSummary = 'Creates a new PositioningContext from a LayoutBoxBase.';
      funcTags = ['function', 'positioning', 'constructor'];
    } else if (fn === 'body_fragment') {
      funcSummary = 'Finds and returns the body element fragment in the fragment tree.';
      funcTags = ['function', 'fragment-tree', 'body'];
    } else if (fn === 'root_box_fragment') {
      funcSummary = 'Returns the root box fragment of the fragment tree.';
      funcTags = ['function', 'fragment-tree', 'root'];
    } else if (fn === 'box_damage_action') {
      funcSummary = 'Determines the damage action for a specific box based on its damage flags.';
      funcTags = ['function', 'damage', 'box-tree'];
    } else if (fn === 'adjust_inline_content_size_damage') {
      funcSummary = 'Adjusts damage based on inline content size changes.';
      funcTags = ['function', 'damage', 'inline-sizing'];
    } else if (fn === 'isolate_incoming_damage') {
      funcSummary = 'Isolates incoming damage to prevent propagation beyond formatting context boundaries.';
      funcTags = ['function', 'damage', 'isolation'];
    } else if (fn === 'propagate_damage_to_children') {
      funcSummary = 'Propagates damage flags from parent to children during traversal.';
      funcTags = ['function', 'damage', 'propagation'];
    } else if (fn === 'box_size' || fn === 'min_box_size' || fn === 'max_box_size') {
      funcSummary = 'Gets the ' + fn.replace('_', ' ') + ' style value for layout.';
      funcTags = ['function', 'style', 'box-sizing'];
    } else if (fn === 'content_box_size_for_box_size' || fn === 'content_min_box_size_for_min_size' || fn === 'content_max_box_size_for_max_size') {
      funcSummary = 'Gets the content box size from box sizing style, accounting for box-sizing property.';
      funcTags = ['function', 'style', 'content-sizing'];
    } else if (fn === 'physical_box_offsets') {
      funcSummary = 'Converts logical box offsets to physical offsets.';
      funcTags = ['function', 'geometry', 'box-offsets'];
    } else if (fn === 'physical_margin') {
      funcSummary = 'Converts logical margin values to physical margin values.';
      funcTags = ['function', 'geometry', 'margin'];
    } else if (fn === 'border_style_color') {
      funcSummary = 'Gets the border style and color for a given side.';
      funcTags = ['function', 'border', 'style-color'];
    } else if (fn === 'is_transformable') {
      funcSummary = 'Checks if the element is transformable based on its display type.';
      funcTags = ['function', 'transform', 'check'];
    } else if (fn === 'z_index_applies') {
      funcSummary = 'Checks if the z-index property applies to this element.';
      funcTags = ['function', 'z-index', 'stacking'];
    } else if (fn === 'used_transform_style') {
      funcSummary = 'Determines the effective transform style for the element.';
      funcTags = ['function', 'transform', 'style'];
    } else if (fn === 'establishes_containing_block_for_absolute_descendants') {
      funcSummary = 'Checks if the element establishes a containing block for absolutely positioned descendants.';
      funcTags = ['function', 'containing-block', 'absolute'];
    } else if (fn === 'establishes_containing_block_for_all_descendants') {
      funcSummary = 'Checks if the element establishes a containing block for all positioned descendants.';
      funcTags = ['function', 'containing-block', 'all-descendants'];
    } else if (fn === 'background_is_transparent') {
      funcSummary = 'Checks if the background is fully transparent.';
      funcTags = ['function', 'background', 'transparency'];
    } else if (fn === 'bidi_control_chars') {
      funcSummary = 'Gets the bidirectional control characters for text layout.';
      funcTags = ['function', 'bidi', 'text'];
    } else if (fn === 'resolve_align_self') {
      funcSummary = 'Resolves the align-self value for a flex/grid item.';
      funcTags = ['function', 'alignment', 'flex-grid'];
    } else if (fn === 'depends_on_block_constraints_due_to_relative_positioning') {
      funcSummary = 'Checks if sizing depends on block constraints due to relative positioning.';
      funcTags = ['function', 'sizing', 'relative'];
    } else if (fn === 'content_box_sizes_and_padding_border_margin') {
      funcSummary = 'Computes content box sizes and padding/border/margin for a box.';
      funcTags = ['function', 'sizing', 'box-model'];
    } else if (fn === 'padding') {
      funcSummary = 'Computes logical padding values from style.';
      funcTags = ['function', 'style', 'padding'];
    } else if (fn === 'border_width') {
      funcSummary = 'Computes border widths from style for each side.';
      funcTags = ['function', 'style', 'border-width'];
    } else if (fn === 'padding_border_margin_with_writing_mode_and_containing_block_inline_size') {
      funcSummary = 'Computes padding, border, and margin with explicit writing mode and containing block inline size.';
      funcTags = ['function', 'style', 'padding-border-margin'];
    } else if (fn === 'padding_border_margin') {
      funcSummary = 'Computes padding, border, and margin from style.';
      funcTags = ['function', 'style', 'padding-border-margin'];
    } else if (fn === 'content_box_sizes_and_padding_border_margin') {
      funcSummary = 'Computes content box sizes and padding/border/margin values together.';
      funcTags = ['function', 'sizing', 'box-model'];
    } else if (fn === 'is_table' || fn === 'is_table' && fp.includes('style_ext')) {
      funcSummary = 'Checks if the element is a table element based on its display type.';
      funcTags = ['function', 'display', 'table'];
    } else if (fn === 'from_natural_size_in_dots') {
      funcSummary = 'Creates NaturalSizes from physical dimensions in dots (pixels).';
      funcTags = ['function', 'replaced', 'natural-sizes'];
    } else if (fn === 'from_width_and_height') {
      funcSummary = 'Creates NaturalSizes from explicit width and height.';
      funcTags = ['function', 'replaced', 'natural-sizes'];
    } else if (fn === 'empty') {
      funcSummary = 'Creates empty NaturalSizes for elements with no intrinsic size.';
      funcTags = ['function', 'replaced', 'natural-sizes'];
    } else if (fn === 'from_content_property') {
      funcSummary = 'Creates ReplacedContents from the CSS content property value.';
      funcTags = ['function', 'replaced', 'content-property'];
    } else if (fn === 'from_image_url') {
      funcSummary = 'Creates ReplacedContents from an image URL.';
      funcTags = ['function', 'replaced', 'image-url'];
    } else if (fn === 'from_image') {
      funcSummary = 'Creates ReplacedContents from a Servo image resource.';
      funcTags = ['function', 'replaced', 'image'];
    } else if (fn === 'zero_sized_invalid_image') {
      funcSummary = 'Creates a zero-sized invalid image placeholder.';
      funcTags = ['function', 'replaced', 'invalid-image'];
    } else if (fn === 'logical_natural_sizes') {
      funcSummary = 'Computes logical-coordinate natural sizes for the replaced element.';
      funcTags = ['function', 'replaced', 'natural-sizes'];
    } else if (fn === 'fallback_inline_size' || fn === 'fallback_block_size') {
      funcSummary = 'Computes the fallback inline/block size for replaced elements.';
      funcTags = ['function', 'replaced', 'fallback-size'];
    } else if (fn === 'try_to_parse_image_data_url') {
      funcSummary = 'Attempts to parse an inline image data URL.';
      funcTags = ['function', 'replaced', 'data-url'];
    } else if (fn === 'svg_kind_size') {
      funcSummary = 'Computes the size of SVG content based on viewBox and intrinsic dimensions.';
      funcTags = ['function', 'svg', 'sizing'];
    } else if (fn === 'outer_inline') {
      funcSummary = 'Computes the outer inline size for a CSS sizing mode.';
      funcTags = ['function', 'sizing', 'outer-inline'];
    } else if (fn === 'is_initial') {
      funcSummary = 'Checks if a size value is the initial value.';
      funcTags = ['function', 'sizing', 'check'];
    } else if (fn === 'to_numeric') {
      funcSummary = 'Converts a size to a numeric value.';
      funcTags = ['function', 'sizing', 'conversion'];
    } else if (fn === 'to_percentage') {
      funcSummary = 'Converts a size to a percentage value.';
      funcTags = ['function', 'sizing', 'conversion'];
    } else if (fn === 'resolve_percentages_for_preferred' || fn === 'resolve_percentages_for_max') {
      funcSummary = 'Resolves percentage values for preferred or max sizes.';
      funcTags = ['function', 'sizing', 'percentage-resolution'];
    } else if (fn === 'percentages_relative_to_basis') {
      funcSummary = 'Computes percentage values relative to a basis size.';
      funcTags = ['function', 'sizing', 'percentage'];
    } else if (fn === 'resolve_for_preferred' || fn === 'resolve_for_min' || fn === 'resolve_for_max') {
      funcSummary = 'Resolves a size constraint for the preferred/min/max size.';
      funcTags = ['function', 'sizing', 'resolution'];
    } else if (fn === 'maybe_resolve_extrinsic') {
      funcSummary = 'Optionally resolves an extrinsic size constraint.';
      funcTags = ['function', 'sizing', 'extrinsic'];
    } else if (fn === 'is_definite' || fn === 'to_definite' || fn === 'definite_or_min') {
      funcSummary = 'Definite-ness check and coercion for sizing constraints.';
      funcTags = ['function', 'sizing', 'definite'];
    } else if (fn === 'resolve_each' || fn === 'resolve_extrinsic' || fn === 'resolve_each_extrinsic') {
      funcSummary = 'Resolves each sizing component with optional extrinsic constraint.';
      funcTags = ['function', 'sizing', 'resolution'];
    } else if (fn === 'intrinsic') {
      funcSummary = 'Creates intrinsic sizing data with min and max content values.';
      funcTags = ['function', 'sizing', 'intrinsic'];
    } else if (fn === 'compute_dependent_size') {
      funcSummary = 'Computes a size value that depends on aspect ratio and another dimension.';
      funcTags = ['function', 'aspect-ratio', 'size'];
    } else if (fn === 'from_logical_content_ratio') {
      funcSummary = 'Creates an AspectRatio from logical content ratio.';
      funcTags = ['function', 'aspect-ratio', 'conversion'];
    } else if (fn === 'from_border') {
      funcSummary = 'Creates PaddingBorderMargin from border data.';
      funcTags = ['function', 'geometry', 'conversion'];
    } else if (fn === 'hidden') {
      funcSummary = 'Creates a PaddingBorderMargin with zero values.';
      funcTags = ['function', 'geometry', 'conversion'];
    } else if (fn === 'sums_auto_is_zero') {
      funcSummary = 'Checks if the sum of logical sides auto values is zero.';
      funcTags = ['function', 'geometry', 'check'];
    } else if (fn === 'from_physical_size') {
      funcSummary = 'Creates a logical size from physical dimensions and writing mode.';
      funcTags = ['function', 'geometry', 'conversion'];
    } else if (fn === 'map_with') {
      funcSummary = 'Maps the logical rect with separate inline and block functions.';
      funcTags = ['function', 'geometry', 'mapping'];
    } else if (fn === 'map_inline_and_block_axes' || fn === 'map_inline_and_block_sizes') {
      funcSummary = 'Maps the logical sides using per-axis functions.';
      funcTags = ['function', 'geometry', 'mapping'];
    } else if (fn === 'to_physical') {
      funcSummary = 'Converts logical sides to physical sides using a writing mode.';
      funcTags = ['function', 'geometry', 'conversion'];
    } else if (fn === 'to_physical_size') {
      funcSummary = 'Converts a logical size to a physical size using writing mode.';
      funcTags = ['function', 'geometry', 'conversion'];
    } else if (fn === 'to_physical_vector') {
      funcSummary = 'Converts a logical vector to a physical vector using writing mode.';
      funcTags = ['function', 'geometry', 'conversion'];
    } else if (fn === 'as_physical') {
      funcSummary = 'Converts logical sides to physical sides with border widths.';
      funcTags = ['function', 'geometry', 'conversion'];
    } else if (fn === 'inflate' || fn === 'deflate') {
      funcSummary = fn.charAt(0).toUpperCase() + fn.slice(1) + 's a physical rect by logical side widths.';
      funcTags = ['function', 'geometry', 'sides'];
    } else if (fn === 'max_inline_position' || fn === 'max_block_position') {
      funcSummary = 'Computes the maximum inline or block position from sides.';
      funcTags = ['function', 'geometry', 'position'];
    } else if (fn === 'either_specified' || fn === 'either_auto') {
      funcSummary = 'Checks if either side value is specified or auto.';
      funcTags = ['function', 'geometry', 'check'];
    } else if (fn === 'inline_sides' || fn === 'block_sides') {
      funcSummary = 'Returns the inline or block sides from a LogicalSides value.';
      funcTags = ['function', 'geometry', 'accessor'];
    } else if (fn === 'start_offset') {
      funcSummary = 'Returns the start offset along a given axis.';
      funcTags = ['function', 'geometry', 'offset'];
    } else if (fn === 'inline_sum' || fn === 'block_sum' || fn === 'sum') {
      funcSummary = 'Sums the inline, block, or all side values.';
      funcTags = ['function', 'geometry', 'sum'];
    } else if (fn === 'percentages_relative_to') {
      funcSummary = 'Returns the percentage-relative basis for this logical size.';
      funcTags = ['function', 'geometry', 'percentage'];
    } else if (fn === 'to_logical') {
      funcSummary = 'Converts physical values to logical coordinates using writing mode.';
      funcTags = ['function', 'geometry', 'conversion'];
    } else if (fn === 'auto_is') {
      funcSummary = 'Checks if the side value is auto.';
      funcTags = ['function', 'geometry', 'check'];
    } else if (fn === 'map') {
      funcSummary = 'Maps the sides using a function.';
      funcTags = ['function', 'geometry', 'mapping'];
    } else if (fn === 'get' || fn === 'set') {
      funcSummary = 'Atomically gets or sets the physical rect value.';
      funcTags = ['function', 'geometry', 'atomic'];
    } else if (fn === 'origin' || fn === 'size') {
      funcSummary = 'Atomically gets the origin or size of the physical rect.';
      funcTags = ['function', 'geometry', 'atomic'];
    } else if (fn === 'set_origin' || fn === 'set_size') {
      funcSummary = 'Atomically sets the origin or size of the physical rect.';
      funcTags = ['function', 'geometry', 'atomic'];
    } else if (fn === 'translate') {
      funcSummary = 'Atomically translates the physical rect by a vector.';
      funcTags = ['function', 'geometry', 'atomic'];
    } else if (fn === 'new' && fp === 'components/layout/geom.rs') {
      funcSummary = 'Creates a new SyncPhysicalRectAu from a physical rect.';
      funcTags = ['function', 'geometry', 'constructor'];
    } else if (fn === 'from_margin') {
      funcSummary = 'Creates collapsed block margins from a resolved margin value.';
      funcTags = ['function', 'margins', 'collapsing'];
    } else if (fn === 'zero') {
      funcSummary = 'Creates a zero value for this type.';
      funcTags = ['function', 'constructor', 'zero'];
    } else if (fn === 'finish') {
      funcSummary = 'Finalizes the construction of a table structure element.';
      funcTags = ['function', 'table', 'construction'];
    } else if (fn === 'compute_damage_and_rebuild_box_tree_above_dirty_root' || fn === 'compute_damage_and_rebuild_box_tree_below_dirty_root') {
      funcSummary = 'Rebuilds the box tree above or below a dirty root during incremental layout.';
      funcTags = ['function', 'damage', 'box-tree', 'rebuild'];
    } else if (fn === 'outer_inline' && fp === 'components/layout/sizing.rs') {
      funcSummary = 'Computes the outer inline size for a CSS sizing mode.';
      funcTags = ['function', 'sizing', 'outer-inline'];
    } else if (fn === 'max') {
      funcSummary = 'Computes the maximum of two content sizes.';
      funcTags = ['function', 'sizing', 'max'];
    } else {
      funcSummary = fn + ' operation on this type.';
      funcTags = ['function'];
    }

    nodes.push({
      id: funcId,
      type: 'function',
      name: f.name,
      filePath: fp,
      lineRange: [f.startLine, f.endLine],
      summary: funcSummary,
      tags: funcTags,
      complexity: complexityLines(lc)
    });

    edges.push({
      source: fileId,
      target: funcId,
      type: 'contains',
      direction: 'forward',
      weight: 1.0
    });

    if (isEx) {
      edges.push({
        source: fileId,
        target: funcId,
        type: 'exports',
        direction: 'forward',
        weight: 0.8
      });
    }
  }

  // Classes
  for (const c of classes) {
    if (!isSigClass(c, exportedNames)) continue;
    const lc = c.endLine - c.startLine + 1;
    const isEx = exportedNames.has(c.name);
    const classId = 'class:' + fp + ':' + c.name;

    let clsSummary, clsTags;
    if (classMeta[c.name]) {
      clsSummary = classMeta[c.name][0];
      clsTags = classMeta[c.name][1];
    } else {
      clsSummary = c.name + ' type used in the Servo layout engine.';
      clsTags = ['class'];
    }

    nodes.push({
      id: classId,
      type: 'class',
      name: c.name,
      filePath: fp,
      lineRange: [c.startLine, c.endLine],
      summary: clsSummary,
      tags: clsTags,
      complexity: complexityLines(lc)
    });

    edges.push({
      source: fileId,
      target: classId,
      type: 'contains',
      direction: 'forward',
      weight: 1.0
    });

    if (isEx) {
      edges.push({
        source: fileId,
        target: classId,
        type: 'exports',
        direction: 'forward',
        weight: 0.8
      });
    }
  }
}

console.log('Total nodes:', nodes.length);
console.log('Total edges:', edges.length);

// Partition
const N = fileOrder.length;
const nodeCount = nodes.length;
const edgeCount = edges.length;
const parts = Math.ceil(Math.max(nodeCount / 60, edgeCount / 120));
console.log('Parts needed:', parts);

const filesPerPart = Math.ceil(N / parts);

// Map file to node IDs
const fileNodeIds = {};
for (const n of nodes) {
  const fp = n.filePath;
  if (!fileNodeIds[fp]) fileNodeIds[fp] = [];
  fileNodeIds[fp].push(n.id);
}

const allNodeIds = new Set(nodes.map(n => n.id));

for (let partIdx = 0; partIdx < parts; partIdx++) {
  const startFile = partIdx * filesPerPart;
  const endFile = Math.min(startFile + filesPerPart, N);
  const partFiles = fileOrder.slice(startFile, endFile);

  const partNodeIds = new Set();
  for (const fp of partFiles) {
    for (const nid of (fileNodeIds[fp] || [])) {
      partNodeIds.add(nid);
    }
  }

  const partEdges = edges.filter(e => partNodeIds.has(e.source));
  const partNodes = nodes.filter(n => partNodeIds.has(n.id));

  const partNum = partIdx + 1;
  const isSingle = parts === 1;
  const outFile = isSingle
    ? 'D:/Projects/servo/.understand-anything/intermediate/batch-7.json'
    : 'D:/Projects/servo/.understand-anything/intermediate/batch-7-part-' + partNum + '.json';

  const outData = { nodes: partNodes, edges: partEdges };
  fs.writeFileSync(outFile, JSON.stringify(outData, null, 2), 'utf8');
  console.log('Wrote', outFile, partNodes.length, 'nodes,', partEdges.length, 'edges');

  // Validate
  let errors = [];
  for (const e of partEdges) {
    if (!allNodeIds.has(e.source)) errors.push('source not in any node: ' + e.source);
    if (!allNodeIds.has(e.target)) errors.push('target not in any node: ' + e.target);
  }
  if (errors.length > 0) {
    console.log('  VALIDATION ERRORS:', errors.slice(0, 10));
  } else {
    console.log('  Validation OK');
  }
}

console.log('Done.');
