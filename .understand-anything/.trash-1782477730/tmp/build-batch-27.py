#!/usr/bin/env python3
"""Generate batch-27.json with nodes and edges for webxr files."""
import json, os, math

OUT = "d:/Projects/servo/.understand-anything/intermediate"
TMP = "d:/Projects/servo/.understand-anything/tmp"

nodes = []
edges = []

def N(id, type_, name, filePath, summary, tags, complexity, lineRange=None, languageNotes=None):
    n = {"id": id, "type": type_, "name": name, "filePath": filePath,
         "summary": summary, "tags": tags, "complexity": complexity}
    if lineRange: n["lineRange"] = lineRange
    if languageNotes: n["languageNotes"] = languageNotes
    nodes.append(n)

def E(source, target, type_, weight=0.7, direction="forward"):
    edges.append({"source": source, "target": target, "type": type_,
                  "direction": direction, "weight": weight})

P = "components/script/dom/webxr"

# ===== 1. xrpose.rs =====
fp = f"{P}/xrpose.rs"
N(f"file:{fp}", "file", "xrpose.rs", fp,
  "DOM binding for XRPose, representing a position and orientation in the WebXR coordinate system.",
  ["webxr", "dom-binding", "rust"], "moderate",
  languageNotes="Rust DOM binding implementing XRPose WebXR interface with transform and velocity accessors.")

N(f"class:{fp}:XRPose", "class", "XRPose", fp,
  "Represents a pose (position and orientation) in the WebXR coordinate system, with methods for accessing the transform, linear velocity, and angular velocity.",
  ["webxr", "pose", "dom-class"], "moderate", [17, 20])

N(f"function:{fp}:new_inherited", "function", "new_inherited", fp,
  "Internal constructor for XRPose that initializes the reflector and transform.",
  ["webxr", "constructor"], "simple", [23, 28])

N(f"function:{fp}:new", "function", "new", fp,
  "Public constructor for XRPose, wrapping an XRRigidTransform into a reflected DOM object.",
  ["webxr", "constructor"], "simple", [30, 37])

E(f"file:{fp}", f"class:{fp}:XRPose", "contains", 1.0)
E(f"file:{fp}", f"function:{fp}:new_inherited", "contains", 1.0)
E(f"file:{fp}", f"function:{fp}:new", "contains", 1.0)
E(f"file:{fp}", f"class:{fp}:XRPose", "exports", 0.8)
E(f"file:{fp}", f"function:{fp}:new_inherited", "exports", 0.8)
E(f"file:{fp}", f"function:{fp}:new", "exports", 0.8)

# ===== 2. xrprojectionlayer.rs =====
fp = f"{P}/xrprojectionlayer.rs"
N(f"file:{fp}", "file", "xrprojectionlayer.rs", fp,
  "Stub DOM binding for XRProjectionLayer, a WebXR projection layer type for immersive rendering. Currently a minimal placeholder with only a composition_layer property.",
  ["webxr", "dom-binding", "rust", "stub"], "simple")

# ===== 3. xrquadlayer.rs =====
fp = f"{P}/xrquadlayer.rs"
N(f"file:{fp}", "file", "xrquadlayer.rs", fp,
  "Stub DOM binding for XRQuadLayer, a WebXR quad layer type for placing 2D content in 3D space. Currently a minimal placeholder with only a composition_layer property.",
  ["webxr", "dom-binding", "rust", "stub"], "simple")

# ===== 4. xrray.rs =====
fp = f"{P}/xrray.rs"
N(f"file:{fp}", "file", "xrray.rs", fp,
  "DOM binding for XRRay, providing raycasting utilities used for hit testing and spatial queries in WebXR. Supports construction from origin/direction or from an XRRigidTransform.",
  ["webxr", "dom-binding", "raycasting", "rust"], "moderate",
  languageNotes="Implements WebXR ray mathematics including matrix computation from direction vectors.")

N(f"class:{fp}:XRRay", "class", "XRRay", fp,
  "Represents a ray in the WebXR coordinate system, with origin, direction, and 4x4 matrix representations. Supports construction from DOMPointInit origin+direction or from XRRigidTransform.",
  ["webxr", "raycasting", "dom-class"], "moderate", [25, 31])

N(f"function:{fp}:Constructor", "function", "Constructor", fp,
  "Constructs an XRRay from origin and direction DOMPointInit values, validating the w-coordinate constraints and normalizing the direction vector.",
  ["webxr", "constructor", "validation"], "moderate", [58, 86])

N(f"function:{fp}:Constructor_", "function", "Constructor_", fp,
  "Alternate constructor for XRRay from an XRRigidTransform, computing the ray direction from the transform's rotation.",
  ["webxr", "constructor"], "simple", [89, 102])

N(f"function:{fp}:Matrix", "function", "Matrix", fp,
  "Computes and returns the 4x4 matrix representation of the ray's rotation and translation, using cross products and quaternion-based rotation to handle non-standard direction vectors.",
  ["webxr", "matrix", "geometry"], "moderate", [129, 163])

E(f"file:{fp}", f"class:{fp}:XRRay", "contains", 1.0)
E(f"file:{fp}", f"function:{fp}:Constructor", "contains", 1.0)
E(f"file:{fp}", f"function:{fp}:Constructor_", "contains", 1.0)
E(f"file:{fp}", f"function:{fp}:Matrix", "contains", 1.0)
E(f"file:{fp}", f"class:{fp}:XRRay", "exports", 0.8)

# ===== 5. xrreferencespace.rs =====
fp = f"{P}/xrreferencespace.rs"
N(f"file:{fp}", "file", "xrreferencespace.rs", fp,
  "DOM binding for XRReferenceSpace, managing spatial reference frames in WebXR sessions. Supports creation of offset spaces and pose computation with floor-level transforms for different reference space types.",
  ["webxr", "dom-binding", "spatial", "rust"], "moderate")

N(f"class:{fp}:XRReferenceSpace", "class", "XRReferenceSpace", fp,
  "Provides a spatial reference frame that can be offset, used to compute poses relative to different base spaces (local, viewer, floor, bounded-floor) in a WebXR session.",
  ["webxr", "reference-space", "dom-class"], "moderate", [24, 28])

N(f"function:{fp}:new_inherited", "function", "new_inherited", fp,
  "Internal constructor for XRReferenceSpace initializing the underlying XRSpace, offset transform, and reference space type.",
  ["webxr", "constructor"], "simple", [31, 41])
N(f"function:{fp}:new", "function", "new", fp,
  "Public constructor creating an XRReferenceSpace with identity offset for the given session and type.",
  ["webxr", "constructor"], "simple", [43, 51])
N(f"function:{fp}:new_offset", "function", "new_offset", fp,
  "Creates an XRReferenceSpace with a specific offset RigidTransform3D and reference space type.",
  ["webxr", "constructor"], "simple", [53, 65])
