const fs = require('fs');

const results = JSON.parse(fs.readFileSync('d:/Projects/servo/.understand-anything/tmp/ua-file-extract-results-24.json', 'utf8'));

const nodes = [];
const edges = [];

function createNode(id, type, name, filePath, summary, tags, complexity, extra) {
  const node = { id, type, name, filePath, summary, tags, complexity };
  if (extra) Object.assign(node, extra);
  return node;
}

function createEdge(source, target, type, weight) {
  return { source, target, type, direction: 'forward', weight };
}

function dropRequest(path) {
  if (path.includes('Buffer') && !path.includes('Usage') && !path.includes('Bind')) return 'DropBuffer';
  if (path.includes('BindGroupLayout')) return 'DropBindGroupLayout';
  if (path.includes('BindGroup')) return 'DropBindGroup';
  if (path.includes('ComputePass')) return 'DropComputePass';
  if (path.includes('ComputePipeline')) return 'DropComputePipeline';
  if (path.includes('CommandEncoder')) return 'DropCommandEncoder';
  if (path.includes('CommandBuffer')) return 'DropCommandBuffer';
  if (path.includes('CanvasContext')) return 'DropCanvasContext';
  if (path.includes('Device')) return 'DropDevice';
  if (path.includes('Adapter')) return 'DropAdapter';
  return 'Drop';
}

