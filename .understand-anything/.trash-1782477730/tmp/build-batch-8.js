const fs = require('fs');
const path = require('path');

// Read extraction results
const results = JSON.parse(fs.readFileSync('d:/Projects/servo/.understand-anything/tmp/ua-file-extract-results-8.json', 'utf8'));

// Build nodes and edges
const nodes = [];
const edges = [];
const nodeIds = new Set();

function addNode(node) {
  if (nodeIds.has(node.id)) return;
  nodeIds.add(node.id);
  nodes.push(node);
}

function addEdge(edge) {
  edges.push(edge);
}

// Process each file result
for (const file of results.results) {
  const filePath = file.path;
  const fileName = path.basename(filePath);

  let nodeType = 'file';
  let tags = [];
  let summary = '';
  let complexity = 'moderate';
  const nonEmptyLines = file.nonEmptyLines || file.totalLines;

  if (nonEmptyLines < 50) complexity = 'simple';
  else if (nonEmptyLines > 200) complexity = 'complex';

  const eventTypeName = fileName.replace('.rs', '');

  if (eventTypeName === 'mod') {
    summary = 'Barrel module that re-exports all DOM event types for the events submodule, providing a centralized import point.';
    tags = ['barrel', 'entry-point', 'exports'];
    complexity = 'simple';
  } else if (eventTypeName === 'event') {
    summary = 'Core DOM Event implementation providing the base event class with dispatch, propagation, and lifecycle management for all DOM events in Servo.';
    tags = ['event-system', 'dom-api', 'core'];
    complexity = 'complex';
  } else if (eventTypeName === 'eventtarget') {
    summary = 'DOM EventTarget implementation managing event listener registration, removal, handler compilation, and event dispatch targeting.';
    tags = ['event-system', 'dom-api', 'listener-management'];
    complexity = 'complex';
  } else if (eventTypeName === 'uievent') {
    summary = 'UIEvent implementation extending Event with UI-specific details like view and detail, serving as the base for mouse, keyboard, and focus events.';
    tags = ['event-system', 'ui-events', 'dom-api'];
    complexity = 'moderate';
  } else if (eventTypeName === 'mouseevent') {
    summary = 'MouseEvent implementation handling mouse interactions with position tracking, button states, modifier keys, and platform event conversion.';
    tags = ['event-system', 'mouse-events', 'pointer-interaction'];
    complexity = 'complex';
  } else if (eventTypeName === 'keyboardevent') {
    summary = 'KeyboardEvent implementation managing key press/release events with key codes, modifier states, and platform keyboard event integration.';
    tags = ['event-system', 'keyboard-events', 'input'];
    complexity = 'complex';
  } else if (eventTypeName === 'pointerevent') {
    summary = 'PointerEvent implementation extending MouseEvent with pointer-specific properties like pointer type, pressure, tilt, and hardware identifier.';
    tags = ['event-system', 'pointer-events', 'input'];
    complexity = 'complex';
  } else if (eventTypeName === 'wheelevent') {
    summary = 'WheelEvent implementation extending MouseEvent for mouse wheel scrolling with delta measurements and mode information.';
    tags = ['event-system', 'wheel-events', 'scrolling'];
    complexity = 'complex';
  } else if (eventTypeName === 'messageevent') {
    summary = 'MessageEvent implementation facilitating cross-origin communication with support for structured data, source tracking, and message ports.';
    tags = ['event-system', 'messaging', 'cross-origin'];
    complexity = 'complex';
  } else if (eventTypeName === 'touchevent') {
    summary = 'TouchEvent implementation managing touch interaction tracking with multi-touch support and touch point enumeration.';
    tags = ['event-system', 'touch-events', 'input'];
    complexity = 'moderate';
  } else if (eventTypeName === 'storageevent') {
    summary = 'StorageEvent implementation for web storage change notifications, carrying old and new values along with affected storage area.';
    tags = ['event-system', 'storage', 'web-api'];
    complexity = 'moderate';
  } else if (eventTypeName === 'focusevent') {
    summary = 'FocusEvent implementation extending UIEvent for focus/blur tracking with related target support.';
    tags = ['event-system', 'focus-events', 'ui-events'];
    complexity = 'moderate';
  } else if (eventTypeName === 'inputevent') {
    summary = 'InputEvent implementation extending UIEvent for text input handling with data, input type, composing state, and selection tracking.';
    tags = ['event-system', 'input-events', 'text-input'];
    complexity = 'moderate';
  } else if (eventTypeName === 'compositionevent') {
    summary = 'CompositionEvent implementation extending UIEvent for IME composition text input with composition data tracking.';
    tags = ['event-system', 'composition-events', 'ime-input'];
    complexity = 'moderate';
  } else if (eventTypeName === 'animationevent') {
    summary = 'AnimationEvent implementation for CSS animation lifecycle notifications including start, end, and iteration.';
    tags = ['event-system', 'animation-events', 'css-animations'];
    complexity = 'simple';
  } else if (eventTypeName === 'beforeunloadevent') {
    summary = 'BeforeUnloadEvent implementation providing page unload confirmation with return value prompting.';
    tags = ['event-system', 'page-lifecycle', 'navigation'];
    complexity = 'simple';
  } else if (eventTypeName === 'closeevent') {
    summary = 'CloseEvent implementation for WebSocket and other connection closures with wasClean, code, and reason tracking.';
    tags = ['event-system', 'networking', 'websocket'];
    complexity = 'moderate';
  } else if (eventTypeName === 'commandevent') {
    summary = 'CommandEvent implementation for command invocation with source element and command identifier tracking.';
    tags = ['event-system', 'commands', 'dom-api'];
    complexity = 'moderate';
  } else if (eventTypeName === 'customevent') {
    summary = 'CustomEvent implementation allowing custom named events with arbitrary detail data payloads.';
    tags = ['event-system', 'custom-events', 'extensibility'];
    complexity = 'moderate';
  } else if (eventTypeName === 'errorevent') {
    summary = 'ErrorEvent implementation for error reporting with message, source URL, line/column numbers, and error object.';
    tags = ['event-system', 'error-handling', 'diagnostics'];
    complexity = 'moderate';
  } else if (eventTypeName === 'formdataevent') {
    summary = 'FormDataEvent implementation for form data changes, carrying the associated FormData object for form serialization.';
    tags = ['event-system', 'forms', 'data-collection'];
    complexity = 'simple';
  } else if (eventTypeName === 'hashchangeevent') {
    summary = 'HashChangeEvent implementation for URL hash/fragment changes with old and new URL tracking.';
    tags = ['event-system', 'navigation', 'url'];
    complexity = 'moderate';
  } else if (eventTypeName === 'popstateevent') {
    summary = 'PopStateEvent implementation for browser history navigation changes, carrying the state object from the history entry.';
    tags = ['event-system', 'history', 'navigation'];
    complexity = 'moderate';
  } else if (eventTypeName === 'progressevent') {
    summary = 'ProgressEvent implementation for resource loading progress tracking with loaded and total byte counts.';
    tags = ['event-system', 'loading', 'progress'];
    complexity = 'moderate';
  } else if (eventTypeName === 'promiserejectionevent') {
    summary = 'PromiseRejectionEvent implementation for unhandled promise rejection notifications with the promise and reason.';
    tags = ['event-system', 'promises', 'async'];
    complexity = 'moderate';
  } else if (eventTypeName === 'submitevent') {
    summary = 'SubmitEvent implementation for form submission events with the submitter element reference.';
    tags = ['event-system', 'forms', 'submission'];
    complexity = 'simple';
  } else if (eventTypeName === 'toggleevent') {
    summary = 'ToggleEvent implementation for element state toggling (open/close) with old and new state tracking.';
    tags = ['event-system', 'toggle', 'ui-events'];
    complexity = 'moderate';
  } else if (eventTypeName === 'transitionevent') {
    summary = 'TransitionEvent implementation for CSS transition lifecycle notifications including run, start, end, and cancel.';
    tags = ['event-system', 'transition-events', 'css-transitions'];
    complexity = 'moderate';
  } else {
    summary = 'DOM event implementation providing event type-specific properties and behavior.';
    tags = ['event-system', 'dom-api'];
  }

  addNode({
    id: 'file:' + filePath,
    type: nodeType,
    name: fileName,
    filePath: filePath,
    summary: summary,
    tags: tags,
    complexity: complexity
  });

  // Process classes
  for (const cls of (file.classes || [])) {
    const clsName = cls.name;
    const clsId = 'class:' + filePath + ':' + clsName;
    const clsLen = cls.endLine - cls.startLine + 1;
    const isExported = (file.exports || []).some(e => e.name === clsName);

    const isSignificant = cls.methods.length >= 2 || clsLen >= 20;
    if (!isSignificant && !isExported) continue;

    let clsSummary = '';
    let clsTags = ['dom-event', 'class'];

    if (clsName === 'Event') {
      clsSummary = 'Base DOM Event class providing event dispatch, propagation phases, path management, and lifecycle state.';
      clsTags = ['event-system', 'base-class', 'dom-api'];
    } else if (clsName === 'EventTarget') {
      clsSummary = 'DOM EventTarget class managing event listener registration and removal, handler compilation, and event dispatch targeting.';
      clsTags = ['event-system', 'listener-management', 'dom-api'];
    } else if (clsName === 'MouseEvent') {
      clsSummary = 'DOM MouseEvent class for mouse interaction tracking with position, button states, and platform event conversion.';
      clsTags = ['event-system', 'mouse-events', 'input'];
    } else if (clsName === 'UIEvent') {
      clsSummary = 'DOM UIEvent class extending Event with view and detail properties for user interface interaction events.';
      clsTags = ['event-system', 'ui-events', 'base-class'];
    } else if (clsName === 'KeyboardEvent') {
      clsSummary = 'DOM KeyboardEvent class for keyboard input with key codes, modifier states, and repeat detection.';
      clsTags = ['event-system', 'keyboard-events', 'input'];
    } else if (clsName === 'PointerEvent') {
      clsSummary = 'DOM PointerEvent class extending MouseEvent with pointer hardware details, pressure, tilt, and event coalescing.';
      clsTags = ['event-system', 'pointer-events', 'input'];
    } else if (clsName === 'WheelEvent') {
      clsSummary = 'DOM WheelEvent class for scroll wheel input with delta measurements and scroll mode information.';
      clsTags = ['event-system', 'wheel-events', 'scrolling'];
    } else if (clsName === 'MessageEvent') {
      clsSummary = 'DOM MessageEvent class for cross-origin messaging with data, origin, source, and port transfer support.';
      clsTags = ['event-system', 'messaging', 'communication'];
    } else if (clsName === 'FocusEvent') {
      clsSummary = 'DOM FocusEvent class for focus and blur tracking with related target identification.';
      clsTags = ['event-system', 'focus-events', 'ui-events'];
    } else if (clsName === 'InputEvent') {
      clsSummary = 'DOM InputEvent class for text input changes with data, input type, and composing state.';
      clsTags = ['event-system', 'input-events', 'text'];
    } else if (clsName === 'CustomEvent') {
      clsSummary = 'DOM CustomEvent class enabling custom event dispatching with arbitrary detail payloads.';
      clsTags = ['event-system', 'custom-events', 'extensibility'];
    } else if (clsName === 'ErrorEvent') {
      clsSummary = 'DOM ErrorEvent class for error reporting with message, source location, and error object details.';
      clsTags = ['event-system', 'error-handling', 'diagnostics'];
    } else if (clsName === 'TouchEvent') {
      clsSummary = 'DOM TouchEvent class for multi-touch interaction with touch point tracking and modifier state.';
      clsTags = ['event-system', 'touch-events', 'input'];
    } else if (clsName === 'StorageEvent') {
      clsSummary = 'DOM StorageEvent class for web storage change notifications with old and new value tracking.';
      clsTags = ['event-system', 'storage', 'web-api'];
    } else if (clsName === 'CloseEvent') {
      clsSummary = 'DOM CloseEvent class for connection closure events with wasClean, code, and reason information.';
      clsTags = ['event-system', 'networking', 'websocket'];
    } else if (clsName === 'AnimationEvent') {
      clsSummary = 'DOM AnimationEvent class for CSS animation lifecycle notifications (start, end, iteration).';
      clsTags = ['event-system', 'animation-events', 'css'];
    } else if (clsName === 'BeforeUnloadEvent') {
      clsSummary = 'DOM BeforeUnloadEvent class for page unload confirmation prompting.';
      clsTags = ['event-system', 'page-lifecycle', 'navigation'];
    } else if (clsName === 'HashChangeEvent') {
      clsSummary = 'DOM HashChangeEvent class for URL fragment changes with old and new URL tracking.';
      clsTags = ['event-system', 'navigation', 'url-history'];
    } else if (clsName === 'PopStateEvent') {
      clsSummary = 'DOM PopStateEvent class for browser history navigation with state object access.';
      clsTags = ['event-system', 'history', 'navigation'];
    } else if (clsName === 'ProgressEvent') {
      clsSummary = 'DOM ProgressEvent class for resource loading progress with loaded and total byte counts.';
      clsTags = ['event-system', 'loading', 'progress'];
    } else if (clsName === 'PromiseRejectionEvent') {
      clsSummary = 'DOM PromiseRejectionEvent class for unhandled promise rejection notification.';
      clsTags = ['event-system', 'promises', 'async-errors'];
    } else if (clsName === 'SubmitEvent') {
      clsSummary = 'DOM SubmitEvent class for form submission with submitter element reference.';
      clsTags = ['event-system', 'forms', 'submission'];
    } else if (clsName === 'ToggleEvent') {
      clsSummary = 'DOM ToggleEvent class for element open/close state transitions.';
      clsTags = ['event-system', 'toggle', 'ui-events'];
    } else if (clsName === 'TransitionEvent') {
      clsSummary = 'DOM TransitionEvent class for CSS transition lifecycle notifications.';
      clsTags = ['event-system', 'transition-events', 'css'];
    } else if (clsName === 'FormDataEvent') {
      clsSummary = 'DOM FormDataEvent class for form data changes with associated FormData payload.';
      clsTags = ['event-system', 'forms', 'data'];
    } else if (clsName === 'CommandEvent') {
      clsSummary = 'DOM CommandEvent class for command invocation with source and command identifier.';
      clsTags = ['event-system', 'commands', 'dom-api'];
    } else if (clsName === 'CompositionEvent') {
      clsSummary = 'DOM CompositionEvent class for IME composition text input tracking.';
      clsTags = ['event-system', 'composition-events', 'ime'];
    } else if (clsName === 'EventPathSegment') {
      clsSummary = 'Internal struct representing a single segment in the event dispatch path, tracking invocation and shadow-adjusted targets.';
      clsTags = ['event-system', 'internal', 'path-tracking'];
    } else if (clsName === 'EventBubbles') {
      clsSummary = 'Enum controlling whether an event bubbles through the DOM tree.';
      clsTags = ['event-system', 'enum', 'propagation'];
    } else if (clsName === 'EventCancelable') {
      clsSummary = 'Enum controlling whether an event can be cancelled with preventDefault.';
      clsTags = ['event-system', 'enum', 'cancellation'];
    } else if (clsName === 'EventComposed') {
      clsSummary = 'Enum controlling whether an event propagates across shadow DOM boundaries.';
      clsTags = ['event-system', 'enum', 'shadow-dom'];
    } else if (clsName === 'EventPhase') {
      clsSummary = 'Enum representing the current phase of event propagation (none, capturing, at target, bubbling).';
      clsTags = ['event-system', 'enum', 'phases'];
    } else if (clsName === 'EventFlags') {
      clsSummary = 'Bitflags struct tracking miscellaneous event state flags during dispatch.';
      clsTags = ['event-system', 'internal', 'flags'];
    } else if (clsName === 'EventTask') {
      clsSummary = 'Task struct for queued event dispatch with target, name, and propagation settings.';
      clsTags = ['event-system', 'task-queue', 'async'];
    } else if (clsName === 'SimpleEventTask') {
      clsSummary = 'Simple task struct for queued event dispatch with target and name only.';
      clsTags = ['event-system', 'task-queue', 'async'];
    } else if (clsName === 'CommonEventHandler') {
      clsSummary = 'Enum representing compiled event handler variants for standard, error, and beforeunload events.';
      clsTags = ['event-system', 'handler', 'enum'];
    } else if (clsName === 'ListenerPhase') {
      clsSummary = 'Enum distinguishing capturing-phase from bubbling-phase event listeners.';
      clsTags = ['event-system', 'listener', 'phase'];
    } else if (clsName === 'EventListenerType') {
      clsSummary = 'Enum distinguishing additive event listeners from inline (on-event-handler) listeners.';
      clsTags = ['event-system', 'listener', 'type'];
    } else if (clsName === 'CompiledEventListener') {
      clsSummary = 'Enum wrapping either an IDL event listener or a compiled event handler for invocation.';
      clsTags = ['event-system', 'listener', 'compilation'];
    } else if (clsName === 'EventListenerEntry') {
      clsSummary = 'Struct holding a registered event listener with phase, once flag, passive flag, and removal state.';
      clsTags = ['event-system', 'listener', 'registration'];
    } else if (clsName === 'EventListeners') {
      clsSummary = 'Newtype wrapper around Vec<EventListenerEntry> providing inline listener lookup and listener presence checks.';
      clsTags = ['event-system', 'listener', 'collection'];
    } else if (clsName === 'FocusEventType') {
      clsSummary = 'Enum distinguishing focus vs blur event types.';
      clsTags = ['event-system', 'enum', 'focus'];
    } else if (clsName === 'SrcObject') {
      clsSummary = 'Enum representing source object types for MessageEvent (WindowProxy, MessagePort, ServiceWorker).';
      clsTags = ['event-system', 'enum', 'messaging'];
    } else if (clsName === 'InlineEventListener') {
      clsSummary = 'Enum tracking the compilation state of an inline event listener (uncompiled, compiled, or null).';
      clsTags = ['event-system', 'listener', 'compilation'];
    } else if (clsName === 'InternalRawUncompiledHandler') {
      clsSummary = 'Struct holding raw source text, URL, and line number for an uncompiled event handler.';
      clsTags = ['event-system', 'internal', 'handler'];
    } else if (clsName === 'HitTestResult') {
      clsSummary = 'Struct containing hit-test results from pointer/input events with node and coordinate information.';
      clsTags = ['event-system', 'input', 'hit-testing'];
    } else {
      const readable = clsName.replace(/([A-Z])/g, ' $1').trim();
      clsSummary = 'Supporting type for ' + readable.toLowerCase() + '.';
      clsTags = ['event-system', 'supporting-type'];
    }

    addNode({
      id: clsId,
      type: 'class',
      name: clsName,
      filePath: filePath,
      lineRange: [cls.startLine, cls.endLine],
      summary: clsSummary,
      tags: clsTags,
      complexity: clsLen > 50 ? 'complex' : (clsLen > 20 ? 'moderate' : 'simple')
    });

    addEdge({ source: 'file:' + filePath, target: clsId, type: 'contains', direction: 'forward', weight: 1.0 });

    if (isExported) {
      addEdge({ source: 'file:' + filePath, target: clsId, type: 'exports', direction: 'forward', weight: 0.8 });
    }
  }

  // Process significant functions
  for (const func of (file.functions || [])) {
    const funcName = func.name;
    const funcLen = func.endLine - func.startLine + 1;
    const isExported = (file.exports || []).some(e => e.name === funcName);
    const funcId = 'function:' + filePath + ':' + funcName;

    // Skip 1-3 line non-exported functions (trivial helpers)
    if (funcLen < 4 && !isExported) continue;
    // Must be 10+ lines or exported
    if (funcLen < 10 && !isExported) continue;
    // Skip trivial exported IDL getter boilerplate (1-3 lines)
    if (funcLen <= 3 && isExported) {
      const trivialGetterPattern = /^(IsTrusted|AnimationName|ElapsedTime|PseudoElement|WasClean|Code$|Reason$|Data$|FormData$|Detail$|OldURL|NewURL|GetData$|IsComposing$|InputType$|GetDataTransfer$|GetTargetRanges$|HasFocus$|Key$|Code$|Location$|CtrlKey|ShiftKey|AltKey|MetaKey|Repeat$|CharCode|KeyCode|ScreenX|ScreenY|ClientX|ClientY|PageX|PageY|X$|Y$|OffsetX|OffsetY|Button$|Buttons$|GetRelatedTarget|GetSource|Origin$|LastEventId|Message$|Filename$|Lineno$|Colno$|Command$|ReturnValue|SetReturnValue|CancelBubble|SetCancelBubble|Bubbles$|Cancelable|Composed|DefaultPrevented|PreventDefault|StopPropagation|StopImmediatePropagation|EventPhase|Type$|GetTarget|GetSrcElement|GetCurrentTarget|TimeStamp|InitEvent|SetReturnValue|CancelBubble|SetCancelBubble|MarkAsHandled|InitMouseEvent|InitKeyboardEvent|InitCustomEvent|InitMessageEvent|IsTrusted|PointInTarget)$/;
      if (trivialGetterPattern.test(funcName)) continue;
    }

    let funcSummary = '';
    let funcTags = ['function'];

    if (funcName === 'dispatch_inner') {
      funcSummary = 'Core event dispatch algorithm implementing capture, at-target, and bubbling phases with shadow DOM support and activation behavior.';
      funcTags = ['event-system', 'dispatch', 'core-algorithm'];
    } else if (funcName === 'init_event' && filePath.includes('event.rs')) {
      funcSummary = 'Initializes event properties including type, bubbles, cancelable, and resets trusted/dispatching state.';
      funcTags = ['event-system', 'initialization'];
    } else if (funcName === 'append_to_path') {
      funcSummary = 'Appends a new segment to the event dispatch path with invocation target, shadow-adjusted target, and related target tracking.';
      funcTags = ['event-system', 'path-tracking'];
    } else if (funcName === 'Constructor') {
      const readable = path.basename(filePath, '.rs').replace(/([A-Z])/g, ' $1').trim().toLowerCase();
      funcSummary = 'JavaScript-accessible constructor for ' + readable + ' with WebIDL init dictionary handling.';
      funcTags = ['event-system', 'constructor', 'webidl'];
    } else if (funcName === 'new_with_proto') {
      funcSummary = 'Creates a new DOM-reflected event instance with an optional prototype chain.';
      funcTags = ['event-system', 'constructor', 'dom-reflection'];
    } else if (funcName === 'new_inherited') {
      funcSummary = 'Internal constructor initializing inherited fields for the event struct.';
      funcTags = ['event-system', 'constructor', 'internal'];
    } else if (funcName === 'new') {
      funcSummary = 'Public Rust constructor creating a fully initialized event instance.';
      funcTags = ['event-system', 'constructor'];
    } else if (funcName === 'ComposedPath') {
      funcSummary = 'Returns the event\'s composed propagation path, computing each node in the shadow-including tree order.';
      funcTags = ['event-system', 'path', 'shadow-dom'];
    } else if (funcName === 'call_or_handle_event') {
      funcSummary = 'Invokes a compiled event listener on the target, handling error/beforeunload special cases and return value processing.';
      funcTags = ['event-system', 'listener-invocation', 'handler'];
    } else if (funcName === 'invoke') {
      funcSummary = 'Core listener invocation phase, setting current target, retrieving listeners, and dispatching to inner_invoke.';
      funcTags = ['event-system', 'invocation', 'dispatch'];
    } else if (funcName === 'inner_invoke') {
      funcSummary = 'Iterates through event listeners for the current phase, calling each listener and handling once/passive/signal tracking.';
      funcTags = ['event-system', 'listener-invocation', 'iteration'];
    } else if (funcName === 'add_event_listener') {
      funcSummary = 'Registers an event listener with phase, passive, once, and signal abort options.';
      funcTags = ['event-system', 'listener', 'registration'];
    } else if (funcName === 'remove_event_listener') {
      funcSummary = 'Removes a previously registered event listener by type and reference matching.';
      funcTags = ['event-system', 'listener', 'removal'];
    } else if (funcName === 'get_the_parent') {
      funcSummary = 'Computes the parent EventTarget in the DOM hierarchy for event propagation, handling documents, shadow roots, nodes, and IDB objects.';
      funcTags = ['event-system', 'propagation', 'parent-traversal'];
    } else if (funcName === 'retarget') {
      funcSummary = 'Retargets an EventTarget across shadow DOM boundaries for event dispatch.';
      funcTags = ['event-system', 'shadow-dom', 'targeting'];
    } else if (funcName === 'set_event_handler_uncompiled') {
      funcSummary = 'Registers an uncompiled inline event handler with source text, performing CSP checks and deferred compilation.';
      funcTags = ['event-system', 'handler', 'compilation'];
    } else if (funcName === 'get_compiled_event_handler') {
      funcSummary = 'Compiles and caches an inline event handler source string into a JS function, building the scope chain.';
      funcTags = ['event-system', 'handler', 'js-compilation'];
    } else if (funcName === 'default_passive_value') {
      funcSummary = 'Determines the default passive value for event listeners based on event type and target element context.';
      funcTags = ['event-system', 'passive', 'defaults'];
    } else if (funcName === 'set_inline_event_listener') {
      funcSummary = 'Sets an inline event listener (on-event handler) on the event target.';
      funcTags = ['event-system', 'listener', 'inline-handler'];
    } else if (funcName === 'dispatch_jsval') {
      funcSummary = 'Creates and dispatches a MessageEvent with JS value data, origin, source, and port transfer.';
      funcTags = ['event-system', 'messaging', 'dispatch'];
    } else if (funcName === 'dispatch_error') {
      funcSummary = 'Creates and dispatches an error MessageEvent for message port error handling.';
      funcTags = ['event-system', 'messaging', 'error-dispatch'];
    } else if (funcName === 'new_with_platform_keyboard_event') {
      funcSummary = 'Creates a KeyboardEvent from platform-level keyboard event data with key, code, and modifier mapping.';
      funcTags = ['event-system', 'keyboard', 'platform-integration'];
    } else if (funcName === 'init_event' && filePath.includes('keyboardevent')) {
      funcSummary = 'Initializes a keyboard event with type, bubbles, cancelable, view, key, location, and repeat settings.';
      funcTags = ['event-system', 'keyboard', 'initialization'];
    } else if (funcName === 'InitKeyboardEvent') {
      funcSummary = 'Legacy keyboard event initializer implementing the IDL InitKeyboardEvent method.';
      funcTags = ['event-system', 'keyboard', 'legacy'];
    } else if (funcName === 'to_pointer_event') {
      funcSummary = 'Converts a MouseEvent into a PointerEvent with mouse-derived pointer properties.';
      funcTags = ['event-system', 'mouse-to-pointer', 'conversion'];
    } else if (funcName === 'to_pointer_hover_event') {
      funcSummary = 'Converts a MouseEvent into a hover PointerEvent retaining position data without button state.';
      funcTags = ['event-system', 'mouse-to-pointer', 'conversion'];
    } else if (funcName === 'initialize_mouse_event') {
      funcSummary = 'Initializes mouse event fields including position, modifiers, button state, and related target.';
      funcTags = ['event-system', 'mouse', 'initialization'];
    } else if (funcName === 'new_for_platform_motion_event') {
      funcSummary = 'Creates a MouseEvent from platform-level mouse motion data with hit-test results and coordinate mapping.';
      funcTags = ['event-system', 'mouse', 'platform-integration'];
    } else if (funcName === 'for_platform_button_event') {
      funcSummary = 'Creates a MouseEvent from platform-level button press/release data with click count and modifier tracking.';
      funcTags = ['event-system', 'mouse', 'platform-integration'];
    } else if (funcName === 'InitMouseEvent') {
      funcSummary = 'Legacy mouse event initializer implementing the IDL InitMouseEvent method with all coordinate and modifier parameters.';
      funcTags = ['event-system', 'mouse', 'legacy'];
    } else if (funcName === 'GetModifierState') {
      const source = filePath.includes('mouse') ? 'mouse' : 'keyboard';
      funcSummary = 'Checks whether a named modifier key is currently pressed during a ' + source + ' event.';
      funcTags = ['event-system', source, 'modifiers'];
    } else if (funcName === 'OffsetX' || funcName === 'OffsetY') {
      funcSummary = 'Computes the mouse/touch position relative to the target element\'s padding edge.';
      funcTags = ['event-system', 'mouse', 'position'];
    } else if (funcName === 'PageX' || funcName === 'PageY') {
      funcSummary = 'Computes the mouse/touch position relative to the document\'s top-left corner including scroll offset.';
      funcTags = ['event-system', 'mouse', 'position'];
    } else if (funcName === 'inner_creation_steps') {
      funcSummary = 'Performs event creation steps from init dictionary including flags, composed, and default properties.';
      funcTags = ['event-system', 'construction', 'initialization'];
    } else if (funcName === 'should_pass_shadow_boundary') {
      funcSummary = 'Determines whether an event should cross shadow boundary based on composed flag and root node comparison.';
      funcTags = ['event-system', 'shadow-dom', 'event-propagation'];
    } else if (funcName === 'summarize_event_listeners_for_devtools') {
      funcSummary = 'Compiles a summary of registered event listeners for DevTools inspection.';
      funcTags = ['event-system', 'devtools', 'listener-inspection'];
    } else if (funcName === 'InitMessageEvent') {
      funcSummary = 'Legacy message event initializer setting data, origin, source, lastEventId, and ports.';
      funcTags = ['event-system', 'messaging', 'legacy'];
    } else if (funcName === 'new_initialized') {
      funcSummary = 'Creates a MessageEvent with fully initialized data, origin, source, and ports.';
      funcTags = ['event-system', 'messaging', 'constructor'];
    } else if (funcName === 'GetSource') {
      funcSummary = 'Returns the source object of a message or command event (WindowProxy, MessagePort, ServiceWorker, or Element).';
      funcTags = ['event-system', 'messaging', 'source-tracking'];
    } else if (funcName.match(/^fire_/)) {
      funcSummary = 'Convenience method creating and dispatching an event on the target with configurable propagation settings.';
      funcTags = ['event-system', 'event-firing', 'dispatch'];
    } else if (funcName === 'Ports') {
      funcSummary = 'Returns a frozen array of MessagePort objects associated with this message event.';
      funcTags = ['event-system', 'messaging', 'ports'];
    } else if (funcName === 'dispatch') {
      funcSummary = 'Entry point for event dispatch with legacy target override support.';
      funcTags = ['event-system', 'dispatch', 'entry-point'];
    } else if (funcName === 'InitCustomEvent') {
      funcSummary = 'Legacy custom event initializer implementing the IDL InitCustomEvent method.';
      funcTags = ['event-system', 'custom-events', 'legacy'];
    } else if (funcName === 'init_custom_event') {
      funcSummary = 'Internal custom event initialization with type, bubbles, cancelable, and detail validation.';
      funcTags = ['event-system', 'custom-events', 'initialization'];
    } else if (funcName === 'is_an_activation_triggering_input_event') {
      funcSummary = 'Determines if this event is a trusted input event that should trigger user activation notification.';
      funcTags = ['event-system', 'activation', 'input-events'];
    } else if (funcName === 'fire' || funcName === 'fire_with_legacy_output_did_listeners_throw') {
      funcSummary = 'Dispatches a trusted event on the given target, optionally tracking listener exceptions.';
      funcTags = ['event-system', 'dispatch', 'trusted-event'];
    } else if (funcName === 'get_compiled_handler') {
      funcSummary = 'Compiles an inline event listener, replacing uncompiled source with the compiled result.';
      funcTags = ['event-system', 'listener', 'compilation'];
    } else if (funcName === 'associated_global') {
      funcSummary = 'Returns the global scope associated with this compiled event listener.';
      funcTags = ['event-system', 'listener', 'global-scope'];
    } else if (funcName === 'new_uninitialized' || funcName === 'new_uninitialized_with_proto') {
      funcSummary = 'Creates a new DOM-reflected event instance without fully initializing event properties.';
      funcTags = ['event-system', 'constructor', 'uninitialized'];
    } else if (funcName === 'is_content_event_handler') {
      funcSummary = 'Checks whether a given attribute name is a recognized content event handler.';
      funcTags = ['event-system', 'handler', 'attribute-detection'];
    } else if (funcName === 'has_handlers') {
      funcSummary = 'Returns whether this EventTarget has any registered handlers.';
      funcTags = ['event-system', 'listener', 'presence-check'];
    } else if (funcName === 'PointInTarget') {
      funcSummary = 'Returns the mouse position relative to the target element\'s coordinate system.';
      funcTags = ['event-system', 'mouse', 'position'];
    } else if (funcName === 'init_event' && filePath.includes('uievent')) {
      funcSummary = 'Initializes UI event with type, bubbles, cancelable, view, and detail parameters.';
      funcTags = ['event-system', 'ui-events', 'initialization'];
    } else if (funcName === 'set_detail') {
      funcSummary = 'Sets the detail counter for click count tracking on UI events.';
      funcTags = ['event-system', 'ui-events', 'detail'];
    } else if (funcName === 'initialize_ui_event') {
      funcSummary = 'Internal UI event initializer setting view and detail fields.';
      funcTags = ['event-system', 'ui-events', 'initialization'];
    } else if (funcName === 'set_which') {
      funcSummary = 'Sets the deprecated which property for keyboard/mouse events.';
      funcTags = ['event-system', 'ui-events', 'legacy'];
    } else if (funcName === 'get_compiled_listener' && filePath.includes('eventtarget')) {
      funcSummary = 'Retrieves a compiled event listener, compiling inline handlers if needed.';
      funcTags = ['event-system', 'listener', 'compilation'];
    } else if (funcName === 'get_inline_event_listener') {
      funcSummary = 'Retrieves the inline event listener for a given event type.';
      funcTags = ['event-system', 'listener', 'inline-listener'];
    } else if (funcName === 'has_non_passive_listener') {
      funcSummary = 'Checks if there are any non-passive listeners registered for a given event type.';
      funcTags = ['event-system', 'listener', 'passive-check'];
    } else if (funcName === 'get_event_handler_common') {
      funcSummary = 'Retrieves a compiled event handler for standard event types.';
      funcTags = ['event-system', 'handler', 'retrieval'];
    } else if (funcName === 'set_event_handler_common') {
      funcSummary = 'Sets an event handler of a common type on the target.';
      funcTags = ['event-system', 'handler', 'registration'];
    } else if (funcName === 'set_error_event_handler') {
      funcSummary = 'Sets an error event handler specifically for onerror handling with ErrorEvent.';
      funcTags = ['event-system', 'handler', 'error-handler'];
    } else if (funcName === 'set_beforeunload_event_handler') {
      funcSummary = 'Sets a beforeunload event handler specifically for unload confirmation.';
      funcTags = ['event-system', 'handler', 'beforeunload'];
    } else if (funcName === 'has_listeners_for') {
      funcSummary = 'Checks if there are any listeners registered for a specific event type.';
      funcTags = ['event-system', 'listener', 'presence-check'];
    } else if (funcName === 'get_listeners_for') {
      funcSummary = 'Returns the set of listeners registered for a specific event type.';
      funcTags = ['event-system', 'listener', 'retrieval'];
    } else if (funcName === 'remove_all_listeners') {
      funcSummary = 'Removes all event listeners from this target.';
      funcTags = ['event-system', 'listener', 'cleanup'];
    } else if (funcName === 'notify_listener_added') {
      funcSummary = 'Notifies the global scope that a new listener has been registered for interest tracking.';
      funcTags = ['event-system', 'listener', 'notification'];
    } else if (funcName === 'notify_listener_removed') {
      funcSummary = 'Notifies the global scope that a listener has been removed for interest tracking.';
      funcTags = ['event-system', 'listener', 'notification'];
    } else if (funcName === 'interest_for_event_type') {
      funcSummary = 'Determines whether the global scope should register interest in a particular event type.';
      funcTags = ['event-system', 'global', 'interest'];
    } else if (funcName === 'remove_listener') {
      funcSummary = 'Removes an inline event listener entry by type and position matching.';
      funcTags = ['event-system', 'listener', 'removal'];
    } else if (funcName === 'is_passive') {
      funcSummary = 'Checks whether a specific listener entry is passive.';
      funcTags = ['event-system', 'listener', 'passive'];
    } else if (funcName === 'DispatchEvent') {
      funcSummary = 'IDL-exposed method dispatching an event synchronously on the target.';
      funcTags = ['event-system', 'dispatch', 'webidl'];
    } else if (funcName === 'AddEventListener') {
      funcSummary = 'IDL-exposed method registering an event listener with options.';
      funcTags = ['event-system', 'listener', 'webidl'];
    } else if (funcName === 'RemoveEventListener') {
      funcSummary = 'IDL-exposed method removing a registered event listener.';
      funcTags = ['event-system', 'listener', 'webidl'];
    } else if (funcName === 'PointerCapture') {
      funcSummary = 'Releases pointer capture, ending exclusive pointer event delivery.';
      funcTags = ['event-system', 'pointer', 'capture'];
    } else if (funcName === 'SetPointerCapture') {
      funcSummary = 'Designates a specific element as the capture target for pointer events.';
      funcTags = ['event-system', 'pointer', 'capture'];
    } else if (funcName === 'HasPointerCapture') {
      funcSummary = 'Checks if the element has active pointer capture for a given pointer ID.';
      funcTags = ['event-system', 'pointer', 'capture'];
    } else if (funcName === 'ReleasePointerCapture') {
      funcSummary = 'Releases pointer capture for a specific pointer ID.';
      funcTags = ['event-system', 'pointer', 'capture'];
    } else if (funcName === 'GetCoalescedEvents') {
      funcSummary = 'Returns coalesced pointer events for high-frequency pointer move handling.';
      funcTags = ['event-system', 'pointer', 'coalescing'];
    } else if (funcName === 'GetPredictedEvents') {
      funcSummary = 'Returns predicted pointer positions for latency compensation.';
      funcTags = ['event-system', 'pointer', 'prediction'];
    } else if (funcName === 'super_type') {
      funcSummary = 'Returns the supertype for this EventTarget.';
      funcTags = ['event-system', 'type-hierarchy'];
    } else if (funcName === 'convert') {
      funcSummary = 'Converts AddEventListenerOptions to EventListenerOptions or boolean.';
      funcTags = ['event-system', 'options', 'conversion'];
    } else {
      const readable = funcName.replace(/([A-Z])/g, ' $1').replace(/^./, s => s.toUpperCase()).trim();
      funcSummary = 'Handler for ' + readable + ' on ' + path.basename(filePath, '.rs').replace(/([A-Z])/g, ' $1').trim().toLowerCase() + '.';
      funcTags = ['event-system', 'method'];
    }

    addNode({
      id: funcId,
      type: 'function',
      name: funcName,
      filePath: filePath,
      lineRange: [func.startLine, func.endLine],
      summary: funcSummary,
      tags: funcTags,
      complexity: funcLen > 50 ? 'complex' : (funcLen > 15 ? 'moderate' : 'simple')
    });

    addEdge({ source: 'file:' + filePath, target: funcId, type: 'contains', direction: 'forward', weight: 1.0 });

    if (isExported) {
      addEdge({ source: 'file:' + filePath, target: funcId, type: 'exports', direction: 'forward', weight: 0.8 });
    }
  }
}

