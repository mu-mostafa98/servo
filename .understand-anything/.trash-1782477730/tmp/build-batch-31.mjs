import fs from 'fs';

const results = JSON.parse(fs.readFileSync('./ua-file-extract-results-31.json', 'utf-8')).results;
const importData = JSON.parse(fs.readFileSync('./ua-file-analyzer-input-31.json', 'utf-8')).batchImportData;

const nodes = [];
const edges = [];

function addNode(id, type, name, filePath, summary, tags, complexity, extra) {
    const node = { id, type, name, filePath, summary, tags, complexity };
    if (extra) Object.assign(node, extra);
    nodes.push(node);
}
function addEdge(source, target, type, weight) {
    edges.push({ source, target, type, direction: 'forward', weight });
}
function isSignificantFunction(f, exports) {
    const lineCount = f.endLine - f.startLine + 1;
    const isExported = exports.some(e => e.name === f.name);
    return isExported || lineCount >= 10;
}
function isSignificantClass(c, exports) {
    const isExported = exports.some(e => e.name === c.name);
    return isExported || c.methods.length >= 2 || (c.endLine - c.startLine + 1) >= 20;
}

const fileMeta = {
    'ports/servoshell/desktop/tracing.rs': { summary: 'Provides winit event logging infrastructure via the trace_winit_event macro and LogTarget trait for categorizing winit events by type for trace-level logging.', tags: ['logging', 'events', 'desktop'], complexity: 'moderate' },
    'ports/servoshell/desktop/webxr.rs': { summary: 'Implements WebXR discovery and registration for desktop platforms, supporting both GL window and OpenXR backends with preference-driven mock testing.', tags: ['webxr', 'virtual-reality', 'desktop'], complexity: 'moderate' },
    'ports/servoshell/egl/android/mod.rs': { summary: 'Android JNI bridge providing the native interface between Java (ServoView) and the Servo engine, handling initialization, touch events, keyboard input, pinch zoom, media session, and lifecycle management.', tags: ['android', 'jni', 'platform-bridge', 'input-handling'], complexity: 'complex' },
    'ports/servoshell/egl/app.rs': { summary: 'Core EGL application state manager defining the App and EmbeddedPlatformWindow structs for platform-embedded rendering, handling webview management, input events, media session, and window lifecycle.', tags: ['application', 'rendering', 'platform-layer'], complexity: 'complex' },
    'ports/servoshell/egl/host_trait.rs': { summary: 'Defines the HostTrait interface that embedders must implement to receive callbacks from Servo, including alerts, load status, title/URL changes, IME events, media session, and select element prompts.', tags: ['trait', 'interface', 'embedder-api'], complexity: 'simple' },
    'ports/servoshell/egl/log.rs': { summary: 'Redirects stdout and stderr to the logging system via a pipe-based mechanism for environments like Android where console output is not available, running a dedicated reader thread.', tags: ['logging', 'stdout-redirect', 'pipe'], complexity: 'moderate' },
    'ports/servoshell/egl/mod.rs': { summary: 'Barrel module that re-exports the EGL submodules for Android and OpenHarmony, along with shared app, host_trait, and log modules.', tags: ['barrel', 'module', 'egl'], complexity: 'simple' },
    'ports/servoshell/egl/ohos/mod.rs': { summary: 'OpenHarmony (OHOS) native bridge providing the interface between ArkUI/JS and the Servo engine, handling surface management, touch/key input, IME, vsync, and lifecycle callbacks.', tags: ['ohos', 'openharmony', 'platform-bridge', 'input-handling'], complexity: 'complex' },
    'ports/servoshell/egl/ohos/resources.rs': { summary: 'Implements a ResourceReader for OpenHarmony that resolves resource directory paths and reads bundled resource files from the OHOS filesystem.', tags: ['resources', 'ohos', 'file-reading'], complexity: 'simple' },
    'ports/servoshell/lib.rs': { summary: 'Main library entry point for servoshell, conditionally composing platform modules, defining the init_crypto and init_tracing setup functions, and implementing the HitraceLayer for tracing-subscriber on OHOS.', tags: ['entry-point', 'library-root', 'platform-init'], complexity: 'moderate' },
    'ports/servoshell/panic_hook.rs': { summary: 'Custom panic hook that formats panic messages with thread info and location, optionally triggers a segfault for hard-fail crash detection in tests.', tags: ['panic', 'error-handling', 'crash-detection'], complexity: 'simple' },
    'ports/servoshell/parser.rs': { summary: 'URL parsing utilities for resolving command-line URLs, file paths, domain names, and search queries into ServoUrl values with fallback to homepage or about:blank.', tags: ['url-parsing', 'input-processing', 'utility'], complexity: 'moderate' },
    'ports/servoshell/prefs.rs': { summary: 'Comprehensive preferences and command-line argument parsing system defining ServoShellPreferences, CmdArgs, and experimental feature flags with bpaf-based CLI option definitions.', tags: ['configuration', 'cli', 'argument-parsing', 'preferences'], complexity: 'complex' },
    'ports/servoshell/resources/mod.rs': { summary: 'Resolves resource directory paths for the servoshell application, locating resource files relative to the executable path and handling bundled deployment scenarios.', tags: ['resources', 'path-resolution'], complexity: 'simple' },
    'ports/servoshell/running_app_state.rs': { summary: 'Central application state manager for servoshell, coordinating windows, webviews, webdriver integration, gamepad input, accessibility, and embedder control callbacks across the application lifecycle.', tags: ['state-management', 'application-core', 'event-dispatch'], complexity: 'complex' },
    'ports/servoshell/test.rs': { summary: 'Test suite for URL parsing, command-line argument processing, and file path resolution, verifying the parser module behavior across multiple input scenarios.', tags: ['test', 'url-parsing', 'validation'], complexity: 'moderate' },
    'ports/servoshell/webdriver.rs': { summary: 'WebDriver embedder control implementation handling dialog management, script commands, screenshots, and navigation, bridging WebDriver protocol messages to the Servo engine.', tags: ['webdriver', 'automation', 'embedder-control'], complexity: 'moderate' },
    'ports/servoshell/window.rs': { summary: 'Window management layer defining ServoShellWindow and PlatformWindow abstractions for webview lifecycle, input dispatch, embedder controls, and platform-window rendering coordination.', tags: ['window-management', 'webview', 'platform-interface'], complexity: 'complex' }
};

