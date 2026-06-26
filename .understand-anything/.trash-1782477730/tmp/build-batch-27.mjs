import fs from "fs";
const OUT = "d:/Projects/servo/.understand-anything/intermediate";
const P = "components/script/dom/webxr";

const nodes = [];
const edges = [];

function N(id, type, name, filePath, summary, tags, complexity, lineRange, languageNotes) {
  const n = { id, type, name, filePath, summary, tags, complexity };
  if (lineRange) n.lineRange = lineRange;
  if (languageNotes) n.languageNotes = languageNotes;
  nodes.push(n);
}
function E(source, target, type, weight, direction) {
  edges.push({ source, target, type, direction: direction || "forward", weight: weight || 0.7 });
}

// 1. xrpose.rs
{
  const fp = P+"/xrpose.rs";
  N("file:"+fp, "file", "xrpose.rs", fp, "DOM binding for XRPose, representing position and orientation in the WebXR coordinate system with transform, linear velocity, and angular velocity accessors.", ["webxr","dom-binding","rust"], "moderate", null, "Rust DOM binding implementing XRPose WebXR interface.");
  N("class:"+fp+":XRPose", "class", "XRPose", fp, "Represents a pose (position and orientation) in the WebXR coordinate system.", ["webxr","pose","dom-class"], "moderate", [17,20]);
  N("function:"+fp+":new_inherited", "function", "new_inherited", fp, "Internal constructor initializing the reflector and transform.", ["webxr","constructor"], "simple", [23,28]);
  N("function:"+fp+":new", "function", "new", fp, "Public constructor wrapping an XRRigidTransform into a reflected DOM object.", ["webxr","constructor"], "simple", [30,37]);
  E("file:"+fp, "class:"+fp+":XRPose", "contains", 1.0);
  E("file:"+fp, "function:"+fp+":new_inherited", "contains", 1.0);
  E("file:"+fp, "function:"+fp+":new", "contains", 1.0);
  E("file:"+fp, "class:"+fp+":XRPose", "exports", 0.8);
  E("file:"+fp, "function:"+fp+":new_inherited", "exports", 0.8);
  E("file:"+fp, "function:"+fp+":new", "exports", 0.8);
}

// 2-3. stubs
["xrprojectionlayer","xrquadlayer"].forEach(f => {
  const fp = P+"/"+f+".rs";
  N("file:"+fp, "file", f+".rs", fp, "Stub DOM binding for a WebXR layer type.", ["webxr","dom-binding","rust","stub"], "simple");
});

// 4. xrray.rs
{
  const fp = P+"/xrray.rs";
  N("file:"+fp, "file", "xrray.rs", fp, "DOM binding for XRRay, providing raycasting utilities for hit testing and spatial queries. Supports construction from origin/direction or XRRigidTransform.", ["webxr","dom-binding","raycasting","rust"], "moderate");
  N("class:"+fp+":XRRay", "class", "XRRay", fp, "Represents a ray with origin, direction, and 4x4 matrix representations.", ["webxr","raycasting","dom-class"], "moderate", [25,31]);
  N("function:"+fp+":Constructor", "function", "Constructor", fp, "Constructs XRRay from origin/direction, validating w-coordinate constraints and normalizing direction.", ["webxr","constructor","validation"], "moderate", [58,86]);
  N("function:"+fp+":Constructor_", "function", "Constructor_", fp, "Alternate constructor from XRRigidTransform, computing ray direction from rotation.", ["webxr","constructor"], "simple", [89,102]);
  N("function:"+fp+":Matrix", "function", "Matrix", fp, "Computes 4x4 matrix representation using cross products and quaternion-based rotation.", ["webxr","matrix","geometry"], "moderate", [129,163]);
  E("file:"+fp, "class:"+fp+":XRRay", "contains", 1.0);
  ["Constructor","Constructor_","Matrix"].forEach(fn => { E("file:"+fp, "function:"+fp+":"+fn, "contains", 1.0); });
  E("file:"+fp, "class:"+fp+":XRRay", "exports", 0.8);
}

