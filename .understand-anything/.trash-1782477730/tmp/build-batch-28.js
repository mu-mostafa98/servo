const fs = require('fs');
const results = JSON.parse(fs.readFileSync('d:/Projects/servo/.understand-anything/tmp/ua-file-extract-results-28.json', 'utf8'));
const inputData = JSON.parse(fs.readFileSync('d:/Projects/servo/.understand-anything/tmp/ua-file-analyzer-input-28.json', 'utf8'));

function isFunctionSignificant(fn, exports) {
    const lineCount = fn.endLine - fn.startLine + 1;
    const isExported = exports.some(e => e.name === fn.name && e.line === fn.startLine);
    return lineCount >= 10 || isExported;
}

function isClassSignificant(cls, exports) {
    const lineCount = cls.endLine - cls.startLine + 1;
    const isExported = exports.some(e => e.name === cls.name && e.line === cls.startLine);
    const hasEnoughMethods = cls.methods && cls.methods.length >= 2;
    const hasEnoughLines = lineCount >= 20;
    return (hasEnoughMethods || hasEnoughLines);
}

function getFileSummary(path, file) {
    const name = path.split('/').pop();
    const map = {
        'assert.rs': 'Provides thread assertion functions (assert_in_layout, assert_in_script) used to verify code executes on the correct thread in the Servo browser engine.',
        'callback.rs': 'Implements Web IDL callback types including CallbackObject, CallbackContainer, CallbackFunction, and CallbackInterface with JS handle management, call setup, and exception handling.',
        'cell.rs': 'Defines DomRefCell, a thread-safe cell type that integrates with Servo DOM threading, providing borrow methods with layout/script thread assertion guards.',
        'constant.rs': 'Provides ConstantSpec and ConstantVal types for defining Web IDL interface constants with JS value conversion and registration on JS objects.',
        'constructor.rs': 'Implements Web IDL constructor logic including default constructor invocation, namespace/callback/interface object creation, and prototype chain setup.',
        'conversions.rs': 'Implements JS-to-DOM and DOM-to-JS type conversion traits (FromJSValConvertible, ToJSValConvertible) for Rust types including strings, nullable types, and sequences.',
        'dom.rs': 'Provides DOM pointer wrapper types (MutDom, UnrootedDom, MutNullableDom) for safe Rust-DOM memory management with thread assertion enforcement.',
        'domstring.rs': 'Implements the DOMString type with lazy encoding representation (Latin1, UTF-8, JS string handle) and comprehensive string manipulation methods.',
        'error.rs': 'Defines the Error enum covering all standard Web IDL DOM exception types and provides helpers for throwing JS type errors.',
        'finalize.rs': 'Implements SpiderMonkey GC finalization hooks for DOM objects, including global object cleanup and weak referenceable object finalization.',
        'guard.rs': 'Provides Guard and Condition types for conditional Web IDL feature exposure based on preferences, secure context, and runtime predicates.',
        'import.rs': 'Contains pub mod declarations importing external dependency modules used across the script bindings crate.',
        'inheritance.rs': 'Defines Castable and HasParent traits for safe runtime-checked upcasting and downcasting through Servo DOM inheritance hierarchies.',
        'interface.rs': 'Implements Web IDL interface object construction including global objects, callback interfaces, prototype objects, named constructors, and compartment selection.',
        'interfaces.rs': 'Defines helper traits (DomHelpers, GlobalScopeHelpers, DocumentHelpers, WindowHelpers) providing DOM binding methods for Servo script types.',
        'iterable.rs': 'Implements Web IDL iterable interface support with IterableIterator for key/value/entries iteration over DOM collections.',
        'lib.rs': 'Crate root for script_bindings that declares and re-exports all internal modules as the public API surface for Servo JS-DOM bindings.',
        'like.rs': 'Defines Setlike and Maplike traits implementing Web IDL set-like and map-like interface behavior on DOM data structures.',
    };
    return map[name] || 'Part of Servo script bindings for JS-DOM interoperability.';
}

