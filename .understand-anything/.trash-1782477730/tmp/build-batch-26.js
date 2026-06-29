const fs = require('fs');

const extractResults = JSON.parse(fs.readFileSync('D:/Projects/servo/.understand-anything/tmp/ua-file-extract-results-26.json', 'utf8'));
const inputData = JSON.parse(fs.readFileSync('D:/Projects/servo/.understand-anything/tmp/ua-file-analyzer-input-26.json', 'utf8'));

const batchImportData = inputData.batchImportData;
const results = extractResults.results;

const nodes = [];
const edges = [];

function fileId(path) { return 'file:' + path; }
function classId(path, name) { return 'class:' + path + ':' + name; }
function funcId(path, name) { return 'function:' + path + ':' + name; }

function complexityFromLines(nonEmptyLines) {
  if (nonEmptyLines < 50) return 'simple';
  if (nonEmptyLines < 200) return 'moderate';
  return 'complex';
}

const SUMMARY_MAP = {
  'fakexrdevice.rs': 'Mock XR device implementation for testing, providing methods to simulate views, input sources, and pose changes via a channel-based command interface.',
  'fakexrinputcontroller.rs': 'Mock XR input controller for testing, supporting simulation of pointer/grip origins, selection, handedness, and button state changes.',
  'mod.rs': 'Module declaration file that re-exports all WebXR DOM types as the public interface of the webxr module.',
  'xrboundedreferencespace.rs': 'Implements XRBoundedReferenceSpace, a reference space with a geometry boundary for room-scale XR experiences.',
  'xrcompositionlayer.rs': 'Stub type definition for XRCompositionLayer, a base layer type for composited XR content.',
  'xrcubelayer.rs': 'Stub type definition for XRCubeLayer, representing a cube map layer in the XR compositor.',
  'xrcylinderlayer.rs': 'Stub type definition for XRCylinderLayer, representing a cylindrical projection layer for XR media.',
  'xrequirectlayer.rs': 'Stub type definition for XREquirectLayer, representing an equirectangular projection layer for XR media.',
  'xrframe.rs': 'Implements XRFrame, providing pose queries, hit test results, joint pose tracking, and animation frame state for XR sessions.',
  'xrhand.rs': 'Implements XRHand, a maplike collection of XRJointSpace entries representing the joints of a tracked hand.',
  'xrhittestresult.rs': 'Implements XRHitTestResult, providing the pose of a hit test intersection relative to a base space.',
  'xrhittestsource.rs': 'Implements XRHitTestSource, representing an active hit test subscription that can be cancelled.',
  'xrinputsource.rs': 'Implements XRInputSource, representing a tracked XR controller with hand, gamepad, and spatial tracking support.',
  'xrinputsourcearray.rs': 'Implements XRInputSourceArray, a dynamic collection of input sources with add/remove tracking and change event dispatch.',
  'xrinputsourceevent.rs': 'Implements XRInputSourceEvent, a DOM event fired when an XR input source triggers a select or squeeze action.',
  'xrinputsourceschangeevent.rs': 'Implements XRInputSourcesChangeEvent, a DOM event fired when the set of available XR input sources changes.',
  'xrjointpose.rs': 'Implements XRJointPose, extending XRPose with a radius for finger joint tracking.',
  'xrjointspace.rs': 'Implements XRJointSpace, a tracked XR space representing a single hand joint with pose lookup per frame.',
  'xrlayer.rs': 'Implements XRLayer, a base class for XR compositor layers with frame lifecycle management.',
  'xrlayerevent.rs': 'Implements XRLayerEvent, a DOM event for XR layer lifecycle notifications.',
  'xrmediabinding.rs': 'Implements XRMediaBinding, creating media-bound XR layers (quad, cylinder, equirect) from HTML media elements.'
};

