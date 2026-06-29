const fs = require('fs');
const data = require('d:/Projects/servo/.understand-anything/tmp/ua-file-extract-results-20.json');

const nodes = [];
const edges = [];
const usedIds = new Set();

function makeId(prefix, path, name) {
    if (name) return prefix + ':' + path + ':' + name;
    return prefix + ':' + path;
}

function addNode(node) {
    if (usedIds.has(node.id)) {
        console.error('Duplicate node id:', node.id);
        return;
    }
    usedIds.add(node.id);
    nodes.push(node);
}

function addEdge(source, target, type, weight) {
    if (source === target) return;
    edges.push({
        source: source,
        target: target,
        type: type,
        direction: 'forward',
        weight: weight
    });
}

// Process each file
for (const r of data.results) {
    const path = r.path;
    const fileId = 'file:' + path;

    // Determine summary based on file content
    let summary = '';
    let tags = [];
    let complexity = 'moderate';
    let nodeType = 'file';
    let languageNotes = '';

    if (path === 'components/net/async_runtime.rs') {
        summary = 'Tokio async runtime initialization and management, providing a global runtime handle for spawning tasks and blocking tasks across the networking layer.';
        tags = ['async-runtime', 'tokio', 'task-spawning', 'concurrency'];
        complexity = 'moderate';
    } else if (path === 'components/net/connector.rs') {
        summary = 'HTTP/HTTPS connector infrastructure with TLS configuration, certificate override management, proxy support, and instrumented connection tracking for the networking stack.';
        tags = ['networking', 'tls', 'proxy', 'http-client', 'certificate-management'];
        complexity = 'complex';
        languageNotes = 'Implements tower::Service for composable HTTP connection handling with TLS via rustls.';
    } else if (path === 'components/net/cookie.rs') {
        summary = 'HTTP cookie creation and matching as specified by RFC 6265, wrapping the cookie-rs crate with Servo-specific behaviors like host-only flags and expiry tracking.';
        tags = ['cookies', 'http', 'rfc6265', 'networking'];
        complexity = 'complex';
    } else if (path === 'components/net/cookie_storage.rs') {
        summary = 'Cookie jar storage with per-host quotas, expiry eviction, and session cookie management for HTTP cookie persistence.';
        tags = ['cookies', 'storage', 'http', 'networking'];
        complexity = 'complex';
    } else if (path === 'components/net/decoder.rs') {
        summary = 'Non-blocking HTTP response body decoder supporting gzip, brotli, deflate, and zstd decompression transparently.';
        tags = ['decoding', 'compression', 'http', 'streaming'];
        complexity = 'moderate';
    } else if (path === 'components/net/devtools.rs') {
        summary = 'DevTools protocol integration for the networking layer, forwarding HTTP request and response events to the browser developer tools inspector.';
        tags = ['devtools', 'debugging', 'http', 'instrumentation'];
        complexity = 'moderate';
    } else if (path === 'components/net/embedder.rs') {
        summary = 'Enum defining messages sent from the networking threads to the embedder for file selection, web resource requests, and authentication prompts.';
        tags = ['ipc', 'embedder', 'messaging', 'networking'];
        complexity = 'simple';
    } else if (path === 'components/net/filemanager_thread.rs') {
        summary = 'File manager for handling file reads, blob URL management, file token validation, and chunked file serving across the networking layer.';
        tags = ['file-management', 'blob-url', 'file-io', 'networking'];
        complexity = 'complex';
    } else if (path === 'components/net/hosts.rs') {
        summary = 'Hosts file parser that reads system /etc/hosts-style files and provides hostname-to-IP-address replacement for HTTP connections.';
        tags = ['hosts', 'dns', 'networking', 'name-resolution'];
        complexity = 'simple';
    } else if (path === 'components/net/hsts.rs') {
        summary = 'HTTP Strict Transport Security (HSTS) implementation with preload list support, domain matching, and response-based HSTS policy updates.';
        tags = ['hsts', 'security', 'https', 'networking'];
        complexity = 'moderate';
    } else if (path === 'components/net/http_cache.rs') {
        summary = 'HTTP cache implementation supporting cache key management, response caching, freshness computation, range requests, and invalidation per the HTTP caching specification.';
        tags = ['http-cache', 'caching', 'networking', 'performance'];
        complexity = 'complex';
    } else if (path === 'components/net/http_loader.rs') {
        summary = 'Core HTTP fetch implementation following the Fetch specification, handling request/response lifecycle, redirects, CORS, authentication, and cache integration.';
        tags = ['http', 'fetch', 'cors', 'networking', 'spec-implementation'];
        complexity = 'complex';
        languageNotes = 'Async-recursive implementation of the Fetch specification with extensive request/response processing.';
    } else if (path === 'components/net/image_cache.rs') {
        summary = 'Image cache infrastructure managing decoded image storage, SVG rasterization, WebRender integration, and background image loading and eviction.';
        tags = ['image-cache', 'rasterization', 'svg', 'webrender'];
        complexity = 'complex';
    } else if (path === 'components/net/lib.rs') {
        summary = 'Library root module for the net crate, re-exporting all networking submodules including HTTP loading, caching, cookie management, and protocol handlers.';
        tags = ['barrel', 'module', 'networking', 'entry-point'];
        complexity = 'simple';
    } else if (path === 'components/net/local_directory_listing.rs') {
        summary = 'Directory listing generator that produces HTML directory indexes for file:// protocol requests to directories.';
        tags = ['directory-listing', 'file-protocol', 'html-generation'];
        complexity = 'moderate';
    } else if (path === 'components/net/protocols/blob.rs') {
        summary = 'Blob protocol handler implementing blob: URL resource loading by resolving blob URL claims and streaming blob data.';
        tags = ['blob', 'protocol', 'networking', 'url'];
        complexity = 'moderate';
    } else if (path === 'components/net/protocols/data.rs') {
        summary = 'Data URI protocol handler implementing data: URL parsing, decoding, and resource loading per RFC 2397.';
        tags = ['data-uri', 'protocol', 'networking', 'url'];
        complexity = 'simple';
    } else if (path === 'components/net/protocols/file.rs') {
        summary = 'File protocol handler loading local filesystem resources with path validation, MIME type detection, and range request support.';
        tags = ['file-protocol', 'protocol', 'networking', 'url'];
        complexity = 'moderate';
    } else if (path === 'components/net/protocols/mod.rs') {
        summary = 'Protocol registry managing custom protocol handlers (blob:, data:, file:) with scheme-based routing, security checks, and range request utilities.';
        tags = ['protocol', 'registry', 'networking', 'url-routing'];
        complexity = 'complex';
    } else if (path === 'components/net/request_interceptor.rs') {
        summary = 'Request interceptor that delegates outgoing HTTP requests to the embedder for interception and potential modification before they reach the network.';
        tags = ['request-intercept', 'embedder', 'networking', 'interceptor'];
        complexity = 'moderate';
    } else if (path === 'components/net/resource_thread.rs') {
        summary = 'Core resource thread managing HTTP state, certificate loading, worker thread lifecycle, and message-based request processing for the networking layer.';
        tags = ['resource-thread', 'networking', 'ipc', 'worker'];
        complexity = 'complex';
    } else if (path === 'components/net/subresource_integrity.rs') {
        summary = 'Subresource Integrity (SRI) validation implementation parsing integrity metadata, computing hash digests, and verifying response integrity.';
        tags = ['security', 'sri', 'integrity', 'validation'];
        complexity = 'moderate';
    } else if (path === 'components/net/test_util.rs') {
        summary = 'Test utilities providing embedder proxy creation, HTTP and HTTPS test server setup with certificate loading for integration testing.';
        tags = ['test', 'testing-utility', 'server', 'ssl'];
        complexity = 'moderate';
    } else if (path === 'components/net/websocket_loader.rs') {
        summary = 'WebSocket loader implementing the WebSocket handshake, message framing, and connection lifecycle management per RFC 6455.';
        tags = ['websocket', 'networking', 'protocol', 'real-time-communication'];
        complexity = 'complex';
    }

    // Create file node
    const fileNode = {
        id: fileId,
        type: nodeType,
        name: path.split('/').pop(),
        filePath: path,
        summary: summary,
        tags: tags,
        complexity: complexity
    };
    if (languageNotes) fileNode.languageNotes = languageNotes;
    addNode(fileNode);

    // Process functions
    const processedFuncs = new Set();
    for (const f of (r.functions || [])) {
        const lineCount = f.endLine - f.startLine + 1;
        const isExported = (r.exports || []).some(e => e.name === f.name);

        // Significance filter: 10+ lines OR exported
        if (lineCount < 10 && !isExported) continue;

        const dedupKey = f.name;
        if (processedFuncs.has(dedupKey)) continue;
        processedFuncs.add(dedupKey);

        const funcId = 'function:' + path + ':' + f.name;
        let funcSummary = '';
        let funcComplexity = lineCount < 30 ? 'simple' : (lineCount < 100 ? 'moderate' : 'complex');

        const sumMap = {
            'init_async_runtime': 'Initializes the global Tokio multi-threaded async runtime with configurable worker thread count.',
            'spawn_task': 'Spawns a future on the global Tokio runtime for concurrent execution.',
            'spawn_blocking_task': 'Runs a blocking future synchronously on the global Tokio runtime via block_on.',
            'parse_hostsfile': 'Parses a hosts-file formatted string into a hostname-to-IP-address mapping.',
            'replace_host': 'Replaces a hostname with its IP address from the static host table, if available.',
            'detect': 'Detects the content encoding of an HTTP response and creates the appropriate decoder.',
            'http_fetch': 'Main HTTP fetch entry point implementing the fetch algorithm with cache and CORS handling.',
            'http_redirect_fetch': 'Handles HTTP redirect responses by following the redirect chain according to fetch spec.',
            'determine_requests_referrer': 'Determines the appropriate Referer header value based on referrer policy and security context.',
            'serialize_origin': 'Serializes a request origin as a string for use in Origin headers.',
            'new_resource_threads': 'Creates the resource thread infrastructure including public and private HTTP states.',
            'new_core_resource_thread': 'Creates a core resource manager thread with file manager and request interceptor.',
            'create_http_client': 'Creates a configured HTTP client with TLS settings and proxy support.',
            'start_websocket': 'Initiates a WebSocket connection by creating a handshake request and managing the connection lifecycle.',
            'create_handshake_request': 'Constructs the initial WebSocket upgrade HTTP request with required headers.',
            'from_cookie_string': 'Parses a Set-Cookie header string into a ServoCookie with request context.',
            'parse_date': 'Parses cookie date strings using nom-based combinators for RFC-compliant date parsing.',
            'apply_hsts_rules': 'Applies HSTS rules to upgrade HTTP URLs to HTTPS if the domain is in the HSTS list.',
            'update_hsts_list_from_response': 'Updates the HSTS list from Strict-Transport-Security response headers.',
            'store': 'Stores an HTTP response in the cache for future requests.',
            'cache_entry_descriptors': 'Returns summary descriptors of all entries currently in the HTTP cache.',
            'with_internal_protocols': 'Registers the default internal protocol handlers (blob, data, file) in the protocol registry.',
            'intercept_request': 'Delegates a request to the embedder for interception, allowing custom handling.',
            'is_response_integrity_valid': 'Validates a response body against SRI integrity metadata by computing hash digests.',
            'parsed_metadata': 'Parses SRI integrity metadata strings into structured SriEntry objects.',
            'make_server': 'Creates a test HTTP server bound to a random port with a custom request handler.',
            'make_ssl_server': 'Creates a test HTTPS server with TLS certificates for integration testing.',
            'build_html_directory_listing': 'Generates an HTML page for directory listings with file entries and icons.',
            'metadata_to_file_size_string': 'Converts file metadata size into a human-readable string representation.',
            'is_url_potentially_trustworthy': 'Checks if a URL is potentially trustworthy based on scheme and protocol registry.',
            'get_range_request_bounds': 'Parses range request headers and computes byte range bounds for partial content responses.',
            'promote_memory': 'Converts a file reading task from memory-backed to fully loaded state.',
            'process_msg': 'Dispatches incoming resource thread messages to appropriate handlers for load, fetch, and manage operations.',
            'prewarm_tls': 'Pre-establishes TLS connections to commonly-used hosts for faster subsequent requests.',
            'create_tls_config': 'Creates a TLS client configuration with the given CA certificates and certificate override settings.',
            'verify_server_cert': 'Custom TLS server certificate verification logic supporting certificate error overrides.',
            'run_ws_loop': 'Main WebSocket message processing loop handling incoming frames and outgoing messages.',
            'obtain_response': 'Performs the actual HTTP request and obtains the response from the server.',
            'http_network_fetch': 'Performs the network-level HTTP fetch with redirect and authentication handling.',
            'cors_preflight_fetch': 'Performs a CORS preflight OPTIONS request to check cross-origin access permissions.',
            'remove_certificate_failing_verification': 'Removes a certificate that was previously flagged as failing verification for a given host.',
            'construct_response': 'Constructs a fetch Response from cached HTTP response data with validation logic.',
            'refresh': 'Refreshes a cached HTTP response with updated data from a server response.',
            'invalidate_cached_resources': 'Marks cached resources as invalid for subsequent requests.',
            'update_awaiting_consumers': 'Notifies pending consumers when a cached response has been updated.'
        };

        // For functions with generic names, generate descriptive summaries
        if (sumMap[f.name]) {
            funcSummary = sumMap[f.name];
        } else if (f.name === 'load') {
            if (path.includes('protocols')) {
                funcSummary = 'Loads a resource for this protocol handler and produces a Response.';
            } else {
                funcSummary = 'Loads a resource from the network and produces a Response.';
            }
        } else if (f.name === 'remove') {
            funcSummary = 'Removes a specific item from the collection by identifier.';
        } else if (f.name === 'new') {
            funcSummary = 'Constructs a new instance of the containing struct with provided parameters.';
        } else if (f.name.includes('devtools') || f.name.includes('Devtools')) {
            funcSummary = 'Sends HTTP event data to DevTools for inspection and debugging.';
        } else {
            funcSummary = f.name.replace(/_/g, ' ') + ' function in ' + path.split('/').pop();
        }

        const funcNode = {
            id: funcId,
            type: 'function',
            name: f.name,
            filePath: path,
            lineRange: [f.startLine, f.endLine],
            summary: funcSummary.charAt(0).toUpperCase() + funcSummary.slice(1),
            tags: ['function'],
            complexity: funcComplexity
        };
        addNode(funcNode);

        // Add contains and exports edges
        addEdge(fileId, funcId, 'contains', 1.0);
        if (isExported) {
            addEdge(fileId, funcId, 'exports', 0.8);
        }
    }

    // Process classes
    for (const c of (r.classes || [])) {
        const isExported = (r.exports || []).some(e => e.name === c.name);
        const lineCount = c.endLine - c.startLine + 1;
        const methodCount = (c.methods || []).length;

        // Significance filter: 2+ methods OR 20+ lines OR exported
        if (methodCount < 2 && lineCount < 20 && !isExported) continue;

        const classId = 'class:' + path + ':' + c.name;
        let classSummary = '';

        const sumMap = {
            'ServoCookie': 'Wraps a cookie-rs Cookie with Servo-specific metadata including host-only flag, persistence, and timing.',
            'CookieStorage': 'Cookie jar implementing RFC 6265 storage with per-host limits, expiry management, and session cookie tracking.',
            'Decoder': 'Non-blocking HTTP response decompressor supporting gzip, brotli, deflate, and zstd content encodings.',
            'HstsEntry': 'Single HSTS entry with host, subdomain flag, and expiry timestamp.',
            'HstsList': 'Ordered collection of HSTS entries with domain matching and policy application.',
            'HstsPreloadList': 'Static HSTS preload list loaded from Servo built-in data for well-known HTTPS-only sites.',
            'CacheKey': 'Cache key identifying a cached HTTP response by URL.',
            'CachedResponse': 'Wrapper for a cached HTTP response with revalidation tracking.',
            'CachedResource': 'Represents a cached HTTP resource with request/response metadata and body.',
            'HttpCache': 'Main HTTP cache container managing cached entries with freshness-based validation and eviction.',
            'HttpState': 'Aggregate HTTP state including HSTS list, cookie jar, cache, auth cache, and HTTP client.',
            'ServoHttpConnector': 'Basic TCP connector wrapping hyper HTTP connector with host replacement.',
            'TlsHandshakeInfo': 'Captures TLS handshake metadata including protocol version, cipher suite, and ALPN negotiation.',
            'CertificateErrorOverrideManager': 'Manages user-approved certificate error overrides for hosts with certificate validation failures.',
            'ConnectionError': 'Connection error type distinguishing HTTP transport errors from proxy errors.',
            'ProxyConnector': 'HTTP proxy connector that wraps an inner connector and routes requests through configured proxies.',
            'CACertificates': 'TLS CA certificate configuration supporting both default system roots and custom override certificates.',
            'InstrumentedConnector': 'Connector wrapper that collects TLS handshake info for each established connection.',
            'InstrumentedStream': 'Wraps an HTTPS stream to expose negotiated TLS handshake information.',
            'AsyncRuntimeHolder': 'Holds the Tokio runtime instance and provides graceful shutdown capability.',
            'NetToEmbedderMsg': 'Messages sent from networking threads to the embedder for file dialogs, auth requests, and cookie operations.',
            'FileManager': 'Manages file/blob data access with token-based security, file reading, and blob URL lifecycle.',
            'FileManagerStore': 'Internal storage for file manager entries with ref counting, sliced URL resolution, and token management.',
            'ProtocolHandler': 'Trait defining the interface for custom URL scheme protocol handlers with load and security methods.',
            'ProtocolRegistry': 'Registry mapping URL schemes to their ProtocolHandler implementations with security constraints.',
            'ProtocolRegisterError': 'Errors that can occur when registering a protocol handler: forbidden scheme or duplicate registration.',
            'WebPageContentProtocolHandler': 'Protocol handler for registered web page content schemes, delegating loads to the fetch infrastructure.',
            'BlobProtocolHander': 'Handles blob: URL loading by resolving blob URL claims and streaming blob data chunks.',
            'DataProtocolHander': 'Handles data: URL loading by decoding Base64 or percent-encoded data URIs.',
            'FileProtocolHander': 'Handles file: URL loading by reading local filesystem resources with MIME detection.',
            'RequestInterceptor': 'Intercepts and delegates HTTP requests to the embedder for optional modification or blocking.',
            'CoreResourceManager': 'Manages core resource operations including fetching, websocket connections, and file/blob operations.',
            'AuthCache': 'Thread-safe cache for HTTP authentication credentials keyed by origin.',
            'AuthCacheEntry': 'Stores a single authentication credential entry with username and password.',
            'ResourceChannelManager': 'Manages resource thread message loop, cancellation listeners, and cookie response routing.',
            'SriEntry': 'Represents a parsed SRI integrity metadata entry with hash algorithm and value.',
            'Server': 'Test HTTP server wrapper with close channel for controlled shutdown in integration tests.',
            'ImageCacheFactoryImpl': 'Factory creating ImageCacheImpl instances with broken image icon, thread pool, and font database.',
            'ImageCacheImpl': 'Complete image cache implementation managing pending loads, completed images, rasterization tasks, and WebRender integration.',
            'ImageCacheStore': 'Internal storage for the image cache holding pending and completed image loads with key management.',
            'KeyCache': 'Manages mapping between image keys and their WebRender resource cache entries.',
            'PendingLoad': 'Tracks an in-progress image load with accumulated bytes, listeners, and metadata.',
            'CompletedLoad': 'Stores a completed image load result with response data and identifier.',
            'LoadKeyGenerator': 'Atomic counter generating unique load key identifiers for tracking image loads.',
            'HttpCacheEntryState': 'Enum representing the state of an HTTP cache entry: ready or pending concurrent stores.',
            'CachedResourcesOrGuard': 'Enum representing either a set of cached resources or a guard for concurrent access.',
            'DomMsg': 'WebSocket DOM communication message types for send and close operations.',
            'BodyStream': 'HTTP response body stream variant as either chunked or buffered transfer.',
            'BodySink': 'HTTP response body sink accepting chunked or buffered writes.',
            'RemoveCookieError': 'Error enum for cookie removal indicating overlapping or non-HTTP cookie scenarios.',
            'AllPendingLoads': 'Tracks all in-progress image loads with URL-to-load-key mapping.',
            'CacheResult': 'Enum representing an image cache hit or miss result.',
            'VectorImageData': 'Stores a parsed SVG tree with associated CORS status for rasterization.',
            'DecodedImage': 'Enum representing a decoded image as either raster or vector data.',
            'DecoderMsg': 'Message from the image decoder containing the decoded image key and data.',
            'ImageBytes': 'Stores image byte data in either in-progress or complete state.',
            'LoadResult': 'Enum representing the result of an image load: raster, vector, or failure.',
            'RasterizationTask': 'Tracks a pending SVG rasterization task with listeners and result.',
            'PendingKey': 'Enum for pending image key types: raster or SVG.',
            'KeyCacheState': 'Enum representing key cache state: pending batch of keys or ready.',
            'SvgRasterizationTaskStore': 'Tracks SVG rasterization tasks to avoid duplicate rasterization of the same SVG.',
            'BlobBounds': 'Enum representing blob byte range bounds as unresolved or resolved.'
        };

        if (sumMap[c.name]) {
            classSummary = sumMap[c.name];
        } else {
            classSummary = c.name.replace(/_/g, ' ') + ' structure in ' + path.split('/').pop();
        }

        const classNode = {
            id: classId,
            type: 'class',
            name: c.name,
            filePath: path,
            lineRange: [c.startLine, c.endLine],
            summary: classSummary,
            tags: ['class'],
            complexity: methodCount < 3 ? 'simple' : (methodCount < 8 ? 'moderate' : 'complex')
        };
        addNode(classNode);

        // Add contains and exports edges
        addEdge(fileId, classId, 'contains', 1.0);
        if (isExported) {
            addEdge(fileId, classId, 'exports', 0.8);
        }
    }
}

