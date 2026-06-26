import { readFileSync, writeFileSync, mkdirSync } from 'fs';
import { join, dirname } from 'path';

const DATA = JSON.parse(readFileSync('d:/Projects/servo/.understand-anything/tmp/ua-file-extract-results-9.json', 'utf8'));
const BATCH_INDEX = 9;
const OUT_DIR = 'd:/Projects/servo/.understand-anything/intermediate';

mkdirSync(OUT_DIR, { recursive: true });

// ============================================================
// Analysis data per file
// ============================================================

const analysis = {};

// --- storage.rs ---
analysis['components/script/dom/storage/storage.rs'] = {
  summary: 'Implements the Storage Web API (localStorage/sessionStorage) with CRUD operations, named property access, and cross-tab change notification broadcasting via DOM storage events.',
  tags: ['dom', 'web-api', 'storage', 'data-persistence'],
  complexity: 'moderate',
  fnNodes: [
    { name: 'Length', lines: [68,80], summary: 'Returns the number of key-value pairs in the storage via IPC to the storage thread.', tags: ['web-api', 'storage'], complexity: 'simple' },
    { name: 'Key', lines: [83,96], summary: 'Retrieves the key at the given index from storage via IPC.', tags: ['web-api', 'storage'], complexity: 'simple' },
    { name: 'GetItem', lines: [99,113], summary: 'Retrieves the value for a given key name from storage via IPC.', tags: ['web-api', 'storage'], complexity: 'simple' },
    { name: 'SetItem', lines: [116,143], summary: 'Sets a key-value pair in storage, broadcasting a storage event to other tabs on success.', tags: ['web-api', 'storage'], complexity: 'moderate' },
    { name: 'RemoveItem', lines: [146,162], summary: 'Removes a key-value pair from storage and broadcasts change notification.', tags: ['web-api', 'storage'], complexity: 'moderate' },
    { name: 'Clear', lines: [165,179], summary: 'Clears all key-value pairs from storage and broadcasts change notification.', tags: ['web-api', 'storage'], complexity: 'moderate' },
    { name: 'SupportedPropertyNames', lines: [182,199], summary: 'Returns all storage key names for Web IDL named property enumeration support.', tags: ['web-api', 'storage'], complexity: 'moderate' },
    { name: 'broadcast_change_notification', lines: [217,232], summary: 'Sends a notification about storage changes to other browsing contexts via the script thread.', tags: ['internal', 'event', 'notification'], complexity: 'moderate' },
    { name: 'queue_storage_event', lines: [235,263], summary: 'Queues a StorageEvent on the event loop for the given URL and key change details.', tags: ['web-api', 'event'], complexity: 'moderate' },
  ],
  classNodes: [
    { name: 'Storage', lines: [27,31], summary: 'DOM Storage interface providing CRUD access to local/session storage with IPC-backed mutation and cross-origin isolation.', tags: ['dom', 'storage', 'web-api'], complexity: 'moderate' },
  ]
};

// --- storagemanager.rs ---
analysis['components/script/dom/storage/storagemanager.rs'] = {
  summary: 'Implements the StorageManager Web API providing navigator.storage methods (persisted, persist, estimate) with promise-based IPC response handling.',
  tags: ['dom', 'web-api', 'storage-manager', 'promise'],
  complexity: 'moderate',
  fnNodes: [
    { name: 'new', lines: [40,42], summary: 'Constructs a new StorageManager DOM object rooted to the given global scope.', tags: ['constructor', 'dom'], complexity: 'simple' },
    { name: 'handle (boolean)', lines: [68,82], summary: 'Resolves a StorageManager persistence query promise with the boolean IPC result.', tags: ['promise', 'handler'], complexity: 'moderate' },
    { name: 'handle (estimate)', lines: [98,119], summary: 'Resolves a StorageManager estimate promise with usage and quota data from IPC.', tags: ['promise', 'handler'], complexity: 'moderate' },
    { name: 'Persisted', lines: [124,166], summary: 'Checks whether storage has been persisted, returning a promise resolved via IPC.', tags: ['web-api', 'storage', 'promise'], complexity: 'moderate' },
    { name: 'Persist', lines: [169,222], summary: 'Requests persistent storage, returning a promise that resolves when IPC confirms the change.', tags: ['web-api', 'storage', 'promise'], complexity: 'complex' },
    { name: 'Estimate', lines: [225,270], summary: 'Returns storage usage and quota estimates as a promise resolved via IPC communication.', tags: ['web-api', 'storage', 'promise'], complexity: 'complex' },
  ],
  classNodes: [
    { name: 'StorageManager', lines: [29,31], summary: 'DOM interface for navigator.storage allowing persistence queries and storage estimation.', tags: ['dom', 'storage-manager', 'web-api'], complexity: 'simple' },
    { name: 'StorageManagerBooleanResponseHandler', lines: [55,58], summary: 'IPC response handler that resolves a boolean promise with the persistence state result.', tags: ['handler', 'promise', 'ipc'], complexity: 'simple' },
    { name: 'StorageManagerEstimateResponseHandler', lines: [85,88], summary: 'IPC response handler that resolves an estimate promise with usage and quota data.', tags: ['handler', 'promise', 'ipc'], complexity: 'simple' },
  ]
};

// --- textcontrol.rs ---
analysis['components/script/dom/textcontrol.rs'] = {
  summary: 'Implements text control selection and range management for editable text elements, providing DOM API bindings for selection start, end, direction, and range replacement.',
  tags: ['dom', 'text', 'selection', 'editing'],
  complexity: 'complex',
  fnNodes: [
    { name: 'new', lines: [48,53], summary: 'Constructs a TextControlElement for the given HTML element and text input controller.', tags: ['constructor'], complexity: 'simple' },
    { name: 'dom_select', lines: [56,70], summary: 'Selects all text in the control by setting the full range and dispatching a select event.', tags: ['dom', 'selection', 'event'], complexity: 'moderate' },
    { name: 'dom_start', lines: [73,81], summary: 'Returns the selection start offset within the editable text control.', tags: ['dom', 'selection', 'getter'], complexity: 'simple' },
    { name: 'set_dom_start', lines: [84,104], summary: 'Sets the selection start position, clamping to valid range and dispatching events.', tags: ['dom', 'selection', 'setter'], complexity: 'moderate' },
    { name: 'dom_end', lines: [107,119], summary: 'Returns the selection end offset within the editable text control.', tags: ['dom', 'selection', 'getter'], complexity: 'simple' },
    { name: 'set_dom_end', lines: [122,134], summary: 'Sets the selection end position with clamping and event dispatch.', tags: ['dom', 'selection', 'setter'], complexity: 'moderate' },
    { name: 'dom_direction', lines: [137,144], summary: 'Returns the selection direction (none, forward, backward) for the text control.', tags: ['dom', 'selection', 'getter'], complexity: 'simple' },
    { name: 'set_dom_direction', lines: [147,161], summary: 'Sets the selection direction with validation and event dispatch.', tags: ['dom', 'selection', 'setter'], complexity: 'moderate' },
    { name: 'set_dom_range', lines: [164,183], summary: 'Sets both start, end, and direction of the selection in one operation, dispatching a select event.', tags: ['dom', 'selection', 'range'], complexity: 'moderate' },
    { name: 'set_dom_range_text', lines: [186,333], summary: 'Replaces text within a given selection range, handling undo/redo and layout invalidation.', tags: ['dom', 'editing', 'text-replacement'], complexity: 'complex' },
    { name: 'set_range', lines: [348,408], summary: 'Internal method to set the visual text selection range with undo tracking and layout notification.', tags: ['internal', 'selection', 'layout'], complexity: 'complex' },
  ],
  classNodes: [
    { name: 'TextControlElement', lines: [26,40], summary: 'DOM abstraction bridging editable HTML elements and their text input controller for selection management.', tags: ['dom', 'text', 'editing'], complexity: 'moderate' },
    { name: 'TextControlSelection', lines: [42,45], summary: 'Data struct holding selection state (start, end, direction) for a text control element.', tags: ['data', 'selection'], complexity: 'simple' },
  ]
};

// --- timeranges.rs ---
analysis['components/script/dom/timeranges.rs'] = {
  summary: 'Implements the TimeRanges Web API for representing time ranges in media elements, with a container type supporting normalized range addition and overlap detection.',
  tags: ['dom', 'media', 'timeranges', 'web-api'],
  complexity: 'moderate',
  fnNodes: [
    { name: 'union', lines: [24,27], summary: 'Returns the union of this time range with another as a new TimeRange.', tags: ['utility', 'math'], complexity: 'simple' },
    { name: 'is_before', lines: [43,45], summary: 'Checks whether this time range is entirely before another range.', tags: ['utility', 'comparison'], complexity: 'simple' },
    { name: 'len', lines: [67,69], summary: 'Returns the number of time ranges in the container.', tags: ['getter'], complexity: 'simple' },
    { name: 'is_empty', lines: [71,73], summary: 'Returns whether the time range container is empty.', tags: ['getter'], complexity: 'simple' },
    { name: 'start', lines: [75,80], summary: 'Returns the start time of the range at the given index.', tags: ['getter'], complexity: 'simple' },
    { name: 'end', lines: [82,87], summary: 'Returns the end time of the range at the given index.', tags: ['getter'], complexity: 'simple' },
    { name: 'add', lines: [89,126], summary: 'Adds a time range, normalizing by merging overlapping or contiguous ranges.', tags: ['utility', 'normalization'], complexity: 'moderate' },
    { name: 'new', lines: [143,149], summary: 'Constructs a new TimeRanges DOM object wrapping a container.', tags: ['constructor', 'dom'], complexity: 'simple' },
  ],
  classNodes: [
    { name: 'TimeRange', lines: [18,21], summary: 'Representation of a single time range with start and end values supporting set operations.', tags: ['data', 'time'], complexity: 'simple' },
    { name: 'TimeRangesError', lines: [55,58], summary: 'Error type for invalid TimeRanges operations such as out-of-bounds index access.', tags: ['error'], complexity: 'simple' },
    { name: 'TimeRangesContainer', lines: [61,63], summary: 'Internal container managing a sorted, normalized list of non-overlapping time ranges.', tags: ['internal', 'container'], complexity: 'moderate' },
    { name: 'TimeRanges', lines: [130,133], summary: 'DOM-exposed TimeRanges interface providing indexed access to buffered/played/seekable media ranges.', tags: ['dom', 'media', 'web-api'], complexity: 'simple' },
  ]
};