// 5. xrreferencespace.rs
{
  const fp = P+"/xrreferencespace.rs";
  N("file:"+fp, "file", "xrreferencespace.rs", fp, "DOM binding for XRReferenceSpace, managing spatial reference frames with offset spaces and pose computation.", ["webxr","dom-binding","spatial","rust"], "moderate");
  N("class:"+fp+":XRReferenceSpace", "class", "XRReferenceSpace", fp, "Spatial reference frame with offset support for different base space types.", ["webxr","reference-space","dom-class"], "moderate", [24,28]);
  const funcs = {
    new_inherited: ["Internal constructor initializing XRSpace, offset, and type.", [31,41], "simple"],
    new: ["Public constructor with identity offset.", [43,51], "simple"],
    new_offset: ["Constructor with specific offset transform.", [53,65], "simple"],
    space: ["Returns base space as RigidTransform3D composing floor transform and offset.", [67,77], "simple"],
    ty: ["Returns the XRReferenceSpaceType.", [79,81], "simple"],
    GetOffsetReferenceSpace: ["Creates new space with additional offset.", [86,96], "simple"],
    get_base_transform: ["Computes inverse pose transform for base space.", [107,110], "simple"],
    get_pose: ["Computes pose by combining unoffset pose with offset.", [117,125], "simple"],
    get_unoffset_pose: ["Computes raw pose from session floor transform.", [130,149], "moderate"],
    get_bounds: ["Returns boundary geometry for bounded-floor spaces.", [151,155], "simple"]
  };
  Object.entries(funcs).forEach(([name,[summary,lr,complexity]]) => {
    N("function:"+fp+":"+name, "function", name, fp, summary, ["webxr","method"], complexity, lr);
    E("file:"+fp, "function:"+fp+":"+name, "contains", 1.0);
    E("file:"+fp, "function:"+fp+":"+name, "exports", 0.8);
  });
  E("file:"+fp, "class:"+fp+":XRReferenceSpace", "contains", 1.0);
  E("file:"+fp, "class:"+fp+":XRReferenceSpace", "exports", 0.8);
}

// 6. xrreferencespaceevent.rs
{
  const fp = P+"/xrreferencespaceevent.rs";
  N("file:"+fp, "file", "xrreferencespaceevent.rs", fp, "DOM binding for XRReferenceSpaceEvent, representing reference space change events.", ["webxr","dom-binding","event","rust"], "moderate");
  N("class:"+fp+":XRReferenceSpaceEvent", "class", "XRReferenceSpaceEvent", fp, "Event dispatched when reference space transform changes.", ["webxr","event","dom-class"], "moderate", [25,29]);
  N("function:"+fp+":new", "function", "new", fp, "Public constructor with type, bubbles, cancelable, space, and transform.", ["webxr","constructor"], "simple", [43,55]);
  N("function:"+fp+":new_with_proto", "function", "new_with_proto", fp, "Constructor with explicit prototype, initializing underlying Event.", ["webxr","constructor"], "simple", [58,79]);
  N("function:"+fp+":Constructor", "function", "Constructor", fp, "JS constructor parsing init dictionary for space and transform.", ["webxr","constructor"], "simple", [84,101]);
  ["new","new_with_proto","Constructor"].forEach(fn => { E("file:"+fp, "function:"+fp+":"+fn, "contains", 1.0); });
  E("file:"+fp, "class:"+fp+":XRReferenceSpaceEvent", "contains", 1.0);
  E("file:"+fp, "class:"+fp+":XRReferenceSpaceEvent", "exports", 0.8);
  E("file:"+fp, "function:"+fp+":new", "exports", 0.8);
}

// 7. xrrenderstate.rs
{
  const fp = P+"/xrrenderstate.rs";
  N("file:"+fp, "file", "xrrenderstate.rs", fp, "DOM binding for XRRenderState, managing depth clipping, FOV, base layer, and layer arrays.", ["webxr","dom-binding","rendering","rust"], "moderate");
  N("class:"+fp+":XRRenderState", "class", "XRRenderState", fp, "Container for rendering configuration with sub-image validation.", ["webxr","rendering","dom-class"], "moderate", [24,31]);
  const funcs = {
    new_inherited: ["Internal constructor initializing all render state fields.", [34,50], "simple"],
    new: ["Public constructor reflecting the DOM object.", [52,72], "simple"],
    clone_object: ["Clones render state with same depth, FOV, base layer, and layers.", [74,84], "simple"],
    with_layers: ["Applies closure to layers list.", [102,108], "simple"],
    has_sub_images: ["Validates all layers have corresponding sub-images.", [109,128], "moderate"]
  };
  Object.entries(funcs).forEach(([name,[summary,lr,complexity]]) => {
    N("function:"+fp+":"+name, "function", name, fp, summary, ["webxr","method"], complexity, lr);
    E("file:"+fp, "function:"+fp+":"+name, "contains", 1.0);
    E("file:"+fp, "function:"+fp+":"+name, "exports", 0.8);
  });
  E("file:"+fp, "class:"+fp+":XRRenderState", "contains", 1.0);
  E("file:"+fp, "class:"+fp+":XRRenderState", "exports", 0.8);
}

