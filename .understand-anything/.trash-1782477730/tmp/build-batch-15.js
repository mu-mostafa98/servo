const fs = require('fs');
const data = JSON.parse(fs.readFileSync('d:/Projects/servo/.understand-anything/tmp/ua-file-extract-results-15.json', 'utf-8'));

function makeNode(id, type, name, filePath, summary, tags, complexity, extra) {
  const node = { id, type, name, filePath, summary, tags, complexity };
  if (extra) Object.assign(node, extra);
  return node;
}

function makeEdge(source, target, type, weight) {
  return { source, target, type, direction: 'forward', weight };
}

const nodes = [];
const edges = [];

function fileNodeId(path) { return 'file:' + path; }
function classNodeId(path, clsName) { return 'class:' + path + ':' + clsName; }
function funcNodeId(path, fnName) { return 'function:' + path + ':' + fnName; }

const significantClasses = {
  'HTMLAnchorElement': { summary: 'DOM class for <a> hyperlink elements with URL component accessors, rel-list management, referrer policy, and click activation behavior for navigation.', tags: ['dom', 'anchor', 'hyperlink'] },
  'Area': { summary: 'Coordinate-based shape representation for HTML area elements, supporting circle, rectangle, and polygon shape parsing and hit-testing.', tags: ['shape', 'coordinate', 'geometry'] },
  'HTMLAreaElement': { summary: 'DOM class for <area> image map elements with hyperlink URL handling and coordinate-based shape definitions.', tags: ['dom', 'area', 'image-map'] },
  'HTMLBaseElement': { summary: 'DOM class for <base> elements that sets the document base URL for relative link resolution, with CSP policy validation.', tags: ['dom', 'base', 'url-resolution'] },
  'HTMLBodyElement': { summary: 'DOM class for <body> elements handling legacy presentational attributes and tree binding lifecycle.', tags: ['dom', 'body'] },
  'HTMLButtonElement': { summary: 'DOM class for <button> elements with form submission, command dispatch, and type management.', tags: ['dom', 'button', 'form-control'] },
  'HTMLCanvasElement': { summary: 'DOM class for <canvas> elements providing 2D, WebGL, WebGPU, and bitmap renderer context creation, image data access, and offscreen canvas support.', tags: ['dom', 'canvas', 'rendering', 'graphics'] },
  'HTMLCollection': { summary: 'Live, ordered collection of DOM elements providing indexed and named access with automatic cache invalidation on document mutations.', tags: ['dom', 'collection', 'live-list'] },
  'HTMLDetailsElement': { summary: 'DOM class for <details> disclosure elements with shadow DOM content management, summary element discovery, and exclusive-accordion group coordination.', tags: ['dom', 'details', 'disclosure', 'shadow-dom'] },
  'HTMLDialogElement': { summary: 'DOM class for <dialog> elements with modal/open dialog management, focus stepping, and toggle event lifecycle.', tags: ['dom', 'dialog', 'modal', 'focus-management'] },
  'HTMLElement': { summary: 'Base DOM class for all HTML elements, providing shared functionality for event handling, focus management, editing, attribute parsing, and tree lifecycle callbacks.', tags: ['dom', 'base-class', 'event-handler'] },
  'HTMLFieldSetElement': { summary: 'DOM class for <fieldset> form grouping elements with validation state delegation and control disablement.', tags: ['dom', 'fieldset', 'validation'] },
  'HTMLFontElement': { summary: 'DOM class for legacy <font> elements handling deprecated face, color, and size attributes through presentational hints.', tags: ['dom', 'font', 'legacy', 'presentational-hints'] },
  'HTMLFormControlsCollection': { summary: 'DOM collection providing named and indexed access to form control elements within a HTMLFormElement.', tags: ['dom', 'collection', 'form-control'] },
  'HTMLFormElement': { summary: 'DOM class for <form> elements managing field enumeration, submission with multipart/URL-encoded/plaintext encoding, validation, and form reset.', tags: ['dom', 'form', 'submission', 'validation'] },
  'FormControl': { summary: 'Helper trait handling form control element lifecycle, providing add/remove control, name management, and disabled-state operations for form-associated elements.', tags: ['form-control', 'trait', 'lifecycle'] }
};

