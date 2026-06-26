const fs = require('fs');
const g = JSON.parse(fs.readFileSync('d:/Projects/servo/.understand-anything/knowledge-graph.json','utf8'));

const fileNodes = g.nodes.filter(n => ['file','config','document','service','pipeline','table','schema','resource','endpoint'].includes(n.type));

// Build layers from path patterns
const layerMap = {};
fileNodes.forEach(n => {
  const path = n.filePath || n.id.replace(/^[^:]+:/, '');
  let layer = 'other';
  if (path.startsWith('components/script/dom/')) layer = 'dom';
  else if (path.startsWith('components/layout/')) layer = 'layout';
  else if (path.startsWith('components/net/') || path.startsWith('components/shared/net/')) layer = 'networking';
  else if (path.startsWith('components/script/')) layer = 'script-engine';
  else if (path.startsWith('components/paint/')) layer = 'painting';
  else if (path.startsWith('components/constellation/')) layer = 'constellation';
  else if (path.startsWith('components/servo/')) layer = 'servo-crate';
  else if (path.startsWith('components/fonts/') || path.startsWith('components/shared/fonts/')) layer = 'fonts';
  else if (path.startsWith('components/media/')) layer = 'media';
  else if (path.startsWith('components/webgl/') || path.startsWith('components/webgpu/')) layer = 'webgl-gpu';
  else if (path.startsWith('components/webxr/') || path.startsWith('components/shared/webxr/')) layer = 'webxr';
  else if (path.startsWith('components/webdriver_server/')) layer = 'webdriver';
  else if (path.startsWith('components/devtools/')) layer = 'devtools';
  else if (path.startsWith('components/canvas/')) layer = 'canvas';
  else if (path.startsWith('components/storage/')) layer = 'storage';
  else if (path.startsWith('components/xpath/')) layer = 'xpath';
  else if (path.startsWith('components/script_bindings/')) layer = 'script-bindings';
  else if (path.startsWith('ports/')) layer = 'ports';
  else if (path.startsWith('python/') || path.startsWith('etc/')) layer = 'tooling';
  else if (path.startsWith('components/shared/')) layer = 'shared-infra';
  else if (path.startsWith('components/background_hang_monitor/')) layer = 'hang-monitor';
  else if (path.startsWith('ffi/')) layer = 'ffi-capi';
  else if (path.startsWith('components/svg_engine/')) layer = 'svg-engine';
  else if (path.startsWith('components/profile/') || path.startsWith('components/shared/profile/')) layer = 'profiling';

  if (!layerMap[layer]) layerMap[layer] = [];
  layerMap[layer].push(n.id);
});

const layerDefs = {
  'dom': 'DOM API implementations for WebIDL interfaces (HTML, SVG, events, canvas, etc.)',
  'layout': 'Layout engine: flow construction, fragment tree, display list, flexbox, tables',
  'script-engine': 'Script engine core: thread management, task queues, module loading, JS runtime',
  'script-bindings': 'SpiderMonkey JS bindings, reflection, GC tracing, DOM root management',
  'networking': 'HTTP loader, caching, WebSocket, image fetch, MIME classification, HSTS',
  'constellation': 'Central orchestrator: pipeline management, browsing contexts, navigation',
  'painting': 'WebRender integration: paint thread, display list builder, compositing, screenshots',
  'servo-crate': 'Top-level Servo crate: event loop, webview management, embedder bridge',
  'webgl-gpu': 'WebGL and WebGPU rendering context implementations and API bindings',
  'webxr': 'WebXR Device API: VR/AR session management, OpenXR integration, headless simulation',
  'media': 'Media playback: audio/video decoding via GStreamer and OhOS backends',
  'fonts': 'Font loading, shaping via HarfBuzz, FreeType platform integration',
  'canvas': 'Canvas 2D rendering: Vello backend, paint thread, data management',
  'storage': 'IndexedDB and WebStorage: SQLite-backed engines, client API',
  'webdriver': 'WebDriver remote control protocol server for browser automation',
  'devtools': 'Firefox DevTools protocol integration for debugging and inspection',
  'ports': 'Platform-specific shells: desktop, Android/EGL, OhOS',
  'tooling': 'Build and CI: Mach, Python scripts, servo-tidy, WPT test harness',
  'shared-infra': 'Shared IPC channels, IDs, text/rope utilities, thread pools',
  'xpath': 'XPath expression evaluation: parser, AST, tokenizer, functions',
  'ffi-capi': 'C FFI API for embedding Servo in native applications',
  'svg-engine': 'SVG rendering engine: shapes, path flattening, tessellation',
  'hang-monitor': 'Background hang monitor: detects frozen threads via platform samplers',
  'profiling': 'Performance profiling: time/memory instrumentation, IPC profiling'
};