// --- touch/mod.rs ---
analysis['components/script/dom/touch/mod.rs'] = {
  summary: 'Barrel module that re-exports Touch and TouchList DOM types for the touch events API.',
  tags: ['dom', 'touch', 'barrel', 'entry-point'],
  complexity: 'simple',
  fnNodes: [],
  classNodes: []
};

// --- touch/touch.rs ---
analysis['components/script/dom/touch/touch.rs'] = {
  summary: 'Implements the Touch Web API representing a single contact point on a touch-sensitive surface, with conversion to pointer events for unified event handling.',
  tags: ['dom', 'touch', 'web-api', 'events', 'pointer'],
  complexity: 'moderate',
  fnNodes: [
    { name: 'new_inherited', lines: [36,57], summary: 'Initializes a Touch object with identifier, target element, and screen/client/page coordinates.', tags: ['constructor', 'init'], complexity: 'moderate' },
    { name: 'new', lines: [60,79], summary: 'Constructs a new Touch DOM object rooted to the global scope.', tags: ['constructor', 'dom'], complexity: 'moderate' },
    { name: 'to_pointer_event', lines: [84,182], summary: 'Converts a Touch event to a PointerEvent with calculated pointer properties, pressure, and modifiers.', tags: ['conversion', 'pointer', 'events'], complexity: 'complex' },
  ],
  classNodes: [
    { name: 'Touch', lines: [22,32], summary: 'DOM Touch interface representing a single touch point on a surface with position tracking.', tags: ['dom', 'touch', 'web-api'], complexity: 'moderate' },
  ]
};

// --- touch/touchlist.rs ---
analysis['components/script/dom/touch/touchlist.rs'] = {
  summary: 'Implements the TouchList Web API providing an ordered list of touch points with indexed access.',
  tags: ['dom', 'touch', 'web-api', 'list'],
  complexity: 'simple',
  fnNodes: [
    { name: 'new', lines: [28,34], summary: 'Constructs a new TouchList DOM object from a vector of Touch objects.', tags: ['constructor', 'dom'], complexity: 'simple' },
  ],
  classNodes: [
    { name: 'TouchList', lines: [15,18], summary: 'DOM TouchList interface providing indexed read access to a collection of touch points.', tags: ['dom', 'touch', 'list'], complexity: 'simple' },
  ]
};

// --- useractivation.rs ---
analysis['components/script/dom/useractivation.rs'] = {
  summary: 'Implements the UserActivation Web API for tracking window-level user activation state with timestamp-based sticky activation support.',
  tags: ['dom', 'user-activation', 'web-api', 'security'],
  complexity: 'moderate',
  fnNodes: [
    { name: 'new', lines: [34,36], summary: 'Constructs a new UserActivation DOM object.', tags: ['constructor', 'dom'], complexity: 'simple' },
    { name: 'handle_user_activation_notification', lines: [39,80], summary: 'Handles a user activation notification by tracking activation state per document with timestamp.', tags: ['handler', 'activation', 'event'], complexity: 'moderate' },
  ],
  classNodes: [
    { name: 'UserActivation', lines: [23,25], summary: 'DOM interface providing hasBeenActive and isActive queries for tracking user interaction state.', tags: ['dom', 'user-activation', 'web-api'], complexity: 'simple' },
    { name: 'UserActivationTimestamp', lines: [102,107], summary: 'Data type tracking activation timestamp with an add operation for merging activation windows.', tags: ['data', 'timestamp', 'activation'], complexity: 'simple' },
  ]
};

// --- userscripts.rs ---
analysis['components/script/dom/userscripts.rs'] = {
  summary: 'Provides user script loading functionality that injects custom CSS and JavaScript from profile- or extension-defined user scripts into document loading.',
  tags: ['dom', 'script', 'user-scripts', 'injection'],
  complexity: 'simple',
  fnNodes: [
    { name: 'load_script', lines: [12,34], summary: 'Injects user script content (CSS/JS) into the document head during page load.', tags: ['script', 'injection', 'loading'], complexity: 'moderate' },
  ],
  classNodes: []
};

// --- values.rs ---
analysis['components/script/dom/values.rs'] = {
  summary: 'Contains a bare DOMTypesValues type alias used across DOM value serialization, providing a unified interface for different value types.',
  tags: ['dom', 'types', 'utility'],
  complexity: 'simple',
  fnNodes: [],
  classNodes: []
};

// --- visualviewport.rs ---
analysis['components/script/dom/visualviewport.rs'] = {
  summary: 'Implements the VisualViewport Web API providing layout viewport dimensions, scroll offsets, and zoom scale with pinch-zoom integration and event dispatching.',
  tags: ['dom', 'viewport', 'visual-viewport', 'layout', 'events'],
  complexity: 'moderate',
  fnNodes: [
    { name: 'new_inherited', lines: [54,65], summary: 'Initializes VisualViewport with window reference, viewport rectangle, and initial scale.', tags: ['constructor', 'init'], complexity: 'moderate' },
    { name: 'new_from_layout_viewport', lines: [69,83], summary: 'Constructs VisualViewport from layout-level viewport dimensions.', tags: ['constructor', 'layout'], complexity: 'moderate' },
    { name: 'update_scale', lines: [88,90], summary: 'Updates the visual viewport scale factor.', tags: ['setter', 'scale'], complexity: 'simple' },
    { name: 'check_for_update', lines: [92,105], summary: 'Compares old and new viewport state to determine if an event should be dispatched.', tags: ['internal', 'update', 'event'], complexity: 'moderate' },
    { name: 'update_from_pinch_zoom_infos', lines: [108,116], summary: 'Updates viewport state from pinch-zoom gesture data.', tags: ['handler', 'zoom', 'event'], complexity: 'moderate' },
    { name: 'handle_scroll_event', lines: [122,130], summary: 'Handles scroll events by queuing a VisualViewport resize/scroll event on the window.', tags: ['handler', 'scroll', 'event'], complexity: 'moderate' },
    { name: 'OffsetLeft', lines: [135,144], summary: 'Returns the left offset of the visual viewport relative to the layout viewport.', tags: ['getter', 'layout'], complexity: 'moderate' },
    { name: 'OffsetTop', lines: [147,156], summary: 'Returns the top offset of the visual viewport relative to the layout viewport.', tags: ['getter', 'layout'], complexity: 'moderate' },
    { name: 'PageLeft', lines: [159,169], summary: 'Returns the page X scroll offset from the visual viewport.', tags: ['getter', 'scroll'], complexity: 'moderate' },
    { name: 'PageTop', lines: [172,182], summary: 'Returns the page Y scroll offset from the visual viewport.', tags: ['getter', 'scroll'], complexity: 'moderate' },
    { name: 'Width', lines: [185,195], summary: 'Returns the visual viewport width from layout data.', tags: ['getter', 'layout'], complexity: 'moderate' },
    { name: 'Height', lines: [198,208], summary: 'Returns the visual viewport height from layout data.', tags: ['getter', 'layout'], complexity: 'moderate' },
    { name: 'Scale', lines: [211,222], summary: 'Returns the current visual viewport zoom scale.', tags: ['getter', 'scale'], complexity: 'moderate' },
  ],
  classNodes: [
    { name: 'VisualViewport', lines: [26,40], summary: 'DOM VisualViewport interface providing layout viewport state, scroll offsets, and zoom tracking with event dispatch.', tags: ['dom', 'viewport', 'web-api'], complexity: 'moderate' },
  ]
};

// --- wakelock/mod.rs ---
analysis['components/script/dom/wakelock/mod.rs'] = {
  summary: 'Barrel module that re-exports WakeLock and WakeLockSentinel DOM types for the Screen Wake Lock API.',
  tags: ['dom', 'wakelock', 'barrel', 'entry-point'],
  complexity: 'simple',
  fnNodes: [],
  classNodes: []
};