// 8. xrrigidtransform.rs
{
  const fp = P+"/xrrigidtransform.rs";
  N("file:"+fp, "file", "xrrigidtransform.rs", fp, "DOM binding for XRRigidTransform: 3D rigid transform with position and orientation quaternion.", ["webxr","dom-binding","transform","rust"], "moderate", "Implements 3D rigid body transforms using quaternion rotation and matrix decomposition.");
  N("class:"+fp+":XRRigidTransform", "class", "XRRigidTransform", fp, "3D rigid transform with lazy-computed inverse, cached DOMPoints, and 4x4 matrix.", ["webxr","transform","3d","dom-class"], "moderate", [25,34]);
  const funcs = {
    new: ["Public constructor delegating to new_with_proto.", [48,54], "simple"],
    new_with_proto: ["Constructor with explicit prototype.", [56,68], "simple"],
    identity: ["Static factory for identity transform.", [70,73], "simple"],
    Constructor: ["JS constructor validating finite values, normalizing quaternion, building RigidTransform3D.", [78,129], "moderate"],
    Orientation: ["Returns orientation as DOMPointReadOnly, lazily initialized.", [139,151], "simple"],
    Matrix: ["Returns 4x4 transform matrix as Float64Array.", [162,172], "simple"]
  };
  Object.entries(funcs).forEach(([name,[summary,lr,complexity]]) => {
    N("function:"+fp+":"+name, "function", name, fp, summary, ["webxr","method"], complexity, lr);
    E("file:"+fp, "function:"+fp+":"+name, "contains", 1.0);
  });
  E("file:"+fp, "class:"+fp+":XRRigidTransform", "contains", 1.0);
  E("file:"+fp, "class:"+fp+":XRRigidTransform", "exports", 0.8);
  E("file:"+fp, "function:"+fp+":new", "exports", 0.8);
  E("file:"+fp, "function:"+fp+":identity", "exports", 0.8);
}