const TAG_MAP = {
  'fakexrdevice.rs': ['testing', 'mock', 'webxr', 'device-simulation'],
  'fakexrinputcontroller.rs': ['testing', 'mock', 'webxr', 'input-controller'],
  'mod.rs': ['module', 'webxr', 'barrel', 'exports'],
  'xrboundedreferencespace.rs': ['webxr', 'reference-space', 'room-scale'],
  'xrcompositionlayer.rs': ['webxr', 'layer', 'stub'],
  'xrcubelayer.rs': ['webxr', 'layer', 'stub'],
  'xrcylinderlayer.rs': ['webxr', 'layer', 'stub'],
  'xrequirectlayer.rs': ['webxr', 'layer', 'stub'],
  'xrframe.rs': ['webxr', 'frame', 'pose', 'animation'],
  'xrhand.rs': ['webxr', 'hand-tracking', 'joints'],
  'xrhittestresult.rs': ['webxr', 'hit-test', 'pose'],
  'xrhittestsource.rs': ['webxr', 'hit-test', 'subscription'],
  'xrinputsource.rs': ['webxr', 'input', 'controller', 'gamepad'],
  'xrinputsourcearray.rs': ['webxr', 'input', 'collection', 'events'],
  'xrinputsourceevent.rs': ['webxr', 'events', 'input'],
  'xrinputsourceschangeevent.rs': ['webxr', 'events', 'input', 'change-tracking'],
  'xrjointpose.rs': ['webxr', 'joint-pose', 'hand-tracking'],
  'xrjointspace.rs': ['webxr', 'joint-space', 'hand-tracking'],
  'xrlayer.rs': ['webxr', 'layer', 'compositor'],
  'xrlayerevent.rs': ['webxr', 'events', 'layer'],
  'xrmediabinding.rs': ['webxr', 'media', 'layer', 'binding']
};

const NOTE_MAP = {
  'mod.rs': 'Rust module barrel file re-exporting all WebXR DOM bindings.'
};

// STEP 1: File nodes
for (const r of results) {
  const path = r.path;
  const filename = path.split('/').pop();
  const n = {
    id: fileId(path),
    type: 'file',
    name: filename,
    filePath: path,
    summary: SUMMARY_MAP[filename] || '',
    tags: TAG_MAP[filename] || ['code'],
    complexity: complexityFromLines(r.nonEmptyLines)
  };
  if (NOTE_MAP[filename]) n.languageNotes = NOTE_MAP[filename];
  nodes.push(n);
}

// Class summaries
const CLASS_SUMMARY_MAP = {
  'FakeXRDevice': 'Mock XR device that simulates views, viewer origin, input sources, and bounds geometry for WebXR testing.',
  'FakeXRInputController': 'Mock XR input controller providing programmable pointer/grip origin, selection, and button state simulation.',
  'XRBoundedReferenceSpace': 'Reference space with a room-scale boundary geometry for bounded XR experiences.',
  'XRFrame': 'Provides per-frame XR state including viewer pose, space-relative poses, joint poses, and hit test results.',
  'XRHand': 'Maplike collection of XRJointSpace entries indexed by hand joint name, representing a tracked hand.',
  'XRHitTestResult': 'Result of a hit test containing the pose of the intersection point relative to a base space.',
  'XRHitTestSource': 'Active hit test subscription that can be cancelled to stop receiving hit test results.',
  'XRInputSource': 'Represents a tracked XR controller or hand with handedness, target ray, grip space, gamepad, and hand data.',
  'XRInputSourceArray': 'Dynamic array of active XRInputSource objects with add/remove tracking and automatic event dispatch.',
  'XRInputSourceEvent': 'DOM event fired when an XR input source performs a select or squeeze action.',
  'XRInputSourcesChangeEvent': 'DOM event fired when the set of available XR input sources changes.',
  'XRJointPose': 'Extended XRPose that includes a joint radius for finger joint tracking.',
  'XRJointSpace': 'Tracked XR space representing a hand joint, providing joint name and per-frame pose lookup.',
  'XRLayer': 'Base class for XR compositor layers providing frame lifecycle and context management.',
  'XRLayerEvent': 'DOM event fired for XR layer lifecycle notifications.',
  'XRMediaBinding': 'Creates XR media layers bound to HTML media elements for immersive video playback.'
};

