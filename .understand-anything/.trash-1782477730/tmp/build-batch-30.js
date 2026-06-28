const fs = require('fs');
const results = JSON.parse(fs.readFileSync('.understand-anything/tmp/ua-file-extract-results-30.json', 'utf8')).results;

const nodes = [];
const edges = [];

function makeNode(id, type, name, filePath, summary, tags, complexity, extra) {
  const n = { id, type, name };
  if (filePath !== undefined) n.filePath = filePath;
  n.summary = summary;
  n.tags = tags;
  n.complexity = complexity;
  if (extra) Object.assign(n, extra);
  return n;
}

function sigFilter(f, r) {
  const nonEmpty = f.endLine - f.startLine + 1;
  if (nonEmpty >= 10) return true;
  if (r.exports && r.exports.some(e => e.name === f.name)) return true;
  return false;
}

function sigClass(c, r) {
  if (c.methods.length >= 2) return true;
  if (r.exports && r.exports.some(e => e.name === c.name)) return true;
  return false;
}

const importData = {
  'ports/servoshell/desktop/mod.rs': [
    'ports/servoshell/desktop/accelerated_gl_media.rs',
    'ports/servoshell/desktop/app.rs',
    'ports/servoshell/desktop/cli.rs',
    'ports/servoshell/desktop/dialog.rs',
    'ports/servoshell/desktop/event_loop.rs',
    'ports/servoshell/desktop/gamepad.rs',
    'ports/servoshell/desktop/geometry.rs',
    'ports/servoshell/desktop/gui.rs',
    'ports/servoshell/desktop/headed_window.rs',
    'ports/servoshell/desktop/headless_window.rs',
    'ports/servoshell/desktop/keyutils.rs',
    'ports/servoshell/desktop/protocols/mod.rs',
    'ports/servoshell/desktop/tracing.rs',
    'ports/servoshell/desktop/webxr.rs',
  ],
  'ports/servoshell/desktop/protocols/mod.rs': [
    'ports/servoshell/desktop/protocols/resource.rs',
    'ports/servoshell/desktop/protocols/servo.rs',
    'ports/servoshell/desktop/protocols/urlinfo.rs',
  ],
};

const summaryMap = {
  'ports/servoshell/backtrace.rs': ['Provides low-level backtrace formatting and printing utilities for crash reporting, with platform-specific Windows path decoding and OpenHarmony support.', ['utility', 'debugging', 'crash-reporting']],
  'ports/servoshell/crash_handler.rs': ['Installs signal handlers (SIGSEGV, SIGILL, SIGIOT, SIGBUS) that print backtraces and safely re-raise signals or exit on crash.', ['crash-reporting', 'signal-handling', 'safety']],
  'ports/servoshell/desktop/accelerated_gl_media.rs': ['Platform-conditional GL-accelerated media initialization, setting up EGL or X11 native displays and rendering contexts for hardware-accelerated video playback.', ['media', 'gl-acceleration', 'multimedia']],
  'ports/servoshell/desktop/app.rs': ['Core application lifecycle manager handling initialization, platform window creation, event loop pumping, and userscript loading for the Servo shell.', ['application', 'lifecycle', 'event-handler', 'entry-point']],
  'ports/servoshell/desktop/cli.rs': ['CLI entry point for the Servo shell binary that parses arguments, initializes crash handling, crypto, and tracing, and launches the headed or headless event loop.', ['entry-point', 'command-line', 'process']],
  'ports/servoshell/desktop/dialog.rs': ['Implements all embedder dialog types including file picker, alert, confirm, prompt, authentication, permission, device selection, select element, color picker, and context menu using egui.', ['dialog', 'ui', 'embedder-control', 'egui']],
  'ports/servoshell/desktop/event_loop.rs': ['Abstracts the event loop into winit-backed (headed) and condvar-based (headless) variants, providing a unified interface for application lifecycle and loop wake.', ['event-loop', 'winit', 'lifecycle']],
  'ports/servoshell/desktop/gamepad.rs': ['Gamepad input handling using Gilrs to poll controllers, emit gamepad events to webviews, and manage haptic feedback effects with scheduling and cancellation.', ['gamepad', 'input', 'haptic', 'event-handler']],
  'ports/servoshell/desktop/geometry.rs': ['Thin conversion helpers translating between winit window coordinate types and Servo euclid geometry types (Size2D, Point2D).', ['utility', 'geometry', 'conversion']],
  'ports/servoshell/desktop/gui.rs': ['Main egui-based browser chrome implementation with toolbar, tabs, URL bar, back/forward navigation, status bar, favicon loading, and accessibility integration.', ['browser-ui', 'toolbar', 'egui', 'accessibility']],
  'ports/servoshell/desktop/headed_window.rs': ['Full winit window implementation managing keyboard, mouse, and touch input; embedder controls; XR rendering; cursor handling; and screen geometry for headed mode.', ['window-management', 'winit', 'input-handling', 'xr']],
  'ports/servoshell/desktop/headless_window.rs': ['Minimal offscreen window implementation for headless mode, providing window metrics, rendering context, and basic lifecycle without a visible surface.', ['headless', 'window', 'offscreen']],
  'ports/servoshell/desktop/keyutils.rs': ['Comprehensive winit keyboard event to Servo key event translation, handling physical key mapping, text input composition, and modifier state across Linux, Windows, and Mac.', ['keyboard', 'input', 'key-mapping', 'platform']],
  'ports/servoshell/desktop/mod.rs': ['Module barrel file that declares and re-exports all desktop submodules including app, gui, dialogs, event loop, gamepad, protocols, tracing, and webxr.', ['barrel', 'module', 're-export']],
  'ports/servoshell/desktop/protocols/mod.rs': ['Module barrel file declaring the resource, servo, and urlinfo custom protocol handler submodules.', ['barrel', 'module']],
  'ports/servoshell/desktop/protocols/resource.rs': ['Custom resource:// protocol handler that serves built-in static files including HTML, JS, CSS, and images from the Servo resources directory.', ['protocol', 'resource', 'static-files']],
  'ports/servoshell/desktop/protocols/servo.rs': ['Custom servo:// protocol handler providing privileged internal pages such as about, crash, and OOM, plus JSON status responses through the embedder API.', ['protocol', 'privileged-pages', 'embedder-api']],
  'ports/servoshell/desktop/protocols/urlinfo.rs': ['Custom urlinfo:// protocol handler exposing URL metadata including fetchability and security status to web content through the embedder protocol bridge.', ['protocol', 'url-metadata', 'embedder-api']],
};