// File nodes
for (const r of results) {
    const meta = fileMeta[r.path];
    const fname = r.path.split('/').pop();
    addNode('file:' + r.path, 'file', fname, r.path, meta.summary, meta.tags, meta.complexity);
}

// === Sub-nodes by file ===

// 1. desktop/tracing.rs
addNode('class:ports/servoshell/desktop/tracing.rs:LogTarget', 'class', 'LogTarget', 'ports/servoshell/desktop/tracing.rs',
    'Trait providing a log_target method that returns a static string categorizing winit event types for trace-level logging.',
    ['trait', 'logging', 'winit'], 'simple', { lineRange: [28, 30] });
addEdge('file:ports/servoshell/desktop/tracing.rs', 'class:ports/servoshell/desktop/tracing.rs:LogTarget', 'contains', 1.0);
addEdge('file:ports/servoshell/desktop/tracing.rs', 'class:ports/servoshell/desktop/tracing.rs:LogTarget', 'exports', 0.8);

// 2. desktop/webxr.rs
addNode('function:ports/servoshell/desktop/webxr.rs:new_boxed', 'function', 'new_boxed', 'ports/servoshell/desktop/webxr.rs',
    'Creates a boxed XrDiscoveryWebXrRegistry, initializing XR discovery backend (OpenXR or GLWindow) based on preferences.',
    ['webxr', 'factory', 'initialization'], 'moderate', { lineRange: [29, 58] });
addNode('function:ports/servoshell/desktop/webxr.rs:register', 'function', 'register', 'ports/servoshell/desktop/webxr.rs',
    'Registers XR discovery backends (mock, GLWindow, OpenXR) with the MainThreadRegistry and sets up preference observers.',
    ['webxr', 'registration'], 'simple', { lineRange: [75, 89] });
addNode('class:ports/servoshell/desktop/webxr.rs:XrDiscoveryWebXrRegistry', 'class', 'XrDiscoveryWebXrRegistry', 'ports/servoshell/desktop/webxr.rs',
    'Holds an optional XrDiscovery instance and implements WebXrRegistry for registering XR backends.',
    ['webxr', 'registry'], 'simple', { lineRange: [24, 26] });
for (const n of ['function:ports/servoshell/desktop/webxr.rs:new_boxed', 'function:ports/servoshell/desktop/webxr.rs:register', 'class:ports/servoshell/desktop/webxr.rs:XrDiscoveryWebXrRegistry']) {
    addEdge('file:ports/servoshell/desktop/webxr.rs', n, 'contains', 1.0);
}
addEdge('file:ports/servoshell/desktop/webxr.rs', 'function:ports/servoshell/desktop/webxr.rs:new_boxed', 'exports', 0.8);
addEdge('file:ports/servoshell/desktop/webxr.rs', 'class:ports/servoshell/desktop/webxr.rs:XrDiscoveryWebXrRegistry', 'exports', 0.8);

// 3. egl/host_trait.rs
addNode('class:ports/servoshell/egl/host_trait.rs:HostTrait', 'class', 'HostTrait', 'ports/servoshell/egl/host_trait.rs',
    'Trait defining the embedder callback interface for Servo events including alerts, load status, title/URL changes, IME, media session, and select element prompts.',
    ['trait', 'embedder-api', 'interface'], 'moderate', { lineRange: [8, 45] });
addEdge('file:ports/servoshell/egl/host_trait.rs', 'class:ports/servoshell/egl/host_trait.rs:HostTrait', 'contains', 1.0);
addEdge('file:ports/servoshell/egl/host_trait.rs', 'class:ports/servoshell/egl/host_trait.rs:HostTrait', 'exports', 0.8);