// 9. xrsession.rs
{
  const fp = P+"/xrsession.rs";
  N("file:"+fp, "file", "xrsession.rs", fp, "Core XRSession DOM binding managing rendering loops, input sources, reference spaces, hit testing, frame lifecycle, and framerate control.", ["webxr","dom-binding","session","rust","core"], "complex", "Implements the full XRSession WebXR spec interface with ~40 methods.");
  N("class:"+fp+":XRSession", "class", "XRSession", fp, "Central WebXR session manager with ~40 methods for frame callbacks, render state, input, reference spaces, hit testing, and framerate control.", ["webxr","session","dom-class","core"], "complex", [78,118]);
  const funcs = {
    new_inherited: ["Internal constructor initializing all session state fields.", [121,153], "moderate"],
    new: ["Public constructor creating render state, input sources, RAF loop.", [155,182], "moderate"],
    with_session: ["Applies closure to underlying WebXR session handle.", [184,187], "simple"],
    is_ended: ["Returns whether session has ended.", [189,191], "simple"],
    is_immersive: ["Returns whether this is an immersive VR session.", [193,195], "simple"],
    has_layers_feature: ["Checks if layer management feature is available.", [198,202], "simple"],
    setup_raf_loop: ["Sets up animation frame loop via IPC route and task source.", [204,224], "moderate"],
    is_outside_raf: ["Returns whether session is outside requestAnimationFrame callback.", [226,228], "simple"],
    attach_event_handler: ["Attaches XR event handler through IPC router.", [230,248], "moderate"],
    setup_initial_inputs: ["Processes initial input sources from device session.", [255,273], "moderate"],
    event_callback: ["Main event handler dispatching session, input, and reference space events.", [275,419], "complex"],
    raf_callback: ["Main RAF handler managing render state, frame events, sub-image validation, and callbacks.", [422,506], "complex"],
    update_inline_projection_matrix: ["Computes inline projection matrix from base layer size and FOV.", [508,530], "moderate"],
    inline_view: ["Returns view for inline sessions with identity transform.", [533,540], "simple"],
    session_id: ["Returns unique ID of underlying device session.", [542,544], "simple"],
    dirty_layers: ["Marks active layers as dirty when base layer changes.", [546,550], "simple"],
    handle_frame_event: ["Processes frame events, resolving hit test promises.", [587,603], "moderate"],
    apply_nominal_framerate: ["Applies framerate change, firing frameratechange event.", [606,622], "moderate"],
    UpdateRenderState: ["Updates session render state with validation and device sync.", [670,793], "complex"],
    CancelAnimationFrame: ["Cancels pending RAF callback.", [808,818], "simple"],
    RequestReferenceSpace: ["Creates reference space, validating feature grants.", [831,887], "complex"],
    End: ["Ends session, resolves promise, cleans up input sources.", [895,927], "moderate"],
    RequestHitTestSource: ["Requests hit test source from device session.", [930,988], "complex"],
    GetSupportedFrameRates: ["Returns supported framerates as Float64 array.", [1008,1028], "moderate"],
    UpdateTargetFrameRate: ["Validates and sets target framerate.", [1045,1090], "complex"],
    cast_transform: ["Unsafe transmute between RigidTransform3D types.", [1104,1108], "simple"]
  };
  Object.entries(funcs).forEach(([name,[summary,lr,complexity]]) => {
    N("function:"+fp+":"+name, "function", name, fp, summary, ["webxr","method"], complexity, lr);
    E("file:"+fp, "function:"+fp+":"+name, "contains", 1.0);
  });
  E("file:"+fp, "class:"+fp+":XRSession", "contains", 1.0);
  E("file:"+fp, "class:"+fp+":XRSession", "exports", 0.8);
  ["with_session","is_ended","is_immersive","has_layers_feature","is_outside_raf","setup_initial_inputs","inline_view","session_id","dirty_layers","cast_transform"].forEach(fn => {
    E("file:"+fp, "function:"+fp+":"+fn, "exports", 0.8);
  });
}

// 10. xrsessionevent.rs
{
  const fp = P+"/xrsessionevent.rs";
  N("file:"+fp, "file", "xrsessionevent.rs", fp, "DOM binding for XRSessionEvent: session lifecycle events (end, visibilitychange, frameratechange).", ["webxr","dom-binding","event","rust"], "moderate");
  N("class:"+fp+":XRSessionEvent", "class", "XRSessionEvent", fp, "Event for XRSession lifecycle changes with session reference.", ["webxr","event","dom-class"], "moderate", [22,25]);
  N("function:"+fp+":new", "function", "new", fp, "Public constructor with type, bubbles, cancelable, and session.", ["webxr","constructor"], "simple", [35,44]);
  N("function:"+fp+":new_with_proto", "function", "new_with_proto", fp, "Constructor with explicit prototype.", ["webxr","constructor"], "simple", [46,66]);
  N("function:"+fp+":Constructor", "function", "Constructor", fp, "JS constructor from init dictionary.", ["webxr","constructor"], "simple", [71,87]);
  ["new","new_with_proto","Constructor"].forEach(fn => { E("file:"+fp, "function:"+fp+":"+fn, "contains", 1.0); });
  E("file:"+fp, "class:"+fp+":XRSessionEvent", "contains", 1.0);
  E("file:"+fp, "class:"+fp+":XRSessionEvent", "exports", 0.8);
  E("file:"+fp, "function:"+fp+":new", "exports", 0.8);
}