for (const r of results) {
  const path = r.path;
  const fileId = 'file:' + path;
  const fileName = path.split('/').pop();
  const nonEmpty = r.nonEmptyLines;

  let complexity = 'simple';
  if (nonEmpty >= 200) complexity = 'complex';
  else if (nonEmpty >= 50) complexity = 'moderate';

  let summary = '';
  let tags = [];

  if (path === 'components/script/dom/webgpu/gpu.rs') {
    summary = 'Entry point for the WebGPU API providing surface-level GPU access including adapter discovery and preferred canvas format querying.';
    tags = ['entry-point', 'webgpu', 'dom-binding'];
  } else if (path === 'components/script/dom/webgpu/gpuadapter.rs') {
    summary = 'WebGPU GPUAdapter implementation that manages physical device selection, adapter info lookup, and device request initiation.';
    tags = ['webgpu', 'adapter', 'dom-binding'];
  } else if (path === 'components/script/dom/webgpu/gpuadapterinfo.rs') {
    summary = 'WebGPU GPUAdapterInfo data holder exposing adapter vendor, architecture, device name, and subgroup size properties.';
    tags = ['webgpu', 'adapter-info', 'dom-binding'];
  } else if (path === 'components/script/dom/webgpu/gpubindgroup.rs') {
    summary = 'WebGPU GPUBindGroup implementation managing binding group creation, resource entries, and lifecycle through the WebGPU channel.';
    tags = ['webgpu', 'bind-group', 'dom-binding'];
  } else if (path === 'components/script/dom/webgpu/gpubindgrouplayout.rs') {
    summary = 'WebGPU GPUBindGroupLayout implementation managing layout descriptor creation and resource binding type specification.';
    tags = ['webgpu', 'bind-group-layout', 'dom-binding'];
  } else if (path === 'components/script/dom/webgpu/gpubuffer.rs') {
    summary = 'WebGPU GPUBuffer implementation managing GPU buffer creation, mapping, unmapping, destruction, and async map operations with shared memory support.';
    tags = ['webgpu', 'buffer', 'dom-binding', 'memory'];
  } else if (path === 'components/script/dom/webgpu/gpubufferusage.rs') {
    summary = 'WebGPU GPUBufferUsage constants holder providing bitfield flags for buffer usage modes.';
    tags = ['webgpu', 'buffer-usage', 'constants'];
  } else if (path === 'components/script/dom/webgpu/gpucanvascontext.rs') {
    summary = 'WebGPU GPUCanvasContext implementation managing canvas configuration, swap chain texture lifecycle, and presentation to HTML canvas elements.';
    tags = ['webgpu', 'canvas-context', 'dom-binding', 'rendering'];
  } else if (path === 'components/script/dom/webgpu/gpucolorwrite.rs') {
    summary = 'WebGPU GPUColorWrite constants holder providing bitfield flags for color channel write masks.';
    tags = ['webgpu', 'color-write', 'constants'];
  } else if (path === 'components/script/dom/webgpu/gpucommandbuffer.rs') {
    summary = 'WebGPU GPUCommandBuffer implementation managing recorded command buffer lifecycle and label attachment.';
    tags = ['webgpu', 'command-buffer', 'dom-binding'];
  } else if (path === 'components/script/dom/webgpu/gpucommandencoder.rs') {
    summary = 'WebGPU GPUCommandEncoder implementation managing command recording, compute/render pass creation, buffer/texture copy operations, and debug markers.';
    tags = ['webgpu', 'command-encoder', 'dom-binding'];
  } else if (path === 'components/script/dom/webgpu/gpucompilationinfo.rs') {
    summary = 'WebGPU GPUCompilationInfo implementation wrapping shader compilation messages into a frozen array for JavaScript consumption.';
    tags = ['webgpu', 'compilation-info', 'dom-binding'];
  } else if (path === 'components/script/dom/webgpu/gpucompilationmessage.rs') {
    summary = 'WebGPU GPUCompilationMessage implementation exposing shader compilation message details including message text, type, and source location.';
    tags = ['webgpu', 'compilation-message', 'dom-binding'];
  } else if (path === 'components/script/dom/webgpu/gpucomputepassencoder.rs') {
    summary = 'WebGPU GPUComputePassEncoder implementation managing compute pass recording including workgroup dispatch, bind group/pipeline binding, and debug markers.';
    tags = ['webgpu', 'compute-pass', 'dom-binding'];
  } else if (path === 'components/script/dom/webgpu/gpucomputepipeline.rs') {
    summary = 'WebGPU GPUComputePipeline implementation managing compute pipeline creation, bind group layout retrieval, and lifecycle through the WebGPU channel.';
    tags = ['webgpu', 'compute-pipeline', 'dom-binding'];
  } else if (path === 'components/script/dom/webgpu/gpuconvert.rs') {
    summary = 'WebGPU type conversion utilities providing traits and functions to convert between DOM-facing WebGPU types and wgpu-core backend types.';
    tags = ['webgpu', 'conversion', 'utility'];
  } else if (path === 'components/script/dom/webgpu/gpudevice.rs') {
    summary = 'WebGPU GPUDevice implementation as the central hub for creating all WebGPU resources (buffers, textures, pipelines) and managing error scopes, device loss, and capabilities.';
    tags = ['webgpu', 'device', 'dom-binding', 'resource-creation'];
  } else if (path === 'components/script/dom/webgpu/gpudevicelostinfo.rs') {
    summary = 'WebGPU GPUDeviceLostInfo implementation providing device loss reason and message when a GPUDevice is lost.';
    tags = ['webgpu', 'device-lost', 'dom-binding'];
  } else if (path === 'components/script/dom/webgpu/gpuerror.rs') {
    summary = 'WebGPU GPUError base implementation and AsWebGpu trait for converting between wgpu error types and DOM-exposed GPU error types.';
    tags = ['webgpu', 'error', 'dom-binding', 'trait'];
  } else if (path === 'components/script/dom/webgpu/gpuinternalerror.rs') {
    summary = 'WebGPU GPUInternalError implementation representing internal/backend GPU errors exposed through the DOM API.';
    tags = ['webgpu', 'internal-error', 'dom-binding'];
  } else if (path === 'components/script/dom/webgpu/gpumapmode.rs') {
    summary = 'WebGPU GPUMapMode constants holder providing bitfield flags for buffer mapping mode specification.';
    tags = ['webgpu', 'map-mode', 'constants'];
  } else if (path === 'components/script/dom/webgpu/gpuoutofmemoryerror.rs') {
    summary = 'WebGPU GPUOutOfMemoryError implementation representing out-of-memory GPU errors exposed through the DOM API.';
    tags = ['webgpu', 'out-of-memory-error', 'dom-binding'];
  }

  nodes.push(createNode(fileId, 'file', fileName, path, summary, tags, complexity));

  const exportedNames = new Set(r.exports.map(function(e) { return e.name; }));

  for (const cls of r.classes) {
    const clsId = 'class:' + path + ':' + cls.name;
    const classLines = cls.endLine - cls.startLine + 1;
    const isExported = exportedNames.has(cls.name);
    const hasMethods = cls.methods.length >= 2;
    const isLarge = classLines >= 20;

    if (!isExported && !hasMethods && !isLarge) continue;

    let clsComplexity = 'simple';
    if (cls.methods.length >= 10) clsComplexity = 'complex';
    else if (cls.methods.length >= 5) clsComplexity = 'moderate';

    let clsSummary = '';
    let clsTags = ['webgpu', 'dom-binding'];

    if (cls.name === 'GPU') {
      clsSummary = 'Top-level WebGPU DOM class providing entry-point methods for adapter discovery and WGSL language feature querying.';
    } else if (cls.name === 'GPUAdapter') {
      clsSummary = 'WebGPU adapter representation managing device selection, adapter info, feature/limits reporting, and device request lifecycle.';
    } else if (cls.name === 'GPUAdapterInfo') {
      clsSummary = 'Data class exposing GPU adapter properties including vendor, architecture, device name, subgroup sizes, and fallback status.';
    } else if (cls.name === 'GPUBindGroup') {
      clsSummary = 'WebGPU bind group wrapper managing GPU binding resource sets with create/delete lifecycle through the WebGPU channel.';
    } else if (cls.name === 'GPUBindGroupLayout') {
      clsSummary = 'WebGPU bind group layout wrapper managing layout descriptor lifecycle and creation through the WebGPU channel.';
    } else if (cls.name === 'GPUBuffer') {
      clsSummary = 'Core WebGPU buffer class managing GPU memory allocation, mapping, unmapping, and async operations with shared memory support.';
      clsTags = ['webgpu', 'buffer', 'dom-binding'];
    } else if (cls.name === 'ActiveBufferMapping') {
      clsSummary = 'Helper struct tracking active GPU buffer mapping state including mapped data, mapping mode, and byte range.';
      clsTags = ['webgpu', 'buffer', 'dom-binding'];
    } else if (cls.name === 'GPUBufferUsage') {
      clsSummary = 'Constants holder defining GPUBuffer usage bitfield flags such as MAP_READ, MAP_WRITE, COPY_SRC, COPY_DST, INDEX, VERTEX, UNIFORM, STORAGE, INDIRECT, and QUERY_RESOLVE.';
      clsTags = ['webgpu', 'constants'];
    } else if (cls.name === 'GPUCanvasContext') {
      clsSummary = 'WebGPU canvas context managing swap chain configuration, texture lifecycle, presentation, and integration with HTML/OffscreenCanvas.';
    } else if (cls.name === 'GPUColorWrite') {
      clsSummary = 'Constants holder defining GPUColorWrite bitfield flags for color channel write masks (RED, GREEN, BLUE, ALPHA, ALL).';
      clsTags = ['webgpu', 'constants'];
    } else if (cls.name === 'GPUCommandBuffer') {
      clsSummary = 'WebGPU command buffer wrapper managing recorded GPU commands with label support and lifecycle tracking.';
    } else if (cls.name === 'GPUCommandEncoder') {
      clsSummary = 'WebGPU command encoder managing command recording for compute/render passes, buffer/texture copy operations, debug markers, and query resolution.';
    } else if (cls.name === 'GPUCompilationInfo') {
      clsSummary = 'WebGPU compilation info wrapper that holds shader compilation messages exposed as a frozen array.';
    } else if (cls.name === 'GPUCompilationMessage') {
      clsSummary = 'WebGPU compilation message exposing shader compile error details including message text, type, line number, line position, offset and length.';
    } else if (cls.name === 'GPUComputePassEncoder') {
      clsSummary = 'WebGPU compute pass encoder managing workgroup dispatch, bind group/pipeline binding, and debug marker operations within a compute pass.';
    } else if (cls.name === 'GPUComputePipeline') {
      clsSummary = 'WebGPU compute pipeline managing pipeline creation, bind group layout lookup, and lifecycle through the WebGPU channel.';
    } else if (cls.name === 'GPUDevice') {
      clsSummary = 'Central WebGPU device class responsible for creating all GPU resources (buffers, textures, pipelines, samplers) and managing error scopes, device loss, and feature validation.';
    } else if (cls.name === 'PipelineLayout') {
      clsSummary = 'Enum-like struct distinguishing between implicit and explicit pipeline layout modes for render/compute pipelines.';
    } else if (cls.name === 'GPUDeviceLostInfo') {
      clsSummary = 'Data class exposing device loss reason (destroyed, unknown) and associated message string.';
    } else if (cls.name === 'GPUError') {
      clsSummary = 'Base class for all WebGPU DOM error types with message property and factory method for creating typed GPU errors from wgpu errors.';
      clsTags = ['webgpu', 'error', 'dom-binding'];
    } else if (cls.name === 'AsWebGpu') {
      clsSummary = 'Trait defining conversion from DOM-facing WebGPU error filter types to the wgpu backend representation.';
      clsTags = ['webgpu', 'error', 'trait'];
    } else if (cls.name === 'GPUInternalError') {
      clsSummary = 'DOM-exposed WebGPU error subclass representing internal/backend GPU errors.';
      clsTags = ['webgpu', 'error', 'dom-binding'];
    } else if (cls.name === 'GPUMapMode') {
      clsSummary = 'Constants holder defining GPUMapMode bitfield flags for buffer mapping direction (READ, WRITE).';
      clsTags = ['webgpu', 'constants'];
    } else if (cls.name === 'GPUOutOfMemoryError') {
      clsSummary = 'DOM-exposed WebGPU error subclass representing out-of-memory GPU errors.';
      clsTags = ['webgpu', 'error', 'dom-binding'];
    }

    if (!clsSummary) {
      clsSummary = 'WebGPU DOM binding class ' + cls.name + ' with ' + cls.methods.length + ' method(s).';
    }

    nodes.push(createNode(clsId, 'class', cls.name, path, clsSummary, clsTags, clsComplexity, { lineRange: [cls.startLine, cls.endLine] }));
    edges.push(createEdge(fileId, clsId, 'contains', 1.0));
    if (isExported) {
      edges.push(createEdge(fileId, clsId, 'exports', 0.8));
    }
  }

  for (const fn of r.functions) {
    const fnId = 'function:' + path + ':' + fn.name;
    const fnLines = fn.endLine - fn.startLine + 1;
    const isExported = exportedNames.has(fn.name);
    const isSignificant = fnLines >= 10 || isExported;

    if (!isSignificant) continue;

    let fnComplexity = 'simple';
    if (fnLines >= 50) fnComplexity = 'complex';
    else if (fnLines >= 20) fnComplexity = 'moderate';

    const paramStr = fn.params && fn.params.length > 0 ? '(' + fn.params.join(', ') + ')' : '()';
    let fnSummary = '';
    let fnTags = ['webgpu'];

    if (fn.name === 'RequestAdapter') {
      fnSummary = 'Requests a GPU adapter from the backend via constellation channel, handling power preference, feature level filtering, and promise resolution.';
    } else if (fn.name === 'handle_response' && path.includes('gpu.rs')) {
      fnSummary = 'Processes adapter discovery response from the constellation, creating GPUAdapter instances or rejecting the promise on error.';
    } else if (fn.name === 'RequestDevice') {
      fnSummary = 'Requests a GPU device from the adapter, validating required features and limits, creating device and queue IDs, and sending the request through the WebGPU channel.';
    } else if (fn.name === 'create_adapter_info') {
      fnSummary = 'Constructs a GPUAdapterInfo from backend adapter info, converting vendor, device, architecture, name strings and checking feature support.';
    } else if (fn.name === 'create' && path.includes('gpubindgroup.rs')) {
      fnSummary = 'Creates a GPUBindGroup by converting bind group entries, allocating a bind group ID, and sending creation request through the WebGPU channel.';
    } else if (fn.name === 'create' && path.includes('gpubindgrouplayout.rs')) {
      fnSummary = 'Creates a GPUBindGroupLayout by converting layout entries, allocating an ID, and sending creation through the WebGPU channel with error dispatch.';
    } else if (fn.name === 'create' && path.includes('gpubuffer.rs')) {
      fnSummary = 'Creates a GPUBuffer with optional initial mapping, converting the descriptor, allocating a buffer ID, and sending creation through the WebGPU channel.';
    } else if (fn.name === 'create' && path.includes('gpucommandencoder.rs')) {
      fnSummary = 'Creates a GPUCommandEncoder by allocating an encoder ID and sending creation request through the WebGPU channel.';
    } else if (fn.name === 'create' && path.includes('gpucomputepipeline.rs')) {
      fnSummary = 'Creates a GPUComputePipeline by allocating a pipeline ID, resolving pipeline layout, converting shader stage descriptors, and sending creation through the WebGPU channel.';
    } else if (fn.name === 'MapAsync') {
      fnSummary = 'Initiates async GPU buffer mapping, validating range, creating a pending mapping promise, and sending the map request through the WebGPU channel.';
    } else if (fn.name === 'GetMappedRange') {
      fnSummary = 'Retrieves a mapped range of GPU buffer memory as an ArrayBuffer, handling offset/size validation and shared memory view creation.';
    } else if (fn.name === 'Unmap') {
      fnSummary = 'Unmaps a GPU buffer, cleaning up active mapping data and sending the unmap notification through the channel.';
    } else if (fn.name === 'Destroy' && path.includes('gpubuffer.rs')) {
      fnSummary = 'Destroys a GPU buffer by first unmapping if mapped, then sending the destroy request through the channel.';
    } else if (fn.name === 'map_failure') {
      fnSummary = 'Handles GPU buffer mapping failure by rejecting the pending mapping promise with appropriate error type.';
    } else if (fn.name === 'map_success') {
      fnSummary = 'Handles GPU buffer mapping success by storing the active buffer mapping and resolving the pending mapping promise.';
    } else if (fn.name === 'handle_response' && path.includes('gpubuffer.rs')) {
      fnSummary = 'Dispatches buffer map response to either map_success or map_failure based on the result.';
    } else if (fn.name === 'Configure') {
      fnSummary = 'Configures the GPU canvas context with a device, format, and size, validating texture descriptor and creating swap chain textures.';
    } else if (fn.name === 'GetCurrentTexture') {
      fnSummary = 'Returns the current swap chain texture for the canvas context, creating a new one if needed through the device CreateTexture.';
    } else if (fn.name === 'set_image_key') {
      fnSummary = 'Sets the compositor image key for the canvas context presentation buffer.';
    } else if (fn.name === 'update_rendering') {
      fnSummary = 'Updates rendering state for the canvas context, sending presentation buffer through the WebGPU channel and expiring current texture.';
    } else if (fn.name === 'new_inherited' && path.includes('gpucanvascontext.rs')) {
      fnSummary = 'Initializes the GPUCanvasContext with canvas size, creates presentation buffer IDs, and validates initial texture support.';
    } else if (fn.name === 'BeginComputePass') {
      fnSummary = 'Begins a new compute pass on the command encoder, allocating compute pass ID and returning a GPUComputePassEncoder.';
    } else if (fn.name === 'BeginRenderPass') {
      fnSummary = 'Begins a new render pass on the command encoder, converting color/depth/stencil attachments and returning a GPURenderPassEncoder.';
    } else if (fn.name === 'CopyBufferToBuffer') {
      fnSummary = 'Records a buffer-to-buffer copy command on the command encoder through the WebGPU channel.';
    } else if (fn.name === 'CopyBufferToTexture') {
      fnSummary = 'Records a buffer-to-texture copy command on the command encoder through the WebGPU channel.';
    } else if (fn.name === 'CopyTextureToBuffer') {
      fnSummary = 'Records a texture-to-buffer copy command on the command encoder through the WebGPU channel.';
    } else if (fn.name === 'CopyTextureToTexture') {
      fnSummary = 'Records a texture-to-texture copy command on the command encoder through the WebGPU channel.';
    } else if (fn.name === 'Finish') {
      fnSummary = 'Finishes command recording and returns a GPUCommandBuffer by allocating a command buffer ID and sending the finish request.';
    } else if (fn.name === 'PushDebugGroup') {
      fnSummary = 'Pushes a debug group marker onto the command encoder command stream.';
    } else if (fn.name === 'PopDebugGroup') {
      fnSummary = 'Pops the most recent debug group from the command encoder command stream.';
    } else if (fn.name === 'InsertDebugMarker') {
      fnSummary = 'Inserts a debug marker label into the command encoder command stream.';
    } else if (fn.name === 'ResolveQuerySet') {
      fnSummary = 'Records a query set resolve operation on the command encoder, writing query results to a destination buffer.';
    } else if (fn.name === 'DispatchWorkgroups') {
      fnSummary = 'Dispatches compute workgroups with specified X, Y, Z dimensions through the compute pass.';
    } else if (fn.name === 'DispatchWorkgroupsIndirect') {
      fnSummary = 'Dispatches compute workgroups using indirect parameters from a GPU buffer through the compute pass.';
    } else if (fn.name === 'End') {
      fnSummary = 'Ends the current compute pass recording.';
    } else if (fn.name === 'SetBindGroup') {
      fnSummary = 'Binds a bind group at the specified index for the compute pass.';
    } else if (fn.name === 'SetPipeline') {
      fnSummary = 'Sets the active compute pipeline for the compute pass.';
    } else if (fn.name === 'GetBindGroupLayout') {
      fnSummary = 'Retrieves a bind group layout by index from the compute pipeline, allocating layout ID and sending request through the channel.';
    } else if (fn.name === 'convert_load_op') {
      fnSummary = 'Converts a DOM GPU load operation with clear value to the wgpu backend LoadOp type.';
      fnTags = ['webgpu', 'conversion'];
    } else if (fn.name === 'convert_bind_group_layout_entry') {
      fnSummary = 'Converts a DOM GPUBindGroupLayoutEntry to the wgpu backend BindGroupLayoutEntry, handling buffer/sampler/texture/storageTexture binding types.';
      fnTags = ['webgpu', 'conversion'];
    } else if (fn.name === 'convert_texture_descriptor') {
      fnSummary = 'Converts a DOM GPUTextureDescriptor to the wgpu backend TextureDescriptor, validating format features and usage flags.';
      fnTags = ['webgpu', 'conversion'];
    } else if (fn.name === 'convert_texture_for_wgpu_with_cx') {
      fnSummary = 'Converts a DOM GPU texture view reference to a wgpu backend texture view ID for render pass attachments.';
      fnTags = ['webgpu', 'conversion'];
    } else if (fn.name === 'convert_bind_group_entry') {
      fnSummary = 'Converts a DOM GPUBindGroupEntry to the wgpu backend BindingResource, handling sampler, texture view, and buffer resources.';
      fnTags = ['webgpu', 'conversion'];
    } else if (fn.name === 'fire_uncaptured_error') {
      fnSummary = 'Fires an uncaptured error event on the device, creating a typed GPUError and dispatching through the task system.';
    } else if (fn.name === 'validate_texture_format_required_features') {
      fnSummary = 'Validates that a GPU texture format is supported by the device feature set, rejecting with TypeError if unsupported.';
    } else if (fn.name === 'get_pipeline_layout_data') {
      fnSummary = 'Extracts pipeline layout data (explicit vs implicit) from a GPUPipelineLayout.';
    } else if (fn.name === 'parse_render_pipeline') {
      fnSummary = 'Parses a complete render pipeline descriptor into wgpu format, converting vertex buffers, fragment state, depth/stencil, primitive, and multisample state.';
    } else if (fn.name === 'lose') {
      fnSummary = 'Marks the device as lost with specified reason and message, firing the lost promise through the task system.';
    } else if (fn.name === 'dispatch_error') {
      fnSummary = 'Dispatches a validation error to the WebGPU backend for the device.';
    } else if (fn.name === 'PushErrorScope') {
      fnSummary = 'Pushes a new error scope onto the device error scope stack with specified error filter.';
    } else if (fn.name === 'PopErrorScope') {
      fnSummary = 'Pops the current error scope and returns captured errors as a promise-resolved GPUError.';
    } else if (fn.name === 'from_error') {
      fnSummary = 'Factory method creating typed GPUError subclasses (ValidationError, OutOfMemoryError, InternalError) from wgpu backend error types.';
    } else if (fn.name === 'from' && path.includes('compilation')) {
      fnSummary = 'Creates a GPUCompilationInfo or GPUCompilationMessage from wgpu compilation error data.';
    } else if (fn.name === 'drop') {
      fnSummary = 'Drop handler sending ' + dropRequest(path) + ' request through the WebGPU channel for resource cleanup.';
    } else if (fn.name === 'new_inherited') {
      fnSummary = 'Internal constructor initializing the DOM reflector and properties without exposing to JavaScript.';
    } else if (fn.name === 'new' || fn.name === 'Constructor') {
      fnSummary = 'Constructor wrapping inherited initialization and exposing the object to the DOM through the reflector system.';
    } else if (fn.name === 'new_with_proto') {
      fnSummary = 'Constructor initializing the object with a specified prototype for prototype-chain inheritance in the DOM binding.';
    } else if (fn.name === 'clone_from') {
      fnSummary = 'Clones adapter info data from an existing GPUAdapterInfo source into a new object.';
    } else if (fn.name === 'explicit') {
      fnSummary = 'Returns the inner explicit pipeline layout ID if this is an explicit layout.';
    } else if (fn.name === 'channel') {
      fnSummary = 'Returns a cloned channel reference for sending WebGPU requests.';
    } else if (fn.name === 'device_id') {
      fnSummary = 'Returns the associated device ID for this command encoder.';
    } else if (fn.name === 'id' || fn.name === 'queue_id') {
      fnSummary = 'Returns the internal GPU resource ID.';
    } else if (fn.name === 'is_lost') {
      fnSummary = 'Checks whether the device has been lost by inspecting the lost promise state.';
    } else if (fn.name === 'CreateComputePipeline') {
      fnSummary = 'Creates a synchronous compute pipeline by calling GPUComputePipeline::create and wrapping the result.';
    } else if (fn.name === 'CreateComputePipelineAsync') {
      fnSummary = 'Creates an async compute pipeline, returning a promise that resolves with the new GPUComputePipeline.';
    } else if (fn.name === 'CreateRenderPipeline') {
      fnSummary = 'Creates a synchronous render pipeline by parsing the descriptor and delegating to GPURenderPipeline::create.';
    } else if (fn.name === 'CreateRenderPipelineAsync') {
      fnSummary = 'Creates an async render pipeline, returning a promise that resolves with the new GPURenderPipeline.';
    } else if (fn.name === 'SetLabel') {
      fnSummary = 'Setter for the label attribute.';
    } else if (fn.name === 'Destroy' && path.includes('gpudevice.rs')) {
      fnSummary = 'Destroys the device by marking it invalid and sending DestroyDevice request through the channel.';
    } else if (fn.name === 'handle_response' && path.includes('gpudevice.rs')) {
      fnSummary = 'Processes async pipeline creation response, creating the GPU pipeline object or rejecting with GPUPipelineError.';
    }

    if (!fnSummary || fnSummary === '') {
      fnSummary = 'Method ' + fn.name + paramStr + ' handling WebGPU DOM operations.';
    }

    fnTags.push('dom-binding');

    nodes.push(createNode(fnId, 'function', fn.name, path, fnSummary, fnTags, fnComplexity, { lineRange: [fn.startLine, fn.endLine] }));
    edges.push(createEdge(fileId, fnId, 'contains', 1.0));
    if (isExported) {
      edges.push(createEdge(fileId, fnId, 'exports', 0.8));
    }
  }
}

