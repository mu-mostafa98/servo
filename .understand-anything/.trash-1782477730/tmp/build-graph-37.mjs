// Script to construct graph nodes and edges for batch 37
import fs from 'fs';

const results = JSON.parse(fs.readFileSync('d:/Projects/servo/.understand-anything/tmp/ua-file-extract-results-37.json', 'utf8'));

const allNodes = [];
const allEdges = [];

// Track node IDs to avoid duplicates
const nodeIds = new Set();

function addNode(node) {
  if (nodeIds.has(node.id)) {
    console.error('DUPLICATE NODE:', node.id);
    return;
  }
  nodeIds.add(node.id);
  allNodes.push(node);
}

function addEdge(edge) {
  if (edge.source === edge.target) {
    console.error('SELF-REF EDGE:', edge);
    return;
  }
  allEdges.push(edge);
}

// ===== File summaries and metadata =====

const fileData = {};

// Broadcaster channel file
fileData['components/constellation/broadcastchannel.rs'] = {
  type: 'file',
  summary: 'Manages broadcast channel routing and message dispatch between browsing contexts.',
  tags: ['broadcast-channel', 'message-routing', 'event-dispatch'],
  complexity: 'moderate',
  languageNotes: 'Uresents a HashMap-based channel registry with origin-scoped channel trees.',
};

fileData['components/constellation/browsingcontext.rs'] = {
  type: 'file',
  summary: 'Defines browsing context structures and iterators for traversing the context tree.',
  tags: ['browsing-context', 'tree-traversal', 'session'],
  complexity: 'moderate',
  languageNotes: 'Provides depth-first iterators (FullyActiveBrowsingContextsIterator, AllBrowsingContextsIterator) for traversing the browsing context hierarchy.',
};

fileData['components/constellation/constellation.rs'] = {
  type: 'file',
  summary: 'Core constellation module implementing the browser process orchestrator, managing pipelines, browsing contexts, navigation, and script thread coordination.',
  tags: ['orchestrator', 'process-management', 'navigation', 'event-handling', 'core'],
  complexity: 'complex',
  languageNotes: 'The central message loop processes over 80 message types from embedder, script threads, and background hang monitors using select-based event dispatch.',
};

fileData['components/constellation/constellation_webview.rs'] = {
  type: 'file',
  summary: 'Represents a single web view within the constellation, managing input event routing, themes, and active pipeline tracking.',
  tags: ['webview', 'input-routing', 'pipeline'],
  complexity: 'moderate',
};

fileData['components/constellation/embedder.rs'] = {
  type: 'file',
  summary: 'Defines the message enum for communication from the constellation to the embedder, covering shutdown, navigation, panics, and media events.',
  tags: ['embedder', 'message-types', 'ipc'],
  complexity: 'simple',
};

fileData['components/constellation/event_loop.rs'] = {
  type: 'file',
  summary: 'Manages script event loop lifecycle: spawning in threads or child processes, message sending, and background hang monitor integration.',
  tags: ['event-loop', 'script-execution', 'process-spawning'],
  complexity: 'moderate',
  languageNotes: 'Supports both in-thread (for same-process) and multiprocess spawning via spawn_in_thread/spawn_in_process methods.',
};

fileData['components/constellation/lib.rs'] = {
  type: 'file',
  summary: 'Module root that re-exports all constellation submodules for external consumption.',
  tags: ['barrel', 'module-root', 're-export'],
  complexity: 'simple',
};

fileData['components/constellation/logging.rs'] = {
  type: 'file',
  summary: 'Provides loggers for capturing log records from script threads and the embedder, forwarding them to the constellation as IPC messages.',
  tags: ['logging', 'diagnostics', 'ipc'],
  complexity: 'moderate',
};

fileData['components/constellation/pipeline.rs'] = {
  type: 'file',
  summary: 'Manages individual pipeline lifecycle: spawning, throttling, activity tracking, and parent-child browsing context relationships.',
  tags: ['pipeline', 'lifecycle', 'child-management'],
  complexity: 'moderate',
};

fileData['components/constellation/process_manager.rs'] = {
  type: 'file',
  summary: 'Manages child process lifecycle for sandboxed and unsandboxed subprocesses, supporting registration, selection, and cleanup.',
  tags: ['process-management', 'child-process', 'sandbox'],
  complexity: 'simple',
};