// Add import edges from batchImportData
const importData = {
  'components/script/dom/event/mod.rs': [
    'components/script/dom/event/animationevent.rs',
    'components/script/dom/event/beforeunloadevent.rs',
    'components/script/dom/event/closeevent.rs',
    'components/script/dom/event/commandevent.rs',
    'components/script/dom/event/compositionevent.rs',
    'components/script/dom/event/customevent.rs',
    'components/script/dom/event/errorevent.rs',
    'components/script/dom/event/event.rs',
    'components/script/dom/event/eventtarget.rs',
    'components/script/dom/event/focusevent.rs',
    'components/script/dom/event/formdataevent.rs',
    'components/script/dom/event/hashchangeevent.rs',
    'components/script/dom/event/inputevent.rs',
    'components/script/dom/event/keyboardevent.rs',
    'components/script/dom/event/messageevent.rs',
    'components/script/dom/event/mouseevent.rs',
    'components/script/dom/event/pagetransitionevent.rs',
    'components/script/dom/event/pointerevent.rs',
    'components/script/dom/event/popstateevent.rs',
    'components/script/dom/event/progressevent.rs',
    'components/script/dom/event/promiserejectionevent.rs',
    'components/script/dom/event/storageevent.rs',
    'components/script/dom/event/submitevent.rs',
    'components/script/dom/event/toggleevent.rs',
    'components/script/dom/event/touchevent.rs',
    'components/script/dom/event/transitionevent.rs',
    'components/script/dom/event/uievent.rs',
    'components/script/dom/event/wheelevent.rs'
  ]
};