console.log('Total nodes: ' + nodes.length);
console.log('Total edges: ' + edges.length);

// Sort files alphabetically by path
const fileNodeIds = nodes.filter(function(n) { return n.type === 'file'; }).map(function(n) { return n.id; });
fileNodeIds.sort();

// Partition into 3 parts (since nodes > 60 and edges > 120)
const parts = 3;
const filesPerPart = Math.ceil(fileNodeIds.length / parts);

for (let p = 0; p < parts; p++) {
  const partFiles = fileNodeIds.slice(p * filesPerPart, (p + 1) * filesPerPart);
  const partFilesSet = new Set(partFiles);

  // Collect all paths for this part
  const partPaths = new Set();
  partFiles.forEach(function(fid) {
    partPaths.add(fid.replace('file:', ''));
  });

  // Nodes: file nodes in this part + all function/class nodes belonging to these files
  const partNodes = nodes.filter(function(n) {
    if (n.type === 'file') return partFilesSet.has(n.id);
    return n.filePath && partPaths.has(n.filePath);
  });

  // Edges: all edges whose source is in partNodes
  const partNodeIds = new Set(partNodes.map(function(n) { return n.id; }));
  const partEdges = edges.filter(function(e) {
    return partNodeIds.has(e.source);
  });

  const partIndex = p + 1;
  const outPath = 'd:/Projects/servo/.understand-anything/intermediate/batch-24-part-' + partIndex + '.json';
  fs.writeFileSync(outPath, JSON.stringify({ nodes: partNodes, edges: partEdges }, null, 2));
  console.log('Part ' + partIndex + ': ' + partNodes.length + ' nodes, ' + partEdges.length + ' edges -> ' + outPath);
}