const CLASS_TAG_MAP = {
  'FakeXRDevice': ['webxr', 'mock', 'testing', 'device'],
  'FakeXRInputController': ['webxr', 'mock', 'testing', 'input'],
  'XRBoundedReferenceSpace': ['webxr', 'reference-space', 'boundary'],
  'XRFrame': ['webxr', 'frame', 'pose', 'animation'],
  'XRHand': ['webxr', 'hand-tracking', 'joints'],
  'XRHitTestResult': ['webxr', 'hit-test', 'pose'],
  'XRHitTestSource': ['webxr', 'hit-test', 'subscription'],
  'XRInputSource': ['webxr', 'input', 'controller'],
  'XRInputSourceArray': ['webxr', 'input', 'array', 'events'],
  'XRInputSourceEvent': ['webxr', 'events', 'input'],
  'XRInputSourcesChangeEvent': ['webxr', 'events', 'input', 'change'],
  'XRJointPose': ['webxr', 'joint-pose', 'radius'],
  'XRJointSpace': ['webxr', 'joint-space', 'hand'],
  'XRLayer': ['webxr', 'layer', 'compositor'],
  'XRLayerEvent': ['webxr', 'events', 'layer'],
  'XRMediaBinding': ['webxr', 'media', 'layer-binding']
};

// Function summaries
const FUNC_SUMMARY_MAP = {
  'view': 'Converts a raw MockViewInit into an XRView structure with projection matrix, transform, and field of view.',
  'get_views': 'Converts view initialization data into mock view structures for mono or stereo rendering.',
  'get_origin': 'Converts a raw position/orientation array into a RigidTransform3D for spatial computations.',
  'get_world': 'Converts hit test region data from raw arrays into structured MockRegion objects with triangle faces.',
  'SimulateInputSourceConnection': 'Simulates a new XR input source connection with specified handedness, target ray mode, profiles, and buttons.',
  'Disconnect_fake': 'Simulates device disconnection with a promise-based callback for test synchronization.',
  'SetBoundsGeometry': 'Sets the room-scale boundary geometry from 2D point coordinates for bounded reference spaces.',
  'SetViews': 'Sets the simulated views (mono or stereo) for the mock XR device.',
  'init_to_mock_buttons': 'Converts raw button state descriptors into MockButton structures for input source simulation.',
  'UpdateButtonState': 'Validates and sends button state updates through the mock input channel.',
  'new_offset': 'Creates an XRBoundedReferenceSpace with a specified offset transform from a reference space.',
  'BoundsGeometry': 'Returns the boundary geometry as a frozen array of DOMPointReadOnly objects.',
  'GetViewerPose': 'Computes the viewer pose relative to a given reference space, returning an XRViewerPose if available.',
  'GetPose': 'Computes the relative pose between two XR spaces, returning an XRPose with the transform.',
  'GetJointPose': 'Computes the pose of a hand joint space relative to a base space, returning an XRJointPose with radius.',
  'GetHitTestResults': 'Retrieves hit test results for a given source, filtering by source ID and constructing XRHitTestResult objects.',
  'FillJointRadii': 'Fills an array of joint radii from XRJointSpace frame data, validating session and array length.',
  'FillPoses': 'Fills a Float32Array with 4x4 transform matrices for a list of XR spaces relative to a base space.',
  'Get_hand': 'Retrieves an XRJointSpace by joint name from the hand joint map.',
  'update_gamepad_state': 'Updates the input source gamepad button and axis values from per-frame tracking data.',
  'add_input_sources': 'Adds multiple input sources to the array and dispatches an inputsourceschange event.',
  'remove_input_source': 'Removes an input source by ID from the array and dispatches an inputsourceschange event.',
  'add_remove_input_source': 'Atomically replaces an input source dispatching an inputsourceschange event.',
  'new_with_proto': 'Internal constructor creating a DOM object with a specific prototype for event initialization.',
  'Constructor': 'WebIDL constructor that validates parameters and creates a new DOM object instance from script.'
};