fileData['components/constellation/sandboxing.rs'] = {
  type: 'file',
  summary: 'Implements platform-specific sandboxing profiles and multiprocess spawning for script and service worker processes.',
  tags: ['sandbox', 'security', 'process-spawning', 'multiprocess'],
  complexity: 'complex',
  languageNotes: 'Provides conditional compilation for Windows, Android, and Unix sandbox profiles with different security policies per platform.',
};

fileData['components/constellation/serviceworker.rs'] = {
  type: 'file',
  summary: 'Defines unprivileged content configuration for spawning service worker processes.',
  tags: ['service-worker', 'process-spawning', 'unprivileged'],
  complexity: 'simple',
};

fileData['components/constellation/session_history.rs'] = {
  type: 'file',
  summary: 'Implements joint session history for navigation tracking, diffs, and reload detection across browsing contexts.',
  tags: ['session-history', 'navigation', 'diff-tracking'],
  complexity: 'moderate',
  languageNotes: 'Uses a past/future Vec structure for bidirectional navigation with diff-based change tracking.',
};

fileData['components/constellation/tracing.rs'] = {
  type: 'file',
  summary: 'Defines a log-target wrapper for tracing-subscriber integration, mapping tracing events to the constellation logging pipeline.',
  tags: ['tracing', 'logging', 'instrumentation'],
  complexity: 'simple',
};

// ===== Build file nodes =====
for (const f of results.results) {
  const meta = fileData[f.path];
  const node = {
    id: 'file:' + f.path,
    type: 'file',
    name: f.path.split('/').pop(),
    filePath: f.path,
    summary: meta.summary,
    tags: meta.tags,
    complexity: meta.complexity,
  };
  if (meta.languageNotes) {
    node.languageNotes = meta.languageNotes;
  }
  addNode(node);
}

// ===== Build function and class nodes =====
for (const f of results.results) {
  const funcs = f.functions || [];
  const classes = f.classes || [];
  const exports = f.exports || [];
  const exportedSet = new Set(exports.map(e => e.name));

  // Functions: qualify if exported OR >= 10 lines
  // Track duplicate function names by using line numbers for disambiguation
  const funcCounts = {};
  for (const fn of funcs) {
    funcCounts[fn.name] = (funcCounts[fn.name] || 0) + 1;
  }

  for (const fn of funcs) {
    const lineCount = fn.endLine - fn.startLine + 1;
    if (!exportedSet.has(fn.name) && lineCount < 10) continue;

    // Use line-based suffix for disambiguation when there are duplicates
    const isDup = funcCounts[fn.name] > 1;
    const nodeId = isDup
      ? `function:${f.path}:${fn.name}:L${fn.startLine}`
      : `function:${f.path}:${fn.name}`;
    const displayName = isDup ? `${fn.name}:L${fn.startLine}` : fn.name;

    addNode({
      id: nodeId,
      type: 'function',
      name: displayName,
      filePath: f.path,
      lineRange: [fn.startLine, fn.endLine],
      summary: generateFuncSummary(f.path, fn),
      tags: generateFuncTags(f.path, fn, exportedSet.has(fn.name)),
      complexity: lineCount >= 200 ? 'complex' : lineCount >= 50 ? 'moderate' : 'simple',
    });

    // contains edge
    addEdge({
      source: `file:${f.path}`,
      target: nodeId,
      type: 'contains',
      direction: 'forward',
      weight: 1.0,
    });

    // exports edge if exported
    if (exportedSet.has(fn.name)) {
      addEdge({
        source: `file:${f.path}`,
        target: nodeId,
        type: 'exports',
        direction: 'forward',
        weight: 0.8,
      });
    }
  }

  // Classes: qualify if exported OR (2+ methods) OR (20+ lines)
  for (const cls of classes) {
    const lineCount = cls.endLine - cls.startLine + 1;
    if (!exportedSet.has(cls.name) && cls.methods.length < 2 && lineCount < 20) continue;

    addNode({
      id: `class:${f.path}:${cls.name}`,
      type: 'class',
      name: cls.name,
      filePath: f.path,
      lineRange: [cls.startLine, cls.endLine],
      summary: generateClassSummary(f.path, cls),
      tags: generateClassTags(f.path, cls, exportedSet.has(cls.name)),
      complexity: lineCount >= 200 ? 'complex' : lineCount >= 50 ? 'moderate' : 'simple',
    });

    // contains edge
    addEdge({
      source: `file:${f.path}`,
      target: `class:${f.path}:${cls.name}`,
      type: 'contains',
      direction: 'forward',
      weight: 1.0,
    });

    // exports edge if exported
    if (exportedSet.has(cls.name)) {
      addEdge({
        source: `file:${f.path}`,
        target: `class:${f.path}:${cls.name}`,
        type: 'exports',
        direction: 'forward',
        weight: 0.8,
      });
    }
  }
}