// 4. egl/log.rs
addNode('function:ports/servoshell/egl/log.rs:redirect_stdout_and_stderr', 'function', 'redirect_stdout_and_stderr', 'ports/servoshell/egl/log.rs',
    'Redirects stdout and stderr to the logging system by creating a pipe and spawning a reader thread that forwards output to the log framework.',
    ['logging', 'stdout-redirect', 'pipe', 'thread'], 'moderate', { lineRange: [20, 104] });
addEdge('file:ports/servoshell/egl/log.rs', 'function:ports/servoshell/egl/log.rs:redirect_stdout_and_stderr', 'contains', 1.0);
addEdge('file:ports/servoshell/egl/log.rs', 'function:ports/servoshell/egl/log.rs:redirect_stdout_and_stderr', 'exports', 0.8);

// 5. lib.rs
addNode('function:ports/servoshell/lib.rs:init_tracing', 'function', 'init_tracing', 'ports/servoshell/lib.rs',
    'Initializes the tracing subscriber with optional Perfetto and Hitrace layers, environment-filter-based filtering, and a startup timing event.',
    ['tracing', 'initialization', 'profiling'], 'moderate', { lineRange: [50, 114] });
addNode('function:ports/servoshell/lib.rs:init_crypto', 'function', 'init_crypto', 'ports/servoshell/lib.rs',
    'Initializes the AWS-LC Rust crypto provider as the default TLS implementation.',
    ['crypto', 'initialization', 'tls'], 'simple', { lineRange: [44, 48] });
addNode('function:ports/servoshell/lib.rs:main', 'function', 'main', 'ports/servoshell/lib.rs',
    'Entry point function that delegates to desktop::cli::main for non-embedded platforms.',
    ['entry-point', 'desktop'], 'simple', { lineRange: [40, 42] });
addEdge('file:ports/servoshell/lib.rs', 'function:ports/servoshell/lib.rs:init_tracing', 'contains', 1.0);
addEdge('file:ports/servoshell/lib.rs', 'function:ports/servoshell/lib.rs:init_crypto', 'contains', 1.0);
addEdge('file:ports/servoshell/lib.rs', 'function:ports/servoshell/lib.rs:main', 'contains', 1.0);
addEdge('file:ports/servoshell/lib.rs', 'function:ports/servoshell/lib.rs:init_tracing', 'exports', 0.8);
addEdge('file:ports/servoshell/lib.rs', 'function:ports/servoshell/lib.rs:init_crypto', 'exports', 0.8);

// 6. panic_hook.rs
addNode('function:ports/servoshell/panic_hook.rs:panic_hook', 'function', 'panic_hook', 'ports/servoshell/panic_hook.rs',
    'Custom panic hook that formats panic messages with thread name and source location, optionally triggering a segfault for hard-fail crash test detection.',
    ['panic', 'error-handling', 'crash-detection'], 'moderate', { lineRange: [14, 53] });
addEdge('file:ports/servoshell/panic_hook.rs', 'function:ports/servoshell/panic_hook.rs:panic_hook', 'contains', 1.0);
addEdge('file:ports/servoshell/panic_hook.rs', 'function:ports/servoshell/panic_hook.rs:panic_hook', 'exports', 0.8);

// 7. parser.rs
(function() {
    const r = results.find(rr => rr.path === 'ports/servoshell/parser.rs');
    const ex = r.exports;
    for (const f of r.functions) {
        if (isSignificantFunction(f, ex)) {
            const lc = f.endLine - f.startLine + 1;
            let s = '';
            if (f.name === 'get_default_url') s = 'Resolves a default URL from command-line input, homepage preference, or about:blank.';
            else if (f.name === 'location_bar_input_to_url') s = 'Converts a location bar input to a ServoUrl, trying file, domain, and search page interpretations.';
            else if (f.name === 'parse_url_or_filename') s = 'Parses a string as URL or joins with cwd as a file path.';
            else s = 'URL parsing helper.';
            const nid = 'function:ports/servoshell/parser.rs:' + f.name;
            addNode(nid, 'function', f.name, 'ports/servoshell/parser.rs', s, ['url-parsing', 'utility'], lc > 20 ? 'moderate' : 'simple', { lineRange: [f.startLine, f.endLine] });
            addEdge('file:ports/servoshell/parser.rs', nid, 'contains', 1.0);
            if (ex.some(e => e.name === f.name)) addEdge('file:ports/servoshell/parser.rs', nid, 'exports', 0.8);
        }
    }
})();

// 8. resources/mod.rs
addNode('function:ports/servoshell/resources/mod.rs:resource_root_dir_path', 'function', 'resource_root_dir_path', 'ports/servoshell/resources/mod.rs',
    'Resolves the root directory for Servo resource files by checking the executable path, CARGO_MANIFEST_DIR, and known resource locations.',
    ['resources', 'path-resolution'], 'moderate', { lineRange: [17, 71] });
addEdge('file:ports/servoshell/resources/mod.rs', 'function:ports/servoshell/resources/mod.rs:resource_root_dir_path', 'contains', 1.0);
addEdge('file:ports/servoshell/resources/mod.rs', 'function:ports/servoshell/resources/mod.rs:resource_root_dir_path', 'exports', 0.8);