function getFileTags(path, name, file) {
    const tags = ['rust', 'dom-bindings', 'script'];
    const map = {
        'lib.rs': ['entry-point', 'crate-root', 'module-re-export', 'rust', 'dom-bindings'],
        'conversions.rs': ['type-conversion', 'serialization', 'rust', 'dom-bindings', 'js-interop'],
        'domstring.rs': ['string-handling', 'data-model', 'rust', 'dom-bindings', 'encoding'],
        'interface.rs': ['api-handler', 'interface-definition', 'rust', 'dom-bindings', 'object-creation'],
        'callback.rs': ['callback-handling', 'api-handler', 'rust', 'dom-bindings', 'event-handler'],
        'constructor.rs': ['constructor', 'api-handler', 'rust', 'dom-bindings', 'object-creation'],
        'guard.rs': ['feature-gating', 'security-context', 'rust', 'dom-bindings', 'configuration'],
        'error.rs': ['error-handling', 'validation', 'rust', 'dom-bindings', 'enum'],
        'iterable.rs': ['collection', 'iterator', 'rust', 'dom-bindings', 'data-model'],
        'like.rs': ['data-model', 'collection', 'rust', 'dom-bindings', 'trait'],
        'dom.rs': ['memory-management', 'pointer-wrapper', 'rust', 'dom-bindings', 'thread-safety'],
        'cell.rs': ['memory-management', 'thread-safety', 'rust', 'dom-bindings', 'data-model'],
        'finalize.rs': ['memory-management', 'gc-hook', 'rust', 'dom-bindings', 'lifecycle'],
        'inheritance.rs': ['type-definition', 'inheritance', 'rust', 'dom-bindings', 'trait'],
        'assert.rs': ['thread-safety', 'utility', 'rust', 'dom-bindings', 'assertion'],
        'import.rs': ['module-declaration', 'rust', 'dom-bindings', 'dependency'],
        'constant.rs': ['constant-definition', 'rust', 'dom-bindings', 'configuration'],
        'interfaces.rs': ['type-definition', 'trait', 'rust', 'dom-bindings', 'interface'],
    };
    return map[name] || tags;
}