// Add import edges from batchImportData
const importData = {
    "components/net/lib.rs": ["components/net/async_runtime.rs","components/net/connector.rs","components/net/cookie_storage.rs","components/net/cookie.rs","components/net/decoder.rs","components/net/devtools.rs","components/net/embedder.rs","components/net/filemanager_thread.rs","components/net/hosts.rs","components/net/hsts.rs","components/net/http_cache.rs","components/net/http_loader.rs","components/net/image_cache.rs","components/net/local_directory_listing.rs","components/net/protocols/mod.rs","components/net/request_interceptor.rs","components/net/resource_thread.rs","components/net/subresource_integrity.rs","components/net/test_util.rs","components/net/websocket_loader.rs"],
    "components/net/protocols/mod.rs": ["components/net/protocols/blob.rs","components/net/protocols/data.rs","components/net/protocols/file.rs"]
};

for (const [sourcePath, targets] of Object.entries(importData)) {
    const sourceFileId = 'file:' + sourcePath;
    for (const targetPath of targets) {
        const targetFileId = 'file:' + targetPath;
        addEdge(sourceFileId, targetFileId, 'imports', 0.7);
    }
}

// Write output
const output = { nodes, edges };

const nodeCount = nodes.length;
const edgeCount = edges.length;
console.log('Total nodes:', nodeCount);
console.log('Total edges:', edgeCount);