// 9. prefs.rs
(function() {
    const r = results.find(rr => rr.path === 'ports/servoshell/prefs.rs');
    const ex = r.exports;
    for (const f of r.functions) {
        if (isSignificantFunction(f, ex)) {
            const lc = f.endLine - f.startLine + 1;
            let s = '';
            const name = f.name;
            if (name === 'parse_command_line_arguments') s = 'Entry point for parsing command-line arguments.';
            else if (name === 'parse_arguments_helper') s = 'Parses all command-line arguments via bpaf, building CmdArgs and ServoShellPreferences.';
            else if (name === 'get_preferences') s = 'Loads and merges preferences from files and default config directory.';
            else if (name === 'default') s = 'Returns default ServoShellPreferences.';
            else if (name === 'update_preferences_from_command_line_arguments') s = 'Applies CLI argument values to the Servo preferences system.';
            else if (name === 'parse_diagnostics_logging') s = 'Parses diagnostics logging CLI options.';
            else if (name === 'parse_resolution_string') s = 'Parses a resolution string like 800x600.';
            else if (name === 'flag_with_default_parser') s = 'Generic preference flag parser with transformations and defaults.';
            else if (name === 'parse_user_stylesheets') s = 'Parses user stylesheet paths from CLI arguments.';
            else if (name === 'read_prefs_map') s = 'Reads a JSON preferences map and merges it.';
            else if (name.startsWith('test_')) s = 'Test function for preferences parsing.';
            else s = 'Preferences function.';
            const nid = 'function:ports/servoshell/prefs.rs:' + f.name + ':' + f.startLine;
            addNode(nid, 'function', f.name, 'ports/servoshell/prefs.rs', s, ['configuration', 'cli', 'preferences'], lc > 40 ? 'complex' : lc > 15 ? 'moderate' : 'simple', { lineRange: [f.startLine, f.endLine] });
            addEdge('file:ports/servoshell/prefs.rs', nid, 'contains', 1.0);
            if (ex.some(e => e.name === f.name && e.line === f.startLine)) addEdge('file:ports/servoshell/prefs.rs', nid, 'exports', 0.8);
        }
    }
    for (const c of r.classes) {
        if (isSignificantClass(c, ex) || (c.endLine - c.startLine + 1) > 20) {
            const nid = 'class:ports/servoshell/prefs.rs:' + c.name;
            const s = c.name === 'ServoShellPreferences' ? 'Struct holding all servoshell user preferences including URL, window size, and experimental feature flags.' :
                c.name === 'CmdArgs' ? 'Struct defining all command-line argument fields parsed via bpaf.' :
                c.name === 'ArgumentParsingResult' ? 'Enum for CLI parsing result (Chrome, Content, Exit, Error).' : 'Preferences struct.';
            addNode(nid, 'class', c.name, 'ports/servoshell/prefs.rs', s, ['configuration', 'struct'], (c.endLine - c.startLine + 1) > 50 ? 'complex' : 'moderate', { lineRange: [c.startLine, c.endLine] });
            addEdge('file:ports/servoshell/prefs.rs', nid, 'contains', 1.0);
            if (ex.some(e => e.name === c.name)) addEdge('file:ports/servoshell/prefs.rs', nid, 'exports', 0.8);
        }
    }
})();

// 10. test.rs
(function() {
    const r = results.find(rr => rr.path === 'ports/servoshell/test.rs');
    for (const f of r.functions) {
        if (isSignificantFunction(f, [])) {
            const lc = f.endLine - f.startLine + 1;
            const nid = 'function:ports/servoshell/test.rs:' + f.name + ':' + f.startLine;
            let s = 'Test case for URL parsing and CLI argument processing.';
            if (f.name === 'test_cmdline_and_location_bar_url') s = 'Tests URL resolution from combined command-line and location bar input.';
            else if (f.name === 'test_issue_35754') s = 'Regression test for issue #35754 URL parsing edge cases.';
            else if (f.name === 'test_url') s = 'Helper test for URL parsing with expected output validation.';
            addNode(nid, 'function', f.name, 'ports/servoshell/test.rs', s, ['test', 'url-parsing'], lc > 20 ? 'moderate' : 'simple', { lineRange: [f.startLine, f.endLine] });
            addEdge('file:ports/servoshell/test.rs', nid, 'contains', 1.0);
        }
    }
    addEdge('file:ports/servoshell/test.rs', 'file:ports/servoshell/parser.rs', 'tested_by', 0.5);
})();