N(f"function:{fp}:space", "function", "space", fp,
  "Returns the base WebXR space as a RigidTransform3D by composing the session floor transform with the reference space offset.",
  ["webxr", "transform"], "simple", [67, 77])
N(f"function:{fp}:ty", "function", "ty", fp,
  "Returns the XRReferenceSpaceType (local, viewer, local-floor, bounded-floor) of this reference space.",
  ["webxr", "accessor"], "simple", [79, 81])
N(f"function:{fp}:GetOffsetReferenceSpace", "function", "GetOffsetReferenceSpace", fp,
  "Creates a new XRReferenceSpace with an additional offset applied, chaining transforms for nested spatial references.",
  ["webxr", "spatial"], "simple", [86, 96])
N(f"function:{fp}:get_base_transform", "function", "get_base_transform", fp,
  "Computes the inverse pose transform to derive the base-space-to-origin transform for this reference space.",
  ["webxr", "transform"], "simple", [107, 110])
N(f"function:{fp}:get_pose", "function", "get_pose", fp,
  "Computes the pose within this reference space by combining the unoffset pose with the reference space offset transform.",
  ["webxr", "pose"], "simple", [117, 125])
N(f"function:{fp}:get_unoffset_pose", "function", "get_unoffset_pose", fp,
  "Computes the raw (unoffset) pose by querying the session's floor transform or returning identity for non-immersive sessions.",
  ["webxr", "pose"], "moderate", [130, 149])
N(f"function:{fp}:get_bounds", "function", "get_bounds", fp,
  "Returns the boundary geometry for bounded-floor reference spaces from the session.",
  ["webxr", "bounds"], "simple", [151, 155])

E(f"file:{fp}", f"class:{fp}:XRReferenceSpace", "contains", 1.0)
E(f"file:{fp}", f"function:{fp}:new_inherited", "contains", 1.0)
E(f"file:{fp}", f"function:{fp}:new", "contains", 1.0)
E(f"file:{fp}", f"function:{fp}:new_offset", "contains", 1.0)
E(f"file:{fp}", f"function:{fp}:space", "contains", 1.0)
E(f"file:{fp}", f"function:{fp}:ty", "contains", 1.0)
E(f"file:{fp}", f"function:{fp}:GetOffsetReferenceSpace", "contains", 1.0)
E(f"file:{fp}", f"function:{fp}:get_base_transform", "contains", 1.0)
E(f"file:{fp}", f"function:{fp}:get_pose", "contains", 1.0)
E(f"file:{fp}", f"function:{fp}:get_unoffset_pose", "contains", 1.0)
E(f"file:{fp}", f"function:{fp}:get_bounds", "contains", 1.0)
E(f"file:{fp}", f"class:{fp}:XRReferenceSpace", "exports", 0.8)
for fn in ["new_inherited","new","new_offset","space","ty","get_base_transform","get_pose","get_unoffset_pose","get_bounds"]:
    E(f"file:{fp}", f"function:{fp}:{fn}", "exports", 0.8)

# ===== 6. xrreferencespaceevent.rs =====
fp = f"{P}/xrreferencespaceevent.rs"
N(f"file:{fp}", "file", "xrreferencespaceevent.rs", fp,
  "DOM binding for XRReferenceSpaceEvent, representing reference space change events fired when a reference space's transform changes in a WebXR session.",
  ["webxr", "dom-binding", "event", "rust"], "moderate")

N(f"class:{fp}:XRReferenceSpaceEvent", "class", "XRReferenceSpaceEvent", fp,
  "Event type dispatched when an XRReferenceSpace's native origin or effective transform changes, carrying the affected space and optional transform.",
  ["webxr", "event", "dom-class"], "moderate", [25, 29])

N(f"function:{fp}:new", "function", "new", fp,
  "Public constructor for XRReferenceSpaceEvent with type, bubbles, cancelable, space reference, and optional transform.",
  ["webxr", "constructor"], "simple", [43, 55])
N(f"function:{fp}:new_with_proto", "function", "new_with_proto", fp,
  "Constructor for XRReferenceSpaceEvent with explicit prototype, initializing the underlying Event and tracking the space and transform.",
  ["webxr", "constructor"], "simple", [58, 79])
N(f"function:{fp}:Constructor", "function", "Constructor", fp,
  "JavaScript-exposed constructor for XRReferenceSpaceEvent, parsing init dictionary to extract space and optional transform.",
  ["webxr", "constructor"], "simple", [84, 101])

E(f"file:{fp}", f"class:{fp}:XRReferenceSpaceEvent", "contains", 1.0)
E(f"file:{fp}", f"function:{fp}:new", "contains", 1.0)
E(f"file:{fp}", f"function:{fp}:new_with_proto", "contains", 1.0)
E(f"file:{fp}", f"function:{fp}:Constructor", "contains", 1.0)
E(f"file:{fp}", f"class:{fp}:XRReferenceSpaceEvent", "exports", 0.8)
E(f"file:{fp}", f"function:{fp}:new", "exports", 0.8)

# ===== 7. xrrenderstate.rs =====
fp = f"{P}/xrrenderstate.rs"
N(f"file:{fp}", "file", "xrrenderstate.rs", fp,
  "DOM binding for XRRenderState, managing WebXR session rendering configuration including depth clipping planes, field of view, base layer, and layer arrays.",
  ["webxr", "dom-binding", "rendering", "rust"], "moderate")

N(f"class:{fp}:XRRenderState", "class", "XRRenderState", fp,
  "State container for WebXR rendering configuration: depth near/far, inline vertical FOV, base layer, and ordered layer array with sub-image validation.",
  ["webxr", "rendering", "dom-class"], "moderate", [24, 31])

N(f"function:{fp}:new_inherited", "function", "new_inherited", fp,
  "Internal constructor initializing all render state fields including depth planes, FOV, base layer, and layer list with DomRefCell.",
  ["webxr", "constructor"], "simple", [34, 50])
N(f"function:{fp}:new", "function", "new", fp,
  "Public constructor for XRRenderState, reflecting the DOM object with the provided rendering parameters.",
  ["webxr", "constructor"], "simple", [52, 72])
N(f"function:{fp}:clone_object", "function", "clone_object", fp,
  "Clones this render state by creating a new XRRenderState with the same depth, FOV, base layer, and layer values.",
  ["webxr", "clone"], "simple", [74, 84])
N(f"function:{fp}:with_layers", "function", "with_layers", fp,
  "Applies a closure to the layers list, providing safe mutable access to the stored layer references.",
  ["webxr", "layers"], "simple", [102, 108])
N(f"function:{fp}:has_sub_images", "function", "has_sub_images", fp,
  "Validates that all layers in the render state have corresponding sub-images, checking each layer's layer_id against the provided sub-image list.",
  ["webxr", "validation", "layers"], "moderate", [109, 128])

