const fs = require('fs');
const ext = JSON.parse(fs.readFileSync('.understand-anything/tmp/ua-file-extract-results-27.json', 'utf8'));

const nodes = [];
const edges = [];

function isSignificantFunction(fn, exports) {
  const lineCount = fn.endLine - fn.startLine + 1;
  const isExported = exports.some(e => e.name === fn.name && e.line === fn.startLine);
  return lineCount >= 10 || isExported;
}

function isSignificantClass(cls, exports) {
  const lineCount = cls.endLine - cls.startLine + 1;
  const isExported = exports.some(e => e.name === cls.name && e.line === cls.startLine);
  const methodCount = cls.methods.length;
  return methodCount >= 2 || lineCount >= 20 || isExported;
}

function getFileSummary(file) {
  const summaries = {
    'xrpose': 'DOM binding for XRPose, representing a position and orientation in the WebXR coordinate system.',
    'xrprojectionlayer': 'Stub DOM binding for XRProjectionLayer, a WebXR projection layer placeholder type.',
    'xrquadlayer': 'Stub DOM binding for XRQuadLayer, a WebXR quad layer placeholder type.',
    'xrray': 'DOM binding for XRRay, providing raycasting utilities for hit testing in WebXR.',
    'xrreferencespace': 'DOM binding for XRReferenceSpace, managing spatial reference frames within WebXR sessions.',
    'xrreferencespaceevent': 'DOM binding for XRReferenceSpaceEvent, representing reference space change events dispatched on sessions.',
    'xrrenderstate': 'DOM binding for XRRenderState, managing WebXR session rendering state including depth clipping and layers.',
    'xrrigidtransform': 'DOM binding for XRRigidTransform, representing 3D rigid transforms with position and orientation for WebXR.',
    'xrsession': 'Core DOM binding for XRSession, managing WebXR sessions including rendering loop, input handling, and frame lifecycle.',
    'xrsessionevent': 'DOM binding for XRSessionEvent, representing session lifecycle events such as end and visibility change.',
    'xrspace': 'DOM binding for XRSpace, the base spatial reference type providing pose queries within WebXR coordinate systems.',
    'xrsubimage': 'DOM binding for XRSubImage, representing a sub-image within a WebXR layer for per-view rendering.',
    'xrsystem': 'DOM binding for XRSystem (navigator.xr), the entry point for querying WebXR device support and requesting sessions.',
    'xrtest': 'DOM binding for XRTest, providing WebXR test API for simulating device connections and user activation in automated tests.',
    'xrview': 'DOM binding for XRView, representing a single eye view within a WebXR viewer pose with projection and transform.',
    'xrviewerpose': 'DOM binding for XRViewerPose, representing the viewer pose with associated views in a WebXR frame.',
    'xrviewport': 'DOM binding for XRViewport, representing a rectangular viewport region within a WebXR framebuffer.',
    'xrwebglbinding': 'DOM binding for XRWebGLBinding, enabling WebGL-backed layer creation for immersive WebXR rendering.',
    'xrwebgllayer': 'DOM binding for XRWebGLLayer, managing the WebGL framebuffer and rendering lifecycle for WebXR sessions.',
    'xrwebglsubimage': 'DOM binding for XRWebGLSubImage, representing a WebGL sub-image with color and depth textures for per-view rendering.',
  };
  const name = file.path.split('/').pop().replace('.rs', '');
  return summaries[name] || 'DOM binding for WebXR specification type.';
}