// Significant function summaries
const fnSummaries = {
  'htmlanchorelement.rs': {
    'new_inherited': 'Initializes shared anchor element state including rel-list tracking and URL storage.',
    'new': 'Constructs a new HTMLAnchorElement DOM node with proto reflection.',
    'full_href_url_for_user_interface': 'Returns the resolved absolute URL for display in the user interface, considering the href attribute and document base URL.',
    'attribute_mutated': 'Responds to anchor-specific attribute changes, updating link state and URL on href/rel mutations.',
    'RelList': 'Returns the DOMTokenList for the rel attribute, lazily initialized.',
    'activation_behavior': 'Handles click activation on anchors by resolving the href, checking target element types, and triggering hyperlink navigation.'
  },
  'htmlareaelement.rs': {
    'parse': 'Parses coordinate strings into circle, rectangle, or polygon shape definitions based on coordinate count.',
    'hit_test': 'Tests whether a given point falls within the area element\'s defined shape boundary.',
    'absolute_coords': 'Converts relative shape coordinates to absolute pixel values using the element\'s layout position.',
    'new_inherited': 'Initializes shared area element state including rel-list and URL storage.',
    'new': 'Constructs a new HTMLAreaElement DOM node with proto reflection.',
    'get_shape_from_coords': 'Retrieves and parses the shape and coords attributes into a parsed Area shape definition.',
    'attribute_mutated': 'Handles attribute changes affecting area element state and link relations.',
    'RelList': 'Returns the lazily-initialized DOMTokenList for the rel attribute.'
  },
  'htmlaudioelement.rs': {
    'new': 'Constructs a new HTMLAudioElement DOM node via proto reflection.',
    'Audio': 'Implements the Audio() constructor, creating an audio element with optional src and preload attributes.'
  },
  'htmlbaseelement.rs': {
    'new': 'Constructs a new HTMLBaseElement DOM node via proto reflection.',
    'set_frozen_base_url': 'Computes and freezes the base URL from the href attribute, validating against CSP and URL scheme requirements.',
    'Href': 'Returns the resolved href attribute value, combined with the document\'s fallback base URL when the base element\'s URL is set.',
    'attribute_mutated': 'Recomputes the frozen base URL on href attribute changes and notifies the document.'
  },
  'htmlbodyelement.rs': {
    'new': 'Constructs a new HTMLBodyElement DOM node via proto reflection.',
    'bind_to_tree': 'Called when the body element is inserted into the document tree, registering event handlers.',
    'parse_plain_attribute': 'Parses body-specific attributes like background and text color values.',
    'attribute_mutated': 'Handles attribute mutations on the body element, updating presentational hints.'
  },
  'htmlbrelement.rs': {
    'new': 'Constructs a new HTMLBRElement DOM node via proto reflection.'
  },
  'htmlbuttonelement.rs': {
    'new_inherited': 'Initializes shared button element state including type and disabled-attribute tracking.',
    'new': 'Constructs a new HTMLButtonElement DOM node via proto reflection.',
    'Command': 'Returns the command element associated with this button via the commandfor/command attributes.',
    'form_datum': 'Builds the form submission datum for this button, including name/value/type.',
    'set_type': 'Updates the button type attribute on attribute change.',
    'command_for_element': 'Resolves the element referenced by the commandfor attribute within the same root node.',
    'command_state': 'Determines the command state (disabled/active) for command-based invocation.',
    'determine_if_command_is_valid_for_target': 'Validates whether the button command can be dispatched to the target element.',
    'attribute_mutated': 'Handles button-specific attribute changes for type, disabled, and command-* attributes.',
    'unbind_from_tree': 'Cleans up button state when removed from the document tree.',
    'activation_behavior': 'Implements button click activation, handling form submission and command dispatching.'
  },
  'htmlcanvaselement.rs': {
    'new_inherited': 'Initializes shared canvas element state for dimensions and rendering context.',
    'new': 'Constructs a new HTMLCanvasElement DOM node via proto reflection.',
    'data': 'Returns raw canvas pixel data as a byte vector for image extraction.',
    'set_rendering_context': 'Associates a rendering context (2D, WebGL, WebGPU, bitmap) with the canvas element.',
    'get_or_init_2d_context': 'Lazily initializes and returns the CanvasRenderingContext2D for 2D drawing.',
    'get_or_init_bitmaprenderer_context': 'Lazily initializes and returns the ImageBitmapRenderingContext for bitmap display.',
    'get_or_init_webgl_context': 'Lazily initializes and returns a WebGLRenderingContext for 3D rendering.',
    'get_or_init_webgl2_context': 'Lazily initializes and returns a WebGL2RenderingContext for advanced 3D rendering.',
    'get_or_init_webgpu_context': 'Lazily initializes and returns a WebGPU rendering context.',
    'get_gl_attributes': 'Parses and returns WebGL context creation attributes from a JS object.',
    'get_image_data': 'Extracts image pixel data from the canvas as an ImageData object.',
    'update_rendering': 'Triggers a re-render of the canvas content, regenerating display lists.',
    'SetWidth': 'IDL setter for the width attribute, updating canvas dimensions.',
    'SetHeight': 'IDL setter for the height attribute, updating canvas dimensions.',
    'GetContext': 'IDL method returning a rendering context by type string (2d, webgl, webgpu, bitmaprenderer).',
    'ToDataURL': 'Converts canvas content to a data URL in the specified image format.',
    'ToBlob': 'Converts canvas content to a Blob object in the specified image format with callback.',
    'TransferControlToOffscreen': 'Transfers canvas rendering control to an OffscreenCanvas and returns it.',
    'CaptureStream': 'Creates a CanvasCaptureMediaStream for real-time canvas recording.',
    'attribute_mutated': 'Handles canvas attribute changes for width/height, triggering context re-initialization.'
  },
  'htmlcollection.rs': {
    'new_inherited_with_kind': 'Creates an HTMLCollection with a predefined collection kind (children, tag name, class name, etc.).',
    'always_empty': 'Creates a static always-empty HTMLCollection for consistent return values.',
    'new_with_filter_fn': 'Creates an HTMLCollection using a custom filter function for dynamic element matching.',
    'new_with_source': 'Creates an HTMLCollection from a pre-defined Vec of element sources.',
    'validate_cache': 'Checks if the cached element list is still valid based on the root node\'s mutation count.',
    'set_cached_cursor': 'Updates the cached cursor position for indexed element access.',
    'by_qualified_name': 'Filters HTMLCollection elements by qualified name matching tag local name and namespace.',
    'by_tag_name_ns': 'Creates an HTMLCollection filtered by tag name and namespace.',
    'by_qual_tag_name': 'Creates an HTMLCollection filtered by qualified tag name with namespace handling.',
    'by_class_name': 'Creates an HTMLCollection filtered by class name string.',
    'by_atomic_class_name': 'Creates an HTMLCollection filtered by an atomic (interned) class name.',
    'filter_iter_after': 'Returns an iterator yielding elements matching the collection filter starting after a given node.',
    'filter_iter_before': 'Returns an iterator yielding elements matching the collection filter preceding a given node.',
    'elements_iter': 'Returns the base iterator over all elements in the root, before filter application.',
    'Length': 'Returns the number of elements in the collection.',
    'Item': 'Returns the element at the specified index in the collection.',
    'NamedItem': 'Returns the first element whose id or name matches the given name string.',
    'SupportedPropertyNames': 'Returns an array of all named property names (id/name values) in the collection.'
  },
  'htmldataelement.rs': {
    'new': 'Constructs a new HTMLDataElement DOM node via proto reflection.'
  },
  'htmldatalistelement.rs': {
    'new': 'Constructs a new HTMLDataListElement DOM node via proto reflection.'
  },
  'htmldetailselement.rs': {
    'register_details_element': 'Registers a details element in the name-groups map for exclusive-accordion coordination.',
    'unregister_details_element': 'Removes a details element from the name-groups map on unbind or attribute change.',
    'group_members_for': 'Returns all registered details elements sharing the same group name.',
    'new_inherited': 'Initializes shared details element state including open state and name group.',
    'new': 'Constructs a new HTMLDetailsElement DOM node via proto reflection.',
    'create_shadow_tree': 'Creates the shadow DOM subtree for the details disclosure widget.',
    'update_shadow_tree_contents': 'Refreshes the shadow DOM content to reflect current open/closed state and slotting.',
    'update_shadow_tree_styles': 'Updates the shadow DOM styles to match the element\'s open/closed toggle state.',
    'ensure_details_exclusivity': 'Coordinates exclusive-accordion behavior, closing other same-name details when one opens.',
    'attribute_mutated': 'Handles open/name attribute changes, triggering shadow DOM updates and exclusivity resolution.',
    'bind_to_tree': 'Registers the details element in name groups when inserted into the document.',
    'unbind_from_tree': 'Unregisters the details element from name groups when removed from the document.'
  },
  'htmldialogelement.rs': {
    'new': 'Constructs a new HTMLDialogElement DOM node via proto reflection.',
    'show_a_modal': 'Implements dialog.showModal(), setting up modal dialog layer with focus trap and inert background.',
    'close_the_dialog': 'Implements dialog.close(), cleaning up modal state and returning to previous focus.',
    'queue_dialog_toggle_event_task': 'Queues a dialog toggle event as a DOM task for async event dispatch.',
    'run_dialog_focusing_steps': 'Performs the dialog focus sequence, finding the first focusable element or focusing the dialog itself.',
    'Show': 'IDL method for opening a non-modal dialog with optional return value.',
    'command_steps': 'Handles the dialog\'s response to Escape key events for modal dismissal.'
  },
  'htmldirectoryelement.rs': {
    'new': 'Constructs a new HTMLDirectoryElement DOM node via proto reflection.'
  },
  'htmldivelement.rs': {
    'new': 'Constructs a new HTMLDivElement DOM node via proto reflection.'
  },
  'htmldlistelement.rs': {
    'new': 'Constructs a new HTMLDListElement DOM node via proto reflection.'
  },
  'htmlelement.rs': {
    'Style': 'Returns the CSSStyleDeclaration for this element\'s inline style.',
    'Itemtypes': 'Returns an array of microdata item type URLs from the itemtype attribute.',
    'PropertyNames': 'Returns the microdata property names exposed by this element.',
    'Click': 'Simulates a mouse click on this element, dispatching the event synchronously.',
    'Focus': 'Focuses the element, performing focus steps with scroll-into-view and accessibility notifications.',
    'Blur': 'Removes focus from this element, performing blur cleanup.',
    'ScrollParent': 'Returns the nearest scrollable parent element for scroll-into-view operations.',
    'GetOffsetParent': 'Returns the offset parent element used for CSS offset calculations.',
    'OffsetTop': 'Returns the top offset position relative to the offset parent.',
    'OffsetLeft': 'Returns the left offset position relative to the offset parent.',
    'SetOuterText': 'Replaces the element with the given text content in the DOM tree.',
    'SetContentEditable': 'Updates the contentEditable state and marks the element as editable.',
    'AttachInternals': 'Attaches an ElementInternals object to a custom form-associated element for form participation.',
    'AccessKeyLabel': 'Returns the computed access key label string for keyboard shortcut display.',
    'append_text_node_to_fragment': 'Appends a text node with given text content to a document fragment.',
    'rendered_text_fragment': 'Builds the rendered text fragment for layout, including generated content from ::before/::after pseudo-elements.',
    'merge_with_the_next_text_node': 'Merges adjacent text nodes, collapsing whitespace per DOM spec normalization.',
    'update_assigned_access_key': 'Responds to accesskey attribute changes by updating the keyboard shortcut registration.',
    'attribute_mutated': 'Base class attribute mutation handler dispatching to specialized handlers for editing, access key, and contenteditable.',
    'bind_to_tree': 'Lifecycle callback when element is inserted into the document tree, initializing state.',
    'unbind_from_tree': 'Lifecycle callback when element is removed from the document tree, cleaning up state.',
    'attribute_affects_presentational_hints': 'Determines whether an attribute change affects the element\'s CSS presentational hints.',
    'parse_plain_attribute': 'Parses plain (non-reflected) attribute values, delegating to type-specific parsers.',
    'moving_steps': 'Moves element state between documents when the element is adopted across documents.',
    'get_inner_outer_text': 'Returns the element\'s text content for innerText/outerText accessor.',
    'set_inner_text': 'Sets the element\'s rendered text content via innerText assignment.',
    'is_labelable_element': 'Checks whether this element type can be associated with a label element.',
    'is_listed_element': 'Checks whether this element type appears in form element listings.',
    'is_body_element': 'Checks whether this element is the document body element.',
    'is_submittable_element': 'Checks whether this element type can participate in form submission.',
    'label_at': 'Returns the label element at the given index from this element\'s associated labels list.',
    'labels_count': 'Returns the number of label elements associated with this element.',
    'directionality': 'Computes the element\'s text directionality (ltr/rtl) based on dir attribute and content.',
    'summary_activation_behavior': 'Handles click activation on summary elements inside details disclosure widgets.',
    'is_a_summary_for_its_parent_details': 'Checks whether this element is a summary element directly inside a details element.'
  },
  'htmlembedelement.rs': { 'new': 'Constructs a new HTMLEmbedElement DOM node via proto reflection.' },
  'htmlfieldsetelement.rs': {
    'new_inherited': 'Initializes shared fieldset element state.',
    'new': 'Constructs a new HTMLFieldSetElement DOM node via proto reflection.',
    'update_validity': 'Recalculates the fieldset\'s validity state by checking all descendant form controls.',
    'attribute_mutated': 'Handles fieldset attribute changes, propagating disabled state to descendant form controls.'
  },
  'htmlfontelement.rs': {
    'new': 'Constructs a new HTMLFontElement DOM node via proto reflection.',
    'parse_single_face_value_from_string': 'Parses a single font face value, stripping quotes from a font family string.',
    'attribute_affects_presentational_hints': 'Checks if a font attribute (color, face, size) triggers presentational hints.',
    'parse_plain_attribute': 'Parses font-specific attributes into attribute value variants.',
    'parse_size': 'Parses the HTML font size attribute (+1 to +7, -1 to -7, or 1-7) into a numeric size.'
  },
  'htmlformcontrolscollection.rs': {
    'new_inherited': 'Initializes the form controls collection with the owning form reference.',
    'new': 'Constructs a new HTMLFormControlsCollection DOM node via proto reflection.',
    'NamedItem': 'Finds a form control by name or id within the collection, handling radio button groups specially.'
  },
  'htmlformelement.rs': {
    'new_inherited': 'Initializes shared form element state and constructs the form controls collection.',
    'new': 'Constructs a new HTMLFormElement DOM node via proto reflection.',
    'filter_for_radio_list': 'Filters form controls to produce a radio button list grouped by name.',
    'nth_for_radio_list': 'Returns the n-th radio button in a named radio group.',
    'RequestSubmit': 'Programmatic form submission request with optional submit button, running interactive validation.',
    'Elements': 'Returns the HTMLFormControlsCollection for this form\'s elements.',
    'NamedGetter': 'Returns a form control or RadioNodeList by name for named property access.',
    'RelList': 'Returns the DOMTokenList for the form\'s rel attribute.',
    'SupportedPropertyNames': 'Enumerates all named controls in the form for IDL supported property names.',
    'pick_encoding': 'Selects the multipart/form-data encoding type based on form attributes.',
    'update_validity': 'Updates the form\'s validity state after validation checks.',
    'submit': 'Main form submission logic handling encoding, navigation planning, and form data collection.',
    'mutate_action_url': 'Resolves and encodes the form action URL for submission.',
    'submit_entity_body': 'Creates the request entity body from form data using the selected encoding type.',
    'set_url_query_pairs': 'Encodes form data as URL query string parameters for GET submissions.',
    'plan_to_navigate': 'Plans the navigation target for form submission, handling action URL and target browsing context.',
    'interactive_validation': 'Runs interactive validation on form controls and shows constraint validation errors.',
    'static_validation': 'Runs static validation checks without showing validation UI.',
    'get_unclean_dataset': 'Collects dirty/unchanged form controls for determining which data changed.',
    'get_form_dataset': 'Builds the full form data set from all non-disabled controls.',
    'reset': 'Resets the form to its initial values, firing reset and change events.',
    'add_control': 'Registers a form control as belonging to this form for listing and submission.',
    'remove_control': 'Removes a form control from this form\'s control list.',
    'is_resettable': 'Checks whether a form control type is eligible for form reset.',
    'action': 'Returns the resolved form action URL.',
    'enctype': 'Returns the form\'s encoding type (URL-encoded, multipart, or plaintext).',
    'method': 'Returns the form submission method (GET or POST).',
    'target': 'Returns the browsing context target for form submission results.',
    'no_validate': 'Returns whether form submission should skip validation.',
    'unbind_from_tree': 'Cleans up form state when removed from the document tree.',
    'attribute_mutated': 'Handles form attribute changes affecting action, method, enctype, and target.',
    'as_maybe_form_control': 'Casts an element to a form control if it is form-associated.',
    'encode_plaintext': 'Encodes form data as text/plain for form submission.',
    'encode_multipart_form_data': 'Encodes form data as multipart/form-data with boundary generation and file handling.'
  },
  'htmlframeelement.rs': { 'new': 'Constructs a new HTMLFrameElement DOM node via proto reflection.' },
  'htmlframesetelement.rs': { 'new': 'Constructs a new HTMLFrameSetElement DOM node via proto reflection.' },
  'htmlheadelement.rs': { 'new': 'Constructs a new HTMLHeadElement DOM node via proto reflection.' }
};