function getFuncSummary(fnName, path) {
  if (FUNC_SUMMARY_MAP[fnName]) return FUNC_SUMMARY_MAP[fnName];
  if (fnName === 'new') {
    if (path.includes('fakexrinputcontroller')) return 'Creates a new FakeXRInputController DOM object with a sender channel and input ID.';
    if (path.includes('xrhand')) return 'Creates a new XRHand DOM object with joint spaces mapped from tracked hand data.';
    if (path.includes('xrhittestresult') || path.includes('xrhittestsource')) return 'Creates a new DOM object with the specified parameters and registers it with the JavaScript runtime.';
    if (path.includes('xrjointpose')) return 'Creates a new XRJointPose DOM object from a raw pose and radius.';
    if (path.includes('xrjointspace')) return 'Creates a new XRJointSpace DOM object with joint type, hand mapping, and input source reference.';
    if (path.includes('xrinputsource')) return 'Creates a new XRInputSource with gamepad, profiles, and spatial reference initialization.';
    if (path.includes('xrlayerevent')) return 'Creates a new XRLayerEvent DOM object with a layer reference.';
    if (path.includes('xrmediabinding')) return 'Creates a new XRMediaBinding DOM object for the given XR session.';
    return 'Creates a new DOM object and registers it with the JavaScript runtime.';
  }
  if (fnName === 'new_inherited') {
    if (path.includes('xrboundedreferencespace')) return 'Internal constructor for XRBoundedReferenceSpace with a session reference and offset transform.';
    if (path.includes('xrinputsource')) return 'Internal constructor initializing XRInputSource with gamepad, profiles, and spatial data from session info.';
    if (path.includes('xrjointspace')) return 'Internal constructor for XRJointSpace with joint type, hand joint mapping, and input source reference.';
    if (path.includes('xrlayer')) return 'Internal constructor for XRLayer with session, GPU context, and layer ID initialization.';
    return 'Internal constructor for DOM object initialization.';
  }
  if (fnName === 'Get' && path.includes('xrhand')) return 'Retrieves an XRJointSpace by joint name from the hand joint map.';
  return fnName.replace(/_/g, ' ') + ' function.';
}

function getFuncTags(fnName, path) {
  const tags = ['webxr'];
  if (fnName === 'view' || fnName === 'get_views') tags.push('utility', 'views');
  else if (fnName === 'get_origin') tags.push('utility', 'transform');
  else if (fnName === 'get_world') tags.push('utility', 'hit-test');
  else if (fnName === 'SimulateInputSourceConnection') tags.push('testing', 'input-simulation');
  else if (fnName === 'Disconnect_fake' || (fnName === 'Disconnect' && path.includes('fakexrdevice'))) tags.push('testing', 'disconnection');
  else if (fnName === 'SetBoundsGeometry') tags.push('testing', 'boundary');
  else if (fnName === 'SetViews') tags.push('testing', 'views');
  else if (fnName === 'init_to_mock_buttons') tags.push('testing', 'utility');
  else if (fnName === 'UpdateButtonState') tags.push('testing', 'button-state');
  else if (fnName === 'GetViewerPose') tags.push('pose', 'viewer');
  else if (fnName === 'GetPose') tags.push('pose', 'transform');
  else if (fnName === 'GetJointPose') tags.push('pose', 'joint');
  else if (fnName === 'GetHitTestResults') tags.push('hit-test', 'results');
  else if (fnName === 'FillJointRadii') tags.push('joint', 'radii');
  else if (fnName === 'FillPoses') tags.push('pose', 'batch');
  else if (fnName === 'update_gamepad_state') tags.push('gamepad', 'state-update');
  else if (fnName === 'add_input_sources') tags.push('input', 'add', 'events');
  else if (fnName === 'remove_input_source') tags.push('input', 'remove', 'events');
  else if (fnName === 'add_remove_input_source') tags.push('input', 'replace', 'events');
  else if (fnName === 'Constructor') tags.push('constructor', 'webidl');
  else tags.push('constructor');
  return tags;
}

// STEP 2: Class and function nodes
const nodeSet = new Set();

function shouldEmitClass(cls, r) {
  if (!cls) return false;
  const methodsCount = (cls.methods || []).length;
  const isExported = (r.exports || []).some(e => e.name === cls.name);
  if (isExported) return true;
  if (methodsCount >= 2) return true;
  if (cls.endLine - cls.startLine + 1 >= 20) return true;
  return false;
}