// ===== Build imports edges =====
// Read batchImportData
const inputData = JSON.parse(fs.readFileSync('d:/Projects/servo/.understand-anything/tmp/ua-file-analyzer-input-37.json', 'utf8'));
const batchImportData = inputData.batchImportData;

for (const [filePath, imports] of Object.entries(batchImportData)) {
  for (const target of imports) {
    addEdge({
      source: `file:${filePath}`,
      target: `file:${target}`,
      type: 'imports',
      direction: 'forward',
      weight: 0.7,
    });
  }
}

// ===== Helper functions for summaries and tags =====

function generateFuncSummary(path, fn) {
  const name = fn.name;
  const params = fn.params || [];

  // Constellation-specific handler functions
  if (path.includes('constellation.rs')) {
    if (name.startsWith('handle_')) {
      const topic = name.replace('handle_', '').replace(/_/g, ' ');
      return `Processes ${topic} messages received by the constellation event loop.`;
    }
    if (name === 'start') {
      return 'Entry point that creates the constellation, spawns the event loop thread, and begins processing messages.';
    }
    if (name === 'run') {
      return 'Main event loop that processes messages from embedder, script threads, and background hang monitors.';
    }
    if (name === 'load_url') {
      return 'Initiates page navigation by creating a new pipeline, managing history, and coordinating script thread loading.';
    }
    if (name === 'send_message_to_pipeline') {
      return 'Sends a message to a specific pipeline via its event loop channel.';
    }
    if (name === 'next_pipeline_namespace_id') {
      return 'Allocates and returns the next unique pipeline namespace identifier.';
    }
    if (name === 'next_browsing_context_group_id') {
      return 'Allocates and returns the next browsing context group ID.';
    }
    if (name === 'add_event_loop') {
      return 'Registers an event loop in the constellation event loop set.';
    }
    if (name === 'add_event_loop_join_handle') {
      return 'Registers a join handle for an event loop thread.';
    }
    if (name === 'clean_up_finished_script_event_loops') {
      return 'Removes completed script event loops from tracking.';
    }
    if (name === 'check_origin_against_pipeline') {
      return 'Validates whether an origin matches the pipeline origin for security checks.';
    }
    if (name === 'mutate_user_contents_for_manager_id_and_notify_script_threads') {
      return 'Applies a mutation callback to user content and notifies affected script threads.';
    }
    if (name === 'send_message_to_all_background_hang_monitors') {
      return 'Broadcasts a message to all registered background hang monitors.';
    }
    if (name === 'set_frame_tree_for_webview') {
      return 'Rebuilds and sends the current frame tree structure for a specific webview.';
    }
    if (name === 'browsing_context_to_sendable') {
      return 'Converts a browsing context into its sendable IPC representation.';
    }
    if (name === 'maybe_close_random_pipeline') {
      return 'Probabilistically closes a random pipeline for memory management under load.';
    }
    if (name === 'notify_history_changed') {
      return 'Sends a history change notification to the embedder for UI updates.';
    }
    if (name === 'notify_focus_state') {
      return 'Sends focus state information to the embedder for the given pipeline.';
    }
    if (name === 'set_webview_throttled') {
      return 'Updates the throttle state for all browsing contexts in a webview.';
    }
    if (name === 'send_screenshot_readiness_requests_to_pipelines') {
      return 'Requests screenshot readiness acknowledgments from all active pipelines.';
    }
    if (name === 'create_canvas_paint_thread') {
      return 'Creates a new canvas paint thread for offscreen canvas rendering.';
    }
    if (name === 'script_to_devtools_callback') {
      return 'Forwards DevTools messages from script threads to the DevTools channel.';
    }
    if (name.startsWith('close_')) {
      return `Closes and cleans up ${name.replace('close_', '').replace(/_/g, ' ')} resources.`;
    }
    if (name.startsWith('new_')) {
      return `Creates a new ${name.replace('new_', '').replace(/_/g, ' ')} instance.`;
    }
    if (name.startsWith('get_') || name.startsWith('set_')) {
      return `${name.startsWith('get_') ? 'Retrieves' : 'Updates'} the ${name.replace(/^(get_|set_)/, '').replace(/_/g, ' ')}.`;
    }
    if (name.startsWith('update_')) {
      return `Updates the ${name.replace('update_', '').replace(/_/g, ' ')} state.`;
    }
    if (name.startsWith('send_')) {
      return `Sends ${name.replace('send_', '').replace(/_/g, ' ')} via appropriate channels.`;
    }
    if (name.startsWith('schedule_')) {
      return `Schedules ${name.replace('schedule_', '').replace(/_/g, ' ')} operation.`;
    }
    if (name.startsWith('forward_')) {
      return `Forwards ${name.replace('forward_', '').replace(/_/g, ' ')} to the appropriate target.`;
    }
    if (name.startsWith('resize_')) {
      return `Resizes the ${name.replace('resize_', '').replace(/_/g, ' ')} with new viewport dimensions.`;
    }
    if (name.startsWith('focus_')) {
      return `Handles focus operations for ${name.replace('focus_', '').replace(/_/g, ' ')}.`;
    }
    if (name.startsWith('change_')) {
      return `Processes ${name.replace('change_', '').replace(/_/g, ' ')} updates.`;
    }
    if (name.startsWith('switch_')) {
      return `Switches ${name.replace('switch_', '').replace(/_/g, ' ')} mode.`;
    }
    if (name.startsWith('trim_')) {
      return `Trims ${name.replace('trim_', '').replace(/_/g, ' ')} by removing excess entries.`;
    }
    if (name.startsWith('add_pending')) {
      return 'Records a pending session history change for batch processing.';
    }
  }

  // Broadcast channel functions
  if (path.includes('broadcastchannel.rs')) {
    if (name === 'new_broadcast_channel_router') return 'Registers a new broadcast channel router with associated callback.';
    if (name === 'remove_broadcast_channel_router') return 'Removes a broadcast channel router and its channel entries.';
    if (name === 'new_broadcast_channel_name_in_router') return 'Registers a channel name under a specific origin within a router.';
    if (name === 'remove_broadcast_channel_name_in_router') return 'Removes a channel name from a router, cleaning up empty origins.';
    if (name === 'schedule_broadcast') return 'Dispatches a message to all routers subscribed to the target channel.';
  }

  // Browsing context functions
  if (path.includes('browsingcontext.rs')) {
    if (name === 'new') return 'Constructs a new browsing context with the given identifiers and viewport details.';
    if (name === 'update_current_entry') return 'Updates the current pipeline ID in the browsing context.';
    if (name === 'is_top_level') return 'Returns whether this browsing context has no parent pipeline.';
    if (name === 'next' && fn.startLine < 170) return 'Returns the next fully active browsing context in depth-first traversal order.';
    if (name === 'next' && fn.startLine >= 170) return 'Returns the next browsing context including inactive ones in traversal order.';
  }

  // Event loop functions
  if (path.includes('event_loop.rs')) {
    if (name === 'spawn') return 'Spawns a new event loop either in-process or in a separate thread based on configuration.';
    if (name === 'spawn_in_thread') return 'Spawns the event loop in an OS thread with script thread factory.';
    if (name === 'spawn_in_process') return 'Spawns the event loop in a sandboxed child process.';
    if (name === 'send') return 'Sends a message to the script thread through the event loop channel.';
    if (name === 'id') return 'Returns the unique identifier for this event loop.';
    if (name === 'send_message_to_background_hang_monitor') return 'Sends a hang monitor message for this event loop.';
  }

  // Logging functions
  if (path.includes('logging.rs')) {
    if (name === 'new' && fn.startLine < 50) return 'Creates a new FromScriptLogger that forwards log records to the constellation.';
    if (name === 'new' && fn.startLine >= 80) return 'Creates a new FromEmbedderLogger that forwards embedder log records to the constellation.';
    if (name === 'filter') return 'Returns the log filter level for this logger.';
    if (name === 'log') return 'Captures a log record and forwards it as a LogEntry message.';
    if (name === 'log_entry') return 'Formats a log record into a structured LogEntry with level and optional backtrace.';
  }

  // Pipeline functions
  if (path.includes('pipeline.rs')) {
    if (name === 'spawn') return 'Initiates a new pipeline by sending a SpawnPipeline message to the event loop.';
    if (name === 'new_already_spawned') return 'Records a pipeline that was spawned externally with its properties.';
    if (name === 'send_exit_message_to_script') return 'Sends an ExitPipeline message and optionally discards the browsing context.';
    if (name === 'set_activity') return 'Sends a SetDocumentActivity message to the pipeline script thread.';
    if (name === 'to_sendable') return 'Creates a sendable representation of this pipeline for IPC.';
    if (name === 'add_child') return 'Adds a child browsing context to this pipeline.';
    if (name === 'remove_child') return 'Removes a child browsing context, cleaning up parent references.';
    if (name === 'set_throttled') return 'Sends throttle state updates to both the script thread and paint thread.';
  }

  // Process manager functions
  if (path.includes('process_manager.rs')) {
    if (name === 'new') return 'Creates a new ProcessManager with the given memory profiler channel.';
    if (name === 'add') return 'Registers a new child process with its receiver for selection.';
    if (name === 'register') return 'Registers all process receivers into a Select for message multiplexing.';
    if (name === 'receiver_at') return 'Returns the receiver at the given index in the process list.';
    if (name === 'remove') return 'Removes and terminates a child process, unregistering from the memory profiler.';
    if (name === 'wait') return 'Blocks until the child process exits and returns its exit status.';
  }

  // Sandboxing functions
  if (path.includes('sandboxing.rs')) {
    if (name === 'opts') return 'Returns the command-line options for unprivileged content.';
    if (name === 'prefs') return 'Returns the preferences for unprivileged content.';
    if (name.startsWith('content_process_sandbox_profile')) return 'Builds a platform-specific sandbox profile for content processes.';
    if (name.startsWith('spawn_multiprocess')) return 'Spawns an unprivileged content process with appropriate sandboxing.';
    if (name === 'setup_common') return 'Configures common command-line arguments and environment variables for a child process.';
  }

  // Service worker functions
  if (path.includes('serviceworker.rs')) {
    if (name === 'new') return 'Creates service worker unprivileged content configuration.';
    if (name === 'start') return 'Starts the service worker process using the service worker factory.';
    if (name === 'spawn_multiprocess') return 'Spawns a sandboxed service worker process.';
  }

  // Session history functions
  if (path.includes('session_history.rs')) {
    if (name === 'new') return 'Creates an empty joint session history.';
    if (name === 'history_length') return 'Returns the total number of history entries.';
    if (name === 'push_diff') return 'Records a session history change diff and clears future entries.';
    if (name === 'replace_reloader' && fn.startLine < 100) return 'Updates reloader references across all history entries.';
    if (name === 'replace_history_state') return 'Updates the history state ID and URL for matching pipeline entries.';
    if (name === 'remove_entries_for_browsing_context') return 'Removes all history entries belonging to a specific browsing context.';
    if (name === 'eq') return 'Compares two NeedsToReload variants for equality.';
    if (name === 'alive_pipeline_id') return 'Returns the pipeline ID if this NeedsToReload is Yes variant.';
    if (name === 'alive_old_pipeline') return 'Returns the old pipeline ID if this SessionHistoryDiff variant applies.';
    if (name === 'alive_new_pipeline') return 'Returns the new pipeline ID if this SessionHistoryDiff variant applies.';
    if (name === 'replace_reloader' && fn.startLine >= 240) return 'Replaces a reloader reference in a SessionHistoryDiff when it matches the old reloader.';
  }

  // Tracing
  if (path.includes('tracing.rs')) {
    if (name === 'log_target') return 'Maps a tracing event to a Servo log entry using the LogTarget wrapper.';
  }

  // Catch remaining constellation functions
  if (path.includes('constellation.rs')) {
    if (name.includes('iterate') || name.includes('iter')) return `Iterates over ${name.replace(/_iter$/, '').replace(/_/g, ' ')}.`;
  }

  // Default
  return `${name} function with ${params.length} parameter(s).`;
}