// Create file nodes
for (const r of results) {
  const fp = r.path;
  const fn = fp.split('/').pop();
  const [summary, tags] = summaryMap[fp] || ['', ['code']];
  const nLines = r.nonEmptyLines;
  let complexity = 'simple';
  if (nLines > 200) complexity = 'complex';
  else if (nLines > 50) complexity = 'moderate';
  nodes.push(makeNode('file:' + fp, 'file', fn, fp, summary, tags, complexity));
}

// Function and class nodes
for (const r of results) {
  const fp = r.path;
  const fileId = 'file:' + fp;
  const usedFuncIds = new Set();

  if (r.functions) {
    for (const f of r.functions) {
      if (!sigFilter(f, r)) continue;
      const funcId = 'function:' + fp + ':' + f.name;
      if (usedFuncIds.has(funcId)) continue;
      usedFuncIds.add(funcId);

      const nonEmpty = f.endLine - f.startLine + 1;
      let complexity = 'simple';
      if (nonEmpty >= 100) complexity = 'complex';
      else if (nonEmpty >= 20) complexity = 'moderate';

      nodes.push(makeNode(funcId, 'function', f.name, fp, 'Function ' + f.name + ' in ' + fp.split('/').pop() + '.', ['function'], complexity, {lineRange: [f.startLine, f.endLine]}));
      edges.push({source: fileId, target: funcId, type: 'contains', direction: 'forward', weight: 1.0});
      if (r.exports && r.exports.some(e => e.name === f.name)) {
        edges.push({source: fileId, target: funcId, type: 'exports', direction: 'forward', weight: 0.8});
      }
    }
  }

  if (r.classes) {
    for (const c of r.classes) {
      if (!sigClass(c, r)) continue;
      const classId = 'class:' + fp + ':' + c.name;
      const numMethods = c.methods.length;
      let complexity = 'simple';
      if (numMethods >= 10) complexity = 'complex';
      else if (numMethods >= 5) complexity = 'moderate';

      nodes.push(makeNode(classId, 'class', c.name, fp, 'Class ' + c.name + ' in ' + fp.split('/').pop() + '.', ['class'], complexity));
      edges.push({source: fileId, target: classId, type: 'contains', direction: 'forward', weight: 1.0});
      if (r.exports && r.exports.some(e => e.name === c.name)) {
        edges.push({source: fileId, target: classId, type: 'exports', direction: 'forward', weight: 0.8});
      }
    }
  }

  // Import edges
  if (importData[fp]) {
    for (const target of importData[fp]) {
      edges.push({source: fileId, target: 'file:' + target, type: 'imports', direction: 'forward', weight: 0.7});
    }
  }
}