// 11. webdriver.rs
(function() {
    const r = results.find(rr => rr.path === 'ports/servoshell/webdriver.rs');
    const ex = r.exports;
    for (const f of r.functions) {
        if (isSignificantFunction(f, ex)) {
            const lc = f.endLine - f.startLine + 1;
            let s = '';
            if (f.name === 'handle_webdriver_messages') s = 'Main WebDriver message loop handling navigation, scripts, screenshots, dialogs, and input.';
            else if (f.name === 'current_active_dialog_webdriver_type') s = 'Determines the active dialog type (alert/confirm/prompt) for WebDriver.';
            else if (f.name === 'respond_to_active_simple_dialog') s = 'Accepts or dismisses the active dialog for WebDriver.';
            else if (f.name === 'set_prompt_value_of_newest_dialog') s = 'Sets prompt dialog text for WebDriver.';
            else s = 'WebDriver method.';
            const nid = 'function:ports/servoshell/webdriver.rs:' + f.name;
            addNode(nid, 'function', f.name, 'ports/servoshell/webdriver.rs', s, ['webdriver', 'automation'], lc > 50 ? 'complex' : lc > 15 ? 'moderate' : 'simple', { lineRange: [f.startLine, f.endLine] });
            addEdge('file:ports/servoshell/webdriver.rs', nid, 'contains', 1.0);
            if (ex.some(e => e.name === f.name)) addEdge('file:ports/servoshell/webdriver.rs', nid, 'exports', 0.8);
        }
    }
})();

// 12. window.rs
(function() {
    const r = results.find(rr => rr.path === 'ports/servoshell/window.rs');
    const ex = r.exports;
    for (const f of r.functions) {
        if (isSignificantFunction(f, ex)) {
            const lc = f.endLine - f.startLine + 1;
            let s = '';
            if (f.name === 'create_toplevel_webview') s = 'Creates a top-level webview, registering with Servo and the webview collection.';
            else if (f.name === 'handle_interface_commands') s = 'Processes queued UI commands (navigation, close, new tab).';
            else if (f.name === 'repaint_webviews') s = 'Requests repaint for all webviews.';
            else if (f.name === 'update_and_request_repaint_if_necessary') s = 'Updates webview state and requests repaint if needed.';
            else if (f.name === 'close_webview') s = 'Closes a webview by ID, removes from collection, manages transitions.';
            else if (f.name === 'new' && f.startLine === 71) s = 'Creates a new ServoShellWindow wrapping a platform window.';
            else s = 'Window management method.';
            const nid = 'function:ports/servoshell/window.rs:' + f.name + ':' + f.startLine;
            addNode(nid, 'function', f.name, 'ports/servoshell/window.rs', s, ['window-management', 'webview'], lc > 30 ? 'moderate' : 'simple', { lineRange: [f.startLine, f.endLine] });
            addEdge('file:ports/servoshell/window.rs', nid, 'contains', 1.0);
            if (ex.some(e => e.name === f.name && e.line === f.startLine)) addEdge('file:ports/servoshell/window.rs', nid, 'exports', 0.8);
        }
    }
    for (const c of r.classes) {
        if (isSignificantClass(c, ex)) {
            const nid = 'class:ports/servoshell/window.rs:' + c.name;
            const s = c.name === 'ServoShellWindow' ? 'Window struct managing webview collection, platform window, scheduling, and embedder controls.' :
                c.name === 'PlatformWindow' ? 'Trait defining platform window interface for rendering, input, HIDPI, and accessibility.' : 'Window struct.';
            addNode(nid, 'class', c.name, 'ports/servoshell/window.rs', s, ['window-management'], c.methods.length > 10 ? 'complex' : 'moderate', { lineRange: [c.startLine, c.endLine] });
            addEdge('file:ports/servoshell/window.rs', nid, 'contains', 1.0);
            if (ex.some(e => e.name === c.name)) addEdge('file:ports/servoshell/window.rs', nid, 'exports', 0.8);
        }
    }
})();