function getFileTags(file) {
  const name = file.path.split('/').pop().replace('.rs', '');
  const base = ['webxr', 'dom', 'rust'];
  if (name === 'xrsystem' || name === 'xrsession') base.push('entry-point');
  if (name === 'xrtest') base.push('test');
  if (name.endsWith('event')) base.push('event');
  if (name.includes('layer')) base.push('rendering');
  if (name === 'xrrigidtransform') base.push('geometry');
  if (name === 'xrray') base.push('geometry', 'hit-test');
  if (name === 'xrspace' || name === 'xrreferencespace') base.push('spatial');
  if (name === 'xrview' || name === 'xrviewerpose') base.push('rendering');
  if (name === 'xrrenderstate') base.push('configuration');
  if (name === 'xrwebglbinding' || name === 'xrwebgllayer') base.push('webgl', 'rendering');
  return base;
}

ext.results.forEach(file => {
  const filePath = file.path;
  const fileName = filePath.split('/').pop();
  const fileId = 'file:' + filePath;

  let complexity = 'simple';
  if (file.nonEmptyLines > 200) complexity = 'complex';
  else if (file.nonEmptyLines >= 50) complexity = 'moderate';

  const tags = getFileTags(file);

  nodes.push({
    id: fileId,
    type: 'file',
    name: fileName,
    filePath: filePath,
    summary: getFileSummary(file),
    tags: tags,
    complexity: complexity,
    languageNotes: 'Rust DOM binding for the WebXR Device API specification.'
  });

  // Create class nodes
  (file.classes || []).forEach(cls => {
    if (!isSignificantClass(cls, file.exports)) return;

    const classId = 'class:' + filePath + ':' + cls.name;
    const clsLineCount = cls.endLine - cls.startLine + 1;
    const isExported = file.exports.some(e => e.name === cls.name && e.line === cls.startLine);

    let clsComplexity = 'simple';
    if (clsLineCount > 100) clsComplexity = 'complex';
    else if (clsLineCount > 30) clsComplexity = 'moderate';

    const clsTags = ['webxr', 'dom-class', 'rust'];
    if (cls.name.includes('Event')) clsTags.push('event');
    if (cls.name.includes('Layer')) clsTags.push('rendering');
    if (cls.name.includes('Transform')) clsTags.push('geometry');

    const summaries = {
      'XRSession': 'Core WebXR session class managing rendering loop, input sources, animation frames, and session lifecycle.',
      'XRSystem': 'Entry point for WebXR accessed via navigator.xr, providing session support queries and session requests.',
      'XRViewerPose': 'WebXR viewer pose containing the headset transform and all associated views.',
      'XRRigidTransform': '3D rigid transform with position and orientation, used throughout WebXR for spatial operations.',
      'XRRenderState': 'WebXR render state configuration including depth near/far, inline vertical FOV, and layer assignments.',
      'XRReferenceSpace': 'WebXR reference space providing spatial tracking with offset transforms and pose calculations.',
      'XRSpace': 'Base WebXR spatial reference type providing pose queries for all concrete space types.',
      'XRView': 'WebXR view representing a single eye view with projection matrix and transform.',
      'XRWebGLLayer': 'WebXR WebGL layer managing framebuffer attachments and rendering lifecycle for immersive sessions.',
      'XRWebGLBinding': 'WebXR WebGL binding providing layer creation methods for WebGL-backed immersive rendering.',
      'XRTest': 'WebXR test API for simulating device connections and user activation in automated testing environments.',
      'XRFrame': 'WebXR frame providing pose data, input state, and hit test results for a single animation frame.',
      'XRRay': 'WebXR ray defined by an origin and direction, used for hit testing.',
      'XRPose': 'WebXR pose representing a position and orientation with optional linear and angular velocity.',
      'XRViewport': 'WebXR viewport defining a rectangular region within a framebuffer for rendering.',
    };
    let summary = summaries[cls.name] || cls.name + ' DOM binding implementing the WebXR Device API.';

    nodes.push({
      id: classId,
      type: 'class',
      name: cls.name,
      filePath: filePath,
      lineRange: [cls.startLine, cls.endLine],
      summary: summary,
      tags: clsTags,
      complexity: clsComplexity
    });

    edges.push({
      source: fileId,
      target: classId,
      type: 'contains',
      direction: 'forward',
      weight: 1.0
    });

    if (isExported) {
      edges.push({
        source: fileId,
        target: classId,
        type: 'exports',
        direction: 'forward',
        weight: 0.8
      });
    }
  });

  // Create function nodes
  (file.functions || []).forEach(fn => {
    if (!isSignificantFunction(fn, file.exports)) return;

    const fnId = 'function:' + filePath + ':' + fn.name;
    const fnLineCount = fn.endLine - fn.startLine + 1;
    const isExported = file.exports.some(e => e.name === fn.name && e.line === fn.startLine);

    let fnComplexity = 'simple';
    if (fnLineCount > 50) fnComplexity = 'moderate';

    const fnTags = ['webxr', 'method', 'rust'];
    if (fn.name === 'Constructor' || fn.name === 'Constructor_') fnTags.push('constructor');
    if (fn.name === 'new' || fn.name === 'new_inherited' || fn.name === 'new_with_proto') fnTags.push('constructor');
    if (fn.name.startsWith('Get') || fn.name.startsWith('Is')) fnTags.push('getter');

    // Build summary based on function name and class context
    const className = (file.classes && file.classes.length > 0) ? file.classes[0].name : '';

    const fnSummaries = {
      'Constructor': 'JavaScript-exposed constructor validating inputs and creating a new ' + className + ' DOM object.',
      'Constructor_': 'Alternate JavaScript constructor for XRRay that accepts a transform instead of origin/direction.',
      'new': 'Factory method creating a new ' + className + ' DOM object with reflector and field initialization.',
      'new_inherited': 'Internal constructor initializing the ' + className + ' reflector and struct fields.',
      'new_with_proto': 'Factory method creating a new generated DOM object with a specified prototype chain.',
      'new_offset': 'Creates an XRReferenceSpace with an offset transform applied to the base reference space.',
      'event_callback': 'Handles incoming WebXR device events: session end, input source changes, visibility state, and reference space reset.',
      'raf_callback': 'Processes the animation frame: commits pending render state, renders the frame, invokes user RAF callbacks, and requests the next frame.',
      'UpdateRenderState': 'Updates session render state with depth clipping, layer configuration, and inline vertical FOV from init dictionary.',
      'RequestSession': 'Requests a new WebXR session, validates required/optional features, establishes IPC channels, and resolves with an XRSession.',
      'RequestReferenceSpace': 'Creates a reference space of the requested type after validating feature grants and session compatibility.',
      'RequestHitTestSource': 'Requests a hit test source, configuring the ray origin and entity types for spatial intersection testing.',
      'SimulateDeviceConnection': 'Simulates connecting a WebXR test device with specified views, features, and tracking for WebXR automated testing.',
      'SimulateUserActivation': 'Simulates user activation by setting an interaction guard, executing a callback, and restoring activation state.',
      'DisconnectAllDevices': 'Disconnects all simulated XR test devices by sending disconnect commands to the XR registry.',
      'End': 'Ends the WebXR session, resolving the end promise, notifying the XR system, clearing input sources, and cleaning up IPC.',
      'GetOffsetReferenceSpace': 'Creates a new offset reference space by combining the current offset with a new transform.',
      'update_inline_projection_matrix': 'Computes the inline projection matrix from render state depth and vertical FOV settings.',
      'inline_view': 'Returns the inline view transform and projection matrix for non-immersive sessions.',
      'get_pose': 'Retrieves the pose of a space relative to a base pose, handling reference, joint, and input source spaces.',
      'get_unoffset_pose': 'Retrieves the un-offset pose from the session or uses identity for unbounded spaces.',
      'session_obtained': 'Handles the session creation response from the XR device, resolving the session promise and setting up initial inputs.',
      'session': 'Returns the XRSession associated with this XRSpace or XRView.',
      'Matrix': 'Computes and returns the 4x4 transformation matrix for a ray or rigid transform.',
      'Origin': 'Returns the ray origin as a DOMPointReadOnly representing the starting point.',
      'Direction': 'Returns the ray direction as a DOMPointReadOnly representing the direction vector.',
      'SetupRAF': 'Sets up the render loop IPC route and starts the session rendering.'.replace('SetupRAF', 'setup_raf_loop'),
      'setup_initial_inputs': 'Processes initial input sources from the session during startup.',
      'begin_frame': 'Prepares the WebGL layer for a new frame by binding textures and framebuffer attachments.',
      'end_frame': 'Finalizes the WebGL layer frame by unbinding textures and flushing GL commands.',
      'GetViewport': 'Retrieves the XRViewport for a given view within the WebGL layer framebuffer.',
      'GetSubImage': 'Stub method for retrieving a sub-image from the WebGL binding (currently unsupported).',
      'GetViewSubImage': 'Stub method for retrieving a view sub-image from the WebGL binding (currently unsupported).',
      'CreateProjectionLayer': 'Stub method for creating a projection layer (currently returns NotSupported error).',
      'CreateQuadLayer': 'Stub method for creating a quad layer (currently returns NotSupported error).',
      'clone_object': 'Creates a deep clone of the render state with the same depth, FOV, and layer configuration.',
      'has_sub_images': 'Checks whether the render state layers have sub-images available for the current frame.',
      'apply_nominal_framerate': 'Applies a nominal framerate update from the XR device, firing a framerate change event.',
    };

    let summary = fnSummaries[fn.name] || fn.name + ' method in the ' + className + ' DOM binding.';

    nodes.push({
      id: fnId,
      type: 'function',
      name: fn.name,
      filePath: filePath,
      lineRange: [fn.startLine, fn.endLine],
      summary: summary,
      tags: fnTags,
      complexity: fnComplexity
    });

    edges.push({
      source: fileId,
      target: fnId,
      type: 'contains',
      direction: 'forward',
      weight: 1.0
    });

    if (isExported) {
      edges.push({
        source: fileId,
        target: fnId,
        type: 'exports',
        direction: 'forward',
        weight: 0.8
      });
    }
  });
});