// --- wakelock/wakelock.rs ---
analysis['components/script/dom/wakelock/wakelock.rs'] = {
  summary: 'Implements the WakeLock Web API providing navigator.wakeLock.request() with IPC-based lock acquisition and permission handling.',
  tags: ['dom', 'wakelock', 'web-api', 'screen'],
  complexity: 'moderate',
  fnNodes: [
    { name: 'new_inherited', lines: [37,42], summary: 'Initializes a new WakeLock instance.', tags: ['constructor', 'init'], complexity: 'simple' },
    { name: 'new', lines: [44,46], summary: 'Constructs a WakeLock DOM object rooted to the global scope.', tags: ['constructor', 'dom'], complexity: 'simple' },
    { name: 'Request', lines: [51,95], summary: 'Requests a wake lock of the given type via IPC, returning a promise resolved with a WakeLockSentinel.', tags: ['web-api', 'wakelock', 'promise'], complexity: 'moderate' },
    { name: 'handle_response', lines: [100,122], summary: 'Handles the IPC response from the wake lock request, resolving or rejecting the JS promise.', tags: ['handler', 'promise', 'ipc'], complexity: 'moderate' },
  ],
  classNodes: [
    { name: 'WakeLock', lines: [31,34], summary: 'DOM WakeLock interface providing request() for acquiring screen wake locks.', tags: ['dom', 'wakelock', 'web-api'], complexity: 'simple' },
  ]
};

// --- wakelock/wakelocksentinel.rs ---
analysis['components/script/dom/wakelock/wakelocksentinel.rs'] = {
  summary: 'Implements the WakeLockSentinel Web API providing a handle for an acquired wake lock with release tracking and type inspection.',
  tags: ['dom', 'wakelock', 'web-api', 'sentinel'],
  complexity: 'simple',
  fnNodes: [
    { name: 'new_inherited', lines: [26,32], summary: 'Initializes a new WakeLockSentinel with the given lock type.', tags: ['constructor', 'init'], complexity: 'simple' },
    { name: 'new', lines: [34,40], summary: 'Constructs a WakeLockSentinel DOM object.', tags: ['constructor', 'dom'], complexity: 'simple' },
  ],
  classNodes: [
    { name: 'WakeLockSentinel', lines: [19,23], summary: 'DOM WakeLockSentinel interface providing access to the wake lock type and release state.', tags: ['dom', 'wakelock', 'web-api'], complexity: 'simple' },
  ]
};

// --- webcrypto/crypto.rs ---
analysis['components/script/dom/webcrypto/crypto.rs'] = {
  summary: 'Implements the Crypto Web API providing SubtleCrypto access, cryptographically secure random number generation via getRandomValues, and randomUUID generation.',
  tags: ['dom', 'webcrypto', 'crypto', 'random', 'security'],
  complexity: 'moderate',
  fnNodes: [
    { name: 'new', lines: [39,41], summary: 'Constructs a Crypto DOM object.', tags: ['constructor', 'dom'], complexity: 'simple' },
    { name: 'GetRandomValues', lines: [53,82], summary: 'Fills the provided typed array with cryptographically secure random values, throwing on non-integer types.', tags: ['web-api', 'random', 'crypto', 'security'], complexity: 'moderate' },
    { name: 'is_integer_buffer', lines: [94,107], summary: 'Checks whether a typed array is an integer-based buffer type for getRandomValues validation.', tags: ['utility', 'validation', 'type-check'], complexity: 'simple' },
  ],
  classNodes: [
    { name: 'Crypto', lines: [26,29], summary: 'DOM Crypto interface providing SubtleCrypto access, getRandomValues, and randomUUID.', tags: ['dom', 'crypto', 'web-api'], complexity: 'simple' },
  ]
};

// --- webcrypto/cryptokey.rs ---
analysis['components/script/dom/webcrypto/cryptokey.rs'] = {
  summary: 'Implements the CryptoKey Web API with key algorithm metadata, usage tracking, and serialization/deserialization for storage across browsing sessions.',
  tags: ['dom', 'webcrypto', 'cryptokey', 'serialization', 'security'],
  complexity: 'complex',
  fnNodes: [
    { name: 'new_inherited', lines: [112,129], summary: 'Initializes CryptoKey with key type, extractability, algorithm, usages, and backend handle.', tags: ['constructor', 'init'], complexity: 'moderate' },
    { name: 'new', lines: [131,175], summary: 'Constructs a CryptoKey DOM object with full initialization of key properties.', tags: ['constructor', 'dom'], complexity: 'moderate' },
    { name: 'algorithm', lines: [177,179], summary: 'Returns the algorithm identifier string for this key.', tags: ['getter', 'algorithm'], complexity: 'simple' },
    { name: 'usages', lines: [181,183], summary: 'Returns the registered usage flags for this key.', tags: ['getter', 'usages'], complexity: 'simple' },
    { name: 'handle', lines: [185,187], summary: 'Returns the backend-specific key handle.', tags: ['getter', 'handle'], complexity: 'simple' },
    { name: 'set_extractable', lines: [189,191], summary: 'Sets whether the key material can be extracted.', tags: ['setter', 'extractable'], complexity: 'simple' },
    { name: 'set_usages', lines: [193,204], summary: 'Sets the allowed usages for this key with validity checking.', tags: ['setter', 'usages', 'validation'], complexity: 'moderate' },
    { name: 'serialize', lines: [240,261], summary: 'Serializes the CryptoKey to bytes for persistent storage across sessions.', tags: ['serialization', 'persistence'], complexity: 'moderate' },
    { name: 'deserialize', lines: [264,290], summary: 'Deserializes a CryptoKey from previously stored byte representation.', tags: ['deserialization', 'persistence'], complexity: 'moderate' },
    { name: 'as_bytes', lines: [303,311], summary: 'Returns the raw byte representation of the key for serialization.', tags: ['utility', 'bytes'], complexity: 'simple' },
    { name: 'size_of', lines: [315,350], summary: 'Calculates the memory size of the CryptoKey for heap profiling and measurement.', tags: ['utility', 'memory', 'profiling'], complexity: 'moderate' },
    { name: 'try_from (356)', lines: [356,476], summary: 'Converts a raw JWK or key data value into a CryptoKey with algorithm-specific parsing.', tags: ['conversion', 'jwk', 'parsing'], complexity: 'complex' },
    { name: 'try_from (487)', lines: [487,617], summary: 'Converts key data from Web IDL representations into a CryptoKey with validation.', tags: ['conversion', 'validation'], complexity: 'complex' },
  ],
  classNodes: [
    { name: 'CryptoKeyOrCryptoKeyPair', lines: [30,33], summary: 'Enum-like type representing either a single CryptoKey or a key pair for Web Crypto operations.', tags: ['type', 'key-pair'], complexity: 'simple' },
    { name: 'Handle', lines: [40,73], summary: 'Backend-specific key handle wrapping cryptographic key material with algorithm metadata.', tags: ['backend', 'handle', 'crypto'], complexity: 'simple' },
    { name: 'CryptoKey', lines: [77,109], summary: 'DOM CryptoKey interface representing an opaque cryptographic key with type, algorithm, and usage metadata.', tags: ['dom', 'cryptokey', 'web-api'], complexity: 'moderate' },
  ]
};

// --- webcrypto/mod.rs ---
analysis['components/script/dom/webcrypto/mod.rs'] = {
  summary: 'Barrel module that re-exports Crypto, CryptoKey, and SubtleCrypto types for the Web Crypto API.',
  tags: ['dom', 'webcrypto', 'barrel', 'entry-point'],
  complexity: 'simple',
  fnNodes: [],
  classNodes: []
};