function getFunctionSummary(path, fnName) {
    const key = path + ':' + fnName;
    const map = {
        'components/script_bindings/callback.rs:init': 'Initializes a CallbackObject with a JS callback value and permanent root for GC protection.',
        'components/script_bindings/callback.rs:get_callable_property': 'Retrieves a callable property from a JS object by name, validating it is a function.',
        'components/script_bindings/callback.rs:wrap_call_this_value': 'Wraps a JS this value for callback invocation, handling null/undefined and cross-compartment wrapping.',
        'components/script_bindings/callback.rs:call_setup': 'Sets up execution context for callback invocation including realm management, exception reporting, and incumbent global handling.',
        'components/script_bindings/constant.rs:define_constants': 'Defines Web IDL constants on a JS object by converting ConstantSpec entries to JS values.',
        'components/script_bindings/constructor.rs:call_default_constructor': 'Invokes the default constructor for a Web IDL interface with new-target validation and proto chain setup.',
        'components/script_bindings/constructor.rs:create_namespace_interface_objects': 'Creates and registers JS namespace interface objects on the global scope.',
        'components/script_bindings/constructor.rs:create_callback_interface_objects': 'Creates and registers JS callback interface objects with post-barrier management.',
        'components/script_bindings/constructor.rs:create_interface': 'Creates a complete Web IDL interface object including prototype chain, constructor function, and post-barrier registration.',
        'components/script_bindings/conversions.rs:safe_from_jsval': 'Converts a JS value to a DOMString with null handling and string encoding detection.',
        'components/script_bindings/conversions.rs:get_dom_class': 'Retrieves the DOM class descriptor from a JS object, supporting both native and proxy DOM objects.',
        'components/script_bindings/conversions.rs:private_from_object': 'Extracts a DOM object private pointer from a JS object reserved slot.',
        'components/script_bindings/conversions.rs:private_from_proto_check': 'Extracts a DOM private pointer with prototype chain validation for type-safe downcasting.',
        'components/script_bindings/conversions.rs:root_from_handlevalue': 'Converts a JS handle value to a rooted DOM reference by extracting native pointer with type checking.',
        'components/script_bindings/conversions.rs:jsid_to_string': 'Converts a JS property ID (jsid) to a Rust string handling string, int, and void ID variants.',
        'components/script_bindings/conversions.rs:is_array_like': 'Checks if a JS value is array-like by testing against DOM collection types (DOMTokenList, HTMLCollection, NodeList, etc.).',
        'components/script_bindings/conversions.rs:modifiers': 'Detects keyboard event modifier states (Alt, Ctrl, Meta, Shift, etc.) from a JS event value.',
        'components/script_bindings/dom.rs:upcast': 'Safely transmutes a Dom reference to a supertype in the DOM inheritance hierarchy.',
        'components/script_bindings/dom.rs:downcast': 'Safely downcasts a Dom reference with runtime type checking via the is::<U>() method.',
        'components/script_bindings/dom.rs:or_init': 'Returns the current nullable DOM value or initializes it with a provided callback if empty.',
        'components/script_bindings/dom.rs:if_is_some': 'Executes a closure with the inner DOM reference if the nullable pointer is populated.',
        'components/script_bindings/domstring.rs:from_js_string': 'Creates a DOMString from a JS string value, preserving Latin1 encoding for efficiency.',
        'components/script_bindings/domstring.rs:str': 'Returns a StringView reference to DOMString content, converting JS handles to Rust strings if needed.',
        'components/script_bindings/domstring.rs:strip_leading_and_trailing_ascii_whitespace': 'Removes leading and trailing ASCII whitespace from the DOMString in place.',
        'components/script_bindings/domstring.rs:to_ascii_lowercase': 'Converts the DOMString to ASCII lowercase with optimized paths for Latin1 bytes.',
        'components/script_bindings/domstring.rs:as_bytes': 'Returns a BytesView of the DOMString as Latin1 or UTF-8 bytes depending on internal encoding.',
        'components/script_bindings/domstring.rs:is_ascii_lowercase': 'Checks if all characters are lowercase ASCII with optimized Latin1 and UTF-8 detection.',
        'components/script_bindings/domstring.rs:with_str_reference': 'Invokes a callback with a &str reference, avoiding allocation for ASCII-compatible strings.',
        'components/script_bindings/domstring.rs:normalize_crlf': 'Normalizes CRLF line endings to LF in the DOM string.',
        'components/script_bindings/domstring.rs:to_jsval': 'Converts the DOMString to a JS Value with Latin1-to-UTF-8 optimization.',
        'components/script_bindings/domstring.rs:parse_floating_point_number': 'Parses a string as f64, trimming whitespace and rejecting infinity/NaN/invalid formats.',
        'components/script_bindings/finalize.rs:finalize_weak_referenceable': 'Finalizes a weak-referenceable DOM object by cleaning weak reference state before common finalization.',
        'components/script_bindings/guard.rs:expose': 'Checks all guard conditions (prefs, secure context, exposure) and returns the guarded value if all satisfied.',
        'components/script_bindings/guard.rs:is_satisfied': 'Evaluates a single Condition by checking pref values, secure context, or calling a predicate function.',
        'components/script_bindings/interface.rs:new': 'Creates a NonCallbackInterfaceObjectClass with constructor behavior, string representation, and prototype metadata.',
        'components/script_bindings/interface.rs:throw': 'Returns a constructor behavior that throws a type error when called as a constructor.',
        'components/script_bindings/interface.rs:call': 'Returns a constructor behavior that delegates to a provided constructor hook function.',
        'components/script_bindings/interface.rs:create_global_object': 'Creates a SpiderMonkey JS global object with realm, principal, reserved slots, and auto-realm setup.',
        'components/script_bindings/interface.rs:select_compartment': 'Selects a JS compartment for execution, preferring sharable or system compartments.',
        'components/script_bindings/interface.rs:create_callback_interface_object': 'Creates a JS callback interface object with constants, name property, and global registration.',
        'components/script_bindings/interface.rs:create_interface_prototype_object': 'Creates a Web IDL interface prototype with methods, properties, constants, and unscopable names.',
        'components/script_bindings/interface.rs:create_noncallback_interface_object': 'Creates a non-callback interface constructor with name, length, static members, and global registration.',
        'components/script_bindings/interface.rs:create_named_constructors': 'Creates named constructor functions for legacy Web IDL constructors on the global object.',
        'components/script_bindings/interface.rs:create_object': 'Creates a JS object with a given prototype, defining guarded methods, properties, and constants.',
        'components/script_bindings/interface.rs:define_guarded_constants': 'Defines constants on a JS object respecting Guard-based conditional exposure.',
        'components/script_bindings/interface.rs:define_guarded_methods': 'Defines methods on a JS object respecting Guard-based conditional exposure.',
        'components/script_bindings/interface.rs:define_guarded_properties': 'Defines properties on a JS object respecting Guard-based conditional exposure.',
        'components/script_bindings/interface.rs:define_on_global_object': 'Registers a named property on a global object scope with cross-compartment handling.',
        'components/script_bindings/interface.rs:fun_to_string_hook': 'Provides Function.prototype.toString implementation returning the DOM interface representation string.',
        'components/script_bindings/interface.rs:create_unscopable_object': 'Creates an @@unscopables object excluding specified property names from with-environment scope.',
        'components/script_bindings/interface.rs:define_name': 'Sets the name property on a JS function object to a given string.',
        'components/script_bindings/interface.rs:define_length': 'Sets the length property on a JS function object to the parameter count.',
        'components/script_bindings/interface.rs:get_per_interface_object_handle': 'Retrieves or lazily creates a per-interface prototype/constructor for a global and interface ID.',
        'components/script_bindings/interface.rs:define_dom_interface': 'Defines a DOM interface on a global by creating the prototype/constructor chain.',
        'components/script_bindings/interface.rs:get_proto_id_for_new_target': 'Extracts the prototype ID from a new.target object for Web IDL constructor proto resolution.',
        'components/script_bindings/interface.rs:get_desired_proto': 'Resolves the desired prototype for a constructor call, handling legacy factory chain and cross-realm references.',
        'components/script_bindings/iterable.rs:new': 'Creates a new IterableIterator for DOM iterable with specified iterator type.',
        'components/script_bindings/iterable.rs:Next': 'Implements iterator protocol Next() returning done/value for keys, values, or entries iteration.',
        'components/script_bindings/iterable.rs:dict_return': 'Creates a JS iterator result object with done and value properties.',
        'components/script_bindings/iterable.rs:key_and_value_return': 'Creates a JS iterator result array with key and value for entries iteration.',
    };
    return map[key] || 'Function in Servo script bindings.';
}