// 11. xrspace.rs
{
  const fp = P+"/xrspace.rs";
  N("file:"+fp, "file", "xrspace.rs", fp, "Base XRSpace DOM binding providing generic pose lookups across space subtypes.", ["webxr","dom-binding","spatial","rust"], "moderate");
  N("class:"+fp+":XRSpace", "class", "XRSpace", fp, "Base class for all WebXR spatial reference types with downcasting support.", ["webxr","spatial","base","dom-class"], "moderate", [21,27]);
  const funcs = {
    new_inherited: ["Internal constructor initializing EventTarget and session.", [30,37], "simple"],
    new_inputspace_inner: ["Internal input-source space constructor.", [39,50], "simple"],
    new_inputspace: ["Public input-source space constructor.", [52,64], "simple"],
    space: ["Resolves underlying RigidTransform3D by downcasting or input source lookup.", [66,84], "moderate"],
    get_pose: ["Gets pose by downcasting or input source grip/target-ray lookup.", [93,118], "moderate"],
    session: ["Returns associated XRSession.", [120,122], "simple"]
  };
  Object.entries(funcs).forEach(([name,[summary,lr,complexity]]) => {
    N("function:"+fp+":"+name, "function", name, fp, summary, ["webxr","method"], complexity, lr);
    E("file:"+fp, "function:"+fp+":"+name, "contains", 1.0);
  });
  E("file:"+fp, "class:"+fp+":XRSpace", "contains", 1.0);
  E("file:"+fp, "class:"+fp+":XRSpace", "exports", 0.8);
  ["new_inherited","new_inputspace","space","get_pose","session"].forEach(fn => {
    E("file:"+fp, "function:"+fp+":"+fn, "exports", 0.8);
  });
}

// 12. xrsubimage.rs
{
  const fp = P+"/xrsubimage.rs";
  N("file:"+fp, "file", "xrsubimage.rs", fp, "Base XRSubImage DOM binding providing viewport access for WebXR layer sub-images.", ["webxr","dom-binding","layers","rust"], "simple");
  N("class:"+fp+":XRSubImage", "class", "XRSubImage", fp, "Base sub-image class with viewport access for layer rendering regions.", ["webxr","sub-image","dom-class"], "simple", [13,16]);
  E("file:"+fp, "class:"+fp+":XRSubImage", "contains", 1.0);
  E("file:"+fp, "class:"+fp+":XRSubImage", "exports", 0.8);
}

// 13. xrsystem.rs
{
  const fp = P+"/xrsystem.rs";
  N("file:"+fp, "file", "xrsystem.rs", fp, "XRSystem (navigator.xr) DOM binding: entry point for WebXR device discovery, session creation, and device registration.", ["webxr","dom-binding","system","rust","entry-point"], "complex");
  N("class:"+fp+":XRSystem", "class", "XRSystem", fp, "Entry point for WebXR API managing device queries, session requests, and test API.", ["webxr","system","dom-class","entry-point"], "complex", [41,50]);
  const funcs = {
    new: ["Public constructor with pipeline ID.", [65,71], "simple"],
    pending_or_active_session: ["Returns whether there is a pending or active immersive session.", [73,75], "simple"],
    set_pending: ["Marks system as having pending immersive session.", [77,79], "simple"],
    set_active_immersive_session: ["Transitions pending session to active.", [81,86], "simple"],
    end_session: ["Ends active immersive session and cleans up.", [89,102], "moderate"],
    IsSessionSupported: ["Queries device registry for session mode support.", [117,155], "moderate"],
    RequestSession: ["Requests new WebXR session with feature validation and IPC setup.", [158,265], "complex"],
    session_obtained: ["Callback creating DOM XRSession when device session obtained.", [274,305], "moderate"],
    dispatch_sessionavailable: ["Dispatches sessionavailable event on device availability.", [308,319], "simple"]
  };
  Object.entries(funcs).forEach(([name,[summary,lr,complexity]]) => {
    N("function:"+fp+":"+name, "function", name, fp, summary, ["webxr","method"], complexity, lr);
    E("file:"+fp, "function:"+fp+":"+name, "contains", 1.0);
  });
  E("file:"+fp, "class:"+fp+":XRSystem", "contains", 1.0);
  E("file:"+fp, "class:"+fp+":XRSystem", "exports", 0.8);
  ["new","pending_or_active_session","set_pending","set_active_immersive_session","end_session","dispatch_sessionavailable"].forEach(fn => {
    E("file:"+fp, "function:"+fp+":"+fn, "exports", 0.8);
  });
}