E(f"file:{fp}", f"class:{fp}:XRRenderState", "contains", 1.0)
E(f"file:{fp}", f"function:{fp}:new_inherited", "contains", 1.0)
E(f"file:{fp}", f"function:{fp}:new", "contains", 1.0)
E(f"file:{fp}", f"function:{fp}:clone_object", "contains", 1.0)
E(f"file:{fp}", f"function:{fp}:with_layers", "contains", 1.0)
E(f"file:{fp}", f"function:{fp}:has_sub_images", "contains", 1.0)
E(f"file:{fp}", f"class:{fp}:XRRenderState", "exports", 0.8)
for fn in ["new_inherited","new","clone_object","with_layers","has_sub_images"]:
    E(f"file:{fp}", f"function:{fp}:{fn}", "exports", 0.8)

# ===== 8. xrrigidtransform.rs =====
fp = f"{P}/xrrigidtransform.rs"
N(f"file:{fp}", "file", "xrrigidtransform.rs", fp,
  "DOM binding for XRRigidTransform, representing a 3D rigid transform with position and orientation quaternion. Used extensively throughout WebXR for coordinate system transforms.",
  ["webxr", "dom-binding", "transform", "rust"], "moderate",
  languageNotes="Implements 3D rigid body transforms using quaternion rotation and matrix decomposition. Validates finite values for position and orientation.")

N(f"class:{fp}:XRRigidTransform", "class", "XRRigidTransform", fp,
  "Encapsulates a 3D rigid transform (rotation + translation) with lazy-computed inverse, cached position/orientation DOMPoints, and 4x4 matrix representation.",
  ["webxr", "transform", "3d", "dom-class"], "moderate", [25, 34])

N(f"function:{fp}:new", "function", "new", fp,
  "Public constructor for XRRigidTransform delegating to new_with_proto with the given RigidTransform3D.",
  ["webxr", "constructor"], "simple", [48, 54])
N(f"function:{fp}:new_with_proto", "function", "new_with_proto", fp,
  "Constructor with explicit prototype, reflecting the DOM object and initializing the transform's internal RigidTransform3D.",
  ["webxr", "constructor"], "simple", [56, 68])
N(f"function:{fp}:identity", "function", "identity", fp,
  "Static factory returning an XRRigidTransform representing the identity transform (zero translation, unit quaternion).",
  ["webxr", "constructor", "factory"], "simple", [70, 73])
N(f"function:{fp}:Constructor", "function", "Constructor", fp,
  "JavaScript constructor for XRRigidTransform from DOMPointInit position and orientation, validating finite values, normalizing the quaternion, and building the RigidTransform3D.",
  ["webxr", "constructor", "validation"], "moderate", [78, 129])
N(f"function:{fp}:Orientation", "function", "Orientation", fp,
  "Returns the orientation as a DOMPointReadOnly, lazily initialized from the rotation quaternion components.",
  ["webxr", "accessor", "orientation"], "simple", [139, 151])
N(f"function:{fp}:Matrix", "function", "Matrix", fp,
  "Computes and returns the 4x4 transformation matrix from the rigid transform, cached as a Float64Array typed array.",
  ["webxr", "matrix", "transform"], "simple", [162, 172])

E(f"file:{fp}", f"class:{fp}:XRRigidTransform", "contains", 1.0)
E(f"file:{fp}", f"function:{fp}:new", "contains", 1.0)
E(f"file:{fp}", f"function:{fp}:new_with_proto", "contains", 1.0)
E(f"file:{fp}", f"function:{fp}:identity", "contains", 1.0)
E(f"file:{fp}", f"function:{fp}:Constructor", "contains", 1.0)
E(f"file:{fp}", f"function:{fp}:Orientation", "contains", 1.0)
E(f"file:{fp}", f"function:{fp}:Matrix", "contains", 1.0)
E(f"file:{fp}", f"class:{fp}:XRRigidTransform", "exports", 0.8)
E(f"file:{fp}", f"function:{fp}:new", "exports", 0.8)
E(f"file:{fp}", f"function:{fp}:identity", "exports", 0.8)

# ===== 9. xrsession.rs =====
fp = f"{P}/xrsession.rs"
N(f"file:{fp}", "file", "xrsession.rs", fp,
  "Core DOM binding for XRSession, the central WebXR session manager handling rendering loops, input sources, reference spaces, hit testing, frame lifecycle, and framerate management. The largest and most complex file in the WebXR module.",
  ["webxr", "dom-binding", "session", "rust", "core"], "complex",
  languageNotes="Implements the full XRSession WebXR spec interface with ~40 methods covering frame callbacks, render state updates, input handling, reference space creation, hit testing, and framerate control.")

N(f"class:{fp}:XRSession", "class", "XRSession", fp,
  "Central class managing a WebXR session lifecycle: rendering loop via requestAnimationFrame, render state transitions, device input handling, reference space management, hit testing, and framerate control.",
  ["webxr", "session", "dom-class", "core"], "complex", [78, 118])

# Significant functions in XRSession
N(f"function:{fp}:new_inherited", "function", "new_inherited", fp,
  "Internal constructor for XRSession, initializing all session state fields: blend mode, visibility, render state, input sources, RAF state, hit test state, and reference spaces.",
  ["webxr", "constructor"], "moderate", [121, 153])
N(f"function:{fp}:new", "function", "new", fp,
  "Public constructor for XRSession, creating the render state with defaults, input source array, setting up RAF loop and event handler.",
  ["webxr", "constructor"], "moderate", [155, 182])
N(f"function:{fp}:with_session", "function", "with_session", fp,
  "Applies a closure to the underlying WebXR session handle, providing safe access to the session's device-level API.",
  ["webxr", "session"], "simple", [184, 187])
N(f"function:{fp}:is_ended", "function", "is_ended", fp,
  "Returns whether the session has ended.",
  ["webxr", "state"], "simple", [189, 191])
N(f"function:{fp}:is_immersive", "function", "is_immersive", fp,
  "Returns whether this is an immersive (VR) session rather than inline.",
  ["webxr", "state"], "simple", [193, 195])
N(f"function:{fp}:has_layers_feature", "function", "has_layers_feature", fp,
  "Checks if the layer management feature is available for this session.",
  ["webxr", "features"], "simple", [198, 202])
N(f"function:{fp}:setup_raf_loop", "function", "setup_raf_loop", fp,
  "Sets up the animation frame loop by registering a typed IPC route and queuing render loop messages through the DOM manipulation task source.",
  ["webxr", "rendering", "raf"], "moderate", [204, 224])
N(f"function:{fp}:is_outside_raf", "function", "is_outside_raf", fp,
  "Returns whether the session is currently outside of a requestAnimationFrame callback, used to control synchronous operations.",
  ["webxr", "state"], "simple", [226, 228])
N(f"function:{fp}:attach_event_handler", "function", "attach_event_handler", fp,
  "Attaches the XR event handler callback, registering a ProfileGenericCallback through the IPC router and setting the event destination on the device session.",
  ["webxr", "events"], "moderate", [230, 248])