function getClassSummary(path, clsName) {
    const key = path + ':' + clsName;
    const map = {
        'components/script_bindings/callback.rs:CallbackObject': 'Struct holding a JS callback handle, permanent GC root, and incumbent global scope for callback lifecycle management.',
        'components/script_bindings/callback.rs:CallbackContainer': 'Struct wrapping a CallbackObject with named accessor methods for the callback handle and incumbent global.',
        'components/script_bindings/error.rs:Error': 'Enum covering all standard Web IDL DOM exception error types including IndexSize, NotFound, Syntax, Type, and Range.',
        'components/script_bindings/inheritance.rs:Castable': 'Trait providing runtime-checked upcast and downcast between DOM types using prototype chain IDs.',
        'components/script_bindings/interfaces.rs:DomHelpers': 'Trait defining DOM binding operations including exception throwing, HTML constructor invocation, and reflection.',
        'components/script_bindings/interfaces.rs:GlobalScopeHelpers': 'Trait defining global scope operations for realm management, secure context checks, and microtask processing.',
        'components/script_bindings/iterable.rs:Iterable': 'Trait requiring get_iterable_length, get_value_at_index, and get_key_at_index for DOM iterable collections.',
        'components/script_bindings/iterable.rs:IterableIterator': 'Struct implementing JS iterator protocol for DOM collections with index tracking and iterator type selection.',
        'components/script_bindings/like.rs:Setlike': 'Trait implementing Web IDL set-like behavior with add, has, delete, clear, size, and indexed access methods.',
        'components/script_bindings/like.rs:Maplike': 'Trait implementing Web IDL map-like behavior with get, set, has, delete, clear, size, and indexed access methods.',
        'components/script_bindings/guard.rs:Condition': 'Enum representing feature gate conditions: function predicate, pref value, exposure check, secure context, or always satisfied.',
    };
    return map[key] || 'Type in Servo script bindings.';
}

const allNodes = [];
const allEdges = [];