function generateFuncTags(path, fn, isExported) {
  const tags = [];
  if (isExported) tags.push('exported');

  const name = fn.name;

  if (path.includes('constellation.rs')) {
    if (name.startsWith('handle_')) {
      tags.push('event-handler');
      if (name.includes('messageport') || name.includes('message_port')) tags.push('message-port');
      else if (name.includes('script') || name.includes('pipeline')) tags.push('pipeline');
      else if (name.includes('navigation') || name.includes('history') || name.includes('load') || name.includes('reload') || name.includes('traverse')) tags.push('navigation');
      else if (name.includes('webdriver')) tags.push('webdriver');
      else if (name.includes('input') || name.includes('mouse') || name.includes('keyboard')) tags.push('input');
      else if (name.includes('focus')) tags.push('focus');
      else if (name.includes('animation') || name.includes('paint')) tags.push('rendering');
      else if (name.includes('screenshot')) tags.push('screenshot');
      else if (name.includes('exit') || name.includes('shutdown') || name.includes('panic')) tags.push('lifecycle');
      else if (name.includes('canvas')) tags.push('canvas');
      else tags.push('message-handler');
    } else if (name === 'start') {
      tags.push('entry-point', 'initialization');
    } else if (name === 'run') {
      tags.push('event-loop', 'orchestration');
    } else if (name === 'load_url') {
      tags.push('navigation', 'entry-point');
    } else if (name.includes('browsing_context')) {
      tags.push('browsing-context');
    } else if (name.includes('pipeline')) {
      tags.push('pipeline');
    } else if (name.includes('event_loop')) {
      tags.push('event-loop');
    } else if (name.includes('close_')) {
      tags.push('cleanup', 'lifecycle');
    } else if (name.includes('new_')) {
      tags.push('factory', 'lifecycle');
    } else {
      tags.push('utility');
    }
  } else if (path.includes('broadcastchannel.rs')) {
    tags.push('broadcast-channel', 'messaging');
  } else if (path.includes('browsingcontext.rs')) {
    if (name === 'next') tags.push('iterator', 'traversal');
    else tags.push('browsing-context');
  } else if (path.includes('event_loop.rs')) {
    tags.push('event-loop');
    if (name.includes('spawn') || name === 'new') tags.push('spawning');
    if (name === 'send') tags.push('messaging');
  } else if (path.includes('logging.rs')) {
    tags.push('logging');
    if (name === 'new') tags.push('factory');
  } else if (path.includes('pipeline.rs')) {
    tags.push('pipeline');
    if (name === 'spawn' || name === 'new_already_spawned') tags.push('factory');
    if (name === 'set_throttled' || name === 'set_activity') tags.push('lifecycle');
  } else if (path.includes('process_manager.rs')) {
    tags.push('process-management');
  } else if (path.includes('sandboxing.rs')) {
    tags.push('sandbox', 'security');
  } else if (path.includes('serviceworker.rs')) {
    tags.push('service-worker');
  } else if (path.includes('session_history.rs')) {
    tags.push('session-history', 'navigation');
  } else if (path.includes('tracing.rs')) {
    tags.push('tracing');
  }

  // Deduplicate and ensure we have at least 3 tags
  const uniqueTags = [...new Set(tags)];
  const fallbacks = ['internal', 'method', 'utility'];
  while (uniqueTags.length < 3) {
    uniqueTags.push(fallbacks[uniqueTags.length - 1]);
  }
  return uniqueTags.slice(0, 5);
}

