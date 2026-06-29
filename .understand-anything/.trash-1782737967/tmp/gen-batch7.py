import json, math, os, sys

with open('D:/Projects/servo/.understand-anything/tmp/ua-file-extract-results-7.json', 'r') as f:
    data = json.load(f)

def complexity_class(nel):
    if nel < 50: return 'simple'
    if nel < 200: return 'moderate'
    return 'complex'

def complexity_lines(n):
    if n < 10: return 'simple'
    if n < 30: return 'moderate'
    return 'complex'

def is_sig_func(f, exported_names):
    lc = f['endLine'] - f['startLine'] + 1
    is_ex = f['name'] in exported_names
    if f['name'] == 'fmt' and lc < 15: return False
    if f['name'] == 'from' and len(f.get('params', [])) <= 1 and lc < 10: return False
    return (is_ex and lc >= 3) or lc >= 10

def is_sig_class(c, exported_names):
    lc = c['endLine'] - c['startLine'] + 1
    is_ex = c['name'] in exported_names
    return (len(c.get('methods', [])) >= 2) or lc >= 20 or is_ex

# File summaries map
file_meta = {}
file_meta['components/layout/fragment_tree/base_fragment.rs'] = {
    'summary': 'Defines the BaseFragment, FragmentStatus, BaseFragmentInfo, and Tag types that form the core fragment data structure, providing shared fields (rect, style, status, flags) and construction logic for all fragment types in the layout tree.',
    'tags': ['fragment-tree', 'data-model', 'layout', 'core-types']
}
file_meta['components/layout/fragment_tree/box_fragment.rs'] = {
    'summary': 'Implements BoxFragment, the primary fragment type representing CSS boxes with children, containing block tracking, scrollable overflow calculation, baseline management, background modes, and resolved positioning insets.',
    'tags': ['fragment-tree', 'box-model', 'layout', 'scrollable-overflow']
}
file_meta['components/layout/fragment_tree/containing_block.rs'] = {
    'summary': 'Provides ContainingBlockManager that tracks the containing block chain for fragments, supporting non-absolute, absolute, and fixed descendant containment strategies during layout.',
    'tags': ['fragment-tree', 'containing-block', 'layout', 'positioning']
}
file_meta['components/layout/fragment_tree/fragment.rs'] = {
    'summary': 'Defines the Fragment enum (the top-level fragment abstraction) with variants for LayoutRoot, Box, Float, Positioning, Text, Image, and IFrame fragments, plus CollapsedBlockMargins/CollapsedMargin for margin collapsing and the ContainingBlockCalculation state machine.',
    'tags': ['fragment-tree', 'data-model', 'layout', 'enum']
}
file_meta['components/layout/fragment_tree/fragment_tree.rs'] = {
    'summary': 'Implements FragmentTree, the top-level container that holds the root fragment tree for a layout document, providing scrolling area computation, fragment finding by position, and body fragment resolution.',
    'tags': ['fragment-tree', 'tree-structure', 'layout', 'root']
}
file_meta['components/layout/fragment_tree/hoisted_shared_fragment.rs'] = {
    'summary': 'Defines HoistedSharedFragment, a lightweight wrapper for fragments hoisted out of the normal tree flow (e.g., for absolutely positioned elements), with an optional style override.',
    'tags': ['fragment-tree', 'hoisted', 'absolute-positioning', 'layout']
}
file_meta['components/layout/fragment_tree/mod.rs'] = {
    'summary': 'Module barrel file re-exporting all fragment tree types for public use by other layout modules.',
    'tags': ['fragment-tree', 'barrel', 'module', 're-exports']
}
file_meta['components/layout/fragment_tree/positioning_fragment.rs'] = {
    'summary': 'Implements PositioningFragment, which wraps fragments needing special positioning treatment (e.g., floats), providing containing block offset, scrollable overflow, and print debugging support.',
    'tags': ['fragment-tree', 'positioning', 'float', 'layout']
}
file_meta['components/layout/geom.rs'] = {
    'summary': 'Defines logical geometry types (LogicalVec2, LogicalRect, LogicalSides, LogicalSides1D) for writing-mode-aware CSS layout, plus SyncPhysicalRectAu for thread-safe physical rect storage and conversion between physical and logical coordinate spaces.',
    'tags': ['geometry', 'logical-coordinates', 'writing-modes', 'layout']
}
file_meta['components/layout/layout_box_base.rs'] = {
    'summary': 'Implements LayoutBoxBase, the foundational structure for layout boxes, managing fragment storage, inline content sizing, caching of independent formatting context and same-formatting-context block layout results, and invalidation of layout caches.',
    'tags': ['layout-box', 'base', 'fragment-storage', 'caching']
}
file_meta['components/layout/layout_impl.rs'] = {
    'summary': 'Implements LayoutThread, the core layout engine loop: reflow orchestration, style resolution, font loading, display list building, accessibility tree updates, stacking context tree construction, and query handling for resolved styles, box areas, scroll containers, and font metrics.',
    'tags': ['layout-engine', 'reflow', 'display-list', 'styling', 'queries']
}
file_meta['components/layout/layout_root.rs'] = {
    'summary': 'Provides LayoutRoot, the initial containing block layout root that performs the top-level layout pass, determining viewport size and running the full box tree layout algorithm.',
    'tags': ['layout-root', 'initial-containing-block', 'viewport', 'layout']
}
file_meta['components/layout/lib.rs'] = {
    'summary': 'Module entry point for the layout crate, re-exporting core types (ConstraintSpace, ContainingBlock, PropagatedBoxTreeData) and establishing the crate public API surface.',
    'tags': ['crate-root', 'barrel', 'module', 'layout']
}
file_meta['components/layout/lists.rs'] = {
    'summary': 'Provides CSS list marker generation including ordered list counter representation, marker string formatting (decimal, roman, alpha styles), and marker box construction for list items.',
    'tags': ['lists', 'counters', 'markers', 'css']
}
file_meta['components/layout/positioned.rs'] = {
    'summary': 'Implements absolute and fixed positioning layout: PositioningContext management, hoisting of absolutely positioned boxes, static position adjustment, inset resolution, margin auto-solving, and layout of the initial containing block children.',
    'tags': ['positioned-layout', 'absolute', 'fixed', 'insets', 'hoisting']
}
file_meta['components/layout/query.rs'] = {
    'summary': 'Handles layout thread queries from script (DOM APIs): resolved styles, box areas, client rects, scroll containers, offset parents, font metrics, containing blocks, text indices, and element-from-point hit testing.',
    'tags': ['queries', 'dom-api', 'resolved-style', 'layout-query']
}
file_meta['components/layout/quotes.rs'] = {
    'summary': 'Provides CSS quotes data, mapping language codes to their corresponding open/close quote pairs for the quotes CSS property.',
    'tags': ['quotes', 'css', 'localization', 'typography']
}
file_meta['components/layout/replaced.rs'] = {
    'summary': 'Implements replaced element layout (images, iframes, svg, video, canvas): content sizing from natural dimensions, aspect ratio preservation, SVG render tree construction, and fragment creation for replaced content.',
    'tags': ['replaced-elements', 'images', 'svg', 'iframe', 'content-sizing']
}
file_meta['components/layout/sizing.rs'] = {
    'summary': 'Provides content sizing infrastructure: ContentSizes (min/max content), SizeConstraint, Sizes resolution, intrinsic sizing modes, inline content size computation, and preferred/max/min size resolution for CSS layout.',
    'tags': ['sizing', 'intrinsic-sizes', 'content-sizes', 'constraints']
}
file_meta['components/layout/style_ext.rs'] = {
    'summary': 'Extends Servo computed styles with layout-specific methods: writing mode queries, box size accessors, overflow handling, transform processing, stacking context establishment, containing block creation, padding/border/margin calculations, and aspect ratio resolution.',
    'tags': ['style-extensions', 'css-properties', 'layout-style', 'overflow']
}
file_meta['components/layout/table/construct.rs'] = {
    'summary': 'Implements anonymous table object construction: walks the DOM tree to build table structure with rows, cells, columns, and column groups, handling rowspan/colspan and creating anonymous wrappers as needed per the CSS table model.',
    'tags': ['table', 'anonymous-objects', 'construct', 'rowspan-colspan']
}
file_meta['components/layout/table/layout.rs'] = {
    'summary': 'Implements table layout algorithm: column width distribution, row height calculation, collapsed border resolution, cell layout, caption placement, and table-specific inline content sizing.',
    'tags': ['table', 'layout-algorithm', 'column-widths', 'border-collapse', 'row-heights']
}
file_meta['components/layout/table/mod.rs'] = {
    'summary': 'Module re-exporting table-level box types: Table, TableSlot, TableTrack, TableTrackGroup, TableCaption, collapsed borders, and related types for table layout.',
    'tags': ['table', 'barrel', 'module', 'layout']
}
file_meta['components/layout/taffy/layout.rs'] = {
    'summary': 'Implements Taffy-based layout (Flexbox/Grid) integration: computes child layouts using the Taffy library, resolves content sizes, handles inline content sizing, and provides layout entry points for flex/grid containers.',
    'tags': ['taffy', 'flexbox', 'grid', 'layout']
}
file_meta['components/layout/taffy/mod.rs'] = {
    'summary': 'Defines TaffyContainer and TaffyItemBox types wrapping the Taffy layout library flexbox/grid layout capabilities, with tree attachment and box repair operations.',
    'tags': ['taffy', 'flexbox', 'grid', 'module']
}
file_meta['components/layout/taffy/stylo_taffy/convert.rs'] = {
    'summary': 'Converts Servo/Stylo computed style values into Taffy layout library style representations, including length, dimension, margin, inset, position, overflow, alignment, gap, grid, and track size conversions.',
    'tags': ['taffy', 'style-conversion', 'stylo', 'bridge']
}
file_meta['components/layout/taffy/stylo_taffy/mod.rs'] = {
    'summary': 'Module barrel for the stylo-to-taffy bridge types, re-exporting StyleTaffyConverter, TaffyStyloStyle, and related types.',
    'tags': ['taffy', 'stylo', 'module', 'barrel']
}
file_meta['components/layout/taffy/stylo_taffy/wrapper.rs'] = {
    'summary': 'Wraps Servo computed values into Taffy-compatible style objects (TaffyStyloStyle), providing layout-specific access to computed style properties for inset, border, grid template, and line name data.',
    'tags': ['taffy', 'style-wrapper', 'stylo', 'grid-properties']
}
file_meta['components/layout/traversal.rs'] = {
    'summary': 'Implements style damage computation and box tree rebuilding: traverses the layout tree to compute element damage sets, propagates damage to children, rebuilds the box tree above/below dirty roots, and handles inline content size adjustment damage.',
    'tags': ['traversal', 'damage', 'box-tree', 'style-recalc']
}