for (const [sourceFile, targets] of Object.entries(importData)) {
  for (const targetFile of targets) {
    addEdge({
      source: 'file:' + sourceFile,
      target: 'file:' + targetFile,
      type: 'imports',
      direction: 'forward',
      weight: 0.7
    });
  }
}

// Add inherits edges (class hierarchy)
addEdge({ source: 'class:components/script/dom/event/animationevent.rs:AnimationEvent', target: 'class:components/script/dom/event/event.rs:Event', type: 'inherits', direction: 'forward', weight: 0.9 });
addEdge({ source: 'class:components/script/dom/event/beforeunloadevent.rs:BeforeUnloadEvent', target: 'class:components/script/dom/event/event.rs:Event', type: 'inherits', direction: 'forward', weight: 0.9 });
addEdge({ source: 'class:components/script/dom/event/closeevent.rs:CloseEvent', target: 'class:components/script/dom/event/event.rs:Event', type: 'inherits', direction: 'forward', weight: 0.9 });
addEdge({ source: 'class:components/script/dom/event/commandevent.rs:CommandEvent', target: 'class:components/script/dom/event/event.rs:Event', type: 'inherits', direction: 'forward', weight: 0.9 });
addEdge({ source: 'class:components/script/dom/event/customevent.rs:CustomEvent', target: 'class:components/script/dom/event/event.rs:Event', type: 'inherits', direction: 'forward', weight: 0.9 });
addEdge({ source: 'class:components/script/dom/event/errorevent.rs:ErrorEvent', target: 'class:components/script/dom/event/event.rs:Event', type: 'inherits', direction: 'forward', weight: 0.9 });
addEdge({ source: 'class:components/script/dom/event/formdataevent.rs:FormDataEvent', target: 'class:components/script/dom/event/event.rs:Event', type: 'inherits', direction: 'forward', weight: 0.9 });
addEdge({ source: 'class:components/script/dom/event/hashchangeevent.rs:HashChangeEvent', target: 'class:components/script/dom/event/event.rs:Event', type: 'inherits', direction: 'forward', weight: 0.9 });
addEdge({ source: 'class:components/script/dom/event/messageevent.rs:MessageEvent', target: 'class:components/script/dom/event/event.rs:Event', type: 'inherits', direction: 'forward', weight: 0.9 });
addEdge({ source: 'class:components/script/dom/event/popstateevent.rs:PopStateEvent', target: 'class:components/script/dom/event/event.rs:Event', type: 'inherits', direction: 'forward', weight: 0.9 });
addEdge({ source: 'class:components/script/dom/event/progressevent.rs:ProgressEvent', target: 'class:components/script/dom/event/event.rs:Event', type: 'inherits', direction: 'forward', weight: 0.9 });
addEdge({ source: 'class:components/script/dom/event/promiserejectionevent.rs:PromiseRejectionEvent', target: 'class:components/script/dom/event/event.rs:Event', type: 'inherits', direction: 'forward', weight: 0.9 });
addEdge({ source: 'class:components/script/dom/event/storageevent.rs:StorageEvent', target: 'class:components/script/dom/event/event.rs:Event', type: 'inherits', direction: 'forward', weight: 0.9 });
addEdge({ source: 'class:components/script/dom/event/submitevent.rs:SubmitEvent', target: 'class:components/script/dom/event/event.rs:Event', type: 'inherits', direction: 'forward', weight: 0.9 });
addEdge({ source: 'class:components/script/dom/event/toggleevent.rs:ToggleEvent', target: 'class:components/script/dom/event/event.rs:Event', type: 'inherits', direction: 'forward', weight: 0.9 });
addEdge({ source: 'class:components/script/dom/event/transitionevent.rs:TransitionEvent', target: 'class:components/script/dom/event/event.rs:Event', type: 'inherits', direction: 'forward', weight: 0.9 });