const layers = Object.entries(layerMap)
  .filter(([name]) => name !== 'other')
  .map(([name, ids]) => ({
    id: 'layer:' + name,
    name: name.split('-').map(w => w.charAt(0).toUpperCase() + w.slice(1)).join(' '),
    description: layerDefs[name] || name + ' layer',
    nodeIds: ids
  }));

g.layers = layers;
console.log('Set', layers.length, 'layers');

// Tour
const tour = [
  { order: 1, title: 'Project Overview', description: 'Servo is a prototype web browser engine written in Rust, developed on macOS, Linux, Windows, OpenHarmony, and Android.', nodeIds: ['document:README.md'] },
  { order: 2, title: 'DOM Implementation', description: 'The DOM layer implements WebIDL interfaces for all HTML, SVG, and CSSOM elements, forming the bridge between web content and browser internals.', nodeIds: ['file:components/script/dom/mod.rs'] },
  { order: 3, title: 'Layout Engine', description: 'The layout crate manages document flow, flexbox, tables, inline layout, and generates display lists for WebRender.', nodeIds: ['file:components/layout/lib.rs'] },
  { order: 4, title: 'Script Engine & Bindings', description: 'The script crate runs JavaScript via SpiderMonkey, manages DOM lifecycle, task queues, and communicates with the constellation.', nodeIds: ['file:components/script/lib.rs'] },
  { order: 5, title: 'Networking Stack', description: 'The networking layer handles HTTP/HTTPS requests, caching, WebSocket connections, cookie management, and image loading.', nodeIds: ['file:components/net/lib.rs'] },
  { order: 6, title: 'Constellation', description: 'The constellation orchestrates pipelines, browsing contexts, navigation, and manages the browser tab lifecycle.', nodeIds: ['file:components/constellation/constellation.rs'] },
  { order: 7, title: 'Painting & Rendering', description: 'The paint crate coordinates with WebRender to composite web content, handle scrolling, pinch-zoom, and screenshots.', nodeIds: ['file:components/paint/lib.rs'] },
  { order: 8, title: 'Web APIs', description: 'Servo implements extensive Web APIs: WebGL, WebGPU, WebXR, Web Audio, WebRTC, Canvas 2D, IndexedDB, and more.', nodeIds: ['file:components/webgl/lib.rs'] },
  { order: 9, title: 'Media Playback', description: 'Media playback is handled via GStreamer and native APIs with audio/video decoding support.', nodeIds: ['file:components/media/audio/lib.rs'] },
  { order: 10, title: 'Platform Shells', description: 'Platform-specific browser shells: desktop (winit), Android (EGL/JNI), and OhOS.', nodeIds: ['file:ports/servoshell/lib.rs'] },
];

// Only include tour steps that reference existing nodes
g.tour = tour.filter(s => s.nodeIds.some(id => g.nodes.some(n => n.id === id)));
console.log('Set', g.tour.length, 'tour steps');

fs.writeFileSync('d:/Projects/servo/.understand-anything/knowledge-graph.json', JSON.stringify(g));
console.log('Saved!');