console.log('Total nodes:', nodes.length);
console.log('Total edges:', edges.length);

// Partition: sort files and split into 3 parts
const filePaths = ext.results.map(r => r.path).sort();
console.log('Files sorted:', filePaths);

// 20 files / 3 parts = ~7 each
const parts = [
  filePaths.slice(0, 7),
  filePaths.slice(7, 14),
  filePaths.slice(14)
];

parts.forEach((fileList, idx) => {
  const partNum = idx + 1;

  const partNodeIds = new Set();
  const partNodes = [];

  nodes.forEach(n => {
    if (n.filePath && fileList.includes(n.filePath)) {
      partNodeIds.add(n.id);
      partNodes.push(n);
    }
  });

  const partEdges = [];
  edges.forEach(e => {
    if (partNodeIds.has(e.source)) {
      partEdges.push(e);
    }
  });

  const filename = '.understand-anything/intermediate/batch-27-part-' + partNum + '.json';
  fs.writeFileSync(filename, JSON.stringify({ nodes: partNodes, edges: partEdges }, null, 2));

  console.log('Part ' + partNum + ': files=' + fileList.length + ' nodes=' + partNodes.length + ' edges=' + partEdges.length + ' -> ' + filename);

  // Validate
  const allNodeIds = new Set(partNodes.map(n => n.id));
  const invalidEdges = partEdges.filter(e => {
    const sValid = allNodeIds.has(e.source);
    const tValid = allNodeIds.has(e.target);
    return !(sValid && tValid);
  });

  if (invalidEdges.length > 0) {
    console.log('  WARNING:' + invalidEdges.length + ' edges with invalid targets');
    invalidEdges.forEach(e => console.log('    ' + e.source + ' -> ' + e.target));
  }
});