for (const file of results.results) {
    const fileId = 'file:' + file.path;

    // Create file node
    let complexity = 'simple';
    if (file.nonEmptyLines >= 200) complexity = 'complex';
    else if (file.nonEmptyLines >= 50) complexity = 'moderate';

    const fileName = file.path.split('/').pop();

    allNodes.push({
        id: fileId,
        type: 'file',
        name: fileName,
        filePath: file.path,
        summary: getFileSummary(file.path, file),
        tags: getFileTags(file.path, fileName, file),
        complexity: complexity
    });

    // Create function nodes
    if (file.functions) {
        for (const fn of file.functions) {
            if (isFunctionSignificant(fn, file.exports)) {
                // Disambiguate overloaded functions by adding line number
                const fnId = 'function:' + file.path + ':' + fn.name + ':L' + fn.startLine;
                const fnLineCount = fn.endLine - fn.startLine + 1;

                let fnComplexity = 'simple';
                if (fnLineCount >= 50) fnComplexity = 'complex';
                else if (fnLineCount >= 15) fnComplexity = 'moderate';

                let tags = ['function', 'dom-bindings', 'rust'];
                if (fn.name.includes('init') || fn.name === 'call_setup') tags.push('initialization');
                if (fn.name.includes('create_')) tags.push('object-creation');
                if (fn.name.includes('define_') || fn.name === 'expose') tags.push('property-definition');
                if (fn.name.includes('from_') || fn.name.includes('to_') || fn.name === 'safe_from_jsval' || fn.name === 'str' || fn.name === 'as_bytes' || fn.name === 'encoded_bytes') tags.push('conversion');
                if (fn.name === 'modifiers' || fn.name === 'is_array_like' || fn.name === 'is_ascii_lowercase' || fn.name === 'is_satisfied' || fn.name === 'is_exposed_in' || fn.name.includes('contains_') || fn.name.startsWith('is_') || fn.name.startsWith('has_')) tags.push('predicate');
                if (fn.name.includes('throw') || fn.name === 'Error') tags.push('error-handling');
                if (fn.name === 'get_dom_class' || fn.name === 'private_from_object' || fn.name === 'private_from_proto_check' || fn.name.includes('native_from') || fn.name.includes('root_from') || fn.name === 'windowproxy_from_handlevalue') tags.push('type-extraction');
                if (fn.name === 'upcast' || fn.name === 'downcast') tags.push('type-casting');
                if (fn.name === 'or_init' || fn.name === 'if_is_some' || fn.name === 'get' || fn.name === 'set' || fn.name === 'take' || fn.name === 'clear' || fn.name === 'new') tags.push('accessor');
                if (fn.name === 'Next' || fn.name === 'dict_return' || fn.name === 'key_and_value_return') tags.push('iterator');
                if (fn.name === 'finalize_weak_referenceable' || fn.name === 'finalize_common' || fn.name === 'finalize_global' || fn.name === 'do_finalize_global') tags.push('gc-hook');
                if (fn.name.includes('guard') || fn.name === 'define_guarded_constants' || fn.name === 'define_guarded_methods' || fn.name === 'define_guarded_properties') tags.push('feature-gate');
                if (tags.length > 5) tags = tags.slice(0, 5);

                allNodes.push({
                    id: fnId,
                    type: 'function',
                    name: fn.name,
                    filePath: file.path,
                    lineRange: [fn.startLine, fn.endLine],
                    summary: getFunctionSummary(file.path, fn.name),
                    tags: tags,
                    complexity: fnComplexity
                });

                allEdges.push({
                    source: fileId,
                    target: fnId,
                    type: 'contains',
                    direction: 'forward',
                    weight: 1.0
                });

                if (file.exports.some(e => e.name === fn.name && e.line === fn.startLine)) {
                    allEdges.push({
                        source: fileId,
                        target: fnId,
                        type: 'exports',
                        direction: 'forward',
                        weight: 0.8
                    });
                }
            }
        }
    }

    // Create class nodes
    if (file.classes) {
        for (const cls of file.classes) {
            if (isClassSignificant(cls, file.exports)) {
                const clsId = 'class:' + file.path + ':' + cls.name;
                const clsLineCount = cls.endLine - cls.startLine + 1;

                let clsComplexity = 'simple';
                if (clsLineCount >= 50) clsComplexity = 'complex';
                else if (clsLineCount >= 15) clsComplexity = 'moderate';

                let tags = ['class', 'dom-bindings', 'rust'];
                if (cls.name.endsWith('Helpers')) { tags.push('trait'); tags.push('interface'); }
                if (cls.name === 'Error') tags.push('error-handling', 'enum');
                if (cls.name === 'CallbackObject' || cls.name === 'CallbackContainer') tags.push('callback');
                if (cls.name === 'Castable') tags.push('inheritance', 'type-casting');
                if (cls.name === 'Iterable' || cls.name === 'IterableIterator') tags.push('iterator', 'collection');
                if (cls.name === 'Setlike' || cls.name === 'Maplike') tags.push('collection', 'data-structure');
                if (cls.name === 'NonCallbackInterfaceObjectClass' || cls.name === 'InterfaceConstructorBehavior') tags.push('interface-definition');
                if (cls.name === 'DOMStringType' || cls.name === 'DOMString') tags.push('string-handling', 'data-model');
                if (tags.length > 5) tags = tags.slice(0, 5);

                allNodes.push({
                    id: clsId,
                    type: 'class',
                    name: cls.name,
                    filePath: file.path,
                    lineRange: [cls.startLine, cls.endLine],
                    summary: getClassSummary(file.path, cls.name),
                    tags: tags,
                    complexity: clsComplexity
                });

                allEdges.push({
                    source: fileId,
                    target: clsId,
                    type: 'contains',
                    direction: 'forward',
                    weight: 1.0
                });

                if (file.exports.some(e => e.name === cls.name && e.line === cls.startLine)) {
                    allEdges.push({
                        source: fileId,
                        target: clsId,
                        type: 'exports',
                        direction: 'forward',
                        weight: 0.8
                    });
                }
            }
        }
    }
}