addEdge({ source: 'class:components/script/dom/event/uievent.rs:UIEvent', target: 'class:components/script/dom/event/event.rs:Event', type: 'inherits', direction: 'forward', weight: 0.9 });
addEdge({ source: 'class:components/script/dom/event/compositionevent.rs:CompositionEvent', target: 'class:components/script/dom/event/uievent.rs:UIEvent', type: 'inherits', direction: 'forward', weight: 0.9 });
addEdge({ source: 'class:components/script/dom/event/focusevent.rs:FocusEvent', target: 'class:components/script/dom/event/uievent.rs:UIEvent', type: 'inherits', direction: 'forward', weight: 0.9 });
addEdge({ source: 'class:components/script/dom/event/inputevent.rs:InputEvent', target: 'class:components/script/dom/event/uievent.rs:UIEvent', type: 'inherits', direction: 'forward', weight: 0.9 });
addEdge({ source: 'class:components/script/dom/event/keyboardevent.rs:KeyboardEvent', target: 'class:components/script/dom/event/uievent.rs:UIEvent', type: 'inherits', direction: 'forward', weight: 0.9 });
addEdge({ source: 'class:components/script/dom/event/mouseevent.rs:MouseEvent', target: 'class:components/script/dom/event/uievent.rs:UIEvent', type: 'inherits', direction: 'forward', weight: 0.9 });
addEdge({ source: 'class:components/script/dom/event/touchevent.rs:TouchEvent', target: 'class:components/script/dom/event/uievent.rs:UIEvent', type: 'inherits', direction: 'forward', weight: 0.9 });

