#!/usr/bin/env node
/**
 * Generate batch-24 graph nodes and edges for the WebGPU DOM implementation.
 * Uses the extraction results and import analysis from source files.
 */

import { readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import { join, dirname } from 'node:path';

const OUTPUT_DIR = 'd:/Projects/servo/.understand-anything/intermediate';
const EXTRACT_PATH = 'd:/Projects/servo/.understand-anything/tmp/ua-file-extract-results-24.json';

const extract = JSON.parse(readFileSync(EXTRACT_PATH, 'utf-8'));

// Helper to find a file result by path
function findFile(path) {
  return extract.results.find(r => r.path === path);
}

// Determine if a function meets significance filter
function isFunctionSignificant(fn, fileResult) {
  const lineCount = fn.endLine - fn.startLine + 1;
  // Check if exported
  const isExported = fileResult.exports?.some(e => e.name === fn.name);
  if (isExported) return true;
  // 10+ lines threshold
  return lineCount >= 10;
}

// Determine if a class meets significance filter
function isClassSignificant(cls, fileResult) {
  const lineCount = cls.endLine - cls.startLine + 1;
  // Check if exported
  const isExported = fileResult.exports?.some(e => e.name === cls.name);
  if (isExported) return true;
  // Classes with 2+ methods or 20+ lines
  return cls.methods.length >= 2 || lineCount >= 20;
}

// Complexity based on non-empty lines
function complexity(nonEmptyLines) {
  if (nonEmptyLines < 50) return 'simple';
  if (nonEmptyLines < 200) return 'moderate';
  return 'complex';
}

// ============================================================================
// INTER-BATCH IMPORTS
// ============================================================================
// Map of file -> [imported file paths within this batch]
// Derived from grep of 'use crate::dom::webgpu::' statements in source files
const importMap = {
  'components/script/dom/webgpu/gpu.rs': [
    'components/script/dom/webgpu/gpuadapter.rs'
  ],
  'components/script/dom/webgpu/gpuadapter.rs': [
    'components/script/dom/webgpu/gpudevice.rs'
  ],
  'components/script/dom/webgpu/gpuadapterinfo.rs': [],
  'components/script/dom/webgpu/gpubindgroup.rs': [
    'components/script/dom/webgpu/gpubindgrouplayout.rs',
    'components/script/dom/webgpu/gpudevice.rs'
  ],
  'components/script/dom/webgpu/gpubindgrouplayout.rs': [
    'components/script/dom/webgpu/gpuconvert.rs',
    'components/script/dom/webgpu/gpudevice.rs'
  ],
  'components/script/dom/webgpu/gpubuffer.rs': [
    'components/script/dom/webgpu/gpudevice.rs'
  ],
  'components/script/dom/webgpu/gpubufferusage.rs': [],
  'components/script/dom/webgpu/gpucanvascontext.rs': [],
  'components/script/dom/webgpu/gpucolorwrite.rs': [],
  'components/script/dom/webgpu/gpucommandbuffer.rs': [],
  'components/script/dom/webgpu/gpucommandencoder.rs': [
    'components/script/dom/webgpu/gpubuffer.rs',
    'components/script/dom/webgpu/gpucommandbuffer.rs',
    'components/script/dom/webgpu/gpucomputepassencoder.rs',
    'components/script/dom/webgpu/gpudevice.rs'
  ],
  'components/script/dom/webgpu/gpucompilationinfo.rs': [],
  'components/script/dom/webgpu/gpucompilationmessage.rs': [],
  'components/script/dom/webgpu/gpucomputepassencoder.rs': [
    'components/script/dom/webgpu/gpubindgroup.rs',
    'components/script/dom/webgpu/gpubuffer.rs',
    'components/script/dom/webgpu/gpucommandencoder.rs',
    'components/script/dom/webgpu/gpucomputepipeline.rs'
  ],
  'components/script/dom/webgpu/gpucomputepipeline.rs': [
    'components/script/dom/webgpu/gpubindgrouplayout.rs',
    'components/script/dom/webgpu/gpudevice.rs'
  ],
  'components/script/dom/webgpu/gpuconvert.rs': [],
  'components/script/dom/webgpu/gpudevice.rs': [
    'components/script/dom/webgpu/gpuadapter.rs',
    'components/script/dom/webgpu/gpuadapterinfo.rs',
    'components/script/dom/webgpu/gpubindgroup.rs',
    'components/script/dom/webgpu/gpubindgrouplayout.rs',
    'components/script/dom/webgpu/gpubuffer.rs',
    'components/script/dom/webgpu/gpucommandencoder.rs',
    'components/script/dom/webgpu/gpucomputepipeline.rs'
  ],
  'components/script/dom/webgpu/gpudevicelostinfo.rs': [],
  'components/script/dom/webgpu/gpuerror.rs': [],
  'components/script/dom/webgpu/gpuinternalerror.rs': [],
  'components/script/dom/webgpu/gpumapmode.rs': [],
  'components/script/dom/webgpu/gpuoutofmemoryerror.rs': []
};

// ============================================================================
// File Summaries
// ============================================================================
const summaries = {
  'components/script/dom/webgpu/gpu.rs': 'Entry point for the WebGPU API, providing the GPU object that allows requesting adapters and querying WGSLLanguageFeatures and the preferred canvas format.',
  'components/script/dom/webgpu/gpuadapter.rs': 'Implements the GPUAdapter DOM object for requesting GPU devices and querying adapter capabilities including features, limits, and info.',
  'components/script/dom/webgpu/gpuadapterinfo.rs': 'Data object exposing GPU adapter properties including vendor, architecture, device name, description, subgroup sizes, and fallback adapter status.',
  'components/script/dom/webgpu/gpubindgroup.rs': 'Implements GPUBindGroup for grouping bindings (buffers, textures, samplers) together for use in compute and render passes.',
  'components/script/dom/webgpu/gpubindgrouplayout.rs': 'Implements GPUBindGroupLayout defining the layout and types of bindings expected by a bind group in a pipeline.',
  'components/script/dom/webgpu/gpubuffer.rs': 'Implements GPUBuffer for allocating and managing GPU-side buffers with mapping, unmapping, and destruction operations.',
  'components/script/dom/webgpu/gpubufferusage.rs': 'Constants-only class defining GPU buffer usage flags (MAP_READ, MAP_WRITE, COPY_SRC, COPY_DST, INDEX, VERTEX, UNIFORM, STORAGE, INDIRECT, QUERY_RESOLVE).',
  'components/script/dom/webgpu/gpucanvascontext.rs': 'Implements GPUCanvasContext for configuring and obtaining the current texture from a canvas element for WebGPU rendering.',
  'components/script/dom/webgpu/gpucolorwrite.rs': 'Constants-only class defining GPU color write mask flags (RED, GREEN, BLUE, ALPHA, ALL) for color attachment blending.',
  'components/script/dom/webgpu/gpucommandbuffer.rs': 'Implements GPUCommandBuffer representing a recorded list of GPU commands that can be submitted to a queue for execution.',
  'components/script/dom/webgpu/gpucommandencoder.rs': 'Implements GPUCommandEncoder for recording GPU commands including compute/render passes, buffer/texture copies, and debug markers.',
  'components/script/dom/webgpu/gpucompilationinfo.rs': 'Implements GPUCompilationInfo containing a list of GPUCompilationMessage items from shader compilation.',
  'components/script/dom/webgpu/gpucompilationmessage.rs': 'Implements GPUCompilationMessage with message text, type (error/warning/info), and source position (line, offset, length).',
  'components/script/dom/webgpu/gpucomputepassencoder.rs': 'Implements GPUComputePassEncoder for dispatching compute workgroups and managing compute pipeline bindings.',
  'components/script/dom/webgpu/gpucomputepipeline.rs': 'Implements GPUComputePipeline representing a compiled compute shader pipeline with bind group layout query support.',
  'components/script/dom/webgpu/gpuconvert.rs': 'Conversion utilities translating WebGPU DOM types (color, texture, bind group entries, shader stages) to wgpu-native types.',
  'components/script/dom/webgpu/gpudevice.rs': 'Central GPUDevice implementation creating all WebGPU resources (buffers, textures, pipelines, samplers) and managing error scopes and device lifecycle.',
  'components/script/dom/webgpu/gpudevicelostinfo.rs': 'Implements GPUDeviceLostInfo providing the reason and message when a GPU device becomes lost.',
  'components/script/dom/webgpu/gpuerror.rs': 'Base GPUError type hierarchy with factory methods, filter conversion, and subclasses for validation, out-of-memory, and internal errors.',
  'components/script/dom/webgpu/gpuinternalerror.rs': 'Implements GPUInternalError subclass for internal (unexpected) GPU errors with constructor.',
  'components/script/dom/webgpu/gpumapmode.rs': 'Constants-only class defining GPU map mode flags (READ, WRITE) for buffer mapping operations.',
  'components/script/dom/webgpu/gpuoutofmemoryerror.rs': 'Implements GPUOutOfMemoryError subclass for out-of-memory GPU errors with constructor.'
};

// ============================================================================
// File Language Notes
// ============================================================================
const languageNotes = {
  'components/script/dom/webgpu/gpu.rs': 'Uses the Reflector-based DOM object pattern with message passing to the constellation process for adapter enumeration.',
  'components/script/dom/webgpu/gpudevice.rs': 'Central factory pattern creating all WebGPU resource types via message passing to the WebGPU render thread, with error scope management and pipeline layout parsing.',
  'components/script/dom/webgpu/gpuconvert.rs': 'Trait-based conversion pattern using Convert/TryConvert traits to transform WebGPU API types (GPUColor, GPUExtent3D, GPUOrigin3D) into wgpu-native equivalents.',
  'components/script/dom/webgpu/gpubuffer.rs': 'Implements the WebGPU buffer mapping lifecycle with shared memory (GenericSharedMemory) for zero-copy data transfer between CPU and GPU.',
  'components/script/dom/webgpu/gpucanvascontext.rs': 'Uses a swap-chain-like presentation buffer pattern with ArrayVec for managing multiple presentation buffers and texture lifetime.',
  'components/script/dom/webgpu/gpubindgroup.rs': 'Uses Droppable wrapper pattern to send cleanup messages to the WebGPU render thread on drop.',
  'components/script/dom/webgpu/gpuerror.rs': 'Implements error class hierarchy using DOM reflectors with discriminated factory method from_error.',
  'components/script/dom/webgpu/gpucommandencoder.rs': 'Implements extensive command recording with enums for copy operations, pass encoding, and debug marker commands.',
  'components/script/dom/webgpu/gpucomputepipeline.rs': 'Async pipeline compilation support via callback-based message passing with GPUPipelineError handling.'
};

// ============================================================================
// BUILD NODES
// ============================================================================
const nodes = [];
const edges = [];

// For dedup tracking
const nodeIds = new Set();

function addNode(node) {
  if (nodeIds.has(node.id)) {
    console.error(`DUPLICATE NODE: ${node.id}`);
    return;
  }
  nodeIds.add(node.id);
  nodes.push(node);
}

function addEdge(edge) {
  // Validate source and target exist as nodes
  if (!nodeIds.has(edge.source)) {
    // Cross-batch reference - OK
  }
  if (!nodeIds.has(edge.target)) {
    // Cross-batch reference - OK
  }
  // Check for self-reference
  if (edge.source === edge.target) {
    console.error(`SELF-REF EDGE: ${edge.source}`);
    return;
  }
  edges.push(edge);
}

// ============================================================================
// File Nodes
// ============================================================================
const batchFiles = extract.results;

for (const file of batchFiles) {
  const fpath = file.path;
  const isEntryPoint = fpath === 'components/script/dom/webgpu/gpu.rs';
  const tags = [];

  // Tags based on file purpose
  if (fpath.includes('gpuconvert')) {
    tags.push('utility', 'conversion', 'type-mapping');
  } else if (fpath.includes('gpudevice')) {
    tags.push('factory', 'resource-creation', 'device-lifecycle');
  } else if (fpath.includes('gpuadapter')) {
    tags.push('adapter', 'device-selection');
  } else if (fpath.includes('gpubuffer')) {
    tags.push('buffer', 'memory-management');
  } else if (fpath.includes('gpucommandencoder')) {
    tags.push('command-recording', 'gpu-commands');
  } else if (fpath.includes('gpucomputepass')) {
    tags.push('compute', 'pass-encoder');
  } else if (fpath.includes('gpucomputepipeline')) {
    tags.push('compute-pipeline', 'shader');
  } else if (fpath.includes('gpubindgroup')) {
    tags.push('bind-group', 'resource-binding');
  } else if (fpath.includes('gpucompilationinfo')) {
    tags.push('compilation', 'shader-info');
  } else if (fpath.includes('gpucompilationmessage')) {
    tags.push('compilation', 'shader-message');
  } else if (fpath.includes('gpucanvascontext')) {
    tags.push('canvas', 'presentation');
  } else if (fpath.includes('gpuerror')) {
    tags.push('error', 'error-handling');
  } else if (fpath.includes('gpuinternalerror') || fpath.includes('gpuoutofmemoryerror')) {
    tags.push('error', 'error-subclass');
  } else if (fpath.includes('gpudevicelostinfo')) {
    tags.push('device-lifecycle', 'error');
  } else if (fpath.includes('gpubufferusage') || fpath.includes('gpucolorwrite') || fpath.includes('gpumapmode')) {
    tags.push('constants', 'flags');
  } else {
    tags.push('webgpu', 'dom-binding');
  }

  // Add webgpu tag to all
  tags.push('webgpu');

  const fileNode = {
    id: `file:${fpath}`,
    type: 'file',
    name: fpath.split('/').pop(),
    filePath: fpath,
    summary: summaries[fpath] || `WebGPU DOM implementation for the ${fpath.split('/').pop().replace('.rs', '')} API.`,
    tags,
    complexity: complexity(file.nonEmptyLines),
  };

  if (languageNotes[fpath]) {
    fileNode.languageNotes = languageNotes[fpath];
  }

  addNode(fileNode);
}

// ============================================================================
// Function and Class Nodes
// ============================================================================
for (const file of batchFiles) {
  const fpath = file.path;

  // Classes
  for (const cls of (file.classes || [])) {
    if (!isClassSignificant(cls, file)) continue;

    const clsNode = {
      id: `class:${fpath}:${cls.name}`,
      type: 'class',
      name: cls.name,
      filePath: fpath,
      lineRange: [cls.startLine, cls.endLine],
      summary: `WebGPU ${cls.name} DOM class with ${cls.methods.length} methods exposing WebGPU API functionality.`,
      tags: ['webgpu', 'class'],
      complexity: cls.methods.length >= 10 ? 'moderate' : (cls.endLine - cls.startLine + 1 >= 20 ? 'moderate' : 'simple'),
    };

    // Custom summaries for key classes
    if (cls.name === 'GPU') {
      clsNode.summary = 'Entry point for the WebGPU API providing RequestAdapter, GetPreferredCanvasFormat, and WGSLLanguageFeatures access.';
    } else if (cls.name === 'GPUAdapter') {
      clsNode.summary = 'GPU adapter object exposing adapter properties (name, features, limits, info) and creating GPU devices via RequestDevice.';
    } else if (cls.name === 'GPUAdapterInfo') {
      clsNode.summary = 'Data class exposing GPU adapter properties including vendor, architecture, device name, description, and subgroup sizes.';
    } else if (cls.name === 'GPUBindGroup') {
      clsNode.summary = 'Resource binding group containing bindings (buffers, textures, samplers) for use in shader execution.';
    } else if (cls.name === 'GPUBindGroupLayout') {
      clsNode.summary = 'Layout definition for a bind group specifying the types and visibility of each binding entry.';
    } else if (cls.name === 'GPUBuffer') {
      clsNode.summary = 'GPU buffer resource with mapping, unmapping, and destruction capabilities for data transfer.';
    } else if (cls.name === 'GPUCanvasContext') {
      clsNode.summary = 'Canvas context for WebGPU rendering, managing configuration, presentation textures, and swap chain lifecycle.';
    } else if (cls.name === 'GPUCommandBuffer') {
      clsNode.summary = 'Recorded list of GPU commands ready for submission to a queue.';
    } else if (cls.name === 'GPUCommandEncoder') {
      clsNode.summary = 'Command encoder for recording GPU operations including compute/render passes, buffer/texture copies, and debug markers.';
    } else if (cls.name === 'GPUCompilationInfo') {
      clsNode.summary = 'Container for GPU shader compilation messages reporting errors, warnings, and info.';
    } else if (cls.name === 'GPUCompilationMessage') {
      clsNode.summary = 'Single shader compilation message with text, type classification, and source position information.';
    } else if (cls.name === 'GPUComputePassEncoder') {
      clsNode.summary = 'Encoder for recording compute pass operations including workgroup dispatch and pipeline binding.';
    } else if (cls.name === 'GPUComputePipeline') {
      clsNode.summary = 'Compiled compute pipeline with bind group layout introspection and async creation support.';
    } else if (cls.name === 'GPUDevice') {
      clsNode.summary = 'Central GPU device managing resource creation (buffers, textures, pipelines, samplers), error scopes, and device lifecycle.';
    } else if (cls.name === 'GPUDeviceLostInfo') {
      clsNode.summary = 'Provides the reason and human-readable message when a GPU device is lost.';
    } else if (cls.name === 'GPUError') {
      clsNode.summary = 'Base GPU error class with factory methods for creating typed GPU errors (validation, out-of-memory, internal).';
    } else if (cls.name === 'GPUInternalError') {
      clsNode.summary = 'GPU error subclass representing unexpected internal GPU errors.';
    } else if (cls.name === 'GPUOutOfMemoryError') {
      clsNode.summary = 'GPU error subclass representing out-of-memory conditions on the GPU.';
    } else if (cls.name === 'ActiveBufferMapping') {
      clsNode.summary = 'Tracks an active buffer mapping with its data block, access mode, and mapped byte range.';
    } else if (cls.name === 'PipelineLayout') {
      clsNode.summary = 'Pipeline layout representation distinguishing implicit and explicit layout modes for compute and render pipelines.';
    }

    addNode(clsNode);

    // contains edge
    addEdge({
      source: `file:${fpath}`,
      target: `class:${fpath}:${cls.name}`,
      type: 'contains',
      direction: 'forward',
      weight: 1.0
    });

    // exports edge if exported
    const isExported = file.exports?.some(e => e.name === cls.name);
    if (isExported) {
      addEdge({
        source: `file:${fpath}`,
        target: `class:${fpath}:${cls.name}`,
        type: 'exports',
        direction: 'forward',
        weight: 0.8
      });
    }
  }

  // Functions
  for (const fn of (file.functions || [])) {
    if (!isFunctionSignificant(fn, file)) continue;

    // Disambiguate duplicate function names with line number
    const existingWithName = file.functions.filter(f => f.name === fn.name);
    const fnId = existingWithName.length > 1
      ? `function:${fpath}:${fn.name}_l${fn.startLine}`
      : `function:${fpath}:${fn.name}`;

    const fnNode = {
      id: fnId,
      type: 'function',
      name: fn.name,
      filePath: fpath,
      lineRange: [fn.startLine, fn.endLine],
      summary: '',
      tags: ['webgpu'],
      complexity: fn.endLine - fn.startLine + 1 >= 30 ? 'moderate' : 'simple',
    };

    // Generate meaningful summaries
    const lineCount = fn.endLine - fn.startLine + 1;

    if (fn.name === 'RequestAdapter') {
      fnNode.summary = 'Requests a GPU adapter from the WebGPU backend, sending the request to the constellation process and resolving via callback promise.';
    } else if (fn.name === 'RequestDevice') {
      fnNode.summary = 'Requests a GPU device from the adapter, validating features and limits, and sends the device creation request to the WebGPU render thread.';
    } else if (fn.name === 'create' && fpath.includes('gpubindgroup')) {
      fnNode.summary = 'Creates a GPUBindGroup from a descriptor, converting bind group entries and sending creation to the WebGPU render thread.';
    } else if (fn.name === 'create' && fpath.includes('gpubindgrouplayout')) {
      fnNode.summary = 'Creates a GPUBindGroupLayout from a descriptor by converting bind group layout entries and sending to the render thread.';
    } else if (fn.name === 'create' && fpath.includes('gpubuffer')) {
      fnNode.summary = 'Creates a GPUBuffer from a descriptor with optional initial mapping setup, sending buffer creation to the render thread.';
    } else if (fn.name === 'create' && fpath.includes('gpucommandencoder')) {
      fnNode.summary = 'Creates a GPUCommandEncoder from a descriptor, allocating encoder ID and sending creation to the render thread.';
    } else if (fn.name === 'create' && fpath.includes('gpucomputepipeline')) {
      fnNode.summary = 'Creates a GPUComputePipeline from a descriptor, computing pipeline layout data and sending creation to the render thread.';
    } else if (fn.name === 'MapAsync') {
      fnNode.summary = 'Initiates an asynchronous buffer mapping request with specified mode, offset, and size, returning a promise.';
    } else if (fn.name === 'GetMappedRange') {
      fnNode.summary = 'Returns an ArrayBuffer view into the mapped buffer range, handling sub-range offset and alignment validation.';
    } else if (fn.name === 'Unmap') {
      fnNode.summary = 'Unmaps the buffer, clearing views and sending the updated mapping data back to the GPU via shared memory.';
    } else if (fn.name === 'CreateBuffer' || fn.name === 'CreateBindGroup' || fn.name === 'CreateBindGroupLayout' ||
               fn.name === 'CreatePipelineLayout' || fn.name === 'CreateShaderModule' ||
               fn.name === 'CreateComputePipeline' || fn.name === 'CreateCommandEncoder' ||
               fn.name === 'CreateTexture' || fn.name === 'CreateSampler' ||
               fn.name === 'CreateRenderPipeline' || fn.name === 'CreateRenderPipelineAsync' ||
               fn.name === 'CreateComputePipelineAsync' || fn.name === 'CreateRenderBundleEncoder' ||
               fn.name === 'CreateQuerySet') {
      const resourceName = fn.name.replace('Create', '');
      fnNode.summary = `Creates a ${resourceName} resource by delegating to the appropriate factory method.`;
    } else if (fn.name === 'handle_response') {
      fnNode.summary = 'Handles an asynchronous WebGPU response, resolving or rejecting the associated promise based on response status.';
    } else if (fn.name === 'new_inherited') {
      fnNode.summary = 'Initializes the DOM object with Reflector and default field values during construction.';
    } else if (fn.name === 'new') {
      fnNode.summary = 'Constructs a new reflected DOM object with the given global scope and parameters.';
    } else if (fn.name === 'drop') {
      fnNode.summary = 'Sends a drop/resource cleanup message to the WebGPU render thread via the channel.';
    } else if (fn.name === 'from_error') {
      fnNode.summary = 'Factory method that creates a GPUError subclass (GPUValidationError, GPUOutOfMemoryError, GPUInternalError) from a wgpu error type.';
    } else if (fn.name === 'convert_bind_group_layout_entry') {
      fnNode.summary = 'Converts a GPUBindGroupLayoutEntry descriptor to a wgpu BindGroupLayoutEntry, validating buffer/sampler/texture binding types.';
    } else if (fn.name === 'convert_texture_descriptor') {
      fnNode.summary = 'Converts a GPUTextureDescriptor to a wgpu TextureDescriptor, validating format features and view formats.';
    } else if (fn.name === 'convert_texture_for_wgpu_with_cx') {
      fnNode.summary = 'Converts a GPUTextureView to a wgpu texture reference with default view fallback.';
    } else if (fn.name === 'convert_bind_group_entry') {
      fnNode.summary = 'Converts a GPUBindGroupEntry to a wgpu BindGroupEntry, handling sampler, texture view, and buffer binding resources.';
    } else if (fn.name === 'convert_load_op') {
      fnNode.summary = 'Converts a GPU load operation with optional clear value to a wgpu LoadOp.';
    } else if (fn.name === 'BeginComputePass') {
      fnNode.summary = 'Begins a new compute pass, creating a GPUComputePassEncoder from the given descriptor.';
    } else if (fn.name === 'BeginRenderPass') {
      fnNode.summary = 'Begins a new render pass, converting depth/stencil and color attachment descriptors and creating a GPURenderPassEncoder.';
    } else if (fn.name === 'Finish') {
      fnNode.summary = 'Finishes command encoding and produces a GPUCommandBuffer from the recorded commands.';
    } else if (fn.name === 'CopyBufferToBuffer' || fn.name === 'CopyBufferToTexture' ||
               fn.name === 'CopyTextureToBuffer' || fn.name === 'CopyTextureToTexture') {
      fnNode.summary = 'Records a copy operation between buffer/texture resources in the command buffer.';
    } else if (fn.name === 'PushDebugGroup' || fn.name === 'PopDebugGroup' || fn.name === 'InsertDebugMarker') {
      fnNode.summary = 'Records a debug annotation command for GPU debugging tools.';
    } else if (fn.name === 'DispatchWorkgroups') {
      fnNode.summary = 'Dispatches compute workgroups with specified X, Y, Z dimensions.';
    } else if (fn.name === 'DispatchWorkgroupsIndirect') {
      fnNode.summary = 'Dispatches compute workgroups using parameters from a GPU buffer for indirect dispatch.';
    } else if (fn.name === 'End') {
      fnNode.summary = 'Ends the compute pass and sends the completion to the render thread.';
    } else if (fn.name === 'SetBindGroup') {
      fnNode.summary = 'Binds a bind group at the specified index for the compute pass.';
    } else if (fn.name === 'SetPipeline') {
      fnNode.summary = 'Sets the active compute pipeline for the compute pass.';
    } else if (fn.name === 'GetBindGroupLayout') {
      fnNode.summary = 'Retrieves the bind group layout at the given index from the compute pipeline.';
    } else if (fn.name === 'ParseRenderPipeline') {
      fnNode.summary = 'Parses a render pipeline descriptor converting vertex buffers, fragment targets, depth/stencil state, and primitive topology.';
    } else if (fn.name === 'fire_uncaptured_error') {
      fnNode.summary = 'Dispatches an uncaptured error event to the device error handler via the WebGPU task source.';
    } else if (fn.name === 'validate_texture_format_required_features') {
      fnNode.summary = 'Validates that the device supports the required features for a given texture format.';
    } else if (fn.name === 'PushErrorScope') {
      fnNode.summary = 'Pushes a new error scope filter onto the device error scope stack.';
    } else if (fn.name === 'PopErrorScope') {
      fnNode.summary = 'Pops the top error scope and returns a promise resolving to the captured error or null.';
    } else if (fn.name === 'Destroy') {
      fnNode.summary = 'Destroys the resource and sends the destruction message to the WebGPU render thread.';
    } else if (fn.name === 'create_adapter_info') {
      fnNode.summary = 'Creates a GPUAdapterInfo from adapter information data with subgroup feature detection.';
    } else if (fn.name === 'Configure') {
      fnNode.summary = 'Configures the canvas context with a device, format, and presentation mode, validating and creating texture descriptors.';
    } else if (fn.name === 'GetCurrentTexture') {
      fnNode.summary = 'Returns the current GPUTexture for rendering, creating a new one if needed via the device.';
    } else if (fn.name === 'supported_context_format') {
      fnNode.summary = 'Validates whether a texture format is supported as a canvas context format.';
    } else if (fn.name === 'from_error') {
      fnNode.summary = 'Creates appropriate GPUError subclass from a wgpu error type with message extraction.';
    } else if (fn.name === 'from' && fpath.includes('gpucompilationmessage')) {
      fnNode.summary = 'Creates a GPUCompilationMessage from shader compilation info data.';
    } else if (fn.name === 'from' && fpath.includes('gpucompilationinfo')) {
      fnNode.summary = 'Creates a GPUCompilationInfo from an optional shader compilation error.';
    } else if (fn.name === 'clone_from') {
      fnNode.summary = 'Clones adapter info data from another GPUAdapterInfo source into a new DOM object.';
    } else if (fn.name === 'SupportedContextFormat') {
      fnNode.summary = 'Validates supported canvas context texture formats.';
    } else if (fn.name === 'map_failure') {
      fnNode.summary = 'Handles a buffer map failure by rejecting the pending map promise with an appropriate error.';
    } else if (fn.name === 'map_success') {
      fnNode.summary = 'Handles a successful buffer map by creating the mapped view and resolving the pending promise.';
    } else if (fn.name === 'expire_current_texture') {
      fnNode.summary = 'Destroys the current presentation texture and marks the canvas as dirty.';
    } else if (fn.name === 'replace_drawing_buffer') {
      fnNode.summary = 'Replaces the drawing buffer by expiring the current texture and clearing dirty state.';
    } else if (fn.name === 'context_configuration') {
      fnNode.summary = 'Returns the current canvas context configuration with device and texture descriptor info.';
    } else if (fn.name === 'pending_texture') {
      fnNode.summary = 'Returns the pending presentation texture info for readback operations.';
    } else if (fn.name === 'ResolveQuerySet') {
      fnNode.summary = 'Resolves a query set result into a destination buffer.';
    } else if (fn.name.startsWith('convert') || fn.name.startsWith('try_convert')) {
      fnNode.summary = `Converts a WebGPU DOM type to its corresponding wgpu-native representation.`;
    } else if (fn.name === 'parse_render_pipeline') {
      fnNode.summary = 'Parses a complete render pipeline descriptor including vertex buffers, fragment state, depth/stencil, and primitive configuration.';
    } else if (fn.name === 'lose') {
      fnNode.summary = 'Marks the device as lost with a reason and message, dispatching the loss event.';
    } else if (fn.name === 'get_pipeline_layout_data') {
      fnNode.summary = 'Extracts pipeline layout data, determining if the layout is explicit or implicit.';
    } else if (fn.name === 'update_rendering') {
      fnNode.summary = 'Updates the canvas rendering state, sending the current texture for presentation.';
    } else if (fn.name === 'set_image_key') {
      fnNode.summary = 'Sets the image key for the canvas context presentation.';
    } else if (fn.name === 'get_image_data') {
      fnNode.summary = 'Retrieves image data from the GPU canvas context via a blocking channel request.';
    } else if (fn.name === 'Constructor') {
      fnNode.summary = 'Constructor method for creating new GPU error subclass instances.';
    } else if (fn.name === 'new_with_proto') {
      fnNode.summary = 'Creates a new DOM object with a specific prototype for error type hierarchy.';
    } else if (fn.name === 'Unconfigure') {
      fnNode.summary = 'Unconfigures the canvas context, releasing configuration and current texture resources.';
    } else if (fn.name === 'resize') {
      fnNode.summary = 'Resizes the canvas context, replacing the drawing buffer and re-computing texture descriptors.';
    } else if (fn.name === 'Features' || fn.name === 'Limits' || fn.name === 'Info') {
      fnNode.summary = `Accessor returning the ${fn.name.toLowerCase()} object reference.`;
    } else if (fn.name === 'Messages') {
      fnNode.summary = 'Returns the frozen array of GPUCompilationMessage objects.';
    } else if (fn.name === 'WgslLanguageFeatures') {
      fnNode.summary = 'Returns the WGSL language features set, lazily initializing if needed.';
    } else if (fn.name === 'GetPreferredCanvasFormat') {
      fnNode.summary = 'Returns the preferred canvas texture format for the current platform.';
    } else if (fn.name === 'GetQueue') {
      fnNode.summary = 'Returns the default GPU queue associated with this device.';
    } else if (fn.name === 'set_device') {
      fnNode.summary = 'Sets the device reference on the queue after construction.';
    } else if (fn.name === 'label' || fn.name === 'Label') {
      fnNode.summary = 'Returns the current label of the GPU object.';
    } else if (fn.name === 'set_label' || fn.name === 'SetLabel') {
      fnNode.summary = 'Sets the label of the GPU object.';
    } else if (fn.name === 'Canvas') {
      fnNode.summary = 'Returns the canvas element associated with this context.';
    } else if (fn.name === 'mark_as_dirty') {
      fnNode.summary = 'Marks the canvas as dirty, triggering re-rendering.';
    } else if (fn.name.startsWith('set_')) {
      fnNode.summary = `Sets the ${fn.name.replace('set_', '')} property value.`;
    } else if (fn.name === 'Lost') {
      fnNode.summary = 'Returns a promise that resolves when the device is lost.';
    } else if (fn.name === 'Size') {
      fnNode.summary = 'Returns the size of the GPU buffer in bytes.';
    } else if (fn.name === 'Usage') {
      fnNode.summary = 'Returns the usage flags of the GPU buffer.';
    } else if (fn.name === 'MapState') {
      fnNode.summary = 'Returns the current mapping state: unmapped, pending, or mapped.';
    } else if (fn.name === 'device_id') {
      fnNode.summary = 'Returns the device ID associated with this command encoder.';
    } else if (fn.name === 'texture_descriptor_for_canvas_and_configuration') {
      fnNode.summary = 'Builds a texture descriptor for the canvas based on current configuration and size.';
    } else if (fn.name === 'id') {
      fnNode.summary = 'Returns the internal WebGPU ID of the resource.';
    } else {
      fnNode.summary = `WebGPU ${fn.name} method with ${fn.params.length} parameters.`;
    }

    addNode(fnNode);

    // contains edge
    addEdge({
      source: `file:${fpath}`,
      target: fnId,
      type: 'contains',
      direction: 'forward',
      weight: 1.0
    });

    // exports edge if exported
    const isExported = file.exports?.some(e => e.name === fn.name);
    if (isExported) {
      addEdge({
        source: `file:${fpath}`,
        target: fnId,
        type: 'exports',
        direction: 'forward',
        weight: 0.8
      });
    }
  }
}

// ============================================================================
// IMPORT EDGES
// ============================================================================
for (const [sourceFile, targets] of Object.entries(importMap)) {
  for (const targetFile of targets) {
    addEdge({
      source: `file:${sourceFile}`,
      target: `file:${targetFile}`,
      type: 'imports',
      direction: 'forward',
      weight: 0.7
    });
  }
}

// ============================================================================
// DEPENDS_ON EDGES (from call graphs - file-level relations)
// ============================================================================
// Map of which files reference functions from other files in call graphs
const callRelations = {
  'components/script/dom/webgpu/gpu.rs': ['components/script/dom/webgpu/gpuadapter.rs'],
  'components/script/dom/webgpu/gpuadapter.rs': ['components/script/dom/webgpu/gpudevice.rs'],
  'components/script/dom/webgpu/gpubindgroup.rs': ['components/script/dom/webgpu/gpubindgrouplayout.rs', 'components/script/dom/webgpu/gpuconvert.rs', 'components/script/dom/webgpu/gpudevice.rs'],
  'components/script/dom/webgpu/gpubindgrouplayout.rs': ['components/script/dom/webgpu/gpuconvert.rs', 'components/script/dom/webgpu/gpudevice.rs'],
  'components/script/dom/webgpu/gpubuffer.rs': ['components/script/dom/webgpu/gpudevice.rs'],
  'components/script/dom/webgpu/gpucommandencoder.rs': ['components/script/dom/webgpu/gpubuffer.rs', 'components/script/dom/webgpu/gpucommandbuffer.rs', 'components/script/dom/webgpu/gpucomputepassencoder.rs', 'components/script/dom/webgpu/gpuconvert.rs', 'components/script/dom/webgpu/gpudevice.rs'],
  'components/script/dom/webgpu/gpucomputepassencoder.rs': ['components/script/dom/webgpu/gpubindgroup.rs', 'components/script/dom/webgpu/gpubuffer.rs', 'components/script/dom/webgpu/gpucommandencoder.rs', 'components/script/dom/webgpu/gpucomputepipeline.rs'],
  'components/script/dom/webgpu/gpucomputepipeline.rs': ['components/script/dom/webgpu/gpubindgrouplayout.rs', 'components/script/dom/webgpu/gpuconvert.rs', 'components/script/dom/webgpu/gpudevice.rs'],
  'components/script/dom/webgpu/gpudevice.rs': ['components/script/dom/webgpu/gpuadapter.rs', 'components/script/dom/webgpu/gpuadapterinfo.rs', 'components/script/dom/webgpu/gpubindgroup.rs', 'components/script/dom/webgpu/gpubindgrouplayout.rs', 'components/script/dom/webgpu/gpubuffer.rs', 'components/script/dom/webgpu/gpucommandencoder.rs', 'components/script/dom/webgpu/gpucomputepipeline.rs', 'components/script/dom/webgpu/gpuconvert.rs', 'components/script/dom/webgpu/gpuerror.rs'],
  'components/script/dom/webgpu/gpuerror.rs': ['components/script/dom/webgpu/gpuinternalerror.rs', 'components/script/dom/webgpu/gpuoutofmemoryerror.rs'],
  'components/script/dom/webgpu/gpuinternalerror.rs': ['components/script/dom/webgpu/gpuerror.rs'],
  'components/script/dom/webgpu/gpuoutofmemoryerror.rs': ['components/script/dom/webgpu/gpuerror.rs']
};

// ============================================================================
// KNOWN CROSS-BATCH CALLS from call graphs
// ============================================================================
// GPUDevice calls many Create* methods on types in other batches
// gpuconvert.rs references texture types
// gpucanvascontext references gputexture

// ============================================================================
// OUTPUT
// ============================================================================
const output = { nodes, edges };

console.log(`Nodes: ${nodes.length}, Edges: ${edges.length}`);

// Check if we need to split
const NODE_THRESHOLD = 60;
const EDGE_THRESHOLD = 120;

if (nodes.length <= NODE_THRESHOLD && edges.length <= EDGE_THRESHOLD) {
  writeFileSync(join(OUTPUT_DIR, 'batch-24.json'), JSON.stringify(output, null, 2), 'utf-8');
  console.log('Written as single file: batch-24.json');
} else {
  // Split into parts
  const parts = Math.max(1, Math.ceil(Math.max(nodes.length / NODE_THRESHOLD, edges.length / EDGE_THRESHOLD)));
  console.log(`Splitting into ${parts} parts`);

  // Sort files alphabetically
  const sortedBatchFiles = [...batchFiles].sort((a, b) => a.path.localeCompare(b.path));
  const filesPerPart = Math.ceil(sortedBatchFiles.length / parts);

  for (let p = 0; p < parts; p++) {
    const partFiles = sortedBatchFiles.slice(p * filesPerPart, (p + 1) * filesPerPart);
    const partFilePaths = new Set(partFiles.map(f => f.path));

    // Collect nodes whose filePath is in this part's files
    const partNodes = nodes.filter(n => {
      if (n.filePath) {
        return partFilePaths.has(n.filePath);
      }
      return false;
    });

    const partNodeIds = new Set(partNodes.map(n => n.id));

    // Collect edges where source is in this part's nodes
    const partEdges = edges.filter(e => partNodeIds.has(e.source));

    const partOutput = { nodes: partNodes, edges: partEdges };
    const partNum = p + 1;
    const filename = `batch-24-part-${partNum}.json`;
    writeFileSync(join(OUTPUT_DIR, filename), JSON.stringify(partOutput, null, 2), 'utf-8');
    console.log(`Part ${partNum}: ${partNodes.length} nodes, ${partEdges.length} edges -> ${filename}`);
  }
}

console.log('Done.');