// 13. egl/android/mod.rs
(function() {
    const r = results.find(rr => rr.path === 'ports/servoshell/egl/android/mod.rs');
    const ex = r.exports;
    for (const f of r.functions) {
        if (isSignificantFunction(f, ex)) {
            const lc = f.endLine - f.startLine + 1;
            const name = f.name;
            let s = '';
            if (name === 'Java_org_servo_servoview_JNIServo_init') s = 'Initializes Servo from Java, setting up logging, CLI args, preferences, and window handles.';
            else if (name.startsWith('Java_org_servo_servoview_JNIServo_')) {
                const action = name.replace('Java_org_servo_servoview_JNIServo_', '');
                s = 'JNI bridge handling ' + action + ' from the Java layer.';
            } else if (name === 'android_main') s = 'No-op C entry for Android compatibility with winit/android-activity.';
            else if (name === 'get_options') s = 'Parses JNI init options from Java objects (args, URL, viewport, density, surface).';
            else if (name === 'show_alert') s = 'Dispatches alert dialog to Java via JNI callbacks.';
            else if (name === 'notify_load_status_changed') s = 'Notifies Java about page load status changes.';
            else if (name.startsWith('on_')) s = 'Notifies Java about ' + name.replace('on_', '').replace(/_/g, ' ') + '.';
            else if (name === 'try_from') s = 'Converts Android keycode to NamedKey.';
            else if (name === 'from') s = 'Converts Android keycode to Key.';
            else if (name === 'new_string_as_jvalue') s = 'Creates a JNI JValue from a Rust string.';
            else if (name === 'jni_coordinate_to_rust_viewport_rect') s = 'Converts JNI coordinates to Rust viewport rect.';
            else if (name === 'get_field_as_string') s = 'Extracts a string field from a JNI Java object.';
            else if (name === 'display_and_window_handle') s = 'Extracts display/window handles from Android surface.';
            else s = 'Android JNI helper.';
            const nid = 'function:ports/servoshell/egl/android/mod.rs:' + f.name;
            addNode(nid, 'function', f.name, 'ports/servoshell/egl/android/mod.rs', s,
                f.name.startsWith('Java_org_servo_servoview_JNIServo_') ? ['jni', 'android', 'bridge'] : ['android', 'helper'],
                lc > 50 ? 'complex' : lc > 15 ? 'moderate' : 'simple', { lineRange: [f.startLine, f.endLine] });
            addEdge('file:ports/servoshell/egl/android/mod.rs', nid, 'contains', 1.0);
            if (ex.some(e => e.name === f.name)) addEdge('file:ports/servoshell/egl/android/mod.rs', nid, 'exports', 0.8);
        }
    }
    for (const c of r.classes) {
        if (isSignificantClass(c, ex)) {
            const nid = 'class:ports/servoshell/egl/android/mod.rs:' + c.name;
            const s = c.name === 'HostCallbacks' ? 'Host callback forwarding embedder notifications to Android Java via JNI.' :
                c.name === 'WakeupCallback' ? 'Event loop waker using JNI to wake the Android UI thread.' : 'Android JNI struct.';
            addNode(nid, 'class', c.name, 'ports/servoshell/egl/android/mod.rs', s, ['android', 'jni'], c.methods.length > 5 ? 'moderate' : 'simple', { lineRange: [c.startLine, c.endLine] });
            addEdge('file:ports/servoshell/egl/android/mod.rs', nid, 'contains', 1.0);
            if (ex.some(e => e.name === c.name)) addEdge('file:ports/servoshell/egl/android/mod.rs', nid, 'exports', 0.8);
        }
    }
})();

// 14. egl/app.rs
(function() {
    const r = results.find(rr => rr.path === 'ports/servoshell/egl/app.rs');
    const ex = r.exports;
    for (const f of r.functions) {
        if (isSignificantFunction(f, ex)) {
            const lc = f.endLine - f.startLine + 1;
            const name = f.name;
            let s = '';
            if (name === 'update_user_interface_state') s = 'Updates platform window title, URL, back/forward state, and load status.';
            else if (name === 'new' && f.startLine === 308) s = 'Creates a new App with given init options.';
            else if (name === 'add_platform_window') s = 'Adds a platform window with display/window handles and viewport config.';
            else if (name === 'observe_next_frame') s = 'Sets up next-vsync frame callback.';
            else if (name === 'show_embedder_control') s = 'Shows an embedder control on the platform window.';
            else if (name === 'hide_embedder_control') s = 'Hides an embedder control from the platform window.';
            else if (name === 'notify_media_session_event') s = 'Notifies the platform window about media session events.';
            else if (name === 'resize') s = 'Resizes the platform window viewport.';
            else if (name === 'ime_insert_text') s = 'Handles IME text composition.';
            else if (name === 'resume_painting') s = 'Resumes painting, re-establishing window/display handles.';
            else if (name.startsWith('touch_')) s = 'Handles touch event on the platform window.';
            else if (name.startsWith('mouse_')) s = 'Handles mouse event on the platform window.';
            else if (name === 'pause_painting') s = 'Pauses painting on the platform window.';
            else if (name.startsWith('pinchzoom_')) s = 'Handles pinch zoom gesture.';
            else s = 'Platform method.';
            const nid = 'function:ports/servoshell/egl/app.rs:' + f.name + ':' + f.startLine;
            addNode(nid, 'function', f.name, 'ports/servoshell/egl/app.rs', s, ['egl', 'platform-window'], lc > 50 ? 'complex' : lc > 15 ? 'moderate' : 'simple', { lineRange: [f.startLine, f.endLine] });
            addEdge('file:ports/servoshell/egl/app.rs', nid, 'contains', 1.0);
            if (ex.some(e => e.name === f.name && e.line === f.startLine)) addEdge('file:ports/servoshell/egl/app.rs', nid, 'exports', 0.8);
        }
    }
    for (const c of r.classes) {
        if (isSignificantClass(c, ex)) {
            const nid = 'class:ports/servoshell/egl/app.rs:' + c.name;
            const s = c.name === 'EmbeddedPlatformWindow' ? 'Platform window for embedded environments managing rendering context, input, and viewport.' :
                c.name === 'App' ? 'Core application struct for EGL platforms.' :
                c.name === 'AppInitOptions' ? 'App creation options for EGL platforms.' :
                c.name === 'VsyncRefreshDriver' ? 'VSync-driven frame callback coordinator.' : 'EGL struct.';
            addNode(nid, 'class', c.name, 'ports/servoshell/egl/app.rs', s, ['egl', 'platform-window'], c.methods.length > 10 ? 'complex' : 'moderate', { lineRange: [c.startLine, c.endLine] });
            addEdge('file:ports/servoshell/egl/app.rs', nid, 'contains', 1.0);
            if (ex.some(e => e.name === c.name)) addEdge('file:ports/servoshell/egl/app.rs', nid, 'exports', 0.8);
        }
    }
})();