// File-level summaries and tags
const fileMeta = {
  'htmlanchorelement.rs': {
    summary: 'DOM implementation of the HTMLAnchorElement interface, providing hyperlink handling with URL component accessors, rel-list management, referrer policy, and click activation behavior.',
    tags: ['dom', 'html-element', 'anchor', 'hyperlink', 'navigation']
  },
  'htmlareaelement.rs': {
    summary: 'DOM implementation of the HTMLAreaElement interface with coordinate parsing (circle, rectangle, polygon shapes), hit-testing, and hyperlink activation for image map areas.',
    tags: ['dom', 'html-element', 'area', 'image-map', 'hit-test']
  },
  'htmlaudioelement.rs': {
    summary: 'DOM implementation of the HTMLAudioElement interface providing the Audio() constructor for creating audio elements programmatically.',
    tags: ['dom', 'html-element', 'audio', 'media', 'constructor']
  },
  'htmlbaseelement.rs': {
    summary: 'DOM implementation of the HTMLBaseElement interface, managing the document\'s frozen base URL for relative URL resolution with CSP validation.',
    tags: ['dom', 'html-element', 'base', 'url-resolution', 'csp']
  },
  'htmlbodyelement.rs': {
    summary: 'DOM implementation of the HTMLBodyElement interface, handling presentational attributes for background color and text color on the body element.',
    tags: ['dom', 'html-element', 'body', 'presentational-hints']
  },
  'htmlbrelement.rs': {
    summary: 'DOM implementation of the HTMLBRElement interface for line break element construction.',
    tags: ['dom', 'html-element', 'line-break']
  },
  'htmlbuttonelement.rs': {
    summary: 'DOM implementation of the HTMLButtonElement interface with command handling, form datum submission, type management, and activation behavior.',
    tags: ['dom', 'html-element', 'button', 'form-control', 'command']
  },
  'htmlcanvaselement.rs': {
    summary: 'DOM implementation of the HTMLCanvasElement interface, providing 2D, WebGL, WebGL2, WebGPU, and bitmap renderer context creation, image data extraction, and canvas rendering.',
    tags: ['dom', 'html-element', 'canvas', 'rendering', 'graphics']
  },
  'htmlcollection.rs': {
    summary: 'DOM implementation of HTMLCollection providing indexed and named access to element collections with filtering, caching, and live update support.',
    tags: ['dom', 'collection', 'live-list', 'iterable']
  },
  'htmldataelement.rs': {
    summary: 'DOM implementation of the HTMLDataElement interface for machine-readable data representation.',
    tags: ['dom', 'html-element', 'data']
  },
  'htmldatalistelement.rs': {
    summary: 'DOM implementation of the HTMLDataListElement interface providing predefined option lists for input elements.',
    tags: ['dom', 'html-element', 'datalist', 'autocomplete']
  },
  'htmldetailselement.rs': {
    summary: 'DOM implementation of the HTMLDetailsElement interface with shadow DOM management, summary element discovery, and exclusive-accordion group support.',
    tags: ['dom', 'html-element', 'details', 'disclosure', 'shadow-dom']
  },
  'htmldialogelement.rs': {
    summary: 'DOM implementation of the HTMLDialogElement interface with modal dialog management, focus stepping, toggle event queuing, and escape-key handling.',
    tags: ['dom', 'html-element', 'dialog', 'modal', 'focus-management']
  },
  'htmldirectoryelement.rs': {
    summary: 'DOM implementation of the HTMLDirectoryElement interface for legacy directory list element construction.',
    tags: ['dom', 'html-element', 'directory', 'legacy']
  },
  'htmldivelement.rs': {
    summary: 'DOM implementation of the HTMLDivElement interface for generic division content element construction.',
    tags: ['dom', 'html-element', 'division', 'container']
  },
  'htmldlistelement.rs': {
    summary: 'DOM implementation of the HTMLDListElement interface for definition/description list element construction.',
    tags: ['dom', 'html-element', 'list', 'definition']
  },
  'htmldocument.rs': {
    summary: 'DOM binding for HTMLDocument, extending the Document interface for HTML-specific document functionality.',
    tags: ['dom', 'document', 'html-document']
  },
  'htmlelement.rs': {
    summary: 'Base class for all HTML element DOM bindings, providing shared functionality for attribute handling, event handler registration, focus management, editing, and text content manipulation.',
    tags: ['dom', 'html-element', 'base-class', 'event-handler', 'focus-management']
  },
  'htmlembedelement.rs': {
    summary: 'DOM implementation of the HTMLEmbedElement interface for embedded content element construction.',
    tags: ['dom', 'html-element', 'embed', 'plugin']
  },
  'htmlfieldsetelement.rs': {
    summary: 'DOM implementation of the HTMLFieldSetElement interface managing form control grouping, validation state, and disabled state propagation.',
    tags: ['dom', 'html-element', 'fieldset', 'form-control', 'validation']
  },
  'htmlfontelement.rs': {
    summary: 'DOM implementation of the HTMLFontElement interface for legacy font face, color, and size attribute handling with presentational hint support.',
    tags: ['dom', 'html-element', 'font', 'presentational-hints', 'legacy']
  },
  'htmlformcontrolscollection.rs': {
    summary: 'DOM implementation of HTMLFormControlsCollection providing named and indexed access to form control elements within a form.',
    tags: ['dom', 'collection', 'form-control', 'named-access']
  },
  'htmlformelement.rs': {
    summary: 'DOM implementation of the HTMLFormElement interface, managing form submission with encoding support, form controls collection, validation, and reset behavior.',
    tags: ['dom', 'html-element', 'form', 'submission', 'validation']
  },
  'htmlframeelement.rs': {
    summary: 'DOM implementation of the HTMLFrameElement interface for legacy frame element construction.',
    tags: ['dom', 'html-element', 'frame', 'legacy']
  },
  'htmlframesetelement.rs': {
    summary: 'DOM implementation of the HTMLFrameSetElement interface for legacy frameset element construction.',
    tags: ['dom', 'html-element', 'frameset', 'legacy']
  },
  'htmlheadelement.rs': {
    summary: 'DOM implementation of the HTMLHeadElement interface, representing the document head element with metadata container support.',
    tags: ['dom', 'html-element', 'head', 'document-metadata']
  }
};