N(f"function:{fp}:setup_initial_inputs", "function", "setup_initial_inputs", fp,
  "Processes initial input sources from the WebXR device session, queuing addinput events via the task source for each initial input.",
  ["webxr", "input"], "moderate", [255, 273])
N(f"function:{fp}:event_callback", "function", "event_callback", fp,
  "Main XR event handler dispatching session events (end, visibilitychange, inputsource change, reference space reset) and managing the input source lifecycle and frame tracking.",
  ["webxr", "events", "dispatcher"], "complex", [275, 419])
N(f"function:{fp}:raf_callback", "function", "raf_callback", fp,
  "Main requestAnimationFrame handler managing render state transitions, inline projection updates, frame event handling, sub-image validation, rendering, and callback invocation.",
  ["webxr", "rendering", "raf"], "complex", [422, 506])
N(f"function:{fp}:update_inline_projection_matrix", "function", "update_inline_projection_matrix", fp,
  "Computes the inline (non-immersive) projection matrix using the base layer size, depth clipping planes, and vertical FOV.",
  ["webxr", "projection"], "moderate", [508, 530])
N(f"function:{fp}:inline_view", "function", "inline_view", fp,
  "Returns the view for inline sessions, using identity transform and the computed inline projection matrix.",
  ["webxr", "view"], "simple", [533, 540])
N(f"function:{fp}:session_id", "function", "session_id", fp,
  "Returns the unique ID of the underlying WebXR device session.",
  ["webxr", "session"], "simple", [542, 544])
N(f"function:{fp}:dirty_layers", "function", "dirty_layers", fp,
  "Marks all active layers as dirty when the render state's base layer changes, triggering recompilation.",
  ["webxr", "layers"], "simple", [546, 550])
N(f"function:{fp}:handle_frame_event", "function", "handle_frame_event", fp,
  "Processes individual frame events from the device, resolving pending hit test promises and applying frame events to the session state.",
  ["webxr", "events"], "moderate", [587, 603])
N(f"function:{fp}:apply_nominal_framerate", "function", "apply_nominal_framerate", fp,
  "Applies a nominal framerate change from the device, firing a frameratechange event on the session if the rate changed.",
  ["webxr", "framerate"], "moderate", [606, 622])
N(f"function:{fp}:UpdateRenderState", "function", "UpdateRenderState", fp,
  "Updates the session render state with new depth values, inline FOV, base layer, and layer array. Validates layer ownership, checks for duplicates, and pushes updates to the device session.",
  ["webxr", "rendering", "state"], "complex", [670, 793])
N(f"function:{fp}:CancelAnimationFrame", "function", "CancelAnimationFrame", fp,
  "Cancels a pending requestAnimationFrame callback by removing it from either the pending or current callback list.",
  ["webxr", "raf"], "simple", [808, 818])
N(f"function:{fp}:RequestReferenceSpace", "function", "RequestReferenceSpace", fp,
  "Creates a reference space of the requested type, validating feature grants, creating bounded or unbounded reference spaces, and tracking them in the session.",
  ["webxr", "reference-space"], "complex", [831, 887])
N(f"function:{fp}:End", "function", "End", fp,
  "Ends the WebXR session, resolving the promise, cleaning up input sources, and notifying the XR system.",
  ["webxr", "session"], "moderate", [895, 927])
N(f"function:{fp}:RequestHitTestSource", "function", "RequestHitTestSource", fp,
  "Requests a hit test source with the given options (space, ray, entity types), interacting with the device session's hit testing API.",
  ["webxr", "hittest"], "complex", [930, 988])
N(f"function:{fp}:GetSupportedFrameRates", "function", "GetSupportedFrameRates", fp,
  "Returns an array of supported framerates from the device session as Float64 values.",
  ["webxr", "framerate"], "moderate", [1008, 1028])
N(f"function:{fp}:UpdateTargetFrameRate", "function", "UpdateTargetFrameRate", fp,
  "Updates the session's target framerate, validating the rate is supported, setting up a callback for the device response, and resolving the returned promise.",
  ["webxr", "framerate"], "complex", [1045, 1090])
N(f"function:{fp}:cast_transform", "function", "cast_transform", fp,
  "Utility function performing an unsafe transmute to cast between RigidTransform3D types with different coordinate units.",
  ["webxr", "utility"], "simple", [1104, 1108])

E(f"file:{fp}", f"class:{fp}:XRSession", "contains", 1.0)
for fn in ["new_inherited","new","with_session","is_ended","is_immersive","has_layers_feature",
           "setup_raf_loop","is_outside_raf","attach_event_handler","setup_initial_inputs",
           "event_callback","raf_callback","update_inline_projection_matrix","inline_view",
           "session_id","dirty_layers","handle_frame_event","apply_nominal_framerate",
           "UpdateRenderState","CancelAnimationFrame","RequestReferenceSpace","End",
           "RequestHitTestSource","GetSupportedFrameRates","UpdateTargetFrameRate","cast_transform"]:
    E(f"file:{fp}", f"function:{fp}:{fn}", "contains", 1.0)
E(f"file:{fp}", f"class:{fp}:XRSession", "exports", 0.8)
for fn in ["with_session","is_ended","is_immersive","has_layers_feature","is_outside_raf",
           "setup_initial_inputs","inline_view","session_id","dirty_layers","cast_transform"]:
    E(f"file:{fp}", f"function:{fp}:{fn}", "exports", 0.8)

# ===== 10. xrsessionevent.rs =====
fp = f"{P}/xrsessionevent.rs"
N(f"file:{fp}", "file", "xrsessionevent.rs", fp,
  "DOM binding for XRSessionEvent, representing session lifecycle events (end, visibilitychange, frameratechange) in WebXR.",
  ["webxr", "dom-binding", "event", "rust"], "moderate")

N(f"class:{fp}:XRSessionEvent", "class", "XRSessionEvent", fp,
  "Event type dispatched for XRSession lifecycle changes, carrying a reference to the affected XRSession and supporting standard event properties.",
  ["webxr", "event", "dom-class"], "moderate", [22, 25])

N(f"function:{fp}:new", "function", "new", fp,
  "Public constructor for XRSessionEvent with type, bubbles, cancelable, and session reference.",
  ["webxr", "constructor"], "simple", [35, 44])
N(f"function:{fp}:new_with_proto", "function", "new_with_proto", fp,
  "Constructor for XRSessionEvent with explicit prototype, initializing the underlying event and storing the session reference.",
  ["webxr", "constructor"], "simple", [46, 66])
N(f"function:{fp}:Constructor", "function", "Constructor", fp,
  "JavaScript-exposed constructor for XRSessionEvent from init dictionary, extracting the session property from XRSessionEventInit.",
  ["webxr", "constructor"], "simple", [71, 87])