// 15. running_app_state.rs
(function() {
    const r = results.find(rr => rr.path === 'ports/servoshell/running_app_state.rs');
    const ex = r.exports;
    for (const f of r.functions) {
        if (isSignificantFunction(f, ex)) {
            const lc = f.endLine - f.startLine + 1;
            const name = f.name;
            let s = '';
            if (name === 'new' && f.startLine === 229) s = 'Initializes RunningAppState with servo, prefs, event loop waker, and user content manager.';
            else if (name === 'open_window') s = 'Opens a new window with platform window and initial URL.';
            else if (name === 'spin_event_loop') s = 'Main event loop iteration processing webdriver events and Servo updates.';
            else if (name === 'schedule_exit') s = 'Schedules graceful application exit.';
            else if (name === 'maybe_request_screenshot') s = 'Requests screenshot if pending conditions are met.';
            else if (name.startsWith('handle_webdriver')) s = 'Processes WebDriver request.';
            else if (name === 'request_create_new') s = 'Creates a new webview in a new window (e.g. target=_blank).';
            else if (name === 'close_empty_windows') s = 'Closes windows without webviews.';
            else if (name === 'handle_gamepad_events') s = 'Polls and dispatches gamepad events.';
            else if (name === 'set_accessibility_active') s = 'Enables/disables accessibility updates.';
            else if (name === 'interrupt_webdriver_script_evaluation') s = 'Interrupts running WebDriver script.';
            else if (name === 'show_embedder_control') s = 'Displays embedder control in the right platform window.';
            else if (name.startsWith('notify_')) s = 'Notification callback from Servo.';
            else s = 'App state method.';
            const nid = 'function:ports/servoshell/running_app_state.rs:' + f.name + ':' + f.startLine;
            addNode(nid, 'function', f.name, 'ports/servoshell/running_app_state.rs', s, ['state-management', 'application-core'], lc > 40 ? 'complex' : lc > 15 ? 'moderate' : 'simple', { lineRange: [f.startLine, f.endLine] });
            addEdge('file:ports/servoshell/running_app_state.rs', nid, 'contains', 1.0);
            if (ex.some(e => e.name === f.name && e.line === f.startLine)) addEdge('file:ports/servoshell/running_app_state.rs', nid, 'exports', 0.8);
        }
    }
    for (const c of r.classes) {
        if (isSignificantClass(c, ex)) {
            const nid = 'class:ports/servoshell/running_app_state.rs:' + c.name;
            const s = c.name === 'RunningAppState' ? 'Central state coordinating windows, webviews, webdriver, gamepad, accessibility.' :
                c.name === 'WebViewCollection' ? 'Webview collection with ordering, activation, and lookup.' : 'State struct.';
            addNode(nid, 'class', c.name, 'ports/servoshell/running_app_state.rs', s, ['state-management'], c.methods.length > 10 ? 'complex' : 'moderate', { lineRange: [c.startLine, c.endLine] });
            addEdge('file:ports/servoshell/running_app_state.rs', nid, 'contains', 1.0);
            if (ex.some(e => e.name === c.name)) addEdge('file:ports/servoshell/running_app_state.rs', nid, 'exports', 0.8);
        }
    }
})();