// 14. xrtest.rs
{
  const fp = P+"/xrtest.rs";
  N("file:"+fp, "file", "xrtest.rs", fp, "XRTest DOM binding for simulating device connections, user activation, and disconnection in automated tests.", ["webxr","dom-binding","test","rust","testing"], "moderate");
  N("class:"+fp+":XRTest", "class", "XRTest", fp, "Test controller for WebXR device simulation with IPC callbacks.", ["webxr","test","dom-class"], "moderate", [34,37]);
  const funcs = {
    new_inherited: ["Internal constructor with reflector and empty devices list.", [40,45], "simple"],
    new: ["Public constructor.", [47,49], "simple"],
    device_obtained: ["Callback when fake device created, adds to connected list.", [51,67], "simple"],
    SimulateDeviceConnection: ["Simulates device connection with specified properties via IPC.", [72,181], "complex"],
    DisconnectAllDevices: ["Disconnects all simulated devices sequentially.", [191,227], "moderate"]
  };
  Object.entries(funcs).forEach(([name,[summary,lr,complexity]]) => {
    N("function:"+fp+":"+name, "function", name, fp, summary, ["webxr","method"], complexity, lr);
    E("file:"+fp, "function:"+fp+":"+name, "contains", 1.0);
  });
  E("file:"+fp, "class:"+fp+":XRTest", "contains", 1.0);
  E("file:"+fp, "class:"+fp+":XRTest", "exports", 0.8);
  E("file:"+fp, "function:"+fp+":new_inherited", "exports", 0.8);
  E("file:"+fp, "function:"+fp+":new", "exports", 0.8);
}

// 15. xrview.rs
{
  const fp = P+"/xrview.rs";
  N("file:"+fp, "file", "xrview.rs", fp, "XRView DOM binding representing a single eye view with projection matrix, transform, and viewport scale.", ["webxr","dom-binding","view","rust"], "moderate");
  N("class:"+fp+":XRView", "class", "XRView", fp, "Single eye view providing eye type, projection matrix, transform, and viewport scale.", ["webxr","view","dom-class"], "moderate", [24,35]);
  const funcs = {
    new_inherited: ["Internal constructor initializing session, transform, eye, projection, and viewport index.", [38,55], "simple"],
    new: ["Public constructor creating rigid transform from view data.", [57,80], "moderate"],
    session: ["Returns the associated XRSession.", [82,84], "simple"],
    viewport_index: ["Returns viewport index in sub-image array.", [86,88], "simple"],
    ProjectionMatrix: ["Returns 4x4 projection matrix as Float64Array.", [98,109], "simple"]
  };
  Object.entries(funcs).forEach(([name,[summary,lr,complexity]]) => {
    N("function:"+fp+":"+name, "function", name, fp, summary, ["webxr","method"], complexity, lr);
    E("file:"+fp, "function:"+fp+":"+name, "contains", 1.0);
  });
  E("file:"+fp, "class:"+fp+":XRView", "contains", 1.0);
  E("file:"+fp, "class:"+fp+":XRView", "exports", 0.8);
  ["new","session","viewport_index"].forEach(fn => {
    E("file:"+fp, "function:"+fp+":"+fn, "exports", 0.8);
  });
}

// 16. xrviewerpose.rs
{
  const fp = P+"/xrviewerpose.rs";
  N("file:"+fp, "file", "xrviewerpose.rs", fp, "XRViewerPose DOM binding representing viewer head pose with associated eye views.", ["webxr","dom-binding","pose","rust"], "moderate");
  N("class:"+fp+":XRViewerPose", "class", "XRViewerPose", fp, "Extends XRPose with XRView array from device viewer pose.", ["webxr","pose","view","dom-class"], "moderate", [28,32]);
  N("function:"+fp+":new", "function", "new", fp, "Constructs XRViewerPose creating eye views from device pose or inline view.", ["webxr","constructor","pose"], "complex", [42,191]);
  E("file:"+fp, "function:"+fp+":new", "contains", 1.0);
  E("file:"+fp, "class:"+fp+":XRViewerPose", "contains", 1.0);
  E("file:"+fp, "class:"+fp+":XRViewerPose", "exports", 0.8);
  E("file:"+fp, "function:"+fp+":new", "exports", 0.8);
}