// --- webcrypto/subtlecrypto.rs ---
analysis['components/script/dom/webcrypto/subtlecrypto.rs'] = {
  summary: 'Implements the SubtleCrypto Web API with full support for encrypt, decrypt, sign, verify, digest, deriveKey, deriveBits, importKey, exportKey, wrapKey, unwrapKey, encapsulate, decapsulate, and getPublicKey operations across multiple algorithm families (RSA, ECDSA, AES, HMAC, HKDF, PBKDF2, Argon2, KangarooTwelve).',
  tags: ['dom', 'webcrypto', 'subtle-crypto', 'cryptography', 'security'],
  complexity: 'complex',
  fnNodes: [
    { name: 'new', lines: [200,205], summary: 'Constructs a SubtleCrypto DOM object.', tags: ['constructor', 'dom'], complexity: 'simple' },
    { name: 'resolve_promise_with_data', lines: [210,229], summary: 'Resolves a JS promise with binary data using buffer source conversion.', tags: ['promise', 'resolution', 'data'], complexity: 'moderate' },
    { name: 'resolve_promise_with_jwk', lines: [234,272], summary: 'Resolves a JS promise with a JSON Web Key representation.', tags: ['promise', 'jwk', 'resolution'], complexity: 'moderate' },
    { name: 'resolve_promise_with_key', lines: [276,287], summary: 'Resolves a JS promise with a CryptoKey object.', tags: ['promise', 'key', 'resolution'], complexity: 'moderate' },
    { name: 'resolve_promise_with_key_pair', lines: [291,306], summary: 'Resolves a JS promise with a CryptoKeyPair containing public and private keys.', tags: ['promise', 'key-pair', 'resolution'], complexity: 'moderate' },
    { name: 'resolve_promise_with_bool', lines: [310,319], summary: 'Resolves a JS promise with a boolean result.', tags: ['promise', 'resolution', 'boolean'], complexity: 'moderate' },
    { name: 'reject_promise_with_error', lines: [323,332], summary: 'Rejects a JS promise with a DOMException wrapping the given error.', tags: ['promise', 'rejection', 'error'], complexity: 'moderate' },
    { name: 'resolve_promise_with_encapsulated_key', lines: [337,349], summary: 'Resolves a JS promise with an encapsulated key result.', tags: ['promise', 'encapsulation', 'resolution'], complexity: 'moderate' },
    { name: 'resolve_promise_with_encapsulated_bits', lines: [354,366], summary: 'Resolves a JS promise with encapsulated bits data.', tags: ['promise', 'encapsulation', 'resolution'], complexity: 'moderate' },
    { name: 'Encrypt', lines: [371,455], summary: 'Performs symmetric/asymmetric encryption using the specified algorithm and key.', tags: ['web-api', 'encrypt', 'crypto'], complexity: 'complex' },
    { name: 'Decrypt', lines: [458,542], summary: 'Performs symmetric/asymmetric decryption using the specified algorithm and key.', tags: ['web-api', 'decrypt', 'crypto'], complexity: 'complex' },
    { name: 'Sign', lines: [545,628], summary: 'Generates a digital signature over data using the specified algorithm and key.', tags: ['web-api', 'sign', 'crypto'], complexity: 'complex' },
    { name: 'Verify', lines: [631,721], summary: 'Verifies a digital signature against data using the specified algorithm and key.', tags: ['web-api', 'verify', 'crypto'], complexity: 'complex' },
    { name: 'Digest', lines: [724,788], summary: 'Computes a cryptographic digest (hash) of the given data.', tags: ['web-api', 'digest', 'hash'], complexity: 'complex' },
    { name: 'GenerateKey', lines: [791,891], summary: 'Generates a new cryptographic key or key pair for the specified algorithm.', tags: ['web-api', 'key-generation', 'crypto'], complexity: 'complex' },
    { name: 'DeriveKey', lines: [894,1041], summary: 'Derives a new key from a master key using a key derivation algorithm.', tags: ['web-api', 'key-derivation', 'crypto'], complexity: 'complex' },
    { name: 'DeriveBits', lines: [1044,1121], summary: 'Derives raw key bits from a master key using a key derivation algorithm.', tags: ['web-api', 'key-derivation', 'crypto'], complexity: 'complex' },
    { name: 'ImportKey', lines: [1124,1268], summary: 'Imports a key from an external format (raw, JWK, PKCS8, SPKI) into a CryptoKey.', tags: ['web-api', 'key-import', 'crypto'], complexity: 'complex' },
    { name: 'ExportKey', lines: [1271,1352], summary: 'Exports a CryptoKey to an external format (raw, JWK, PKCS8, SPKI).', tags: ['web-api', 'key-export', 'crypto'], complexity: 'complex' },
    { name: 'WrapKey', lines: [1355,1526], summary: 'Wraps (encrypts) a CryptoKey for secure export using a wrapping key.', tags: ['web-api', 'key-wrapping', 'crypto'], complexity: 'complex' },
    { name: 'UnwrapKey', lines: [1529,1713], summary: 'Unwraps (decrypts) a previously wrapped CryptoKey.', tags: ['web-api', 'key-unwrapping', 'crypto'], complexity: 'complex' },
    { name: 'EncapsulateKey', lines: [1716,1859], summary: 'Performs key encapsulation to establish a shared key.', tags: ['web-api', 'encapsulation', 'crypto'], complexity: 'complex' },
    { name: 'EncapsulateBits', lines: [1862,1947], summary: 'Performs key encapsulation returning raw bits.', tags: ['web-api', 'encapsulation', 'crypto'], complexity: 'complex' },
    { name: 'DecapsulateKey', lines: [1950,2089], summary: 'Performs key decapsulation to recover a shared key.', tags: ['web-api', 'decapsulation', 'crypto'], complexity: 'complex' },
    { name: 'DecapsulateBits', lines: [2092,2187], summary: 'Performs key decapsulation recovering raw bits.', tags: ['web-api', 'decapsulation', 'crypto'], complexity: 'complex' },
    { name: 'GetPublicKey', lines: [2190,2284], summary: 'Extracts the public key from a CryptoKeyPair or public CryptoKey.', tags: ['web-api', 'public-key', 'crypto'], complexity: 'complex' },
    { name: 'Supports', lines: [2287,2325], summary: 'Checks whether a given algorithm supports a specific operation with optional key size.', tags: ['web-api', 'support-check'], complexity: 'moderate' },
    { name: 'Supports_', lines: [2328,2418], summary: 'Internal support check dispatching to algorithm-specific support validators.', tags: ['internal', 'support-check'], complexity: 'complex' },
    { name: 'check_support_for_algorithm', lines: [2422,2815], summary: 'Comprehensive algorithm support checker validating all crypto operations against algorithm parameters and key sizes.', tags: ['validation', 'crypto', 'algorithm'], complexity: 'complex' },
    { name: 'normalize_algorithm', lines: [4491,4574], summary: 'Normalizes a Web Crypto algorithm parameter to a canonical representation for internal processing.', tags: ['utility', 'normalization', 'algorithm'], complexity: 'complex' },
  ],
  classNodes: [
    { name: 'CryptoAlgorithm', lines: [92,169], summary: 'Enumeration of all supported Web Crypto algorithm identifiers with case-insensitive parsing.', tags: ['enum', 'algorithm', 'crypto'], complexity: 'moderate' },
    { name: 'SubtleCrypto', lines: [189,191], summary: 'DOM SubtleCrypto interface providing low-level cryptographic operations (encrypt, decrypt, sign, verify, digest, key management).', tags: ['dom', 'subtle-crypto', 'web-api'], complexity: 'moderate' },
    { name: 'SubtleAlgorithm', lines: [2859,2862], summary: 'Trait/struct for algorithms used in cryptographic operations.', tags: ['algorithm', 'trait'], complexity: 'simple' },
    { name: 'SubtleKeyAlgorithm', lines: [2898,2901], summary: 'Algorithm params for key-generation-style operations with key length and algorithm identifier.', tags: ['algorithm', 'key-params'], complexity: 'simple' },
    { name: 'SubtleRsaHashedKeyGenParams', lines: [2932,2944], summary: 'RSA key generation parameters with modulus length, public exponent, and hash algorithm.', tags: ['rsa', 'key-gen', 'params'], complexity: 'simple' },
    { name: 'SubtleRsaHashedKeyAlgorithm', lines: [2978,2990], summary: 'RSA key algorithm descriptor with modulus length, public exponent, and hash.', tags: ['rsa', 'key-algorithm'], complexity: 'simple' },
    { name: 'SubtleRsaHashedImportParams', lines: [3042,3048], summary: 'RSA key import parameters specifying the hash algorithm.', tags: ['rsa', 'import', 'params'], complexity: 'simple' },
    { name: 'SubtleRsaPssParams', lines: [3069,3075], summary: 'RSA-PSS signature parameters with salt length.', tags: ['rsa', 'pss', 'params'], complexity: 'simple' },
    { name: 'SubtleRsaOaepParams', lines: [3099,3105], summary: 'RSA-OAEP encryption parameters with label.', tags: ['rsa', 'oaep', 'params'], complexity: 'simple' },
    { name: 'SubtleEcdsaParams', lines: [3124,3130], summary: 'ECDSA signature parameters specifying the hash algorithm.', tags: ['ecdsa', 'params'], complexity: 'simple' },
    { name: 'SubtleEcKeyGenParams', lines: [3151,3157], summary: 'Elliptic curve key generation parameters specifying the named curve.', tags: ['ec', 'key-gen', 'params'], complexity: 'simple' },
    { name: 'SubtleEcKeyAlgorithm', lines: [3181,3187], summary: 'Elliptic curve key algorithm descriptor with named curve.', tags: ['ec', 'key-algorithm'], complexity: 'simple' },
    { name: 'SubtleEcKeyImportParams', lines: [3224,3230], summary: 'Elliptic curve key import parameters specifying the named curve.', tags: ['ec', 'import', 'params'], complexity: 'simple' },
    { name: 'SubtleEcdhKeyDeriveParams', lines: [3254,3260], summary: 'ECDH key derivation parameters with the peer public key.', tags: ['ecdh', 'derive', 'params'], complexity: 'simple' },
    { name: 'SubtleAesCtrParams', lines: [3281,3290], summary: 'AES-CTR encryption/decryption parameters with counter and length.', tags: ['aes', 'ctr', 'params'], complexity: 'simple' },
    { name: 'SubtleAesKeyAlgorithm', lines: [3315,3321], summary: 'AES key algorithm descriptor with key length.', tags: ['aes', 'key-algorithm'], complexity: 'simple' },
    { name: 'SubtleAesKeyGenParams', lines: [3358,3364], summary: 'AES key generation parameters with key length.', tags: ['aes', 'key-gen', 'params'], complexity: 'simple' },
    { name: 'SubtleAesDerivedKeyParams', lines: [3388,3394], summary: 'AES derived key parameters with key length.', tags: ['aes', 'derive', 'params'], complexity: 'simple' },
    { name: 'SubtleAesCbcParams', lines: [3418,3424], summary: 'AES-CBC encryption/decryption parameters with initialization vector.', tags: ['aes', 'cbc', 'params'], complexity: 'simple' },
    { name: 'SubtleAesGcmParams', lines: [3443,3455], summary: 'AES-GCM encryption/decryption parameters with IV, additional data, and tag length.', tags: ['aes', 'gcm', 'params'], complexity: 'simple' },
    { name: 'SubtleHmacImportParams', lines: [3476,3485], summary: 'HMAC key import parameters with hash algorithm and optional key length.', tags: ['hmac', 'import', 'params'], complexity: 'simple' },
    { name: 'SubtleHmacKeyAlgorithm', lines: [3507,3516], summary: 'HMAC key algorithm descriptor with hash and key length.', tags: ['hmac', 'key-algorithm'], complexity: 'simple' },
    { name: 'SubtleHmacKeyGenParams', lines: [3559,3568], summary: 'HMAC key generation parameters with hash and optional key length.', tags: ['hmac', 'key-gen', 'params'], complexity: 'simple' },
    { name: 'SubtleHkdfParams', lines: [3590,3602], summary: 'HKDF key derivation parameters with salt and info.', tags: ['hkdf', 'derive', 'params'], complexity: 'simple' },
    { name: 'SubtlePbkdf2Params', lines: [3625,3637], summary: 'PBKDF2 key derivation parameters with salt, iterations, and hash.', tags: ['pbkdf2', 'derive', 'params'], complexity: 'simple' },
    { name: 'SubtleArgon2Params', lines: [3902,3926], summary: 'Argon2 key derivation parameters with salt, iterations, memory, parallelism, and key length.', tags: ['argon2', 'derive', 'params'], complexity: 'simple' },
    { name: 'JsonWebKeyExt', lines: [4216,4235], summary: 'Extended JSON Web Key representation with parsing, stringify, usage extraction, and field encoding.', tags: ['jwk', 'serialization', 'parsing'], complexity: 'moderate' },
    { name: 'ExportedKey', lines: [4072,4075], summary: 'Represents an exported key in bytes or JWK format.', tags: ['key', 'export', 'format'], complexity: 'simple' },
    { name: 'KeyAlgorithmAndDerivatives', lines: [4092,4098], summary: 'Combined key algorithm descriptor with supported derived algorithm types.', tags: ['algorithm', 'derivation'], complexity: 'simple' },
    { name: 'EncryptOperation', lines: [4644,4644], summary: 'Marker type for encrypt operation dispatching.', tags: ['operation', 'encrypt'], complexity: 'simple' },
    { name: 'EncryptAlgorithm', lines: [4652,4659], summary: 'Encrypt operation algorithm with key and plaintext processing.', tags: ['operation', 'encrypt', 'algorithm'], complexity: 'moderate' },
    { name: 'DecryptOperation', lines: [4731,4731], summary: 'Marker type for decrypt operation dispatching.', tags: ['operation', 'decrypt'], complexity: 'simple' },
    { name: 'DecryptAlgorithm', lines: [4739,4746], summary: 'Decrypt operation algorithm with key and ciphertext processing.', tags: ['operation', 'decrypt', 'algorithm'], complexity: 'moderate' },
    { name: 'SignOperation', lines: [4818,4818], summary: 'Marker type for sign operation dispatching.', tags: ['operation', 'sign'], complexity: 'simple' },
    { name: 'SignAlgorithm', lines: [4826,4833], summary: 'Sign operation algorithm with key and message processing.', tags: ['operation', 'sign', 'algorithm'], complexity: 'moderate' },
    { name: 'VerifyOperation', lines: [4895,4895], summary: 'Marker type for verify operation dispatching.', tags: ['operation', 'verify'], complexity: 'simple' },
    { name: 'VerifyAlgorithm', lines: [4903,4910], summary: 'Verify operation algorithm with key, message, and signature processing.', tags: ['operation', 'verify', 'algorithm'], complexity: 'moderate' },
    { name: 'DigestOperation', lines: [4980,4980], summary: 'Marker type for digest operation dispatching.', tags: ['operation', 'digest'], complexity: 'simple' },
    { name: 'DigestAlgorithm', lines: [4989,4995], summary: 'Digest operation algorithm with message hashing.', tags: ['operation', 'digest', 'algorithm'], complexity: 'moderate' },
    { name: 'DeriveBitsOperation', lines: [5101,5101], summary: 'Marker type for deriveBits operation dispatching.', tags: ['operation', 'derive'], complexity: 'simple' },
    { name: 'DeriveBitsAlgorithm', lines: [5109,5115], summary: 'DeriveBits operation algorithm with key and length.', tags: ['operation', 'derive', 'algorithm'], complexity: 'moderate' },
    { name: 'WrapKeyOperation', lines: [5180,5180], summary: 'Marker type for wrapKey operation dispatching.', tags: ['operation', 'wrap-key'], complexity: 'simple' },
    { name: 'WrapKeyAlgorithm', lines: [5188,5190], summary: 'WrapKey operation algorithm with key and plaintext wrapping.', tags: ['operation', 'wrap-key', 'algorithm'], complexity: 'simple' },
    { name: 'UnwrapKeyOperation', lines: [5225,5225], summary: 'Marker type for unwrapKey operation dispatching.', tags: ['operation', 'unwrap-key'], complexity: 'simple' },
    { name: 'UnwrapKeyAlgorithm', lines: [5233,5235], summary: 'UnwrapKey operation algorithm with key and ciphertext unwrapping.', tags: ['operation', 'unwrap-key', 'algorithm'], complexity: 'simple' },
    { name: 'GenerateKeyOperation', lines: [5270,5270], summary: 'Marker type for generateKey operation dispatching.', tags: ['operation', 'key-gen'], complexity: 'simple' },
    { name: 'GenerateKeyAlgorithm', lines: [5278,5295], summary: 'GenerateKey operation algorithm with algorithm-specific key generation parameters.', tags: ['operation', 'key-gen', 'algorithm'], complexity: 'moderate' },
    { name: 'ImportKeyOperation', lines: [5467,5467], summary: 'Marker type for importKey operation dispatching.', tags: ['operation', 'key-import'], complexity: 'simple' },
    { name: 'ImportKeyAlgorithm', lines: [5475,5495], summary: 'ImportKey operation algorithm with format and key data parameters.', tags: ['operation', 'key-import', 'algorithm'], complexity: 'moderate' },
    { name: 'ExportKeyOperation', lines: [5731,5731], summary: 'Marker type for exportKey operation dispatching.', tags: ['operation', 'key-export'], complexity: 'simple' },
    { name: 'ExportKeyAlgorithm', lines: [5739,5756], summary: 'ExportKey operation algorithm with format and key parameters.', tags: ['operation', 'key-export', 'algorithm'], complexity: 'moderate' },
    { name: 'GetKeyLengthOperation', lines: [5872,5872], summary: 'Marker type for getKeyLength operation dispatching.', tags: ['operation', 'key-length'], complexity: 'simple' },
    { name: 'GetKeyLengthAlgorithm', lines: [5880,5891], summary: 'GetKeyLength operation algorithm with key length validation.', tags: ['operation', 'key-length', 'algorithm'], complexity: 'moderate' },
    { name: 'EncapsulateOperation', lines: [5983,5983], summary: 'Marker type for encapsulate operation dispatching.', tags: ['operation', 'encapsulate'], complexity: 'simple' },
    { name: 'EncapsulateAlgorithm', lines: [5991,5993], summary: 'Encapsulate operation algorithm.', tags: ['operation', 'encapsulate', 'algorithm'], complexity: 'simple' },
    { name: 'DecapsulateOperation', lines: [6030,6030], summary: 'Marker type for decapsulate operation dispatching.', tags: ['operation', 'decapsulate'], complexity: 'simple' },
    { name: 'DecapsulateAlgorithm', lines: [6038,6040], summary: 'Decapsulate operation algorithm.', tags: ['operation', 'decapsulate', 'algorithm'], complexity: 'simple' },
    { name: 'GetPublicKeyOperation', lines: [6079,6079], summary: 'Marker type for getPublicKey operation dispatching.', tags: ['operation', 'public-key'], complexity: 'simple' },
    { name: 'GetPublicKeyAlgorithm', lines: [6087,6097], summary: 'GetPublicKey operation algorithm with usage parameters.', tags: ['operation', 'public-key', 'algorithm'], complexity: 'moderate' },
  ]
};