// 16. egl/ohos/mod.rs
(function() {
    const r = results.find(rr => rr.path === 'ports/servoshell/egl/ohos/mod.rs');
    const ex = r.exports;
    for (const f of r.functions) {
        if (isSignificantFunction(f, ex)) {
            const lc = f.endLine - f.startLine + 1;
            const name = f.name;
            let s = '';
            if (name === 'init_app') s = 'Initializes Servo on OHOS with event loop, prefs, and window system.';
            else if (name === 'do_action') s = 'Dispatches ServoAction (navigation, touch, key, IME, resize) to Servo.';
            else if (name === 'on_dispatch_touch_event_cb') s = 'OHOS touch event callback, converting coordinates and dispatching.';
            else if (name === 'on_dispatch_key_event') s = 'OHOS key event callback, converting keycodes and dispatching.';
            else if (name === 'on_surface_created_cb') s = 'OHOS surface creation callback, setting up rendering window.';
            else if (name === 'on_surface_changed_cb') s = 'OHOS surface resize callback.';
            else if (name === 'register_xcomponent_callbacks') s = 'Registers OHOS XComponent native callbacks.';
            else if (name === 'initialize_logging_once') s = 'Initializes OHOS logging with configurable filters.';
            else if (name === 'get_xcomponent_offset') s = 'Retrieves XComponent offset relative to window.';
            else if (name === 'get_xcomponent_size') s = 'Retrieves XComponent dimensions.';
            else if (name === 'convert_ime_options') s = 'Converts OHOS IME options to Servo InputMethodControl.';
            else if (name === 'show_alert') s = 'Shows alert dialog via OHOS JS callback.';
            else if (name === 'notify_load_status_changed') s = 'Notifies OHOS JS about load status.';
            else if (name === 'on_ime_show') s = 'Shows IME keyboard on OHOS.';
            else if (name === 'on_ime_hide') s = 'Hides IME keyboard on OHOS.';
            else if (name === 'on_url_changed') s = 'Notifies OHOS JS about URL changes.';
            else if (name === 'init') s = 'Initializes OHOS NAPI exports.';
            else if (name === 'register_url_callback') s = 'Registers JS URL change callback.';
            else if (name === 'register_terminate_callback') s = 'Registers JS termination callback.';
            else if (name === 'register_prompt_toast_callback') s = 'Registers JS prompt/toast callback.';
            else if (name === 'init_servo') s = 'Initializes Servo engine on OHOS.';
            else if (name === 'main_thread') s = 'OHOS main thread entry creating app and starting event loop.';
            else if (name === 'get_raw_window_handle') s = 'Extracts native window handle from XComponent.';
            else if (name === 'get_native_values') s = 'Retrieves OHOS system values (cache, density, device info).';
            else if (name === 'set_log_filter') s = 'Sets OHOS log filter.';
            else if (name === 'on_vsync_cb') s = 'VSync callback notifying Servo refresh driver.';
            else if (name === 'request_vsync_callback') s = 'Requests OHOS native vsync callback.';
            else if (name === 'try_create_ime_proxy') s = 'Creates OHOS IME proxy for text input.';
            else if (name === 'dispatch_touch_event') s = 'Dispatches touch event to Servo.';
            else if (name === 'fmt') s = 'Formats ServoAction for display.';
            else if (name === 'focus_webview') s = 'Focuses a webview by index.';
            else if (name === 'delete_webview') s = 'Deletes a webview by index.';
            else if (name === 'init_servo') s = 'Initializes the Servo engine.';
            else s = 'OHOS bridge function.';
            const nid = 'function:ports/servoshell/egl/ohos/mod.rs:' + f.name;
            addNode(nid, 'function', f.name, 'ports/servoshell/egl/ohos/mod.rs', s, ['ohos', 'openharmony', 'bridge'], lc > 50 ? 'complex' : lc > 15 ? 'moderate' : 'simple', { lineRange: [f.startLine, f.endLine] });
            addEdge('file:ports/servoshell/egl/ohos/mod.rs', nid, 'contains', 1.0);
            if (ex.some(e => e.name === f.name)) addEdge('file:ports/servoshell/egl/ohos/mod.rs', nid, 'exports', 0.8);
        }
    }
    for (const c of r.classes) {
        if (isSignificantClass(c, ex)) {
            const nid = 'class:ports/servoshell/egl/ohos/mod.rs:' + c.name;
            const s = c.name === 'ServoAction' ? 'Enum of actions dispatched to the OHOS Servo bridge.' :
                c.name === 'HostCallbacks' ? 'OHOS host callback forwarding to JS layer.' :
                c.name === 'WakeupCallback' ? 'Channel-based event loop waker for OHOS.' :
                c.name === 'ServoIme' ? 'OHOS IME interface.' : 'OHOS struct.';
            addNode(nid, 'class', c.name, 'ports/servoshell/egl/ohos/mod.rs', s, ['ohos', 'bridge'], c.methods.length > 5 ? 'moderate' : 'simple', { lineRange: [c.startLine, c.endLine] });
            addEdge('file:ports/servoshell/egl/ohos/mod.rs', nid, 'contains', 1.0);
            if (ex.some(e => e.name === c.name)) addEdge('file:ports/servoshell/egl/ohos/mod.rs', nid, 'exports', 0.8);
        }
    }
})();

// 17. egl/ohos/resources.rs
addNode('function:ports/servoshell/egl/ohos/resources.rs:read', 'function', 'read', 'ports/servoshell/egl/ohos/resources.rs',
    'Reads a resource file from the OHOS filesystem given a resource path.',
    ['resources', 'ohos', 'file-reading'], 'simple', { lineRange: [27, 37] });
addEdge('file:ports/servoshell/egl/ohos/resources.rs', 'function:ports/servoshell/egl/ohos/resources.rs:read', 'contains', 1.0);
addEdge('file:ports/servoshell/egl/ohos/resources.rs', 'function:ports/servoshell/egl/ohos/resources.rs:read', 'exports', 0.8);

// Import edges
for (const [filePath, imports] of Object.entries(importData)) {
    for (const imp of imports) {
        addEdge('file:' + filePath, 'file:' + imp, 'imports', 0.7);
    }
}

// Output stats
const importEdges = edges.filter(e => e.type === 'imports');
console.log('Import edges:', importEdges.length, '(expected:', Object.values(importData).flat().length + ')');
console.log('Total nodes:', nodes.length);
console.log('Total edges:', edges.length);

const outPath = 'd:/Projects/servo/.understand-anything/intermediate/batch-31.json';
fs.writeFileSync(outPath, JSON.stringify({ nodes, edges }, null, 2));
console.log('Written to', outPath);