function shouldEmitFunction(fn, r) {
  if (!fn) return false;
  const lines = fn.endLine - fn.startLine + 1;
  const isExported = (r.exports || []).some(e => e.name === fn.name);
  if (lines < 3 && !isExported) return false;
  if (isExported && lines >= 3) return true;
  if (lines >= 10) return true;
  // Skip trivial boilerplate
  if ((fn.name === 'new_inherited' || fn.name === 'new') && lines < 8 && !isExported) return false;
  return false;
}

for (const r of results) {
  const path = r.path;
  for (const cls of (r.classes || [])) {
    if (shouldEmitClass(cls, r)) {
      const n = {
        id: classId(path, cls.name),
        type: 'class',
        name: cls.name,
        filePath: path,
        lineRange: [cls.startLine, cls.endLine],
        summary: CLASS_SUMMARY_MAP[cls.name] || cls.name + ' class.',
        tags: CLASS_TAG_MAP[cls.name] || ['webxr'],
        complexity: complexityFromLines(r.nonEmptyLines)
      };
      nodes.push(n);
      nodeSet.add(n.id);
    }
  }
  for (const fn of (r.functions || [])) {
    if (shouldEmitFunction(fn, r)) {
      const lines = fn.endLine - fn.startLine + 1;
      const sm = (fn.name === 'Disconnect' && path.includes('fakexrdevice')) ? FUNC_SUMMARY_MAP['Disconnect_fake'] : getFuncSummary(fn.name, path);
      const tg = getFuncTags(fn.name, path);
      const n = {
        id: funcId(path, fn.name),
        type: 'function',
        name: fn.name,
        filePath: path,
        lineRange: [fn.startLine, fn.endLine],
        summary: sm,
        tags: tg,
        complexity: lines < 15 ? 'simple' : (lines < 50 ? 'moderate' : 'complex')
      };
      nodes.push(n);
      nodeSet.add(n.id);
    }
  }
}

// STEP 3a: Import edges (1:1)
let importEdgeCount = 0;
for (const [sourcePath, targets] of Object.entries(batchImportData)) {
  for (const targetPath of targets) {
    edges.push({
      source: fileId(sourcePath),
      target: fileId(targetPath),
      type: 'imports',
      direction: 'forward',
      weight: 0.7
    });
    importEdgeCount++;
  }
}

// STEP 3b: Contains edges
for (const r of results) {
  const path = r.path;
  const fileNid = fileId(path);
  for (const cls of (r.classes || [])) {
    const cid = classId(path, cls.name);
    if (nodeSet.has(cid)) {
      edges.push({ source: fileNid, target: cid, type: 'contains', direction: 'forward', weight: 1.0 });
    }
  }
  for (const fn of (r.functions || [])) {
    const fid = funcId(path, fn.name);
    if (nodeSet.has(fid)) {
      edges.push({ source: fileNid, target: fid, type: 'contains', direction: 'forward', weight: 1.0 });
    }
  }
}

// STEP 3c: Exports edges
for (const r of results) {
  const path = r.path;
  const fileNid = fileId(path);
  for (const exp of (r.exports || [])) {
    const cid = classId(path, exp.name);
    if (nodeSet.has(cid)) {
      edges.push({ source: fileNid, target: cid, type: 'exports', direction: 'forward', weight: 0.8 });
      continue;
    }
    const fid = funcId(path, exp.name);
    if (nodeSet.has(fid)) {
      edges.push({ source: fileNid, target: fid, type: 'exports', direction: 'forward', weight: 0.8 });
    }
  }
}

// STEP 3d: Calls edges (cross-file from callGraph)
function addCall(sourcePath, callerName, targetSuffix, calleeName) {
  const targetPath = 'components/script/dom/webxr/' + targetSuffix;
  const cid = funcId(sourcePath, callerName);
  const tid = funcId(targetPath, calleeName);
  if (nodeSet.has(cid) && nodeSet.has(tid)) {
    const exists = edges.some(e => e.source === cid && e.target === tid && e.type === 'calls');
    if (!exists) {
      edges.push({ source: cid, target: tid, type: 'calls', direction: 'forward', weight: 0.8 });
    }
  }
}