function generateClassSummary(path, cls) {
  const name = cls.name;
  const methods = cls.methods || [];

  if (path.includes('constellation.rs')) {
    if (name === 'Constellation') return 'Core struct holding all constellation state including pipelines, browsing contexts, event loops, and navigation history.';
    if (name === 'InitialConstellationState') return 'Aggregates initial configuration state passed into the constellation at startup.';
    if (name === 'BrowsingContextGroup') return 'Groups related browsing contexts with shared event loops and WebGPU instances.';
    if (name === 'TransferState') return 'Enum representing the state of a message port transfer operation.';
    if (name === 'ExitPipelineMode') return 'Enum controlling whether pipeline exit is normal or forced.';
    if (name === 'ScreenshotRequestState') return 'Tracks the state of screenshot readiness requests across pipelines.';
    if (name === 'ScreenshotReadinessRequest') return 'Holds webview ID and per-pipeline screenshot readiness states.';
  }

  if (path.includes('broadcastchannel.rs')) {
    if (name === 'BroadcastChannels') return 'Manages broadcast channel routers and origin-scoped channel name registrations.';
  }

  if (path.includes('browsingcontext.rs')) {
    if (name === 'BrowsingContext') return 'Represents a browsing context with its pipeline set, viewport details, and hierarchy information.';
    if (name === 'NewBrowsingContextInfo') return 'Carries initialization parameters for creating a new browsing context.';
    if (name === 'FullyActiveBrowsingContextsIterator') return 'Depth-first iterator yielding only fully active browsing contexts.';
    if (name === 'AllBrowsingContextsIterator') return 'Depth-first iterator yielding all browsing contexts including inactive ones.';
  }

  if (path.includes('constellation_webview.rs')) {
    if (name === 'ConstellationWebView') return 'Represents a web view within the constellation, tracking active pipeline, input state, session history, and theme.';
  }

  if (path.includes('embedder.rs')) {
    if (name === 'ConstellationToEmbedderMsg') return 'Enum of messages sent from the constellation to the embedder including shutdown, navigation, focus, and panic events.';
  }

  if (path.includes('event_loop.rs')) {
    if (name === 'EventLoop') return 'Encapsulates a script event loop with messaging channels and background hang monitor integration.';
    if (name === 'NewScriptEventLoopProcessInfo') return 'Holds initialization data for spawning a new script event loop process.';
  }

  if (path.includes('logging.rs')) {
    if (name === 'FromScriptLogger') return 'Logger implementation that forwards script thread log records to the constellation via IPC.';
    if (name === 'FromEmbedderLogger') return 'Logger implementation that forwards embedder log records to the constellation via IPC.';
  }

  if (path.includes('pipeline.rs')) {
    if (name === 'Pipeline') return 'Represents a single pipeline with its event loop, children browsing contexts, throttling state, and activity level.';
  }

  if (path.includes('process_manager.rs')) {
    if (name === 'Process') return 'Enum representing either an unsandboxed or sandboxed child process with a wait handle.';
    if (name === 'ProcessManager') return 'Manages a collection of child processes with select-based message routing and memory profiler integration.';
  }

  if (path.includes('sandboxing.rs')) {
    if (name === 'UnprivilegedContent') return 'Enum of unprivileged content types (ScriptEventLoop, ServiceWorker) with access to options and preferences.';
    if (name === 'CommandMethods') return 'Helper trait for adding arguments and environment variables to sandbox command builders.';
  }

  if (path.includes('serviceworker.rs')) {
    if (name === 'ServiceWorkerUnprivilegedContent') return 'Configuration for spawning an unprivileged service worker process with options, preferences, and origin.';
  }

  if (path.includes('session_history.rs')) {
    if (name === 'JointSessionHistory') return 'Tracks bidirectional session history using past and future entry stacks.';
    if (name === 'SessionHistoryChange') return 'Describes a session history change with pipeline IDs, load data, and history behavior.';
    if (name === 'NeedsToReload') return 'Enum indicating whether a session history entry requires reloading.';
    if (name === 'SessionHistoryDiff') return 'Enum describing different types of session history diffs: browsing context, pipeline, or hash changes.';
  }

  if (path.includes('tracing.rs')) {
    if (name === 'LogTarget') return 'Tracing subscriber log target wrapper that maps tracing events to Servo log entries.';
  }

  return `Struct/enum ${name} with ${methods.length} method(s).`;
}