// --- websocket.rs ---
analysis['components/script/dom/websocket.rs'] = {
  summary: 'Implements the WebSocket Web API for bi-directional real-time communication, supporting connection establishment, message framing, binary types, and closure negotiation.',
  tags: ['dom', 'websocket', 'web-api', 'networking', 'real-time'],
  complexity: 'complex',
  fnNodes: [
    { name: 'close_the_websocket_connection', lines: [81,93], summary: 'Initiates WebSocket connection close with optional code and reason.', tags: ['websocket', 'close', 'connection'], complexity: 'moderate' },
    { name: 'new_inherited', lines: [119,130], summary: 'Initializes WebSocket with URL and IPC callback channel.', tags: ['constructor', 'init'], complexity: 'moderate' },
    { name: 'new', lines: [132,149], summary: 'Constructs a new WebSocket DOM object with URL and sender channel.', tags: ['constructor', 'dom'], complexity: 'moderate' },
    { name: 'send_impl', lines: [152,183], summary: 'Internal send implementation handling data length tracking and backpressure.', tags: ['internal', 'send', 'websocket'], complexity: 'moderate' },
    { name: 'origin', lines: [185,187], summary: 'Returns the WebSocket connection origin string.', tags: ['getter', 'origin', 'websocket'], complexity: 'simple' },
    { name: 'make_disappear', lines: [191,195], summary: 'Cleans up the WebSocket connection state for garbage collection.', tags: ['cleanup', 'gc'], complexity: 'simple' },
    { name: 'Constructor', lines: [200,339], summary: 'WebSocket constructor implementing URL parsing, protocol negotiation, origin determination, and IPC connection initiation.', tags: ['constructor', 'websocket', 'connection'], complexity: 'complex' },
    { name: 'Send', lines: [384,395], summary: 'Sends a string message through the WebSocket connection.', tags: ['web-api', 'send', 'message'], complexity: 'moderate' },
    { name: 'Send_', lines: [398,414], summary: 'Sends a Blob payload through the WebSocket connection.', tags: ['web-api', 'send', 'blob'], complexity: 'moderate' },
    { name: 'Send__', lines: [417,428], summary: 'Sends an ArrayBuffer payload through the WebSocket connection.', tags: ['web-api', 'send', 'arraybuffer'], complexity: 'moderate' },
    { name: 'Send___', lines: [431,442], summary: 'Sends an ArrayBufferView payload through the WebSocket connection.', tags: ['web-api', 'send', 'buffer-view'], complexity: 'moderate' },
    { name: 'Close', lines: [445,486], summary: 'Initiates WebSocket close handshake with optional status code and reason, dispatching close task.', tags: ['web-api', 'close', 'websocket'], complexity: 'complex' },
    { name: 'run_once (connection)', lines: [510,526], summary: 'Handles WebSocket connection established event, updating ready state and dispatching open event.', tags: ['handler', 'connection', 'event'], complexity: 'moderate' },
    { name: 'run_once (close)', lines: [555,589], summary: 'Handles WebSocket close event with code, reason, and wasClean tracking.', tags: ['handler', 'close', 'event'], complexity: 'complex' },
    { name: 'run_once (message)', lines: [599,661], summary: 'Handles received WebSocket message data, converting to appropriate binary/text format and dispatching message event.', tags: ['handler', 'message', 'event'], complexity: 'complex' },
  ],
  classNodes: [
    { name: 'WebSocketRequestState', lines: [56,61], summary: 'Tracks the pending IPC request state for the WebSocket connection.', tags: ['state', 'request', 'websocket'], complexity: 'simple' },
    { name: 'WebSocket', lines: [105,116], summary: 'DOM WebSocket interface providing bi-directional real-time communication over TCP.', tags: ['dom', 'websocket', 'web-api'], complexity: 'complex' },
    { name: 'ReportCSPViolationTask', lines: [489,492], summary: 'Event loop task that reports a CSP violation for WebSocket connections.', tags: ['task', 'csp', 'security'], complexity: 'simple' },
    { name: 'ConnectionEstablishedTask', lines: [503,506], summary: 'Event loop task that handles the WebSocket connection established event.', tags: ['task', 'connection'], complexity: 'simple' },
    { name: 'BufferedAmountTask', lines: [529,531], summary: 'Event loop task that updates the buffered amount for backpressure tracking.', tags: ['task', 'buffering'], complexity: 'simple' },
    { name: 'CloseTask', lines: [547,552], summary: 'Event loop task that handles WebSocket close with code, reason, and wasClean state.', tags: ['task', 'close'], complexity: 'simple' },
    { name: 'MessageReceivedTask', lines: [592,595], summary: 'Event loop task that handles incoming WebSocket message data.', tags: ['task', 'message'], complexity: 'simple' },
  ]
};