// Call edges from callGraph analysis
// cli.rs
edges.push({source: 'function:ports/servoshell/desktop/cli.rs:main', target: 'function:ports/servoshell/crash_handler.rs:install', type: 'calls', direction: 'forward', weight: 0.8});

// app.rs internal calls
edges.push({source: 'function:ports/servoshell/desktop/app.rs:init', target: 'function:ports/servoshell/desktop/app.rs:create_platform_window', type: 'calls', direction: 'forward', weight: 0.8});
edges.push({source: 'function:ports/servoshell/desktop/app.rs:init', target: 'function:ports/servoshell/desktop/app.rs:load_userscripts', type: 'calls', direction: 'forward', weight: 0.8});
edges.push({source: 'function:ports/servoshell/desktop/app.rs:create_platform_window', target: 'function:ports/servoshell/desktop/headless_window.rs:new', type: 'calls', direction: 'forward', weight: 0.8});
edges.push({source: 'function:ports/servoshell/desktop/app.rs:create_platform_window', target: 'function:ports/servoshell/desktop/headed_window.rs:new', type: 'calls', direction: 'forward', weight: 0.8});
edges.push({source: 'function:ports/servoshell/desktop/app.rs:pump_servo_event_loop', target: 'function:ports/servoshell/desktop/app.rs:create_platform_window', type: 'calls', direction: 'forward', weight: 0.8});

// event_loop internal calls
edges.push({source: 'function:ports/servoshell/desktop/event_loop.rs:run_app', target: 'function:ports/servoshell/desktop/app.rs:pump_servo_event_loop', type: 'calls', direction: 'forward', weight: 0.8});
edges.push({source: 'function:ports/servoshell/desktop/event_loop.rs:run_app', target: 'function:ports/servoshell/desktop/app.rs:init', type: 'calls', direction: 'forward', weight: 0.8});

// gamepad internal calls
edges.push({source: 'function:ports/servoshell/desktop/gamepad.rs:handle_haptic_effect_request', target: 'function:ports/servoshell/desktop/gamepad.rs:play_haptic_effect', type: 'calls', direction: 'forward', weight: 0.8});
edges.push({source: 'function:ports/servoshell/desktop/gamepad.rs:handle_haptic_effect_request', target: 'function:ports/servoshell/desktop/gamepad.rs:stop_haptic_effect', type: 'calls', direction: 'forward', weight: 0.8});

// gui internal calls
edges.push({source: 'function:ports/servoshell/desktop/gui.rs:update', target: 'function:ports/servoshell/desktop/gui.rs:load_pending_favicons', type: 'calls', direction: 'forward', weight: 0.8});
edges.push({source: 'function:ports/servoshell/desktop/gui.rs:update', target: 'function:ports/servoshell/desktop/gui.rs:browser_tab', type: 'calls', direction: 'forward', weight: 0.8});
edges.push({source: 'function:ports/servoshell/desktop/gui.rs:load_pending_favicons', target: 'function:ports/servoshell/desktop/gui.rs:embedder_image_to_egui_image', type: 'calls', direction: 'forward', weight: 0.8});
edges.push({source: 'function:ports/servoshell/desktop/gui.rs:update_webview_data', target: 'function:ports/servoshell/desktop/gui.rs:update_load_status', type: 'calls', direction: 'forward', weight: 0.8});
edges.push({source: 'function:ports/servoshell/desktop/gui.rs:update_webview_data', target: 'function:ports/servoshell/desktop/gui.rs:update_location_in_toolbar', type: 'calls', direction: 'forward', weight: 0.8});
edges.push({source: 'function:ports/servoshell/desktop/gui.rs:new', target: 'function:ports/servoshell/desktop/gui.rs:configure_fonts', type: 'calls', direction: 'forward', weight: 0.8});