addEdge({ source: 'class:components/script/dom/event/wheelevent.rs:WheelEvent', target: 'class:components/script/dom/event/mouseevent.rs:MouseEvent', type: 'inherits', direction: 'forward', weight: 0.9 });
addEdge({ source: 'class:components/script/dom/event/pointerevent.rs:PointerEvent', target: 'class:components/script/dom/event/mouseevent.rs:MouseEvent', type: 'inherits', direction: 'forward', weight: 0.9 });

addEdge({ source: 'class:components/script/dom/event/animationevent.rs:AnimationEvent', target: 'class:components/script/dom/event/transitionevent.rs:TransitionEvent', type: 'related', direction: 'forward', weight: 0.5 });

console.log('Total nodes:', nodes.length);
console.log('Total edges:', edges.length);

// ---- Improved Partitioning ----
// Group all nodes by filePath
const filePathsInOrder = [...new Set(nodes.filter(n => n.filePath).map(n => n.filePath))].sort();

// For each file, calculate how many nodes and edges it contributes
const fileStats = {};
for (const fp of filePathsInOrder) {
  const fileNodes = nodes.filter(n => n.filePath === fp);
  const fileNodeIds = new Set(fileNodes.map(n => n.id));
  const fileEdges = edges.filter(e => fileNodeIds.has(e.source));
  fileStats[fp] = { nodeCount: fileNodes.length, edgeCount: fileEdges.length };
}