// --- xmldocument.rs ---
analysis['components/script/dom/xmldocument.rs'] = {
  summary: 'Implements the XMLDocument Web API as a specialized Document subclass for XML content, supporting location access and named property resolution.',
  tags: ['dom', 'xml', 'document', 'web-api'],
  complexity: 'moderate',
  fnNodes: [
    { name: 'new_inherited', lines: [40,81], summary: 'Initializes XMLDocument with full document creation parameters including browsing context, URL, origin, content type, and custom element support.', tags: ['constructor', 'init', 'document'], complexity: 'complex' },
    { name: 'new', lines: [84,126], summary: 'Constructs a new XMLDocument DOM object with comprehensive document creation parameters.', tags: ['constructor', 'dom', 'document'], complexity: 'complex' },
  ],
  classNodes: [
    { name: 'XMLDocument', lines: [34,36], summary: 'DOM XMLDocument interface extending Document for XML content with location and named property access.', tags: ['dom', 'xml', 'document'], complexity: 'simple' },
  ]
};

// --- xmlhttprequest/mod.rs ---
analysis['components/script/dom/xmlhttprequest/mod.rs'] = {
  summary: 'Barrel module that re-exports XMLHttpRequest, XMLHttpRequestEventTarget, and XMLHttpRequestUpload types.',
  tags: ['dom', 'xmlhttprequest', 'barrel', 'entry-point'],
  complexity: 'simple',
  fnNodes: [],
  classNodes: []
};

// --- xmlhttprequest/xmlhttprequest.rs ---
analysis['components/script/dom/xmlhttprequest/xmlhttprequest.rs'] = {
  summary: 'Implements the XMLHttpRequest Web API with full lifecycle management including synchronous/asynchronous requests, header manipulation, response type switching, timeout control, progress events, and CORS-ready fetch integration.',
  tags: ['dom', 'xmlhttprequest', 'web-api', 'networking', 'ajax'],
  complexity: 'complex',
  fnNodes: [
    { name: 'process_response', lines: [112,123], summary: 'Processes HTTP response metadata from the fetch subsystem.', tags: ['handler', 'response', 'fetch'], complexity: 'moderate' },
    { name: 'process_response_chunk', lines: [125,134], summary: 'Processes an incoming response data chunk during streaming.', tags: ['handler', 'response', 'streaming'], complexity: 'moderate' },
    { name: 'process_response_eof', lines: [136,150], summary: 'Handles response completion with final data and timing info.', tags: ['handler', 'response', 'completion'], complexity: 'moderate' },
    { name: 'new_inherited', lines: [243,278], summary: 'Initializes XHR with default state, headers, and IPC channels.', tags: ['constructor', 'init'], complexity: 'moderate' },
    { name: 'new', lines: [280,291], summary: 'Constructs a new XMLHttpRequest DOM object.', tags: ['constructor', 'dom'], complexity: 'moderate' },
    { name: 'Open_', lines: [332,444], summary: 'Implements the open() method with method/URL validation, async flag, username/password, and fetch init.', tags: ['web-api', 'open', 'request'], complexity: 'complex' },
    { name: 'SetRequestHeader', lines: [447,494], summary: 'Sets an HTTP request header with validation against forbidden headers.', tags: ['web-api', 'headers', 'request'], complexity: 'moderate' },
    { name: 'SetTimeout', lines: [502,526], summary: 'Sets the request timeout in milliseconds, configuring the XHR timeout callback.', tags: ['web-api', 'timeout', 'config'], complexity: 'moderate' },
    { name: 'SetWithCredentials', lines: [534,548], summary: 'Enables or disables cross-site Access-Control credentials.', tags: ['web-api', 'cors', 'credentials'], complexity: 'moderate' },
    { name: 'Send', lines: [556,803], summary: 'Sends the HTTP request with body data, managing sync/async modes, CORS preflight, and fetch pipeline integration.', tags: ['web-api', 'send', 'request'], complexity: 'complex' },
    { name: 'Abort', lines: [806,831], summary: 'Aborts the ongoing request, canceling timeout and fetch, dispatching abort event.', tags: ['web-api', 'abort', 'cancel'], complexity: 'moderate' },
    { name: 'GetResponseHeader', lines: [849,870], summary: 'Returns the value of a specific response header, filtering for CORS-safe headers.', tags: ['web-api', 'headers', 'response'], complexity: 'moderate' },
    { name: 'GetAllResponseHeaders', lines: [873,893], summary: 'Returns all response headers as a string, filtered for CORS safety.', tags: ['web-api', 'headers', 'response'], complexity: 'moderate' },
    { name: 'OverrideMimeType', lines: [896,918], summary: 'Overrides the MIME type returned by the server for response processing.', tags: ['web-api', 'mime', 'override'], complexity: 'moderate' },
    { name: 'SetResponseType', lines: [926,949], summary: 'Sets the response type (text, arraybuffer, blob, document, json) with state validation.', tags: ['web-api', 'response-type', 'config'], complexity: 'moderate' },
    { name: 'Response', lines: [952,988], summary: 'Returns the response body in the configured response type format.', tags: ['web-api', 'response', 'body'], complexity: 'complex' },
    { name: 'GetResponseText', lines: [991,1006], summary: 'Returns the response body as text, handling binary vs text mode.', tags: ['web-api', 'response', 'text'], complexity: 'moderate' },
    { name: 'GetResponseXML', lines: [1009,1026], summary: 'Returns the response body parsed as XML/HTML document.', tags: ['web-api', 'response', 'xml'], complexity: 'moderate' },
    { name: 'change_ready_state', lines: [1032,1045], summary: 'Updates the XHR ready state and dispatches the readystatechange event.', tags: ['internal', 'state', 'event'], complexity: 'moderate' },
    { name: 'process_headers_available', lines: [1047,1084], summary: 'Processes response headers when they become available, updating state and dispatching progress events.', tags: ['handler', 'headers', 'response'], complexity: 'complex' },
    { name: 'process_response_complete', lines: [1095,1114], summary: 'Completes response processing, calculating stats and dispatching load event.', tags: ['handler', 'response', 'completion'], complexity: 'moderate' },
    { name: 'process_partial_response', lines: [1116,1264], summary: 'Processes partial response data, handling streaming, timeout races, and document content type detection.', tags: ['handler', 'response', 'streaming'], complexity: 'complex' },
    { name: 'dispatch_progress_event', lines: [1273,1306], summary: 'Dispatches a progress event with loaded and total byte counts.', tags: ['internal', 'event', 'progress'], complexity: 'moderate' },
    { name: 'dispatch_upload_progress_event', lines: [1308,1326], summary: 'Dispatches upload-specific progress events to the upload object.', tags: ['internal', 'event', 'upload'], complexity: 'moderate' },
    { name: 'set_timeout', lines: [1338,1347], summary: 'Configures the XHR timeout timer via IPC.', tags: ['internal', 'timeout', 'timer'], complexity: 'moderate' },
    { name: 'text_response', lines: [1356,1367], summary: 'Decodes the response body as text using the determined charset.', tags: ['internal', 'response', 'decoding'], complexity: 'moderate' },
    { name: 'blob_response', lines: [1370,1383], summary: 'Wraps the response body as a Blob object.', tags: ['internal', 'response', 'blob'], complexity: 'moderate' },
    { name: 'arraybuffer_response', lines: [1386,1401], summary: 'Wraps the response body as an ArrayBuffer.', tags: ['internal', 'response', 'arraybuffer'], complexity: 'moderate' },
    { name: 'document_response', lines: [1404,1484], summary: 'Parses the response body as an HTML or XML document with charset detection.', tags: ['internal', 'response', 'document'], complexity: 'complex' },
    { name: 'json_response', lines: [1488,1521], summary: 'Parses the response body as JSON.', tags: ['internal', 'response', 'json'], complexity: 'moderate' },
    { name: 'document_text_html', lines: [1523,1539], summary: 'Parses response as HTML document for text/html content type.', tags: ['internal', 'html', 'parsing'], complexity: 'moderate' },
    { name: 'handle_xml', lines: [1541,1556], summary: 'Parses response as XML document.', tags: ['internal', 'xml', 'parsing'], complexity: 'moderate' },
    { name: 'new_doc', lines: [1558,1589], summary: 'Creates a new document for XHR response parsing with specified type.', tags: ['internal', 'document', 'creation'], complexity: 'moderate' },
    { name: 'fetch', lines: [1604,1655], summary: 'Initiates the actual HTTP fetch via the fetch subsystem with CORS and authentication.', tags: ['internal', 'fetch', 'network'], complexity: 'complex' },
    { name: 'final_charset', lines: [1658,1682], summary: 'Determines the final charset for response text decoding from headers, BOM, or sniffing.', tags: ['internal', 'charset', 'detection'], complexity: 'moderate' },
    { name: 'is_field_value', lines: [1737,1792], summary: 'Validates HTTP field value syntax per RFC 7230 for header parsing.', tags: ['utility', 'validation', 'http'], complexity: 'moderate' },
    { name: 'serialize_document', lines: [1723,1733], summary: 'Serializes a document to a string for response text conversion.', tags: ['internal', 'serialization'], complexity: 'moderate' },
    { name: 'invoke', lines: [1712,1720], summary: 'Invokes the timeout callback when the XHR timeout fires.', tags: ['handler', 'timeout'], complexity: 'simple' },
  ],
  classNodes: [
    { name: 'GenerationId', lines: [96,96], summary: 'Newtype wrapper for tracking XHR generation to discard stale responses.', tags: ['type', 'id', 'tracking'], complexity: 'simple' },
    { name: 'XHRProgress', lines: [173,182], summary: 'Tracks partial HTTP response download progress for progress events.', tags: ['data', 'progress'], complexity: 'simple' },
    { name: 'XHRContext', lines: [100,105], summary: 'HTTP response context handler implementing fetch response processing callbacks.', tags: ['handler', 'fetch', 'context'], complexity: 'moderate' },
    { name: 'XMLHttpRequest', lines: [196,240], summary: 'DOM XMLHttpRequest interface providing HTTP request/response lifecycle management, event dispatch, and response type conversion.', tags: ['dom', 'xmlhttprequest', 'web-api'], complexity: 'complex' },
    { name: 'XHRTimeoutCallback', lines: [1705,1709], summary: 'Timeout task that fires when the XHR request exceeds the configured timeout duration.', tags: ['task', 'timeout'], complexity: 'simple' },
  ]
};