if (nodeCount <= 60 && edgeCount <= 120) {
    fs.writeFileSync('d:/Projects/servo/.understand-anything/intermediate/batch-20.json', JSON.stringify(output, null, 2));
    console.log('Written to batch-20.json (single part)');
} else {
    const parts = Math.max(1, Math.ceil(Math.max(nodeCount / 60, edgeCount / 120)));
    console.log('Splitting into', parts, 'parts');

    // Sort nodes by filePath alphabetical
    nodes.sort((a, b) => {
        const fa = a.filePath || a.id;
        const fb = b.filePath || b.id;
        return fa.localeCompare(fb);
    });

    const nodesPerPart = Math.ceil(nodes.length / parts);

    for (let k = 0; k < parts; k++) {
        const start = k * nodesPerPart;
        const end = Math.min(start + nodesPerPart, nodes.length);
        const partNodes = nodes.slice(start, end);
        const partNodeIds = new Set(partNodes.map(n => n.id));

        const partEdges = edges.filter(e => partNodeIds.has(e.source));

        const partOutput = { nodes: partNodes, edges: partEdges };
        const filename = 'd:/Projects/servo/.understand-anything/intermediate/batch-20-part-' + (k+1) + '.json';
        fs.writeFileSync(filename, JSON.stringify(partOutput, null, 2));
        console.log('Written to', filename, '(' + partNodes.length + ' nodes, ' + partEdges.length + ' edges)');
    }
}

console.log('Done.');