print("Loaded", len(file_meta), "file metadata entries")

nodes = []
edges = []
file_order = [r['path'] for r in data['results']]

for r in data['results']:
    fp = r['path']
    nel = r['nonEmptyLines']
    exp = r.get('exports', [])
    exported_names = set(e['name'] for e in exp)
    funcs = r.get('functions', [])
    classes = r.get('classes', [])
    file_id = 'file:' + fp
    name = fp.split('/')[-1]

    fm = file_meta.get(fp, {'summary': name + ' module.', 'tags': ['layout']})

    nodes.append({
        'id': file_id,
        'type': 'file',
        'name': name,
        'filePath': fp,
        'summary': fm['summary'],
        'tags': list(fm['tags']),
        'complexity': complexity_class(nel)
    })

    # Functions
    for f in funcs:
        if not is_sig_func(f, exported_names):
            continue
        lc = f['endLine'] - f['startLine'] + 1
        is_ex = f['name'] in exported_names
        func_id = 'function:' + fp + ':' + f['name']

        fn = f['name']

        # Determine function summary/tags based on name patterns
        if fn in ('new', 'new_for_testing', 'new_empty', 'new_anonymous', 'new_with_base_fragment_info'):
            func_summary = 'Constructs a new instance with the provided parameters.'
            func_tags = ['function', 'constructor']
        elif fn.startswith('compute_') or fn.startswith('calculate_'):
            subject = fn.replace('compute_', '').replace('calculate_', '').replace('_', ' ')
            func_summary = 'Computes ' + subject + ' from the current layout state.'
            func_tags = ['function', 'computation']
        elif fn.startswith('resolve_'):
            subject = fn.replace('resolve_', '').replace('_', ' ')
            func_summary = 'Resolves ' + subject + ' values based on available space and style constraints.'
            func_tags = ['function', 'resolution']
        elif fn == 'layout' or fn.startswith('layout_'):
            func_summary = 'Performs layout for this element or its children, computing sizes and positions.'
            func_tags = ['function', 'layout']
        elif fn == 'print':
            func_summary = 'Debug-prints the fragment tree structure for inspection.'
            func_tags = ['function', 'debug']
        elif 'scrollable' in fn:
            func_summary = 'Calculates or retrieves the scrollable overflow area for this fragment.'
            func_tags = ['function', 'scrollable-overflow']
        elif 'containing_block' in fn:
            func_summary = 'Manages the containing block relationship for this fragment.'
            func_tags = ['function', 'containing-block']
        elif fn.startswith('process_'):
            subject = fn.replace('process_', '').replace('_', ' ')
            func_summary = 'Processes a ' + subject + ' from the script/layout thread.'
            func_tags = ['function', 'query-processing']
        elif fn == 'handle_reflow':
            func_summary = 'Main reflow handler: performs style resolution, box construction, layout, and display list building.'
            func_tags = ['reflow', 'core', 'layout-engine']
        elif fn == 'restyle_and_build_trees':
            func_summary = 'Restyles the document and rebuilds the element/style/box trees, handling incremental style changes.'
            func_tags = ['function', 'styling', 'tree-building']
        elif fn == 'build_display_list':
            func_summary = 'Builds the display list from the stacking context tree for rendering.'
            func_tags = ['function', 'display-list', 'rendering']
        elif 'content_sizes' in fn or 'inline_content' in fn:
            func_summary = 'Computes the min/max inline content sizes for this element.'
            func_tags = ['function', 'content-sizing', 'intrinsic-sizes']
        elif 'cache' in fn:
            func_summary = 'Manages cached layout results to avoid redundant computation.'
            func_tags = ['function', 'caching']
        elif 'damage' in fn or 'repair' in fn:
            func_summary = 'Computes or applies style/layout damage for incremental update tracking.'
            func_tags = ['function', 'damage', 'incremental']
        elif fn == 'find':
            func_summary = 'Searches the fragment tree for fragments matching a processing function.'
            func_tags = ['function', 'search', 'traversal']
        elif fn == 'construct':
            func_summary = 'Constructs the box tree or table structure from DOM elements.'
            func_tags = ['function', 'construction']
        elif 'offset' in fn or 'translate' in fn:
            func_summary = 'Applies a positional offset or translation to the fragment geometry.'
            func_tags = ['function', 'positioning']
        elif 'marker' in fn or 'counter' in fn:
            func_summary = 'Generates CSS list marker content or counter representation.'
            func_tags = ['function', 'lists', 'counters']
        elif fn.startswith('from_') or fn == 'from':
            func_summary = 'Converts from another type or constructs from source parameters.'
            func_tags = ['function', 'conversion']
        elif fn == 'collect_reports':
            func_summary = 'Collects profiling and timing reports from the layout thread.'
            func_tags = ['function', 'profiling']
        elif fn.startswith('query_') and fp == 'components/layout/query.rs':
            subject = fn.replace('query_', '').replace('_', ' ')
            func_summary = 'Handles the ' + subject + ' DOM query from script.'
            func_tags = ['function', 'dom-query', 'layout-query']
        elif fn == 'make_marker':
            func_summary = 'Builds a marker fragment for list items.'
            func_tags = ['function', 'lists', 'markers']
        elif fn == 'generate_counter_representation':
            func_summary = 'Generates the string representation for a CSS counter value.'
            func_tags = ['function', 'lists', 'counters']
        elif fn == 'marker_string':
            func_summary = 'Formats the marker string using the specified list style type.'
            func_tags = ['function', 'lists', 'markers']
        elif fn in ('try_from', 'try_layout'):
            func_summary = 'Performs the top-level layout pass for the initial containing block.'
            func_tags = ['function', 'layout-root']
        elif fn == 'for_element':
            func_summary = 'Creates replaced contents for a given DOM element, detecting its type.'
            func_tags = ['function', 'replaced', 'element-detection']
        elif fn == 'make_fragments':
            func_summary = 'Creates display fragments for replaced content elements.'
            func_tags = ['function', 'fragment-creation', 'replaced']
        elif fn in ('set_theme', 'set_viewport_details'):
            func_summary = 'Configures layout thread state with theme/viewport information.'
            func_tags = ['function', 'configuration']
        elif fn == 'build_stacking_context_tree':
            func_summary = 'Builds the stacking context tree from the fragment tree.'
            func_tags = ['function', 'stacking-context']
        elif 'accessibility' in fn:
            func_summary = 'Updates the accessibility tree based on layout changes.'
            func_tags = ['function', 'accessibility']
        elif 'svg' in fn:
            func_summary = 'Builds SVG render tree nodes from the SVG DOM for rendering.'
            func_tags = ['function', 'svg']
        elif fn in ('quotes_for_lang', 'quotes_data_for_lang'):
            func_summary = 'Retrieves CSS quote pairs for the specified language.'
            func_tags = ['function', 'quotes', 'localization']
        elif fn in ('display_inside', 'used_value_for_contents'):
            func_summary = 'Determines the display type and box generation rules for this element.'
            func_tags = ['function', 'display', 'box-generation']
        elif fn == 'establishes_stacking_context':
            func_summary = 'Determines whether this element establishes a new stacking context.'
            func_tags = ['function', 'stacking-context']
        elif fn == 'establishes_block_formatting_context':
            func_summary = 'Determines whether this element establishes a new block formatting context.'
            func_tags = ['function', 'bfc', 'formatting-context']
        elif fn == 'effective_overflow':
            func_summary = 'Computes the effective overflow behavior for this element.'
            func_tags = ['function', 'overflow']
        elif fn == 'compute_scrollable_overflow' or fn == 'calculate_scrollable_overflow':
            func_summary = 'Calculates the full scrollable overflow area, aggregating child contributions and applying transforms.'
            func_tags = ['function', 'scrollable-overflow', 'computation']
        elif fn == 'layout_many':
            func_summary = 'Lays out all collected absolutely positioned boxes within the positioning context.'
            func_tags = ['function', 'positioned', 'absolute']
        elif fn == 'layout_as_absolute':
            func_summary = 'Performs full absolute positioning layout including inset resolution and margin solving.'
            func_tags = ['function', 'absolute', 'positioning', 'layout']
        elif fn in ('compute_track_constrainedness_and_has_originating_cells',
                     'compute_column_measures', 'compute_grid_min_max'):
            subject = fn.replace('compute_', '').replace('_', ' ')
            func_summary = 'Computes ' + subject + ' for the table layout algorithm.'
            func_tags = ['function', 'table', 'computation']
        elif fn in ('distribute_width_to_columns', 'distribute_extra_width_to_columns'):
            func_summary = 'Distributes available width among table columns according to CSS table width distribution rules.'
            func_tags = ['function', 'table', 'column-widths']
        elif fn in ('layout_grid', 'do_first_row_layout', 'layout_cells_in_row'):
            func_summary = 'Performs a phase of the table layout algorithm.'
            func_tags = ['function', 'table', 'layout']
        elif fn == 'layout_initial_containing_block_children':
            func_summary = 'Lays out absolutely positioned children of the initial containing block.'
            func_tags = ['function', 'absolute', 'initial-containing-block']
        elif fn == 'compute_inline_content_sizes':
            func_summary = 'Computes the inline content sizing (min/max) for this element.'
            func_tags = ['function', 'content-sizing']
        elif fn == 'compute_damage_and_rebuild_box_tree' or fn.startswith('compute_damage_and_rebuild_box_tree_'):
            func_summary = 'Computes element damage and rebuilds the box tree for incremental layout.'
            func_tags = ['function', 'damage', 'box-tree', 'rebuild']
        elif fn == 'process_preorder':
            func_summary = 'Processes elements in preorder during tree traversal for style damage computation.'
            func_tags = ['function', 'traversal', 'damage']
        elif fn == 'apply_damage':
            func_summary = 'Applies the computed damage set to elements, triggering appropriate style recalc and reflow.'
            func_tags = ['function', 'damage', 'style-recalc']
        elif fn == 'compute_child_layout':
            func_summary = 'Computes the layout for a single flex/grid child using the Taffy library.'
            func_tags = ['function', 'taffy', 'child-layout']
        elif fn == 'set_scroll_offset_from_script' or fn == 'set_scroll_offsets_from_renderer':
            func_summary = 'Updates scroll offsets from script or renderer input.'
            func_tags = ['function', 'scrolling']
        elif fn == 'ensure_containing_block_calculation':
            func_summary = 'Ensures containing block calculation is performed for a fragment.'
            func_tags = ['function', 'containing-block']
        elif fn == 'ensure_stacking_context_tree':
            func_summary = 'Ensures the stacking context tree is built and up to date.'
            func_tags = ['function', 'stacking-context']
        elif fn == 'clear_layout_trees_and_send_empty_display_list':
            func_summary = 'Clears layout trees and sends an empty display list for hidden or unloaded pages.'
            func_tags = ['function', 'cleanup', 'display-list']
        elif fn == 'layout_maybe_position_relative_fragment':
            func_summary = 'Performs layout for a possibly position:relative fragment.'
            func_tags = ['function', 'positioned', 'relative']
        elif fn == 'solve_margins':
            func_summary = 'Solves margin auto values for absolute positioning along a single axis.'
            func_tags = ['function', 'margins', 'absolute']
        elif fn == 'origin_for_margin_box':
            func_summary = 'Computes the origin position for the margin box of an absolutely positioned element.'
            func_tags = ['function', 'origin', 'absolute']
        elif fn == 'relative_adjustement':
            func_summary = 'Applies relative positioning adjustment to a fragment.'
            func_tags = ['function', 'relative', 'positioning']
        elif fn == 'compute_border_collapse':
            func_summary = 'Computes collapsed border styles and widths for adjacent table cells.'
            func_tags = ['function', 'table', 'border-collapse']
        elif fn == 'distribute_colspanned_cell_to_columns':
            func_summary = 'Distributes a colspan cell intrinsic sizes across the columns it spans.'
            func_tags = ['function', 'table', 'colspan']
        elif fn == 'compute_table_width':
            func_summary = 'Computes the final table width based on column measures and available space.'
            func_tags = ['function', 'table', 'width']
        elif fn == 'compute_table_height_and_final_row_heights':
            func_summary = 'Computes table height and assigns final row heights.'
            func_tags = ['function', 'table', 'height']
        elif fn == 'layout_caption':
            func_summary = 'Lays out the table caption element.'
            func_tags = ['function', 'table', 'caption']
        elif fn == 'build_svg_render_tree' or fn == 'build_svg_render_node':
            func_summary = 'Builds SVG render trees and nodes from the SVG DOM for rendering.'
            func_tags = ['function', 'svg', 'render-tree']
        elif fn == 'content_size':
            func_summary = 'Computes the content size for replaced elements based on available space.'
            func_tags = ['function', 'replaced', 'content-size']
        elif fn == 'calculate_fragment_rect':
            func_summary = 'Calculates the fragment rectangle for a replaced element based on content size and alignment.'
            func_tags = ['function', 'replaced', 'fragment-rect']
        elif fn == 'preferred_aspect_ratio':
            func_summary = 'Computes the preferred aspect ratio for this element based on CSS properties and natural dimensions.'
            func_tags = ['function', 'aspect-ratio', 'sizing']
        elif fn == 'shrink_to_fit':
            func_summary = 'Computes the shrink-to-fit inline size from available space.'
            func_tags = ['function', 'sizing', 'shrink-to-fit']
        elif fn == 'adjoin' or fn == 'adjoin_assign':
            func_summary = 'Adjoins another collapsed margin for margin collapsing resolution.'
            func_tags = ['function', 'margins', 'collapsing']
        elif fn == 'solve':
            func_summary = 'Resolves the collapsed margin to its final value.'
            func_tags = ['function', 'margins', 'collapsing']
        elif fn == 'ensure':
            func_summary = 'Ensures the containing block calculation state is resolved.'
            func_tags = ['function', 'containing-block', 'lazy']
        else:
            func_summary = fn + ' operation on this type.'
            func_tags = ['function']

        nodes.append({
            'id': func_id,
            'type': 'function',
            'name': f['name'],
            'filePath': fp,
            'lineRange': [f['startLine'], f['endLine']],
            'summary': func_summary,
            'tags': func_tags,
            'complexity': complexity_lines(lc)
        })

        edges.append({
            'source': file_id,
            'target': func_id,
            'type': 'contains',
            'direction': 'forward',
            'weight': 1.0
        })

        if is_ex:
            edges.append({
                'source': file_id,
                'target': func_id,
                'type': 'exports',
                'direction': 'forward',
                'weight': 0.8
            })

    # Classes
    for c in classes:
        if not is_sig_class(c, exported_names):
            continue
        lc = c['endLine'] - c['startLine'] + 1
        is_ex = c['name'] in exported_names
        class_id = 'class:' + fp + ':' + c['name']

        cn = c['name']

        # Class summaries
        class_meta_map = {
            'BaseFragment': ('Core fragment data structure providing shared fields (tag, flags, style, rect, status) and methods common to all fragment box types in the layout tree.', ['fragment', 'core', 'data-model']),
            'BoxFragment': ('Primary fragment type representing a CSS box, containing children, padding/border/margin, baselines, scrollable overflow, containing block tracking, and spatial tree node assignment.', ['fragment', 'box-model', 'children']),
            'Fragment': ('Top-level fragment enum dispatching to specific fragment types (LayoutRoot, Box, Float, Positioning, Text, Image, IFrame) for all fragment operations.', ['enum', 'fragment', 'dispatch']),
            'FragmentTree': ('Root container for a document fragment tree, manages scrollable overflow and provides methods to find fragments and access the body or root box fragments.', ['fragment-tree', 'root', 'container']),
            'PositioningFragment': ('Fragment wrapper for elements needing special positioning (floats), providing offset calculation, scrollable overflow, and tree printing support.', ['fragment', 'positioning', 'float']),
            'LayoutThread': ('Core layout engine thread that manages reflow, styling, font loading, display list construction, stacking context tree management, and DOM query resolution.', ['layout-engine', 'thread', 'reflow', 'queries']),
            'LayoutBoxBase': ('Foundation structure for all layout boxes, managing fragment storage, inline content sizing, layout result caching, and cache invalidation across formatting contexts.', ['layout-box', 'base', 'fragments', 'caching']),
            'ContainingBlockManager': ('Manages the containing block chain for fragments, providing strategies for non-absolute, absolute, and fixed descendants.', ['containing-block', 'positioning', 'chain']),
            'TextFragment': ('Fragment type for text content, storing glyphs, font metrics, selected style, justification data, and providing character offset computation for hit testing.', ['fragment', 'text', 'glyphs', 'hit-test']),
            'ImageFragment': ('Fragment type for replaced image content with clip region, image key, broken image handling, and SVG render tree support.', ['fragment', 'image', 'replaced']),
            'IFrameFragment': ('Fragment type for embedded iframe content, storing the sub-pipeline identifier.', ['fragment', 'iframe', 'embedded']),
            'LayoutRoot': ('Top-level layout root representing the initial containing block, performing viewport-sized layout passes.', ['layout-root', 'viewport', 'initial-containing-block']),
            'ConstraintSpace': ('Represents the available space constraints for layout, including inline and block sizes.', ['constraint', 'space', 'layout-input']),
            'ContainingBlock': ('Represents the containing block size and writing mode for layout computations.', ['containing-block', 'size', 'writing-mode']),
            'ReplacedContents': ('Handles replaced element content (images, SVG, iframes, video, canvas), computing natural sizes and creating appropriate display fragments.', ['replaced', 'content', 'images', 'svg']),
            'ContentSizes': ('Tracks min-content and max-content inline sizes with union/max/shrink-to-fit operations for CSS intrinsic sizing.', ['content-sizing', 'intrinsic', 'min-max']),
            'ComputedValuesExt': ('Extension trait on Servo ComputedValues providing layout-specific methods for accessing box model properties, writing mode, overflow, transforms, and stacking context establishment.', ['style', 'extension-trait', 'computed-values']),
            'TableBuilder': ('Builds the table structure from DOM elements, creating anonymous table objects as needed, managing row groups, cells, columns, and spanning.', ['table', 'builder', 'anonymous']),
            'TableLayout': ('Implements the full CSS table layout algorithm including column width distribution, row height calculation, collapsed borders, and cell positioning.', ['table', 'layout-algorithm', 'columns', 'rows']),
            'TaffyContainer': ('Wraps a flex/grid container for layout via the Taffy library, managing the taffy tree node and style representation.', ['taffy', 'flexbox', 'grid', 'wrapper']),
            'TaffyItemBox': ('Wraps a flex/grid item for layout via the Taffy library, providing layout entry points and style resolution.', ['taffy', 'flexbox', 'grid', 'item']),
            'TaffyStyloStyle': ('Wraps Servo computed values to provide Taffy-compatible style properties for flex/grid layout.', ['taffy', 'style', 'wrapper', 'bridge']),
            'AbsoluteAxisSolver': ('Solves absolute positioning along a single axis, computing resolved insets, margin auto-distribution, and final fragment position.', ['absolute', 'positioning', 'axis-solver', 'insets']),
            'PositioningContext': ('Manages a positioning context for absolutely/fixed positioned elements, including box collection, hoisting, and layout coordination.', ['positioning', 'context', 'hoisting']),
            'LayoutRootLayoutInputs': ('Input parameters for the initial containing block layout pass, including viewport size and optional parent layout data.', ['layout-inputs', 'root', 'viewport']),
            'ElementDamageSet': ('Tracks damage (style change, reflow required, repaint needed) for elements during incremental layout updates.', ['damage', 'incremental', 'style-change']),
            'LayoutStyle': ('Represents the computed style for a layout box, providing access to layout-relevant style properties.', ['style', 'layout', 'computed-values']),
            'QuotePair': ('Represents a pair of open/close quote characters for a specific language.', ['quotes', 'typography', 'pair']),
            'HoistedSharedFragment': ('Lightweight wrapper for fragments hoisted out of normal tree flow (e.g., absolutely positioned elements), with optional style override.', ['hoisted', 'fragment', 'wrapper']),
            'FragmentStatus': ('Enum tracking the layout status of a fragment (New, StyleChanged, OnlyDescendantsChanged, Clean) for incremental update optimization.', ['status', 'incremental', 'dirty-tracking']),
            'BaseFragmentInfo': ('Provides construction information for creating a BaseFragment, wrapping tag and flags data.', ['construction', 'fragment-info']),
            'Tag': ('Fragment tag identifying the originating DOM node and pseudo-element chain for mapping fragments back to their style sources.', ['tag', 'node-mapping', 'pseudo-element']),
            'LayoutFactoryImpl': ('Factory implementation creating LayoutThread instances for the layout engine.', ['factory', 'layout-thread']),
            'AbsolutelyPositionedBox': ('Represents a single absolutely positioned box within a positioning context, tracking its fragment and hoisting status.', ['absolute', 'positioned-box', 'hoisted']),
            'HoistedAbsolutelyPositionedBox': ('An absolutely positioned box that has been hoisted to a higher positioning context for layout.', ['absolute', 'hoisted', 'positioned-box']),
            'ContainingBlockCalculation': ('State machine for lazy containing block calculation with stacking context tree awareness.', ['containing-block', 'lazy-calculation', 'state-machine']),
            'CollapsedBlockMargins': ('Tracks the collapsed through, start, and end margin states for CSS margin collapsing.', ['margin', 'collapsing', 'block']),
            'CollapsedMargin': ('Resolves CSS margin collapsing by tracking max positive and min negative values through adjacent margins.', ['margin', 'collapsing', 'resolution']),
            'LayoutRootFragment': ('Wrapper fragment for accessing the inner layout root fragment and its box fragment.', ['layout-root', 'fragment', 'wrapper']),
            'PropagatedBoxTreeData': ('Tracks box tree propagation data during layout for handling special formatting context interactions.', ['box-tree', 'propagation', 'layout-data']),
            'NaturalSizes': ('Represents the natural (intrinsic) dimensions of a replaced element such as an image or video.', ['intrinsic', 'natural-sizes', 'replaced']),
            'IntrinsicSizingMode': ('Enum representing intrinsic sizing mode (match-legacy or modern) for CSS sizing.', ['sizing', 'intrinsic-mode']),
            'InlineContentSizesResult': ('Result type for inline content size computation, containing min and max content sizes.', ['content-sizing', 'result-type']),
            'ComputeInlineContentSizes': ('Trait for computing inline content sizes of DOM elements.', ['trait', 'content-sizing']),
            'TableSlotCell': ('Represents a cell within the CSS table grid, tracking its row/column placement and spanning.', ['table', 'cell', 'grid-placement']),
            'Table': ('Represents a CSS table-level box in the box tree, with repair, sizing, and testing support.', ['table', 'box-tree']),
            'TableSlot': ('Represents a slot (cell position) in the CSS table grid structure.', ['table', 'slot', 'grid']),
            'TableTrack': ('Represents a track (row or column) in the CSS table grid structure.', ['table', 'track']),
            'TableTrackGroup': ('Represents a group of table tracks (rows or columns).', ['table', 'track-group']),
            'TableTrackGroupType': ('Enum for table track group type (row group, column group).', ['table', 'track-group-type']),
            'TableCaption': ('Represents a table caption box in the table layout model.', ['table', 'caption']),
            'CollapsedBorder': ('Represents a collapsed border between table cells with resolved style and width.', ['table', 'collapsed-border', 'border']),
            'SpecificTableGridInfo': ('Stores the computed grid structure for a table, including column measures and row data.', ['table', 'grid-info']),
            'TableLayoutStyle': ('Enum representing the table layout style (auto or fixed).', ['table', 'layout-style']),
            'TableLevelBox': ('Base type for table-level boxes in the box tree, providing tree attachment and style management.', ['table', 'table-level-box', 'base']),
            'WeakTableLevelBox': ('Weak reference to a TableLevelBox for non-owning access.', ['table', 'weak-reference']),
            'SpecificTaffyGridInfo': ('Stores detailed grid layout information from the Taffy library.', ['taffy', 'grid-info']),
            'RecalcStyle': ('Traversal type for the style recalculation pass over the layout tree.', ['traversal', 'style-recalc']),
            'PaddingBorderMargin': ('Represents padding, border, and margin values for a box in logical coordinates.', ['padding', 'border', 'margin', 'logical']),
            'AspectRatio': ('Represents a preferred aspect ratio for sizing, combining CSS aspect-ratio property with intrinsic ratio.', ['aspect-ratio', 'sizing']),
            'ContentBoxSizesAndPBM': ('Aggregates content box sizes and padding/border/margin for a layout box.', ['sizing', 'box-model', 'content-box']),
            'BorderStyleColor': ('Represents a border side with its style and color for table border rendering.', ['border', 'style', 'color']),
            'OverflowDirection': ('Represents the overflow direction (horizontal/vertical/both/none) for scrollable containers.', ['overflow', 'direction']),
            'Clamp': ('Utility for clamping values within min/max constraints during size resolution.', ['utility', 'clamping']),
            'TransformExt': ('Extension trait for transform matrix calculations on layout boxes.', ['transform', 'extension-trait']),
            'BoxFragmentRareData': ('Optional extended data for a BoxFragment, including sticky insets and generated clip/scroll node IDs.', ['box-fragment', 'rare-data']),
            'BoxFragmentWithStyle': ('Combines a BoxFragment with its style for layout operations that need both.', ['box-fragment', 'style', 'wrapper']),
            'BackgroundMode': ('Enum controlling background rendering mode (Normal, Extra, None) for fragment painting.', ['background', 'rendering-mode']),
            'ExtraBackground': ('Additional background style and rect for fragments needing layered backgrounds.', ['background', 'extra']),
            'SpecificLayoutInfo': ('Enum for layout-specific information (Grid, TableCell, TableGrid, TableWrapper) for specialized layout handling.', ['layout', 'specific-info']),
            'BlockLevelLayoutInfo': ('Block-level layout metadata including clearance and collapsed margin state.', ['block-level', 'clearance', 'collapsed-margins']),
            'Display': ('Display type classification for box generation rules.', ['display', 'box-generation']),
            'DisplayGeneratingBox': ('Display value classification for whether a box generates a principal box, no box, or contents.', ['display', 'box-generation']),
            'DisplayOutside': ('Display outside value (block, inline, run-in) for CSS display property.', ['display', 'outside']),
            'DisplayInside': ('Display inside value (flow, flex, grid, table, ruby) for CSS display property.', ['display', 'inside']),
            'DisplayLayoutInternal': ('Display layout-internal values for table parts and ruby.', ['display', 'layout-internal', 'table']),
            'IndefiniteContainingBlock': ('Represents an indefinite containing block measurement along one axis.', ['containing-block', 'indefinite']),
            'ContainingBlockSize': ('Represents the containing block size for layout with computed values.', ['containing-block', 'size']),
            'SizeConstraint': ('Represents CSS sizing constraints (preferred/min/max) with resolution logic.', ['sizing', 'constraints', 'resolution']),
            'Sizes': ('Container for preferred, min, and max sizing constraints with resolution methods.', ['sizing', 'constraints']),
            'LazySize': ('Lazily-computed size value for deferred resolution.', ['sizing', 'lazy']),
            'LogicalVec2': ('A 2D vector in logical (inline/block) coordinate space for writing-mode-aware layout.', ['geometry', 'logical', 'vector']),
            'LogicalRect': ('A rectangle in logical (inline/block) coordinate space.', ['geometry', 'logical', 'rect']),
            'LogicalSides': ('Logical-side inset values (inline-start, inline-end, block-start, block-end).', ['geometry', 'logical', 'sides']),
            'LogicalSides1D': ('Logical-side inset values along one axis (start/end).', ['geometry', 'logical', 'sides']),
            'ToLogical': ('Trait for converting physical geometry values to logical coordinates.', ['geometry', 'conversion', 'trait']),
            'ToLogicalWithContainingBlock': ('Trait for converting physical geometry to logical coordinates using containing block data.', ['geometry', 'conversion', 'trait']),
            'SyncPhysicalRectAu': ('Thread-safe wrapper around a physical rect using atomic operations for concurrent access.', ['geometry', 'atomic', 'sync']),
            'CellLayout': ('Per-cell layout tracking during table layout, including column mapping and measure information.', ['table', 'cell', 'layout']),
            'CellOrTrackMeasure': ('Measure data for a table cell or track during layout computation.', ['table', 'measure']),
            'RowGroupFragmentLayout': ('Row group fragment layout data including position and border information.', ['table', 'row-group', 'layout']),
            'TableAndTrackDimensions': ('Aggregated table and track dimension data after layout computation.', ['table', 'dimensions']),
            'ColspanToDistribute': ('Tracks colspan cell distribution across table columns for intrinsic sizing.', ['table', 'colspan', 'distribution']),
            'LayoutResultAndInputs': ('Aggregates layout results with their input parameters for cache management.', ['layout', 'caching', 'results']),
            'IndependentFormattingContextLayoutResult': ('Layout result for an independent formatting context (e.g., table, flex).', ['layout', 'ifc', 'results']),
            'IndependentFormattingContextLayoutResultAndInputs': ('Layout result and inputs for independent formatting contexts.', ['layout', 'ifc', 'caching']),
            'SameFormattingContextBlockLayoutResult': ('Layout result for block-level layout in the same formatting context.', ['layout', 'block', 'results']),
            'SameFormattingContextBlockLayoutResultAndInputs': ('Layout result and inputs for same-formatting-context block layout.', ['layout', 'block', 'caching']),
            'StyloLineNameIter': ('Iterator over CSS grid line names from Stylo computed values.', ['grid', 'line-names', 'stylo']),
            'RepetitionWrapper': ('Wraps grid track repetition data for Taffy compatibility.', ['grid', 'repetition', 'taffy']),
            'AnonymousTableContent': ('Tracks anonymous table content during table structure construction.', ['table', 'anonymous', 'construction']),
            'ResolvedSlotAndLocation': ('Resolved grid slot position and location during table construction.', ['table', 'slot', 'construction']),
            'TableBuilderTraversal': ('DOM traversal state for building table structure from child elements.', ['table', 'dom-traversal', 'construction']),
            'TableColumnGroupBuilder': ('Builds column group and column definitions during table construction.', ['table', 'column-group', 'construction']),
            'Size': ('A size classification enum (preferred/min/max) for constraint resolution.', ['sizing', 'size-type', 'enum']),
            'Blank': ('', []),
            'UserAgentStylesheets': ('Container for user agent stylesheet references used by the layout thread.', ['styling', 'user-agent', 'stylesheets']),
            'RegisteredPainterImpl': ('Implements registered CSS paint worklets for custom painting.', ['paint', 'worklet', 'css-paint']),
            'LayoutFontMetricsProvider': ('Provides font metrics for layout calculations, wrapping Servo font data.', ['fonts', 'metrics', 'provider']),
            'PositioningContextLength': ('Wrapper around positioning context length for indexed access.', ['positioning', 'length']),
            'IFrameInfo': ('Information about an iframe replaced element, tracking its pipeline and size.', ['iframe', 'replaced', 'info']),
            'ImageInfo': ('Information about an image replaced element, including URL and load state.', ['image', 'replaced', 'info']),
            'VideoInfo': ('Information about a video replaced element.', ['video', 'replaced', 'info']),
            'CanvasInfo': ('Information about a canvas replaced element.', ['canvas', 'replaced', 'info']),
            'ReplacedContentKind': ('Enum categorizing the kind of replaced content (Image, IFrame, SVG, Video, Canvas).', ['replaced', 'content-kind', 'enum']),
            'TaffyItemBoxInner': ('Inner type for a TaffyItemBox, wrapping the actual fragment or box.', ['taffy', 'item', 'inner']),
            'SpecificTaffyGridTrackInfo': ('Track-level grid information from the Taffy library.', ['taffy', 'grid', 'track-info'])
        }

        if cn in class_meta_map:
            cls_summary, cls_tags = class_meta_map[cn]
        else:
            cls_summary = cn + ' type used in the Servo layout engine.'
            cls_tags = ['class']

        nodes.append({
            'id': class_id,
            'type': 'class',
            'name': c['name'],
            'filePath': fp,
            'lineRange': [c['startLine'], c['endLine']],
            'summary': cls_summary,
            'tags': cls_tags,
            'complexity': complexity_lines(lc)
        })

        edges.append({
            'source': file_id,
            'target': class_id,
            'type': 'contains',
            'direction': 'forward',
            'weight': 1.0
        })

        if is_ex:
            edges.append({
                'source': file_id,
                'target': class_id,
                'type': 'exports',
                'direction': 'forward',
                'weight': 0.8
            })