E(f"file:{fp}", f"class:{fp}:XRSessionEvent", "contains", 1.0)
E(f"file:{fp}", f"function:{fp}:new", "contains", 1.0)
E(f"file:{fp}", f"function:{fp}:new_with_proto", "contains", 1.0)
E(f"file:{fp}", f"function:{fp}:Constructor", "contains", 1.0)
E(f"file:{fp}", f"class:{fp}:XRSessionEvent", "exports", 0.8)
E(f"file:{fp}", f"function:{fp}:new", "exports", 0.8)

# ===== 11. xrspace.rs =====
fp = f"{P}/xrspace.rs"
N(f"file:{fp}", "file", "xrspace.rs", fp,
  "DOM binding for XRSpace, the base spatial reference type in WebXR coordinate systems. Provides generic pose lookups across different space subtypes (reference, joint, input source).",
  ["webxr", "dom-binding", "spatial", "rust"], "moderate")

N(f"class:{fp}:XRSpace", "class", "XRSpace", fp,
  "Base class for all WebXR spatial reference types. Supports downcasting to XRReferenceSpace and XRJointSpace, and resolves input source grip/target-ray spaces.",
  ["webxr", "spatial", "base", "dom-class"], "moderate", [21, 27])

N(f"function:{fp}:new_inherited", "function", "new_inherited", fp,
  "Internal constructor for XRSpace, initializing the EventTarget base and session reference.",
  ["webxr", "constructor"], "simple", [30, 37])
N(f"function:{fp}:new_inputspace_inner", "function", "new_inputspace_inner", fp,
  "Internal constructor for input-source-based spaces, storing the input source reference and grip/ray type flag.",
  ["webxr", "constructor", "input"], "simple", [39, 50])
N(f"function:{fp}:new_inputspace", "function", "new_inputspace", fp,
  "Public constructor for creating an input-source space, reflecting the DOM object and delegating to new_inputspace_inner.",
  ["webxr", "constructor", "input"], "simple", [52, 64])
N(f"function:{fp}:space", "function", "space", fp,
  "Resolves the underlying WebXR RigidTransform3D for this space by downcasting to reference or joint space, or looking up the input source's pose.",
  ["webxr", "transform"], "moderate", [66, 84])
N(f"function:{fp}:get_pose", "function", "get_pose", fp,
  "Gets the pose for this space from the base pose by downcasting or looking up input source grip/target-ray transforms.",
  ["webxr", "pose"], "moderate", [93, 118])
N(f"function:{fp}:session", "function", "session", fp,
  "Returns the XRSession associated with this space.",
  ["webxr", "accessor"], "simple", [120, 122])

E(f"file:{fp}", f"class:{fp}:XRSpace", "contains", 1.0)
for fn in ["new_inherited","new_inputspace_inner","new_inputspace","space","get_pose","session"]:
    E(f"file:{fp}", f"function:{fp}:{fn}", "contains", 1.0)
E(f"file:{fp}", f"class:{fp}:XRSpace", "exports", 0.8)
for fn in ["new_inherited","new_inputspace","space","get_pose","session"]:
    E(f"file:{fp}", f"function:{fp}:{fn}", "exports", 0.8)

# ===== 12. xrsubimage.rs =====
fp = f"{P}/xrsubimage.rs"
N(f"file:{fp}", "file", "xrsubimage.rs", fp,
  "DOM binding for XRSubImage, the base type representing a sub-image within a WebXR layer. Provides access to the associated viewport.",
  ["webxr", "dom-binding", "layers", "rust"], "simple")

N(f"class:{fp}:XRSubImage", "class", "XRSubImage", fp,
  "Base class for sub-image types in WebXR layers, providing viewport access to define the region of a layer used for rendering.",
  ["webxr", "sub-image", "dom-class"], "simple", [13, 16])

E(f"file:{fp}", f"class:{fp}:XRSubImage", "contains", 1.0)
E(f"file:{fp}", f"class:{fp}:XRSubImage", "exports", 0.8)

# ===== 13. xrsystem.rs =====
fp = f"{P}/xrsystem.rs"
N(f"file:{fp}", "file", "xrsystem.rs", fp,
  "DOM binding for XRSystem (navigator.xr), the entry point for WebXR device discovery, session creation, and device registration. Implements isSessionSupported, requestSession, and device simulation interfaces.",
  ["webxr", "dom-binding", "system", "rust", "entry-point"], "complex")

N(f"class:{fp}:XRSystem", "class", "XRSystem", fp,
  "Entry point for the WebXR API, managing device availability queries, session requests (both immersive and inline), active session tracking, and the test API for automated testing.",
  ["webxr", "system", "dom-class", "entry-point"], "complex", [41, 50])

N(f"function:{fp}:new", "function", "new", fp,
  "Public constructor for XRSystem, reflecting the DOM object with the given pipeline ID.",
  ["webxr", "constructor"], "simple", [65, 71])
N(f"function:{fp}:pending_or_active_session", "function", "pending_or_active_session", fp,
  "Returns whether there is a pending or active immersive session.",
  ["webxr", "state"], "simple", [73, 75])
N(f"function:{fp}:set_pending", "function", "set_pending", fp,
  "Marks the XRSystem as having a pending immersive session.",
  ["webxr", "state"], "simple", [77, 79])
N(f"function:{fp}:set_active_immersive_session", "function", "set_active_immersive_session", fp,
  "Transitions a pending session to active and clears the pending flag.",
  ["webxr", "state"], "simple", [81, 86])
N(f"function:{fp}:end_session", "function", "end_session", fp,
  "Ends the active immersive session, cleaning up input sources and removing the session from the active inline sessions list.",
  ["webxr", "session"], "moderate", [89, 102])
N(f"function:{fp}:IsSessionSupported", "function", "IsSessionSupported", fp,
  "Queries the XR device registry to determine if a given session mode is supported, returning the result via a Promise.",
  ["webxr", "capability"], "moderate", [117, 155])
N(f"function:{fp}:RequestSession", "function", "RequestSession", fp,
  "Requests a new WebXR session with the given mode and optional feature descriptors, handling user interaction checks, feature validation, IPC channel setup, and device registry communication.",
  ["webxr", "session", "request"], "complex", [158, 265])
N(f"function:{fp}:session_obtained", "function", "session_obtained", fp,
  "Callback invoked when a device session is obtained, creating the DOM XRSession and registering it as active.",
  ["webxr", "callback"], "moderate", [274, 305])
N(f"function:{fp}:dispatch_sessionavailable", "function", "dispatch_sessionavailable", fp,
  "Dispatches a sessionavailable event on the XRSystem to notify that a device is available for session requests.",
  ["webxr", "events"], "simple", [308, 319])

E(f"file:{fp}", f"class:{fp}:XRSystem", "contains", 1.0)
for fn in ["new","pending_or_active_session","set_pending","set_active_immersive_session",
           "end_session","IsSessionSupported","RequestSession","session_obtained","dispatch_sessionavailable"]:
    E(f"file:{fp}", f"function:{fp}:{fn}", "contains", 1.0)