function generateClassTags(path, cls, isExported) {
  const tags = [];
  if (isExported) tags.push('exported');

  const name = cls.name;

  if (path.includes('constellation.rs')) {
    if (name === 'Constellation') tags.push('core', 'orchestrator', 'singleton');
    else if (name.includes('State')) tags.push('state');
    else if (name.includes('Screenshot')) tags.push('screenshot');
    else tags.push('supporting-type');
  } else if (path.includes('broadcastchannel.rs')) {
    tags.push('broadcast-channel', 'registry');
  } else if (path.includes('browsingcontext.rs')) {
    if (name.includes('Iterator')) tags.push('iterator');
    else tags.push('browsing-context');
  } else if (path.includes('constellation_webview.rs')) {
    tags.push('webview');
  } else if (path.includes('embedder.rs')) {
    tags.push('message-types', 'ipc');
  } else if (path.includes('event_loop.rs')) {
    tags.push('event-loop');
  } else if (path.includes('logging.rs')) {
    tags.push('logging');
  } else if (path.includes('pipeline.rs')) {
    tags.push('pipeline');
  } else if (path.includes('process_manager.rs')) {
    tags.push('process-management');
  } else if (path.includes('sandboxing.rs')) {
    tags.push('sandbox', 'security');
  } else if (path.includes('serviceworker.rs')) {
    tags.push('service-worker');
  } else if (path.includes('session_history.rs')) {
    tags.push('session-history', 'navigation');
  } else if (path.includes('tracing.rs')) {
    tags.push('tracing');
  } else {
    tags.push('utility');
  }

  // Deduplicate and ensure we have at least 3 tags
  const uniqueTags = [...new Set(tags)];
  const fallbacks = ['internal', 'type', 'utility'];
  while (uniqueTags.length < 3) {
    uniqueTags.push(fallbacks[uniqueTags.length - 1]);
  }
  return uniqueTags.slice(0, 5);
}