// --- xmlhttprequest/xmlhttprequesteventtarget.rs ---
analysis['components/script/dom/xmlhttprequest/xmlhttprequesteventtarget.rs'] = {
  summary: 'Implements the XMLHttpRequestEventTarget base class providing event target functionality for XHR upload/download progress events.',
  tags: ['dom', 'xmlhttprequest', 'event-target', 'web-api'],
  complexity: 'simple',
  fnNodes: [],
  classNodes: [
    { name: 'XMLHttpRequestEventTarget', lines: [11,13], summary: 'DOM EventTarget base class for XMLHttpRequest upload/download event handling.', tags: ['dom', 'event-target', 'xmlhttprequest'], complexity: 'simple' },
  ]
};

// --- xmlhttprequest/xmlhttprequestupload.rs ---
analysis['components/script/dom/xmlhttprequest/xmlhttprequestupload.rs'] = {
  summary: 'Implements the XMLHttpRequestUpload interface for tracking upload-specific progress events during XHR requests.',
  tags: ['dom', 'xmlhttprequest', 'upload', 'web-api'],
  complexity: 'simple',
  fnNodes: [
    { name: 'new', lines: [24,30], summary: 'Constructs an XMLHttpRequestUpload DOM object.', tags: ['constructor', 'dom'], complexity: 'simple' },
  ],
  classNodes: [
    { name: 'XMLHttpRequestUpload', lines: [14,16], summary: 'DOM XMLHttpRequestUpload interface for upload progress event tracking.', tags: ['dom', 'upload', 'xmlhttprequest'], complexity: 'simple' },
  ]
};

// --- xmlserializer.rs ---
analysis['components/script/dom/xmlserializer.rs'] = {
  summary: 'Implements the XMLSerializer Web API for serializing DOM nodes to XML string representation.',
  tags: ['dom', 'xml', 'serializer', 'web-api'],
  complexity: 'simple',
  fnNodes: [
    { name: 'new', lines: [33,44], summary: 'Constructs an XMLSerializer DOM object.', tags: ['constructor', 'dom'], complexity: 'moderate' },
    { name: 'SerializeToString', lines: [58,72], summary: 'Serializes a DOM node to its XML string representation.', tags: ['web-api', 'serialization', 'xml'], complexity: 'moderate' },
  ],
  classNodes: [
    { name: 'XMLSerializer', lines: [20,23], summary: 'DOM XMLSerializer interface providing XML string serialization of DOM nodes.', tags: ['dom', 'xml', 'serializer'], complexity: 'simple' },
  ]
};

// --- xpath/mod.rs ---
analysis['components/script/dom/xpath/mod.rs'] = {
  summary: 'Barrel module that re-exports XPathEvaluator, XPathExpression, and XPathResult types for the XPath API.',
  tags: ['dom', 'xpath', 'barrel', 'entry-point'],
  complexity: 'simple',
  fnNodes: [],
  classNodes: []
};

// --- xpath/xpathevaluator.rs ---
analysis['components/script/dom/xpath/xpathevaluator.rs'] = {
  summary: 'Implements the XPathEvaluator Web API for creating and evaluating XPath expressions against XML documents.',
  tags: ['dom', 'xpath', 'evaluator', 'web-api'],
  complexity: 'moderate',
  fnNodes: [
    { name: 'new', lines: [38,49], summary: 'Constructs an XPathEvaluator DOM object.', tags: ['constructor', 'dom'], complexity: 'moderate' },
    { name: 'CreateExpression', lines: [63,81], summary: 'Parses and creates a compiled XPathExpression from a string with optional namespace resolver.', tags: ['web-api', 'xpath', 'expression'], complexity: 'moderate' },
    { name: 'Evaluate', lines: [90,107], summary: 'Evaluates an XPath expression string against a context node with result type and optional result wrapper.', tags: ['web-api', 'xpath', 'evaluate'], complexity: 'moderate' },
  ],
  classNodes: [
    { name: 'XPathEvaluator', lines: [25,28], summary: 'DOM XPathEvaluator interface for creating and evaluating XPath expressions.', tags: ['dom', 'xpath', 'web-api'], complexity: 'simple' },
  ]
};

// --- xpath/xpathexpression.rs ---
analysis['components/script/dom/xpath/xpathexpression.rs'] = {
  summary: 'Implements the XPathExpression Web API representing a compiled XPath expression ready for evaluation.',
  tags: ['dom', 'xpath', 'expression', 'web-api'],
  complexity: 'moderate',
  fnNodes: [
    { name: 'new', lines: [38,50], summary: 'Constructs an XPathExpression DOM object from a parsed expression.', tags: ['constructor', 'dom'], complexity: 'moderate' },
    { name: 'evaluate_internal', lines: [52,122], summary: 'Internal XPath evaluation with context node, result type coercion, and XPathResult wrapping.', tags: ['internal', 'xpath', 'evaluate'], complexity: 'complex' },
  ],
  classNodes: [
    { name: 'XPathExpression', lines: [22,27], summary: 'DOM XPathExpression interface for compiled XPath queries with evaluation support.', tags: ['dom', 'xpath', 'expression'], complexity: 'simple' },
  ]
};