E(f"file:{fp}", f"class:{fp}:XRSystem", "exports", 0.8)
for fn in ["new","pending_or_active_session","set_pending","set_active_immersive_session",
           "end_session","dispatch_sessionavailable"]:
    E(f"file:{fp}", f"function:{fp}:{fn}", "exports", 0.8)

# ===== 14. xrtest.rs =====
fp = f"{P}/xrtest.rs"
N(f"file:{fp}", "file", "xrtest.rs", fp,
  "DOM binding for XRTest, providing the WebXR test API for simulating device connections, user activation, and device disconnection in automated testing environments.",
  ["webxr", "dom-binding", "test", "rust", "testing"], "moderate")

N(f"class:{fp}:XRTest", "class", "XRTest", fp,
  "Test controller for WebXR that allows automated tests to simulate XR device connections, user activation events, and device disconnection with callback-based IPC.",
  ["webxr", "test", "dom-class"], "moderate", [34, 37])

N(f"function:{fp}:new_inherited", "function", "new_inherited", fp,
  "Internal constructor for XRTest, initializing the reflector and empty devices list.",
  ["webxr", "constructor"], "simple", [40, 45])
N(f"function:{fp}:new", "function", "new", fp,
  "Public constructor for XRTest, reflecting the DOM object.",
  ["webxr", "constructor"], "simple", [47, 49])
N(f"function:{fp}:device_obtained", "function", "device_obtained", fp,
  "Callback when a fake device is created, adding it to the connected devices list and resolving the simulation promise.",
  ["webxr", "callback", "test"], "simple", [51, 67])
N(f"function:{fp}:SimulateDeviceConnection", "function", "SimulateDeviceConnection", fp,
  "Simulates connecting a WebXR device with specified properties (origin, views, features, world), communicating with the XR device registry through IPC.",
  ["webxr", "test", "simulation"], "complex", [72, 181])
N(f"function:{fp}:DisconnectAllDevices", "function", "DisconnectAllDevices", fp,
  "Disconnects all simulated devices sequentially, waiting for each disconnect callback before resolving the promise.",
  ["webxr", "test", "cleanup"], "moderate", [191, 227])

E(f"file:{fp}", f"class:{fp}:XRTest", "contains", 1.0)
for fn in ["new_inherited","new","device_obtained","SimulateDeviceConnection","DisconnectAllDevices"]:
    E(f"file:{fp}", f"function:{fp}:{fn}", "contains", 1.0)
E(f"file:{fp}", f"class:{fp}:XRTest", "exports", 0.8)
for fn in ["new_inherited","new"]:
    E(f"file:{fp}", f"function:{fp}:{fn}", "exports", 0.8)

# ===== 15. xrview.rs =====
fp = f"{P}/xrview.rs"
N(f"file:{fp}", "file", "xrview.rs", fp,
  "DOM binding for XRView, representing a single eye view within a WebXR viewer pose with projection matrix, transform, and viewport scale functionality.",
  ["webxr", "dom-binding", "view", "rust"], "moderate")

N(f"class:{fp}:XRView", "class", "XRView", fp,
  "Represents a single view (eye) in a WebXR frame, providing eye type, projection matrix, view transform, recommended viewport scale, and first-person observer status.",
  ["webxr", "view", "dom-class"], "moderate", [24, 35])

N(f"function:{fp}:new_inherited", "function", "new_inherited", fp,
  "Internal constructor for XRView initializing session reference, view transform, eye type, projection matrix, and viewport index.",
  ["webxr", "constructor"], "simple", [38, 55])
N(f"function:{fp}:new", "function", "new", fp,
  "Public constructor for XRView, creating the rigid transform from view data and reflecting the DOM object.",
  ["webxr", "constructor"], "moderate", [57, 80])
N(f"function:{fp}:session", "function", "session", fp,
  "Returns the XRSession that this view belongs to.",
  ["webxr", "accessor"], "simple", [82, 84])
N(f"function:{fp}:viewport_index", "function", "viewport_index", fp,
  "Returns the viewport index of this view within the layer's sub-image array.",
  ["webxr", "accessor"], "simple", [86, 88])
N(f"function:{fp}:ProjectionMatrix", "function", "ProjectionMatrix", fp,
  "Returns the 4x4 projection matrix for this view as a Float64Array typed array, cached to avoid recomputation.",
  ["webxr", "projection", "matrix"], "simple", [98, 109])

E(f"file:{fp}", f"class:{fp}:XRView", "contains", 1.0)
for fn in ["new_inherited","new","session","viewport_index","ProjectionMatrix"]:
    E(f"file:{fp}", f"function:{fp}:{fn}", "contains", 1.0)
E(f"file:{fp}", f"class:{fp}:XRView", "exports", 0.8)
for fn in ["new","session","viewport_index"]:
    E(f"file:{fp}", f"function:{fp}:{fn}", "exports", 0.8)

# ===== 16. xrviewerpose.rs =====
fp = f"{P}/xrviewerpose.rs"
N(f"file:{fp}", "file", "xrviewerpose.rs", fp,
  "DOM binding for XRViewerPose, representing the viewer's head pose with all associated eye views (left/right) constructed from the device's viewer pose data.",
  ["webxr", "dom-binding", "pose", "rust"], "moderate")

N(f"class:{fp}:XRViewerPose", "class", "XRViewerPose", fp,
  "Extends XRPose with an array of XRViews, populated from the device's viewer pose, including both inline single-view and immersive stereo/multi-view configurations.",
  ["webxr", "pose", "view", "dom-class"], "moderate", [28, 32])

N(f"function:{fp}:new", "function", "new", fp,
  "Constructs XRViewerPose by creating all eye views from the device viewer pose or session inline view, building the rigid transform from the pose data, and setting up the views array.",
  ["webxr", "constructor", "pose"], "complex", [42, 191])

E(f"file:{fp}", f"class:{fp}:XRViewerPose", "contains", 1.0)
E(f"file:{fp}", f"function:{fp}:new", "contains", 1.0)
E(f"file:{fp}", f"class:{fp}:XRViewerPose", "exports", 0.8)
E(f"file:{fp}", f"function:{fp}:new", "exports", 0.8)

# ===== 17. xrviewport.rs =====
fp = f"{P}/xrviewport.rs"
N(f"file:{fp}", "file", "xrviewport.rs", fp,
  "DOM binding for XRViewport, representing a rectangular viewport (x, y, width, height) within a WebXR framebuffer.",
  ["webxr", "dom-binding", "viewport", "rust"], "simple")

N(f"class:{fp}:XRViewport", "class", "XRViewport", fp,
  "Represents a rectangular viewport region used to render WebXR content into a framebuffer, providing x, y, width, and height properties.",
  ["webxr", "viewport", "dom-class"], "simple", [16, 20])