// Import edges from batchImportData
const importData = inputData.batchImportData;
for (const [filePath, imports] of Object.entries(importData)) {
    for (const targetPath of imports) {
        allEdges.push({
            source: 'file:' + filePath,
            target: 'file:' + targetPath,
            type: 'imports',
            direction: 'forward',
            weight: 0.7
        });
    }
}

// Compute split
const nodeCount = allNodes.length;
const edgeCount = allEdges.length;
console.log('Total nodes:', nodeCount);
console.log('Total edges:', edgeCount);

const parts = Math.ceil(Math.max(nodeCount / 60, edgeCount / 120));
console.log('Parts needed:', parts);

// Sort files alphabetically
const filePaths = results.results.map(r => r.path).sort();
const filesPerPart = Math.ceil(filePaths.length / parts);
console.log('Files per part:', filesPerPart);

// Group files by part
const partFiles = [];
for (let p = 0; p < parts; p++) {
    const startIdx = p * filesPerPart;
    const endIdx = Math.min((p + 1) * filesPerPart, filePaths.length);
    partFiles.push(filePaths.slice(startIdx, endIdx));
}

// Write each part
for (let p = 0; p < parts; p++) {
    const fileSet = new Set(partFiles[p]);
    const fileIdSet = new Set(Array.from(fileSet).map(f => 'file:' + f));

    // Nodes whose filePath or id is in this part
    const partNodes = allNodes.filter(n => {
        if (n.id && fileIdSet.has(n.id)) return true;
        if (n.filePath && fileSet.has(n.filePath)) return true;
        return false;
    });

    const partNodeIds = new Set(partNodes.map(n => n.id));
    const partEdges = allEdges.filter(e => partNodeIds.has(e.source));

    // Validate edges
    for (const edge of partEdges) {
        const sourceOk = partNodeIds.has(edge.source);
        const targetOk = partNodeIds.has(edge.target) ||
            allNodes.some(n => n.id === edge.target) ||
            edge.target.startsWith('file:');
        if (!targetOk) {
            console.log('WARNING: Edge target not found in any node:', edge.source, '->', edge.target);
        }
    }

    console.log('Part ' + (p+1) + ' files:', partFiles[p]);
    console.log('  Nodes:', partNodes.length);
    console.log('  Edges:', partEdges.length);

    const partNum = p + 1;
    const outputPath = parts === 1 ?
        'd:/Projects/servo/.understand-anything/intermediate/batch-28.json' :
        'd:/Projects/servo/.understand-anything/intermediate/batch-28-part-' + partNum + '.json';

    fs.writeFileSync(outputPath, JSON.stringify({nodes: partNodes, edges: partEdges}, null, 2));
    console.log('  Written to:', outputPath);
}

// Verify import edge count
const importEdgeCount = allEdges.filter(e => e.type === 'imports').length;
let expectedImportCount = 0;
for (const [fp, imports] of Object.entries(importData)) {
    expectedImportCount += imports.length;
}
console.log('Import edges:', importEdgeCount, 'expected:', expectedImportCount);
console.log('Match:', importEdgeCount === expectedImportCount ? 'YES' : 'NO');