// Process each file
for (const r of data.results) {
  const path = r.path;
  const fname = path.split('/').pop();
  const fileId = fileNodeId(path);
  const exportsArr = r.exports || [];
  const classesArr = r.classes || [];
  const fnsArr = r.functions || [];

  // Complexity
  let complexity = 'simple';
  if (r.nonEmptyLines > 200) complexity = 'complex';
  else if (r.nonEmptyLines >= 50) complexity = 'moderate';

  // File node
  const meta = fileMeta[fname] || { summary: fname + ' DOM implementation.', tags: ['dom', 'html-element'] };
  nodes.push(makeNode(fileId, 'file', fname, path, meta.summary, meta.tags, complexity));

  // Class nodes for significant exported classes
  for (const cls of classesArr) {
    if (!significantClasses[cls.name]) continue;
    const clsInfo = significantClasses[cls.name];
    const clsId = classNodeId(path, cls.name);
    const clsSize = cls.endLine - cls.startLine;
    const clsComplexity = clsSize > 100 ? 'complex' : clsSize > 40 ? 'moderate' : 'simple';

    nodes.push(makeNode(clsId, 'class', cls.name, path, clsInfo.summary, clsInfo.tags, clsComplexity, {
      lineRange: [cls.startLine, cls.endLine]
    }));
    edges.push(makeEdge(fileId, clsId, 'contains', 1.0));
    edges.push(makeEdge(fileId, clsId, 'exports', 0.8));
  }

  // Function nodes - deduplicate by function name (some may appear twice in extraction)
  const sigFns = fnSummaries[fname];
  if (!sigFns) continue;

  // Deduplicate: when a function name appears multiple times, keep the longest (actual implementation)
  const fnByName = {};
  for (const fn of fnsArr) {
    if (!sigFns[fn.name]) continue;
    const existing = fnByName[fn.name];
    if (!existing || (fn.endLine - fn.startLine) > (existing.endLine - existing.startLine)) {
      fnByName[fn.name] = fn;
    }
  }
  for (const fnName in fnByName) {
    const fn = fnByName[fnName];

    const fnLines = fn.endLine - fn.startLine;
    const fnExported = exportsArr.some(e => e.name === fn.name);
    const fnId = funcNodeId(path, fn.name);

    let fnComplexity = 'simple';
    if (fnLines > 100) fnComplexity = 'complex';
    else if (fnLines >= 30) fnComplexity = 'moderate';

    const fnTags = ['dom'];
    if (fnExported) fnTags.push('api');
    if (fnLines > 30) fnTags.push('core-logic');

    nodes.push(makeNode(fnId, 'function', fn.name, path, sigFns[fn.name], fnTags, fnComplexity, {
      lineRange: [fn.startLine, fn.endLine]
    }));

    edges.push(makeEdge(fileId, fnId, 'contains', 1.0));
    if (fnExported) {
      edges.push(makeEdge(fileId, fnId, 'exports', 0.8));
    }
  }
}