// 17. xrviewport.rs
{
  const fp = P+"/xrviewport.rs";
  N("file:"+fp, "file", "xrviewport.rs", fp, "XRViewport DOM binding representing rectangular viewport in WebXR framebuffer.", ["webxr","dom-binding","viewport","rust"], "simple");
  N("class:"+fp+":XRViewport", "class", "XRViewport", fp, "Rectangular viewport region with x, y, width, height properties.", ["webxr","viewport","dom-class"], "simple", [16,20]);
  N("function:"+fp+":new", "function", "new", fp, "Public constructor from webxr_api::Viewport.", ["webxr","constructor"], "simple", [30,36]);
  E("file:"+fp, "function:"+fp+":new", "contains", 1.0);
  E("file:"+fp, "class:"+fp+":XRViewport", "contains", 1.0);
  E("file:"+fp, "class:"+fp+":XRViewport", "exports", 0.8);
  E("file:"+fp, "function:"+fp+":new", "exports", 0.8);
}

// 18. xrwebglbinding.rs
{
  const fp = P+"/xrwebglbinding.rs";
  N("file:"+fp, "file", "xrwebglbinding.rs", fp, "XRWebGLBinding DOM binding for WebGL integration in WebXR layer creation.", ["webxr","dom-binding","webgl","rust"], "moderate");
  N("class:"+fp+":XRWebGLBinding", "class", "XRWebGLBinding", fp, "WebGL binding providing factory methods for XR layers and sub-images.", ["webxr","webgl","binding","dom-class"], "moderate", [34,38]);
  N("function:"+fp+":new_inherited", "function", "new_inherited", fp, "Internal constructor storing session and context.", ["webxr","constructor"], "simple", [41,50]);
  N("function:"+fp+":new", "function", "new", fp, "Constructor with prototype.", ["webxr","constructor"], "simple", [52,65]);
  N("function:"+fp+":Constructor", "function", "Constructor", fp, "JS constructor validating session and context state.", ["webxr","constructor","validation"], "moderate", [70,101]);
  ["new_inherited","new","Constructor"].forEach(fn => { E("file:"+fp, "function:"+fp+":"+fn, "contains", 1.0); });
  E("file:"+fp, "class:"+fp+":XRWebGLBinding", "contains", 1.0);
  E("file:"+fp, "class:"+fp+":XRWebGLBinding", "exports", 0.8);
  E("file:"+fp, "function:"+fp+":new_inherited", "exports", 0.8);
}

// 19. xrwebgllayer.rs
{
  const fp = P+"/xrwebgllayer.rs";
  N("file:"+fp, "file", "xrwebgllayer.rs", fp, "XRWebGLLayer DOM binding managing WebGL framebuffer lifecycle for WebXR rendering.", ["webxr","dom-binding","webgl","layer","rust"], "complex", "Integrates with WebGL command system for texture binding during frame rendering.");
  N("class:"+fp+":XRWebGLLayer", "class", "XRWebGLLayer", fp, "WebGL layer managing framebuffer, begin/end frame texture binding, and viewports.", ["webxr","webgl","layer","dom-class"], "complex", [52,61]);
  const funcs = {
    new_inherited: ["Internal constructor initializing XRLayer and framebuffer.", [64,80], "moderate"],
    new: ["Constructor with prototype.", [83,105], "simple"],
    layer_id: ["Returns device-assigned WebXR layer ID.", [107,109], "simple"],
    context_id: ["Returns WebGL context ID.", [111,113], "simple"],
    session: ["Returns owning XRSession.", [115,117], "simple"],
    size: ["Returns framebuffer size in pixels.", [119,129], "simple"],
    begin_frame: ["Begins XR frame with texture binding and GL state management.", [139,203], "complex"],
    end_frame: ["Ends XR frame detaching textures and flushing GL context.", [205,228], "moderate"],
    context: ["Returns WebGL context.", [230,232], "simple"],
    Constructor: ["JS constructor creating framebuffer and registering layer.", [237,294], "complex"],
    GetViewport: ["Computes viewport rect for given XRView.", [339,361], "moderate"]
  };
  Object.entries(funcs).forEach(([name,[summary,lr,complexity]]) => {
    N("function:"+fp+":"+name, "function", name, fp, summary, ["webxr","method"], complexity, lr);
    E("file:"+fp, "function:"+fp+":"+name, "contains", 1.0);
  });
  E("file:"+fp, "class:"+fp+":XRWebGLLayer", "contains", 1.0);
  E("file:"+fp, "class:"+fp+":XRWebGLLayer", "exports", 0.8);
  ["new_inherited","layer_id","context_id","session","size","begin_frame","end_frame","context"].forEach(fn => {
    E("file:"+fp, "function:"+fp+":"+fn, "exports", 0.8);
  });
}