const maxNodesPerPart = 55;
const maxEdgesPerPart = 110;

// Greedy partition: assign files to parts, ensuring each part stays within limits
const parts = [];
let currentPart = { files: [], nodeCount: 0, edgeCount: 0 };

for (const fp of filePathsInOrder) {
  const stats = fileStats[fp];

  // If this file alone would exceed limits, it needs its own part
  if (stats.nodeCount > maxNodesPerPart || stats.edgeCount > maxEdgesPerPart) {
    // This shouldn't happen for our data, but handle it
    if (currentPart.files.length > 0) {
      parts.push(currentPart);
      currentPart = { files: [], nodeCount: 0, edgeCount: 0 };
    }
    parts.push({ files: [fp], nodeCount: stats.nodeCount, edgeCount: stats.edgeCount });
    continue;
  }

  // Check if adding this file would exceed limits
  if (currentPart.nodeCount + stats.nodeCount > maxNodesPerPart ||
      currentPart.edgeCount + stats.edgeCount > maxEdgesPerPart) {
    parts.push(currentPart);
    currentPart = { files: [], nodeCount: 0, edgeCount: 0 };
  }

  currentPart.files.push(fp);
  currentPart.nodeCount += stats.nodeCount;
  currentPart.edgeCount += stats.edgeCount;
}
if (currentPart.files.length > 0) {
  parts.push(currentPart);
}