const CALL_PATTERNS = [
  ['SimulateInputSourceConnection', 'fakexrinputcontroller.rs', 'new'],
  ['add_input_sources', 'xrinputsource.rs', 'new'],
  ['add_remove_input_source', 'xrinputsource.rs', 'new'],
  ['GetHand', 'xrhand.rs', 'new'],
  ['GetHitTestResults', 'xrhittestresult.rs', 'new'],
  ['add_input_sources', 'xrinputsourceschangeevent.rs', 'new'],
  ['remove_input_source', 'xrinputsourceschangeevent.rs', 'new'],
  ['add_remove_input_source', 'xrinputsourceschangeevent.rs', 'new'],
];

for (const r of results) {
  const path = r.path;
  for (const call of (r.callGraph || [])) {
    const callee = call.callee;
    if (callee === 'FakeXRInputController::new') addCall(path, call.caller, 'fakexrinputcontroller.rs', 'new');
    else if (callee === 'XRInputSource::new') addCall(path, call.caller, 'xrinputsource.rs', 'new');
    else if (callee === 'XRHand::new') addCall(path, call.caller, 'xrhand.rs', 'new');
    else if (callee === 'XRJointSpace::new') addCall(path, call.caller, 'xrjointspace.rs', 'new');
    else if (callee === 'XRJointPose::new') addCall(path, call.caller, 'xrjointpose.rs', 'new');
    else if (callee === 'XRHitTestResult::new') addCall(path, call.caller, 'xrhittestresult.rs', 'new');
    else if (callee === 'XRInputSourcesChangeEvent::new') addCall(path, call.caller, 'xrinputsourceschangeevent.rs', 'new');
  }
}

// STEP 4: Partition and write
const nodeCount = nodes.length;
const edgeCount = edges.length;

console.log('Total nodes: ' + nodeCount);
console.log('Total edges: ' + edgeCount);
console.log('Import edges: ' + importEdgeCount);
console.log('Expected imports: ' + Object.values(batchImportData).flat().length);

const filePaths = results.map(r => r.path).sort();
const nodesByFile = {};
for (const n of nodes) {
  const fp = n.filePath || '';
  if (!nodesByFile[fp]) nodesByFile[fp] = [];
  nodesByFile[fp].push(n);
}

const outDir = 'D:/Projects/servo/.understand-anything/intermediate';

if (nodeCount <= 60 && edgeCount <= 120) {
  const out = { nodes, edges };
  fs.writeFileSync(outDir + '/batch-26.json', JSON.stringify(out, null, 2));
  console.log('Wrote single file: batch-26.json');
} else {
  const parts = Math.ceil(Math.max(nodeCount / 60, edgeCount / 120));
  console.log('Splitting into ' + parts + ' parts');

  const filesPerPart = Math.ceil(filePaths.length / parts);
  for (let p = 1; p <= parts; p++) {
    const startIdx = (p - 1) * filesPerPart;
    const endIdx = Math.min(startIdx + filesPerPart, filePaths.length);
    const partFiles = filePaths.slice(startIdx, endIdx);

    const partNodes = [];
    const partEdgeSources = new Set();
    const partFileNodeIds = new Set();

    for (const fp of partFiles) {
      const fid = fileId(fp);
      partFileNodeIds.add(fid);
      partEdgeSources.add(fid);

      // File nodes have filePath set, so they are included in nodesByFile automatically
      const fpNodes = nodesByFile[fp] || [];
      for (const n of fpNodes) {
        partNodes.push(n);
        partEdgeSources.add(n.id);
      }
    }

    const partEdges = edges.filter(e => partEdgeSources.has(e.source));

    const out = { nodes: partNodes, edges: partEdges };
    const filename = 'batch-26-part-' + p + '.json';
    fs.writeFileSync(outDir + '/' + filename, JSON.stringify(out, null, 2));
    console.log('Wrote ' + filename + ': ' + partNodes.length + ' nodes, ' + partEdges.length + ' edges');
  }
}