// 20. xrwebglsubimage.rs
{
  const fp = P+"/xrwebglsubimage.rs";
  N("file:"+fp, "file", "xrwebglsubimage.rs", fp, "XRWebGLSubImage DOM binding with color/depth textures, image index, and texture dimensions.", ["webxr","dom-binding","webgl","sub-image","rust"], "simple");
  N("class:"+fp+":XRWebGLSubImage", "class", "XRWebGLSubImage", fp, "WebGL sub-image with color texture, depth/stencil texture, and size.", ["webxr","webgl","sub-image","dom-class"], "simple", [15,22]);
  E("file:"+fp, "class:"+fp+":XRWebGLSubImage", "contains", 1.0);
  E("file:"+fp, "class:"+fp+":XRWebGLSubImage", "exports", 0.8);
}

// Cross-file calls
E("function:"+P+"/xrsession.rs:new", "function:"+P+"/xrrenderstate.rs:new", "calls", 0.8);
E("function:"+P+"/xrsession.rs:event_callback", "function:"+P+"/xrsessionevent.rs:new", "calls", 0.8);
E("function:"+P+"/xrsession.rs:apply_nominal_framerate", "function:"+P+"/xrsessionevent.rs:new", "calls", 0.8);
E("function:"+P+"/xrsession.rs:event_callback", "function:"+P+"/xrreferencespaceevent.rs:new", "calls", 0.8);
E("function:"+P+"/xrsession.rs:RequestReferenceSpace", "function:"+P+"/xrreferencespace.rs:new", "calls", 0.8);
E("function:"+P+"/xrreferencespace.rs:new_inherited", "function:"+P+"/xrspace.rs:new_inherited", "calls", 0.8);
E("function:"+P+"/xrviewerpose.rs:new", "function:"+P+"/xrview.rs:new", "calls", 0.8);
E("function:"+P+"/xrviewerpose.rs:new_inherited", "function:"+P+"/xrpose.rs:new_inherited", "calls", 0.8);
E("function:"+P+"/xrrenderstate.rs:clone_object", "function:"+P+"/xrrenderstate.rs:new", "calls", 0.8);
E("function:"+P+"/xrview.rs:new", "function:"+P+"/xrrigidtransform.rs:new", "calls", 0.8);
E("function:"+P+"/xrviewerpose.rs:new", "function:"+P+"/xrrigidtransform.rs:new", "calls", 0.8);
E("function:"+P+"/xrsession.rs:raf_callback", "function:"+P+"/xrviewerpose.rs:new", "calls", 0.8);
E("function:"+P+"/xrsystem.rs:session_obtained", "function:"+P+"/xrsession.rs:new", "calls", 0.8);
E("function:"+P+"/xrwebgllayer.rs:GetViewport", "function:"+P+"/xrviewport.rs:new", "calls", 0.8);
E("function:"+P+"/xrpose.rs:new", "function:"+P+"/xrrigidtransform.rs:new", "calls", 0.8);

// ---- Split logic ----
console.log("Total nodes: " + nodes.length);
console.log("Total edges: " + edges.length);

const THRESH_N = 60, THRESH_E = 120;
const nodeCount = nodes.length;
const edgeCount = edges.length;
let parts = 1;
if (nodeCount > THRESH_N || edgeCount > THRESH_E) {
  parts = Math.ceil(Math.max(nodeCount / THRESH_N, edgeCount / THRESH_E));
}
console.log("Parts: " + parts);

const allFiles = [...new Set(nodes.filter(n => n.filePath).map(n => n.filePath))].sort();
const chunkSize = Math.ceil(allFiles.length / parts);
for (let p = 0; p < parts; p++) {
  const chunk = allFiles.slice(p * chunkSize, (p + 1) * chunkSize);
  const fileSet = new Set(chunk);
  const pn = nodes.filter(n => n.filePath && fileSet.has(n.filePath));
  const pids = new Set(pn.map(n => n.id));
  const pe = edges.filter(e => pids.has(e.source));
  const fname = parts === 1 ? "batch-27.json" : "batch-27-part-" + (p + 1) + ".json";
  fs.writeFileSync(OUT + "/" + fname, JSON.stringify({nodes: pn, edges: pe}, null, 2));
  console.log("Part " + (p+1) + ": " + pn.length + " nodes, " + pe.length + " edges -> " + fname);
}