console.log('Parts needed (improved):', parts.length);
for (let i = 0; i < parts.length; i++) {
  console.log('  Part ' + (i+1) + ': ' + parts[i].files.length + ' files, ' + parts[i].nodeCount + ' nodes, ' + parts[i].edgeCount + ' edges');
}

// Build output for each part
for (let p = 0; p < parts.length; p++) {
  const filesInPart = new Set(parts[p].files);
  const partNodeSet = new Set();
  const partNodesArr = [];
  const partEdgesArr = [];

  // Add nodes for files in this part
  for (const node of nodes) {
    if (node.filePath && filesInPart.has(node.filePath)) {
      partNodeSet.add(node.id);
      partNodesArr.push(node);
    }
  }

  // Add edges whose source is in this part
  for (const edge of edges) {
    if (partNodeSet.has(edge.source)) {
      partEdgesArr.push(edge);
    }
  }

  const partFile = parts.length === 1
    ? 'd:/Projects/servo/.understand-anything/intermediate/batch-8.json'
    : 'd:/Projects/servo/.understand-anything/intermediate/batch-8-part-' + (p + 1) + '.json';

  const output = { nodes: partNodesArr, edges: partEdgesArr };
  fs.writeFileSync(partFile, JSON.stringify(output, null, 2));
  console.log('Wrote ' + partFile + ' (' + partNodesArr.length + ' nodes, ' + partEdgesArr.length + ' edges)');
}