// ===== Write output =====
const totalNodes = allNodes.length;
const totalEdges = allEdges.length;

console.log(`Total nodes: ${totalNodes}, Total edges: ${totalEdges}`);

// Partition
const parts = Math.ceil(Math.max(totalNodes / 60, totalEdges / 120));
console.log(`Parts: ${parts}`);

// Sort files alphabetically and chunk
const sortedFiles = [...results.results].map(f => f.path).sort();
const groupSize = Math.ceil(sortedFiles.length / parts);
const fileGroups = [];
for (let i = 0; i < sortedFiles.length; i += groupSize) {
  fileGroups.push(sortedFiles.slice(i, i + groupSize));
}

console.log('File groups:');
fileGroups.forEach((g, i) => console.log(`  Part ${i+1}: ${g.join(', ')}`));

// Assign nodes to parts
const nodePartMap = {}; // nodeId -> part index
for (let p = 0; p < fileGroups.length; p++) {
  for (const filePath of fileGroups[p]) {
    nodePartMap[`file:${filePath}`] = p;
    // All sub-file nodes in this file
    for (const node of allNodes) {
      if (node.filePath === filePath) {
        nodePartMap[node.id] = p;
      }
    }
  }
}

// Log any nodes not assigned to a part
for (const node of allNodes) {
  if (!(node.id in nodePartMap)) {
    console.error(`NODE NOT MAPPED: ${node.id}`);
  }
}