// Write output
const output = JSON.stringify({ nodes, edges }, null, 2);
console.log('Total nodes:', nodes.length);
console.log('Total edges:', edges.length);

// Decide split
const nodeCount = nodes.length;
const edgeCount = edges.length;
console.log('Node threshold 60:', nodeCount > 60);
console.log('Edge threshold 120:', edgeCount > 120);

if (nodeCount <= 60 && edgeCount <= 120) {
  fs.writeFileSync('d:/Projects/servo/.understand-anything/intermediate/batch-15.json', output, 'utf-8');
  console.log('Written single file: batch-15.json');
} else {
  const parts = Math.ceil(Math.max(nodeCount / 60, edgeCount / 120));
  console.log('Splitting into', parts, 'parts');

  // Sort files alphabetically
  const filePaths = data.results.map(r => r.path).sort();
  const filesPerPart = Math.ceil(filePaths.length / parts);

  for (let p = 0; p < parts; p++) {
    const start = p * filesPerPart;
    const end = Math.min(start + filesPerPart, filePaths.length);
    const partFiles = new Set(filePaths.slice(start, end));

    const partNodes = nodes.filter(n => partFiles.has(n.filePath));
    const partNodeIds = new Set(partNodes.map(n => n.id));
    const partEdges = edges.filter(e => partNodeIds.has(e.source));

    const partOutput = JSON.stringify({ nodes: partNodes, edges: partEdges }, null, 2);
    const partFile = 'd:/Projects/servo/.understand-anything/intermediate/batch-15-part-' + (p + 1) + '.json';
    fs.writeFileSync(partFile, partOutput, 'utf-8');
    console.log('Written part', (p + 1), ':', partNodes.length, 'nodes,', partEdges.length, 'edges');
  }
}