N(f"function:{fp}:new", "function", "new", fp,
  "Public constructor creating an XRViewport from a webxr_api::Viewport, reflecting the DOM object.",
  ["webxr", "constructor"], "simple", [30, 36])

E(f"file:{fp}", f"class:{fp}:XRViewport", "contains", 1.0)
E(f"file:{fp}", f"function:{fp}:new", "contains", 1.0)
E(f"file:{fp}", f"class:{fp}:XRViewport", "exports", 0.8)
E(f"file:{fp}", f"function:{fp}:new", "exports", 0.8)

# ===== 18. xrwebglbinding.rs =====
fp = f"{P}/xrwebglbinding.rs"
N(f"file:{fp}", "file", "xrwebglbinding.rs", fp,
  "DOM binding for XRWebGLBinding, managing the WebGL integration for WebXR layer creation. Currently provides stub methods for various layer types returning NotSupported.",
  ["webxr", "dom-binding", "webgl", "rust"], "moderate")

N(f"class:{fp}:XRWebGLBinding", "class", "XRWebGLBinding", fp,
  "WebGL binding for WebXR, providing factory methods for creating XR layers (projection, quad, cylinder, equirect, cube) and retrieving sub-images. Most layer creation methods are currently stubs.",
  ["webxr", "webgl", "binding", "dom-class"], "moderate", [34, 38])

N(f"function:{fp}:new_inherited", "function", "new_inherited", fp,
  "Internal constructor for XRWebGLBinding, storing session and WebGL context references.",
  ["webxr", "constructor"], "simple", [41, 50])
N(f"function:{fp}:new", "function", "new", fp,
  "Constructor with prototype for XRWebGLBinding, reflecting the DOM object.",
  ["webxr", "constructor"], "simple", [52, 65])
N(f"function:{fp}:Constructor", "function", "Constructor", fp,
  "JavaScript constructor for XRWebGLBinding, validating the session is not ended, context is not lost, and session is immersive.",
  ["webxr", "constructor", "validation"], "moderate", [70, 101])

E(f"file:{fp}", f"class:{fp}:XRWebGLBinding", "contains", 1.0)
E(f"file:{fp}", f"function:{fp}:new_inherited", "contains", 1.0)
E(f"file:{fp}", f"function:{fp}:new", "contains", 1.0)
E(f"file:{fp}", f"function:{fp}:Constructor", "contains", 1.0)
E(f"file:{fp}", f"class:{fp}:XRWebGLBinding", "exports", 0.8)
E(f"file:{fp}", f"function:{fp}:new_inherited", "exports", 0.8)

# ===== 19. xrwebgllayer.rs =====
fp = f"{P}/xrwebgllayer.rs"
N(f"file:{fp}", "file", "xrwebgllayer.rs", fp,
  "DOM binding for XRWebGLLayer, managing the WebGL framebuffer lifecycle for WebXR rendering. Handles begin/end frame operations, texture binding, and viewport computation.",
  ["webxr", "dom-binding", "webgl", "layer", "rust"], "complex",
  languageNotes="Integrates with the WebGL command system to bind/unbind XR textures during frame rendering. Uses texture2d_even_if_opaque for framebuffer attachment.")

N(f"class:{fp}:XRWebGLLayer", "class", "XRWebGLLayer", fp,
  "WebGL layer for WebXR that manages a framebuffer with color and depth/stencil attachments, handles begin_frame/end_frame texture binding, and computes viewports from the session's view configuration.",
  ["webxr", "webgl", "layer", "dom-class"], "complex", [52, 61])

N(f"function:{fp}:new_inherited", "function", "new_inherited", fp,
  "Internal constructor for XRWebGLLayer, initializing the XRLayer base, framebuffer, and rendering properties (antialias, depth/stencil, alpha, ignore_depth_values).",
  ["webxr", "constructor"], "moderate", [64, 80])
N(f"function:{fp}:new", "function", "new", fp,
  "Constructor with prototype for XRWebGLLayer, reflecting the DOM object.",
  ["webxr", "constructor"], "simple", [83, 105])
N(f"function:{fp}:layer_id", "function", "layer_id", fp,
  "Returns the WebXR layer ID assigned by the device session.",
  ["webxr", "layer"], "simple", [107, 109])
N(f"function:{fp}:context_id", "function", "context_id", fp,
  "Returns the WebGL context ID associated with this layer.",
  ["webxr", "webgl"], "simple", [111, 113])
N(f"function:{fp}:session", "function", "session", fp,
  "Returns the XRSession that owns this layer.",
  ["webxr", "session"], "simple", [115, 117])
N(f"function:{fp}:size", "function", "size", fp,
  "Returns the framebuffer size in pixels, falling back to the WebGL context size if no framebuffer exists.",
  ["webxr", "size"], "simple", [119, 129])
N(f"function:{fp}:begin_frame", "function", "begin_frame", fp,
  "Begins an XR frame by binding the WebXR color and depth textures to the framebuffer, saving the current GL state and issuing texture/rendering commands.",
  ["webxr", "rendering", "frame"], "complex", [139, 203])
N(f"function:{fp}:end_frame", "function", "end_frame", fp,
  "Ends an XR frame by detaching textures from the framebuffer, restoring the previous framebuffer binding, and flushing the WebGL context.",
  ["webxr", "rendering", "frame"], "moderate", [205, 228])
N(f"function:{fp}:context", "function", "context", fp,
  "Returns the underlying WebGL context for this layer.",
  ["webxr", "webgl"], "simple", [230, 232])
N(f"function:{fp}:Constructor", "function", "Constructor", fp,
  "JavaScript constructor for XRWebGLLayer, validating session state, creating a WebGL framebuffer, computing the layer size, and registering the layer with the device session.",
  ["webxr", "constructor"], "complex", [237, 294])
N(f"function:{fp}:GetViewport", "function", "GetViewport", fp,
  "Computes the viewport rect for a given XRView by querying the session's viewport configuration or using the full layer size.",
  ["webxr", "viewport"], "moderate", [339, 361])

E(f"file:{fp}", f"class:{fp}:XRWebGLLayer", "contains", 1.0)
for fn in ["new_inherited","new","layer_id","context_id","session","size","begin_frame",
           "end_frame","context","Constructor","GetViewport"]:
    E(f"file:{fp}", f"function:{fp}:{fn}", "contains", 1.0)
E(f"file:{fp}", f"class:{fp}:XRWebGLLayer", "exports", 0.8)
for fn in ["new_inherited","layer_id","context_id","session","size","begin_frame","end_frame","context"]:
    E(f"file:{fp}", f"function:{fp}:{fn}", "exports", 0.8)

# ===== 20. xrwebglsubimage.rs =====
fp = f"{P}/xrwebglsubimage.rs"
N(f"file:{fp}", "file", "xrwebglsubimage.rs", fp,
  "DOM binding for XRWebGLSubImage, extending XRSubImage with access to WebGL color and depth/stencil textures, image index, and texture dimensions.",
  ["webxr", "dom-binding", "webgl", "sub-image", "rust"], "simple")