// --- xpath/xpathresult.rs ---
analysis['components/script/dom/xpath/xpathresult.rs'] = {
  summary: 'Implements the XPathResult Web API providing typed XPath query results with iterator, snapshot, and single-node result access patterns.',
  tags: ['dom', 'xpath', 'result', 'web-api'],
  complexity: 'moderate',
  fnNodes: [
    { name: 'try_from (result type)', lines: [42,56], summary: 'Converts a numeric XPath result type to the XPathResultType enum.', tags: ['conversion', 'type'], complexity: 'moderate' },
    { name: 'from (result value)', lines: [70,79], summary: 'Creates an XPathResultValue from a generic result node value.', tags: ['conversion', 'value'], complexity: 'moderate' },
    { name: 'new_inherited', lines: [95,113], summary: 'Initializes XPathResult with type, value, and document snapshot tracking.', tags: ['constructor', 'init'], complexity: 'moderate' },
    { name: 'new', lines: [127,140], summary: 'Constructs an XPathResult DOM object.', tags: ['constructor', 'dom'], complexity: 'moderate' },
    { name: 'reinitialize_with', lines: [142,152], summary: 'Reinitializes the result with new type and value for result reuse.', tags: ['dom', 'reinitialize'], complexity: 'moderate' },
    { name: 'IterateNext', lines: [192,218], summary: 'Returns the next node in an iterated result set with document change detection.', tags: ['web-api', 'iteration'], complexity: 'moderate' },
    { name: 'GetSnapshotLength', lines: [231,241], summary: 'Returns the number of nodes in a snapshot result.', tags: ['web-api', 'snapshot'], complexity: 'moderate' },
    { name: 'SnapshotItem', lines: [244,254], summary: 'Returns the node at the given index in a snapshot result.', tags: ['web-api', 'snapshot'], complexity: 'moderate' },
    { name: 'GetSingleNodeValue', lines: [257,267], summary: 'Returns the single node result or first matching node.', tags: ['web-api', 'single-node'], complexity: 'moderate' },
  ],
  classNodes: [
    { name: 'XPathResultType', lines: [26,37], summary: 'Enumeration of XPath result type constants (ANY_TYPE, NUMBER_TYPE, STRING_TYPE, etc.).', tags: ['enum', 'type', 'xpath'], complexity: 'simple' },
    { name: 'XPathResultValue', lines: [60,67], summary: 'Wrapper type holding XPath result values (number, string, boolean, node, node-set).', tags: ['enum', 'value', 'xpath'], complexity: 'simple' },
    { name: 'XPathResult', lines: [83,92], summary: 'DOM XPathResult interface for accessing evaluated XPath query results with iterator, snapshot, and single-value modes.', tags: ['dom', 'xpath', 'result'], complexity: 'moderate' },
  ]
};

// ============================================================
// Build all nodes
// ============================================================

const nodes = [];
const edges = [];
const fileNodeIds = new Set();

// Process each file
for (const result of DATA.results) {
  const { path, fileCategory, totalLines, nonEmptyLines, metrics } = result;
  const a = analysis[path];
  if (!a) {
    console.error(`Missing analysis for ${path}`);
    continue;
  }

  // Determine node type
  let nodeType = 'file';
  // All files are 'code' category, so type is 'file'

  const fileId = `file:${path}`;
  fileNodeIds.add(fileId);

  // File node
  nodes.push({
    id: fileId,
    type: nodeType,
    name: path.split('/').pop(),
    filePath: path,
    summary: a.summary,
    tags: a.tags,
    complexity: a.complexity,
  });

  // Function nodes
  for (const fn of a.fnNodes) {
    const fnId = `function:${path}:${fn.name}`;
    nodes.push({
      id: fnId,
      type: 'function',
      name: fn.name,
      filePath: path,
      lineRange: fn.lines,
      summary: fn.summary,
      tags: fn.tags,
      complexity: fn.complexity,
    });
    // contains edge
    edges.push({
      source: fileId,
      target: fnId,
      type: 'contains',
      direction: 'forward',
      weight: 1.0,
    });
  }

  // Class nodes
  for (const cls of a.classNodes) {
    const clsId = `class:${path}:${cls.name}`;
    nodes.push({
      id: clsId,
      type: 'class',
      name: cls.name,
      filePath: path,
      lineRange: cls.lines,
      summary: cls.summary,
      tags: cls.tags,
      complexity: cls.complexity,
    });
    // contains edge
    edges.push({
      source: fileId,
      target: clsId,
      type: 'contains',
      direction: 'forward',
      weight: 1.0,
    });
  }

  // Exports edges - find exported items among fnNodes and classNodes
  for (const exp of (result.exports || [])) {
    // Try to match against fnNodes (handle names with parentheses, like "handle (boolean)")
    const fnMatch = a.fnNodes.find(fn => fn.name === exp.name || fn.name.startsWith(exp.name + ' ('));
    if (fnMatch) {
      const fnId = `function:${path}:${fnMatch.name}`;
      edges.push({
        source: fileId,
        target: fnId,
        type: 'exports',
        direction: 'forward',
        weight: 0.8,
      });
      continue;
    }
    // Try to match against classNodes
    const clsMatch = a.classNodes.find(cls => cls.name === exp.name);
    if (clsMatch) {
      const clsId = `class:${path}:${clsMatch.name}`;
      edges.push({
        source: fileId,
        target: clsId,
        type: 'exports',
        direction: 'forward',
        weight: 0.8,
      });
    }
  }
}

// ============================================================
// Import edges from batchImportData
// ============================================================

// Read the input JSON for import data
const inputData = JSON.parse(readFileSync('d:/Projects/servo/.understand-anything/tmp/ua-file-analyzer-input-9.json', 'utf8'));
const importData = inputData.batchImportData;

for (const [sourcePath, targets] of Object.entries(importData)) {
  const sourceId = `file:${sourcePath}`;
  for (const targetPath of targets) {
    const targetId = `file:${targetPath}`;
    edges.push({
      source: sourceId,
      target: targetId,
      type: 'imports',
      direction: 'forward',
      weight: 0.7,
    });
  }
}

// ============================================================
// Ensure all nodes have at least 3 tags
// ============================================================

const tagFallbacks = {
  'constructor': 'dom',
  'getter': 'web-api',
  'setter': 'web-api',
  'handler': 'internal',
  'internal': 'dom',
  'init': 'dom',
  'storage': 'web-api',
  'selection': 'editing',
  'dom': 'web-api',
  'web-api': 'dom',
  'barrel': 'module',
  'entry-point': 'module',
  'serialization': 'persistence',
  'promise': 'async',
  'ipc': 'communication',
  'timeout': 'timer',
  'task': 'event-loop',
};

for (const n of nodes) {
  while (n.tags.length < 3) {
    // Add a fallback tag based on existing tags
    let added = false;
    for (const t of n.tags) {
      if (tagFallbacks[t] && !n.tags.includes(tagFallbacks[t])) {
        n.tags.push(tagFallbacks[t]);
        added = true;
        break;
      }
    }
    if (!added) {
      // Generic fallbacks by node type
      if (n.type === 'function') n.tags.push('utility');
      else if (n.type === 'class') n.tags.push('type');
      else n.tags.push('misc');
    }
  }
}

// ============================================================
// De-duplicate edges
// ============================================================

const edgeSet = new Set();
const dedupedEdges = [];
for (const e of edges) {
  const key = `${e.source}|${e.target}|${e.type}`;
  if (!edgeSet.has(key)) {
    edgeSet.add(key);
    dedupedEdges.push(e);
  }
}

console.log(`Total nodes: ${nodes.length}`);
console.log(`Total edges: ${dedupedEdges.length}`);

// ============================================================
// Write output (check if we need multi-part)
// ============================================================

const nodeCount = nodes.length;
const edgeCount = dedupedEdges.length;

// Sort files in batch alphabetically
const batchFiles = [...DATA.results].sort((a, b) => a.path.localeCompare(b.path));

if (nodeCount <= 60 && edgeCount <= 120) {
  // Single file
  const outPath = `${OUT_DIR}/batch-${BATCH_INDEX}.json`;
  writeFileSync(outPath, JSON.stringify({ nodes, edges: dedupedEdges }, null, 2));
  console.log(`Written single file: ${outPath}`);
} else {
  // Multi-part
  const parts = Math.ceil(Math.max(nodeCount / 60, edgeCount / 120));
  console.log(`Splitting into ${parts} parts`);

  // Chunk files into parts
  const filesPerPart = Math.ceil(batchFiles.length / parts);

  for (let partIdx = 0; partIdx < parts; partIdx++) {
    const partStart = partIdx * filesPerPart;
    const partEnd = Math.min(partStart + filesPerPart, batchFiles.length);
    const partFiles = batchFiles.slice(partStart, partEnd);
    const partFilePaths = new Set(partFiles.map(f => f.path));

    // Collect nodes for files in this part
    const partNodes = [];
    const partNodeIds = new Set();

    for (const n of nodes) {
      if (n.filePath && partFilePaths.has(n.filePath)) {
        partNodes.push(n);
        partNodeIds.add(n.id);
      }
    }

    // Collect edges where source is in this part's nodes
    const partEdges = dedupedEdges.filter(e => partNodeIds.has(e.source));

    const partOutPath = `${OUT_DIR}/batch-${BATCH_INDEX}-part-${partIdx + 1}.json`;
    writeFileSync(partOutPath, JSON.stringify({ nodes: partNodes, edges: partEdges }, null, 2));
    console.log(`Written part ${partIdx + 1}: ${partOutPath} (${partNodes.length} nodes, ${partEdges.length} edges)`);
  }
}