// Self-validate
let allPassed = true;
for (let p = 0; p < parts.length; p++) {
  const partFile = parts.length === 1
    ? 'd:/Projects/servo/.understand-anything/intermediate/batch-8.json'
    : 'd:/Projects/servo/.understand-anything/intermediate/batch-8-part-' + (p + 1) + '.json';

  const data = JSON.parse(fs.readFileSync(partFile, 'utf8'));
  const partNodeSet = new Set(data.nodes.map(n => n.id));
  let errors = [];

  for (const edge of data.edges) {
    if (!partNodeSet.has(edge.source) && !nodeIds.has(edge.source)) {
      errors.push('source ' + edge.source + ' not found in any node set');
    }
    // Targets can be in other parts or other batches
  }

  if (data.nodes.length > 60) console.log('  WARNING: Part ' + (p+1) + ' has ' + data.nodes.length + ' nodes (limit 60)');
  if (data.edges.length > 120) console.log('  WARNING: Part ' + (p+1) + ' has ' + data.edges.length + ' edges (limit 120)');

  if (errors.length === 0) {
    console.log('  Part ' + (p+1) + ': validation passed');
  } else {
    console.log('  Part ' + (p+1) + ': validation FAILED - ' + errors.join(', '));
    allPassed = false;
  }
}

if (!allPassed) {
  process.exit(1);
}