print("Total nodes:", len(nodes))
print("Total edges:", len(edges))

# Partition
N = len(file_order)
node_count = len(nodes)
edge_count = len(edges)
parts = math.ceil(max(node_count / 60, edge_count / 120))
print("Parts needed:", parts)

files_per_part = math.ceil(N / parts)

# Map file to node IDs
file_node_ids = {}
for n in nodes:
    fp = n['filePath']
    if fp not in file_node_ids:
        file_node_ids[fp] = []
    file_node_ids[fp].append(n['id'])

all_node_ids = set(n['id'] for n in nodes)

for part_idx in range(parts):
    start_file = part_idx * files_per_part
    end_file = min(start_file + files_per_part, N)
    part_files = file_order[start_file:end_file]

    part_node_ids = set()
    for fp in part_files:
        for nid in file_node_ids.get(fp, []):
            part_node_ids.add(nid)

    part_edges = [e for e in edges if e['source'] in part_node_ids]
    part_nodes = [n for n in nodes if n['id'] in part_node_ids]

    part_num = part_idx + 1
    if parts == 1:
        out_file = 'D:/Projects/servo/.understand-anything/intermediate/batch-7.json'
    else:
        out_file = 'D:/Projects/servo/.understand-anything/intermediate/batch-7-part-' + str(part_num) + '.json'

    out_data = {'nodes': part_nodes, 'edges': part_edges}
    with open(out_file, 'w') as f:
        json.dump(out_data, f, indent=2)

    print("Wrote", out_file, len(part_nodes), "nodes,", len(part_edges), "edges")

    # Validate
    errors = []
    for e in part_edges:
        if e['source'] not in all_node_ids:
            errors.append("source not in any node: " + e['source'])
        if e['target'] not in all_node_ids:
            errors.append("target not in any node: " + e['target'])
    if errors:
        print("  VALIDATION ERRORS:", errors[:10])
    else:
        print("  Validation OK")

print("Done.")