// headed_window internal calls
edges.push({source: 'function:ports/servoshell/desktop/headed_window.rs:handle_winit_window_event', target: 'function:ports/servoshell/desktop/headed_window.rs:handle_keyboard_input', type: 'calls', direction: 'forward', weight: 0.8});
edges.push({source: 'function:ports/servoshell/desktop/headed_window.rs:handle_winit_window_event', target: 'function:ports/servoshell/desktop/headed_window.rs:handle_mouse_button_event', type: 'calls', direction: 'forward', weight: 0.8});
edges.push({source: 'function:ports/servoshell/desktop/headed_window.rs:handle_winit_window_event', target: 'function:ports/servoshell/desktop/headed_window.rs:handle_mouse_move_event', type: 'calls', direction: 'forward', weight: 0.8});
edges.push({source: 'function:ports/servoshell/desktop/headed_window.rs:handle_winit_window_event', target: 'function:ports/servoshell/desktop/headed_window.rs:handle_intercepted_key_bindings', type: 'calls', direction: 'forward', weight: 0.8});
edges.push({source: 'function:ports/servoshell/desktop/headed_window.rs:handle_winit_window_event', target: 'function:ports/servoshell/desktop/headed_window.rs:for_each_active_dialog', type: 'calls', direction: 'forward', weight: 0.8});
edges.push({source: 'function:ports/servoshell/desktop/headed_window.rs:handle_winit_window_event', target: 'function:ports/servoshell/desktop/headed_window.rs:maybe_consume_move_button_event', type: 'calls', direction: 'forward', weight: 0.8});
edges.push({source: 'function:ports/servoshell/desktop/headed_window.rs:handle_winit_window_event', target: 'function:ports/servoshell/desktop/headed_window.rs:maybe_consume_mouse_move_event', type: 'calls', direction: 'forward', weight: 0.8});
edges.push({source: 'function:ports/servoshell/desktop/headed_window.rs:new', target: 'function:ports/servoshell/desktop/headed_window.rs:load_icon', type: 'calls', direction: 'forward', weight: 0.8});

// dialog internal calls
edges.push({source: 'function:ports/servoshell/desktop/dialog.rs:update', target: 'function:ports/servoshell/desktop/dialog.rs:new_file_dialog', type: 'calls', direction: 'forward', weight: 0.8});

// window_event calls gui method
edges.push({source: 'function:ports/servoshell/desktop/app.rs:window_event', target: 'function:ports/servoshell/desktop/gui.rs:update', type: 'calls', direction: 'forward', weight: 0.8});

console.log('Node count:', nodes.length);
console.log('Edge count:', edges.length);

// Check for parts
if (nodes.length <= 60 && edges.length <= 120) {
  const output = { nodes, edges };
  fs.writeFileSync('.understand-anything/intermediate/batch-30.json', JSON.stringify(output, null, 2));
  console.log('Written single file: batch-30.json');
} else {
  const nodeParts = Math.ceil(nodes.length / 60);
  const edgeParts = Math.ceil(edges.length / 120);
  const parts = Math.max(nodeParts, edgeParts);
  console.log('Splitting into ' + parts + ' parts');

  const filesPerPart = Math.ceil(results.length / parts);
  for (let p = 0; p < parts; p++) {
    const startIdx = p * filesPerPart;
    const endIdx = Math.min((p + 1) * filesPerPart, results.length);
    const partFilePaths = results.slice(startIdx, endIdx).map(r => r.path);
    const partFileIds = new Set(partFilePaths.map(fp => 'file:' + fp));

    const partNodes = nodes.filter(n => {
      if (!n.filePath) return false;
      return partFilePaths.includes(n.filePath);
    });

    const partNodeIds = new Set(partNodes.map(n => n.id));
    const partEdges = edges.filter(e => {
      return partNodeIds.has(e.source);
    });

    const partOutput = { nodes: partNodes, edges: partEdges };
    const partFile = '.understand-anything/intermediate/batch-30-part-' + (p + 1) + '.json';
    fs.writeFileSync(partFile, JSON.stringify(partOutput, null, 2));
    console.log('Written ' + partFile + ' with ' + partNodes.length + ' nodes, ' + partEdges.length + ' edges');
  }
}

// Validate
console.log('\n--- Validation ---');
for (const e of edges) {
  const sourceExists = nodes.some(n => n.id === e.source);
  const targetExists = nodes.some(n => n.id === e.target);
  if (!sourceExists) console.log('MISSING SOURCE: ' + e.source);
  if (!targetExists) {
    // Cross-batch targets are allowed
    if (!e.target.startsWith('file:ports/servoshell/desktop/tracing') &&
        !e.target.startsWith('file:ports/servoshell/desktop/webxr')) {
      console.log('MISSING TARGET: ' + e.target);
    }
  }
}