N(f"class:{fp}:XRWebGLSubImage", "class", "XRWebGLSubImage", fp,
  "WebGL-specific sub-image providing color texture, depth/stencil texture, image index (for array textures), and texture width/height for WebXR layer rendering.",
  ["webxr", "webgl", "sub-image", "dom-class"], "simple", [15, 22])

E(f"file:{fp}", f"class:{fp}:XRWebGLSubImage", "contains", 1.0)
E(f"file:{fp}", f"class:{fp}:XRWebGLSubImage", "exports", 0.8)

# ===== CROSS-FILE CALLS EDGES =====
# Based on callGraph analysis across files
# XRSession calls XRRenderState::new
E(f"function:{P}/xrsession.rs:new", f"function:{P}/xrrenderstate.rs:new", "calls", 0.8)
# XRSession calls XRSessionEvent::new
E(f"function:{P}/xrsession.rs:event_callback", f"function:{P}/xrsessionevent.rs:new", "calls", 0.8)
E(f"function:{P}/xrsession.rs:apply_nominal_framerate", f"function:{P}/xrsessionevent.rs:new", "calls", 0.8)
# XRSession calls XRReferenceSpaceEvent::new
E(f"function:{P}/xrsession.rs:event_callback", f"function:{P}/xrreferencespaceevent.rs:new", "calls", 0.8)
# XRSession calls XRReferenceSpace::new
E(f"function:{P}/xrsession.rs:RequestReferenceSpace", f"function:{P}/xrreferencespace.rs:new", "calls", 0.8)
# XRReferenceSpace calls XRSpace::new_inherited
E(f"function:{P}/xrreferencespace.rs:new_inherited", f"function:{P}/xrspace.rs:new_inherited", "calls", 0.8)
# XRViewerPose calls XRView::new
E(f"function:{P}/xrviewerpose.rs:new", f"function:{P}/xrview.rs:new", "calls", 0.8)
# XRViewerPose calls XRPose::new_inherited
E(f"function:{P}/xrviewerpose.rs:new_inherited", f"function:{P}/xrpose.rs:new_inherited", "calls", 0.8)
# XRRenderState calls XRRenderState::new (clone_object)
E(f"function:{P}/xrrenderstate.rs:clone_object", f"function:{P}/xrrenderstate.rs:new", "calls", 0.8)
# XRView calls XRRigidTransform::new (via XRView::new)
E(f"function:{P}/xrview.rs:new", f"function:{P}/xrrigidtransform.rs:new", "calls", 0.8)
# XRViewerPose calls XRRigidTransform::new
E(f"function:{P}/xrviewerpose.rs:new", f"function:{P}/xrrigidtransform.rs:new", "calls", 0.8)
# XRSession calls cast_transform (utility)
E(f"function:{P}/xrsession.rs:new", f"function:{P}/xrsession.rs:cast_transform", "calls", 0.8)
# XRSession calls XRViewerPose::new
E(f"function:{P}/xrsession.rs:raf_callback", f"function:{P}/xrviewerpose.rs:new", "calls", 0.8)
# XRSystem calls XRSession::new
E(f"function:{P}/xrsystem.rs:session_obtained", f"function:{P}/xrsession.rs:new", "calls", 0.8)
# XRSystem calls XRTest::new
E(f"function:{P}/xrsystem.rs:Test", f"function:{P}/xrtest.rs:new", "calls", 0.8)
# XRWebGLLayer::Constructor calls XRRigidTransform::new
# XRWebGLLayer calls XRViewport::new
E(f"function:{P}/xrwebgllayer.rs:GetViewport", f"function:{P}/xrviewport.rs:new", "calls", 0.8)
# XRPose::new calls XRRigidTransform::new
E(f"function:{P}/xrpose.rs:new", f"function:{P}/xrrigidtransform.rs:new", "calls", 0.8)

# ===== EDGES SUMMARY & WRITE =====
print(f"Total nodes: {len(nodes)}")
print(f"Total edges: {len(edges)}")

# Determine split
node_count = len(nodes)
edge_count = len(edges)
print(f"Node threshold: 60, Edge threshold: 120")
print(f"Node count: {node_count}, Edge count: {edge_count}")

if node_count <= 60 and edge_count <= 120:
    parts = 1
else:
    parts = math.ceil(max(node_count / 60, edge_count / 120))
print(f"Parts needed: {parts}")

# Group files alphabetically
files = sorted(set(
    n["filePath"] for n in nodes if "filePath" in n
))
print(f"Files: {len(files)}")

chunk_size = math.ceil(len(files) / parts)
file_chunks = [files[i:i+chunk_size] for i in range(0, len(files), chunk_size)]

for part_idx, file_chunk in enumerate(file_chunks):
    part_num = part_idx + 1
    part_nodes = []
    part_edges = []

    file_set = set(file_chunk)

    for n in nodes:
        fp = n.get("filePath", "")
        nid = n["id"]
        # Include if it's a file node in this chunk, or a sub-node whose filePath is in this chunk
        if n["type"] in ("function", "class") and fp in file_set:
            part_nodes.append(n)
        elif n["type"] not in ("function", "class") and fp in file_set:
            part_nodes.append(n)
        elif n["type"] not in ("function", "class") and nid.startswith("file:") and fp in file_set:
            part_nodes.append(n)

    # Also include any non-file-sub nodes that belong to files in this chunk
    for n in nodes:
        fp = n.get("filePath", "")
        if fp in file_set:
            if n not in part_nodes:
                part_nodes.append(n)

    part_node_ids = set(n["id"] for n in part_nodes)

    for e in edges:
        if e["source"] in part_node_ids:
            part_edges.append(e)

    if parts == 1:
        out_path = f"{OUT}/batch-27.json"
    else:
        out_path = f"{OUT}/batch-27-part-{part_num}.json"

    with open(out_path, "w") as f:
        json.dump({"nodes": part_nodes, "edges": part_edges}, f, indent=2)

    # Validate
    for e in part_edges:
        src_ok = e["source"] in part_node_ids
        tgt_ok = e["target"] in part_node_ids
        if not src_ok:
            print(f"WARNING: Edge source {e['source']} not in part {part_num}")
        if not tgt_ok:
            # Check if target is a file: or class: or function: that references a known path
            pass  # cross-batch targets are OK

    print(f"Part {part_num}: {len(part_nodes)} nodes, {len(part_edges)} edges -> {out_path}")

# Also check cross-batch targets
cross_batch_targets = set()
for e in edges:
    nid = e["target"]
    if nid not in set(n["id"] for n in nodes):
        cross_batch_targets.add(nid)
if cross_batch_targets:
    print(f"\nCross-batch reference targets ({len(cross_batch_targets)}):")
    for t in sorted(cross_batch_targets):
        print(f"  {t}")