// Generate each part
const outputDir = 'd:/Projects/servo/.understand-anything/intermediate';
fs.mkdirSync(outputDir, { recursive: true });

const edgeValidationErrors = [];

for (let p = 0; p < fileGroups.length; p++) {
  const partFiles = fileGroups[p];
  const partNodes = [];
  const partNodeIds = new Set();

  // Add nodes for this part's files
  for (const node of allNodes) {
    if (node.filePath && partFiles.includes(node.filePath)) {
      partNodes.push(node);
      partNodeIds.add(node.id);
    }
  }

  // Add edges: source is in this part's nodes
  const partEdges = [];
  for (const edge of allEdges) {
    if (nodePartMap[edge.source] === p) {
      partEdges.push(edge);
    }
  }

  // Validate edges
  for (const edge of partEdges) {
    if (!partNodeIds.has(edge.source) && !nodePartMap.hasOwnProperty(edge.source)) {
      edgeValidationErrors.push(`Part ${p+1}: source ${edge.source} not in nodePartMap`);
    }
  }

  const partFile = `batch-37-part-${p+1}.json`;
  const partPath = `${outputDir}/${partFile}`;
  fs.writeFileSync(partPath, JSON.stringify({ nodes: partNodes, edges: partEdges }, null, 2));
  console.log(`Wrote ${partFile}: ${partNodes.length} nodes, ${partEdges.length} edges`);
}

if (edgeValidationErrors.length > 0) {
  console.error('Edge validation errors:');
  edgeValidationErrors.forEach(e => console.error('  ' + e));
}

console.log(`\nImport edges count: ${allEdges.filter(e => e.type === 'imports').length}`);
const importSum = Object.values(batchImportData).flat().length;
console.log(`Expected import edges (from batchImportData): ${importSum}`);
