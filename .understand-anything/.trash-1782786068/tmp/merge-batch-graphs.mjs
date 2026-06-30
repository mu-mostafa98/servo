#!/usr/bin/env node
/**
 * merge-batch-graphs.mjs — Merge and normalize batch analysis results.
 * Node.js port of merge-batch-graphs.py
 */

import { readFileSync, writeFileSync, readdirSync, statSync } from 'fs';
import { join, resolve } from 'path';

const PROJECT_ROOT = resolve(process.argv[2] || '.');
const INTERMEDIATE_DIR = join(PROJECT_ROOT, '.understand-anything', 'intermediate');

// ── Configuration ─────────────────────────────────────────────────────────
const VALID_NODE_PREFIXES = new Set([
  "file", "func", "function", "class", "module", "concept",
  "config", "document", "service", "table", "endpoint",
  "pipeline", "schema", "resource",
  "domain", "flow", "step",
  "article", "entity", "topic", "claim", "source",
]);

const TYPE_TO_PREFIX = {
  file: "file", function: "function", func: "function",
  class: "class", module: "module", concept: "concept",
  config: "config", document: "document", service: "service",
  table: "table", endpoint: "endpoint", pipeline: "pipeline",
  schema: "schema", resource: "resource",
  domain: "domain", flow: "flow", step: "step",
  article: "article", entity: "entity", topic: "topic",
  claim: "claim", source: "source",
};

const COMPLEXITY_MAP = {
  low: "simple", easy: "simple",
  medium: "moderate", intermediate: "moderate",
  high: "complex", hard: "complex", difficult: "complex",
};

const VALID_COMPLEXITY = new Set(["simple", "moderate", "complex"]);

const DIRECTION_ALIASES = { both: "bidirectional", mutual: "bidirectional" };
const VALID_DIRECTIONS = new Set(["forward", "backward", "bidirectional"]);

// ── Helpers ───────────────────────────────────────────────────────────────

function normalizeDirection(value) {
  if (typeof value !== 'string') return "forward";
  const v = value.toLowerCase();
  return DIRECTION_ALIASES[v] || (VALID_DIRECTIONS.has(v) ? v : "forward");
}

function num(v) {
  const n = parseFloat(v);
  return isNaN(n) ? 0 : n;
}

function pathSegments(p) {
  return p.split('/').filter(Boolean);
}

function basename(p) {
  const i = p.lastIndexOf('/');
  return i >= 0 ? p.slice(i + 1) : p;
}

function classifyIdFix(original, corrected) {
  for (const p of VALID_NODE_PREFIXES) {
    if (original.startsWith(`${p}:${p}:`)) return `${p}:${p}: → ${p}: (double prefix)`;
  }
  const parts = original.split(':');
  if (parts.length >= 3 && !VALID_NODE_PREFIXES.has(parts[0]) && VALID_NODE_PREFIXES.has(parts[1])) {
    return `<project>:${parts[1]}: → ${parts[1]}: (project-name prefix)`;
  }
  if (original.startsWith("func:") && corrected.startsWith("function:")) {
    return "func: → function: (prefix canonicalization)";
  }
  if (![...VALID_NODE_PREFIXES].some(p => original.startsWith(`${p}:`))) {
    const prefix = corrected.split(':')[0];
    return `bare path → ${prefix}: (missing prefix)`;
  }
  return `${original} → ${corrected}`;
}

function normalizeNodeId(nodeId, node) {
  let nid = nodeId;

  // Strip double prefix
  for (const p of VALID_NODE_PREFIXES) {
    const double = `${p}:${p}:`;
    if (nid.startsWith(double)) {
      nid = nid.slice(p.length + 1);
      break;
    }
  }

  // Strip project-name prefix
  const validPrefixes = [...VALID_NODE_PREFIXES].map(p => p.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')).join('|');
  const match = nid.match(new RegExp(`^[^:]+:(${validPrefixes}):(.+)$`));
  if (match) {
    const firstSeg = nid.split(':')[0];
    if (!VALID_NODE_PREFIXES.has(firstSeg)) {
      nid = `${match[1]}:${match[2]}`;
    }
  }

  // Canonicalize func: → function:
  if (nid.startsWith("func:") && !nid.startsWith("function:")) {
    nid = "function:" + nid.slice(5);
  }

  // Add missing prefix for bare paths
  const hasPrefix = [...VALID_NODE_PREFIXES].some(p => nid.startsWith(`${p}:`));
  if (!hasPrefix) {
    const nodeType = node.type || "file";
    const prefix = TYPE_TO_PREFIX[nodeType] || "file";
    if (nodeType === "function" || nodeType === "class") {
      const filePath = node.filePath || "";
      const name = node.name || nid;
      nid = filePath ? `${prefix}:${filePath}:${name}` : `${prefix}:__nofilepath__:${name}`;
    } else {
      nid = `${prefix}:${nid}`;
    }
  }

  return nid;
}

function normalizeComplexity(value) {
  if (typeof value === 'string') {
    const lower = value.trim().toLowerCase();
    if (VALID_COMPLEXITY.has(lower)) return { value: lower, status: "valid" };
    if (COMPLEXITY_MAP[lower]) return { value: COMPLEXITY_MAP[lower], status: "mapped" };
    return { value: "moderate", status: "unknown" };
  }
  if (typeof value === 'number') {
    if (value <= 3) return { value: "simple", status: "mapped" };
    if (value <= 6) return { value: "moderate", status: "mapped" };
    return { value: "complex", status: "mapped" };
  }
  return { value: "moderate", status: "unknown" };
}

// ── Test path helpers ─────────────────────────────────────────────────────

const JS_TS_EXTS = [".ts", ".tsx", ".js", ".jsx", ".mjs", ".cjs", ".vue"];
const JS_TS_TEST_EXTS = new Set(JS_TS_EXTS);
const MIRROR_PRODUCTION_ROOTS = ["src", "app", "lib", ""];

const TEST_NAME_PATTERNS = {
  ".go": [[], ["_test"]],
  ".py": [["test_"], ["_test"]],
  ".java": [[], ["Test", "Tests", "IT"]],
  ".kt": [[], ["Test", "Tests"]],
  ".cs": [[], ["Test", "Tests"]],
  ".c": [["test_"], ["_test"]],
  ".cpp": [["test_"], ["_test"]],
  ".cc": [["test_"], ["_test"]],
};

function isTestPath(path) {
  const bn = basename(path);
  const dot = bn.lastIndexOf('.');
  if (dot < 0) return false;
  const stem = bn.slice(0, dot);
  const ext = bn.slice(dot);

  if (JS_TS_TEST_EXTS.has(ext)) return stem.endsWith(".test") || stem.endsWith(".spec");

  const patterns = TEST_NAME_PATTERNS[ext];
  if (!patterns) return false;
  const [prefixes, suffixes] = patterns;
  return prefixes.some(p => stem.startsWith(p)) || suffixes.some(s => stem.endsWith(s));
}

function stripTestInfix(stem) {
  for (const infix of [".test", ".spec"]) {
    if (stem.endsWith(infix)) return stem.slice(0, -infix.length);
  }
  return null;
}

function joinDir(dirPath, name) {
  return dirPath ? `${dirPath}/${name}` : name;
}

function addUnique(out, path) {
  if (path && !out.includes(path)) out.push(path);
}

function productionCandidates(testPath) {
  const bn = basename(testPath);
  const dot = bn.lastIndexOf('.');
  if (dot < 0) return [];
  const stem = bn.slice(0, dot);
  const ext = bn.slice(dot);
  const segs = pathSegments(testPath);
  const dirSegs = segs.slice(0, -1);
  const dirPath = dirSegs.join('/');
  const candidates = [];

  // JS/TS family
  if (JS_TS_TEST_EXTS.has(ext)) {
    const baseStem = stripTestInfix(stem);
    if (baseStem !== null) {
      addUnique(candidates, joinDir(dirPath, `${baseStem}${ext}`));
      for (const e of JS_TS_EXTS) addUnique(candidates, joinDir(dirPath, `${baseStem}${e}`));
      if (dirSegs.length && ["__tests__", "test", "spec", "tests"].includes(dirSegs[dirSegs.length - 1])) {
        const parentDir = dirSegs.slice(0, -1).join('/');
        addUnique(candidates, joinDir(parentDir, `${baseStem}${ext}`));
        for (const e of JS_TS_EXTS) addUnique(candidates, joinDir(parentDir, `${baseStem}${e}`));
      }
      if (dirSegs.length && ["tests", "test", "__tests__"].includes(dirSegs[0])) {
        const tailPath = dirSegs.slice(1).join('/');
        for (const root of MIRROR_PRODUCTION_ROOTS) {
          const newDir = [root, tailPath].filter(Boolean).join('/');
          addUnique(candidates, joinDir(newDir, `${baseStem}${ext}`));
          for (const e of JS_TS_EXTS) addUnique(candidates, joinDir(newDir, `${baseStem}${e}`));
        }
      }
    }
  }
  // Go
  else if (ext === ".go" && stem.endsWith("_test")) {
    addUnique(candidates, joinDir(dirPath, `${stem.slice(0, -5)}.go`));
  }
  // Python
  else if (ext === ".py" && (stem.startsWith("test_") || stem.endsWith("_test"))) {
    const baseStem = stem.startsWith("test_") ? stem.slice(5) : stem.slice(0, -5);
    addUnique(candidates, joinDir(dirPath, `${baseStem}.py`));
    if (dirSegs.length && ["tests", "test"].includes(dirSegs[dirSegs.length - 1])) {
      addUnique(candidates, joinDir(dirSegs.slice(0, -1).join('/'), `${baseStem}.py`));
    }
    if (dirSegs.length && ["tests", "test"].includes(dirSegs[0])) {
      const tailPath = dirSegs.slice(1).join('/');
      for (const root of MIRROR_PRODUCTION_ROOTS) {
        addUnique(candidates, joinDir([root, tailPath].filter(Boolean).join('/'), `${baseStem}.py`));
      }
    }
  }
  // Java
  else if (ext === ".java") {
    for (const suffix of ["Tests", "Test", "IT"]) {
      if (stem.endsWith(suffix)) {
        const baseStem = stem.slice(0, -suffix.length);
        if (dirSegs.length >= 3 && dirSegs[0] === "src" && dirSegs[1] === "test" && dirSegs[2] === "java") {
          addUnique(candidates, joinDir(["src", "main", "java", ...dirSegs.slice(3)].join('/'), `${baseStem}.java`));
        }
        addUnique(candidates, joinDir(dirPath, `${baseStem}.java`));
        break;
      }
    }
  }
  // Kotlin
  else if (ext === ".kt") {
    for (const suffix of ["Tests", "Test"]) {
      if (stem.endsWith(suffix)) {
        const baseStem = stem.slice(0, -suffix.length);
        if (dirSegs.length >= 3 && dirSegs[0] === "src" && dirSegs[1] === "test" && dirSegs[2] === "kotlin") {
          addUnique(candidates, joinDir(["src", "main", "kotlin", ...dirSegs.slice(3)].join('/'), `${baseStem}.kt`));
        }
        addUnique(candidates, joinDir(dirPath, `${baseStem}.kt`));
        break;
      }
    }
  }
  // C#
  else if (ext === ".cs") {
    for (const suffix of ["Tests", "Test"]) {
      if (stem.endsWith(suffix)) {
        const baseStem = stem.slice(0, -suffix.length);
        addUnique(candidates, joinDir(dirPath, `${baseStem}.cs`));
        let testsIdx = -1;
        for (let i = dirSegs.length - 1; i >= 0; i--) {
          if (["tests", "test"].includes(dirSegs[i].toLowerCase())) { testsIdx = i; break; }
        }
        if (testsIdx >= 0) {
          const parentSegs = dirSegs.slice(0, testsIdx);
          const tailSegs = dirSegs.slice(testsIdx + 1);
          addUnique(candidates, joinDir(parentSegs.join('/'), `${baseStem}.cs`));
          addUnique(candidates, joinDir([...parentSegs, "src", ...tailSegs].join('/'), `${baseStem}.cs`));
        }
        if (dirSegs.length) {
          const top = dirSegs[0];
          const suffix2 = top.endsWith(".Tests") ? ".Tests" : top.endsWith(".Test") ? ".Test" : null;
          if (suffix2) {
            const sibling = top.slice(0, -suffix2.length);
            if (sibling) {
              addUnique(candidates, joinDir([sibling, ...dirSegs.slice(1)].join('/'), `${baseStem}.cs`));
            }
          }
        }
        break;
      }
    }
  }
  // C/C++
  else if ([".c", ".cpp", ".cc"].includes(ext)) {
    let baseStem = null;
    if (stem.startsWith("test_")) baseStem = stem.slice(5);
    else if (stem.endsWith("_test")) baseStem = stem.slice(0, -5);
    if (baseStem !== null) addUnique(candidates, joinDir(dirPath, `${baseStem}${ext}`));
  }

  return candidates;
}

function fileNodePath(node) {
  const nid = node.id || "";
  if (typeof nid !== 'string' || !nid.startsWith("file:")) return null;
  if (typeof node.filePath === 'string' && node.filePath) return node.filePath;
  return nid.slice(5);
}

// ── Main merge ────────────────────────────────────────────────────────────

function mergeAndNormalize(batches) {
  const idFixPatterns = {};
  const complexityFixPatterns = {};
  const unfixable = [];

  // Step 1: Combine all nodes and edges
  const allNodes = batches.flatMap(b => b.nodes || []);
  const allEdges = batches.flatMap(b => b.edges || []);
  const totalInputNodes = allNodes.length;
  const totalInputEdges = allEdges.length;

  // Step 2: Normalize node IDs
  const idMapping = {};
  const nodesWithIds = [];
  const unknownNodeTypes = {};

  for (let i = 0; i < allNodes.length; i++) {
    const node = allNodes[i];
    const originalId = node.id;
    if (!originalId) {
      unfixable.push(`Node[${i}] has no 'id' field (name=${node.name || '?'}, type=${node.type || '?'})`);
      continue;
    }
    const nodeType = node.type || "";
    if (nodeType && !TYPE_TO_PREFIX[nodeType]) {
      unknownNodeTypes[nodeType] = (unknownNodeTypes[nodeType] || 0) + 1;
    }
    nodesWithIds.push(node);
    const correctedId = normalizeNodeId(originalId, node);
    if (correctedId !== originalId) {
      const pattern = classifyIdFix(originalId, correctedId);
      idFixPatterns[pattern] = (idFixPatterns[pattern] || 0) + 1;
      idMapping[originalId] = correctedId;
      node.id = correctedId;
    }
  }

  // Step 3: Normalize complexity
  const complexityUnknownPatterns = {};
  for (const node of nodesWithIds) {
    const original = node.complexity;
    const { value: normalized, status } = normalizeComplexity(original);
    if (status === "mapped") {
      const origRepr = typeof original === 'string' ? `"${original}"` : JSON.stringify(original);
      complexityFixPatterns[`${origRepr} → "${normalized}"`] = (complexityFixPatterns[`${origRepr} → "${normalized}"`] || 0) + 1;
    } else if (status === "unknown") {
      const origRepr = typeof original === 'string' ? `"${original}"` : JSON.stringify(original);
      complexityUnknownPatterns[`complexity ${origRepr} → defaulted to "moderate"`] = (complexityUnknownPatterns[`complexity ${origRepr} → defaulted to "moderate"`] || 0) + 1;
    }
    node.complexity = normalized;
  }

  // Step 4: Rewrite edge references
  let edgesRewritten = 0;
  for (const edge of allEdges) {
    const src = edge.source || "";
    const tgt = edge.target || "";
    const newSrc = idMapping[src] || src;
    const newTgt = idMapping[tgt] || tgt;
    if (newSrc !== src || newTgt !== tgt) {
      edgesRewritten++;
      edge.source = newSrc;
      edge.target = newTgt;
    }
  }

  // Step 5: Deduplicate nodes by ID (keep last)
  let duplicateCount = 0;
  const nodesById = {};
  for (const node of nodesWithIds) {
    const nid = node.id || "";
    if (nodesById[nid]) duplicateCount++;
    nodesById[nid] = node;
  }

  // Step 5b: tested_by linker
  const { added: testedByAdded, dropped: testedByDropped, tagged: testedByTagged, swapped: testedBySwapped } = linkTests(nodesById, allEdges);

  // Step 6: Deduplicate edges, drop dangling
  const nodeIds = new Set(Object.keys(nodesById));
  const edgesByKey = {};
  const duplicateEdges = { count: 0 };
  for (const edge of allEdges) {
    const src = edge.source || "";
    const tgt = edge.target || "";
    const etype = edge.type || "";
    const direction = normalizeDirection(edge.direction);
    edge.direction = direction;

    if (!nodeIds.has(src) || !nodeIds.has(tgt)) {
      const missing = [];
      if (!nodeIds.has(src)) missing.push(`source '${src}'`);
      if (!nodeIds.has(tgt)) missing.push(`target '${tgt}'`);
      unfixable.push(`Edge ${src} → ${tgt} (${etype}): dropped, missing ${missing.join(', ')}`);
      continue;
    }

    const key = `${src}|${tgt}|${etype}|${direction}`;
    const existing = edgesByKey[key];
    if (!existing || num(edge.weight) > num(existing.weight)) {
      if (existing) duplicateEdges.count++;
      edgesByKey[key] = edge;
    }
  }

  // Build report
  const report = [];
  report.push(`Input: ${totalInputNodes} nodes, ${totalInputEdges} edges`);

  const fixedLines = [];
  for (const [pattern, count] of Object.entries(idFixPatterns).sort((a, b) => b[1] - a[1])) {
    fixedLines.push(`  ${String(count).padStart(4)} × ${pattern}`);
  }
  for (const [pattern, count] of Object.entries(complexityFixPatterns).sort((a, b) => b[1] - a[1])) {
    fixedLines.push(`  ${String(count).padStart(4)} × complexity ${pattern}`);
  }
  if (edgesRewritten) fixedLines.push(`  ${String(edgesRewritten).padStart(4)} × edge references rewritten after ID normalization`);
  if (duplicateCount) fixedLines.push(`  ${String(duplicateCount).padStart(4)} × duplicate node IDs removed (kept last)`);
  if (testedBySwapped) fixedLines.push(`  ${String(testedBySwapped).padStart(4)} × tested_by edges flipped (test→production became production→test)`);
  if (testedByDropped) fixedLines.push(`  ${String(testedByDropped).padStart(4)} × tested_by edges dropped (orphan endpoint or test↔test/prod↔prod pair)`);

  if (fixedLines.length) {
    report.push('');
    const totalFixes = Object.values(idFixPatterns).reduce((a, b) => a + b, 0) +
      Object.values(complexityFixPatterns).reduce((a, b) => a + b, 0) +
      edgesRewritten + duplicateCount + testedBySwapped + testedByDropped;
    report.push(`Fixed (${totalFixes} corrections):`);
    report.push(...fixedLines);
  }

  if (testedByAdded || testedByTagged) {
    report.push('');
    report.push('Tested-by linker:');
    report.push(`  ${String(testedByAdded).padStart(4)} × tested_by edges produced (path-convention supplement, production→test)`);
    report.push(`  ${String(testedByTagged).padStart(4)} × production nodes tagged "tested"`);
  }

  const unfixableTotal = unfixable.length +
    Object.values(complexityUnknownPatterns).reduce((a, b) => a + b, 0) +
    Object.values(unknownNodeTypes).reduce((a, b) => a + b, 0);

  if (unfixableTotal) {
    report.push('');
    report.push(`Could not fix (${unfixableTotal} issues — needs agent review):`);
    for (const [ntype, count] of Object.entries(unknownNodeTypes).sort((a, b) => b[1] - a[1])) {
      report.push(`  ${String(count).padStart(4)} × unknown node type "${ntype}" (not in schema, kept as-is)`);
    }
    for (const [pattern, count] of Object.entries(complexityUnknownPatterns).sort((a, b) => b[1] - a[1])) {
      report.push(`  ${String(count).padStart(4)} × ${pattern}`);
    }
    for (const detail of unfixable) {
      report.push(`  - ${detail}`);
    }
  }

  report.push('');
  report.push(`Output: ${Object.keys(nodesById).length} nodes, ${Object.keys(edgesByKey).length} edges`);

  return {
    assembled: {
      nodes: Object.values(nodesById),
      edges: Object.values(edgesByKey),
    },
    report,
  };
}

// ── tested_by linker ──────────────────────────────────────────────────────

function linkTests(nodesById, edges) {
  // Index file nodes
  const filePathsToNodes = {};
  const nodeIdToClassification = {};
  const testNodes = [];

  for (const node of Object.values(nodesById)) {
    const path = fileNodePath(node);
    if (!path) continue;
    filePathsToNodes[path] = node;
    if (isTestPath(path)) {
      nodeIdToClassification[node.id] = "test";
      testNodes.push([path, node]);
    } else {
      nodeIdToClassification[node.id] = "prod";
    }
  }

  // Pass 1: walk existing tested_by edges
  const covered = new Set();
  const pairToIdx = {};
  const swappedPairs = new Set();
  let dropped = 0;
  let writeIdx = 0;

  for (let i = 0; i < edges.length; i++) {
    const edge = edges[i];
    if (edge.type !== "tested_by") {
      edges[writeIdx++] = edge;
      continue;
    }

    const src = edge.source || "";
    const tgt = edge.target || "";
    const srcClass = nodeIdToClassification[src];
    const tgtClass = nodeIdToClassification[tgt];
    let pair, needsSwap;

    if (srcClass === "prod" && tgtClass === "test") {
      pair = [src, tgt].join('|');
      needsSwap = false;
    } else if (srcClass === "test" && tgtClass === "prod") {
      pair = [tgt, src].join('|');
      needsSwap = true;
    } else {
      dropped++;
      continue;
    }

    if (covered.has(pair)) {
      const existingIdx = pairToIdx[pair];
      const existing = edges[existingIdx];
      if (num(edge.weight) > num(existing.weight)) {
        if (needsSwap) {
          swapTestedBy(edge, src, tgt);
          swappedPairs.add(pair);
        } else swappedPairs.delete(pair);
        edges[existingIdx] = edge;
      }
      dropped++;
      continue;
    }

    if (needsSwap) {
      swapTestedBy(edge, src, tgt);
      swappedPairs.add(pair);
    }
    covered.add(pair);
    pairToIdx[pair] = writeIdx;
    edges[writeIdx++] = edge;
  }
  edges.length = writeIdx;
  const swapped = swappedPairs.size;

  // Pass 2: path-convention supplement
  const pairedTestIds = new Set();
  for (const pair of covered) pairedTestIds.add(pair.split('|')[1]);
  let added = 0;

  for (const [testPath, testNode] of testNodes) {
    if (pairedTestIds.has(testNode.id)) continue;
    for (const candPath of productionCandidates(testPath)) {
      const prodNode = filePathsToNodes[candPath];
      if (!prodNode) continue;
      if (isTestPath(candPath)) continue;
      const pair = [prodNode.id, testNode.id].join('|');
      if (covered.has(pair)) continue;
      edges.push({
        source: prodNode.id,
        target: testNode.id,
        type: "tested_by",
        direction: "forward",
        weight: 0.5,
        description: "Path-based pairing (deterministic)",
      });
      covered.add(pair);
      added++;
      break;
    }
  }

  // Tag production nodes
  let tagged = 0;
  for (const pair of covered) {
    const prodId = pair.split('|')[0];
    const prodNode = nodesById[prodId];
    if (!prodNode) continue;
    if (!prodNode.tags || !Array.isArray(prodNode.tags)) prodNode.tags = [];
    if (!prodNode.tags.includes("tested")) {
      prodNode.tags.push("tested");
      tagged++;
    }
  }

  return { added, dropped, tagged, swapped };
}

function swapTestedBy(edge, src, tgt) {
  edge.source = tgt;
  edge.target = src;
  edge.direction = "forward";
  const prev = edge.description;
  edge.description = prev ? `${prev} [direction corrected]` : "Direction corrected (was test → production)";
}

// ── Imports recovery from scan-result.json ────────────────────────────────

function recoverImportsFromScan(assembled, scanResultPath) {
  let scan;
  try {
    scan = JSON.parse(readFileSync(scanResultPath, 'utf8'));
  } catch (e) {
    return { recovered: 0, lines: [`  importMap recovery skipped — ${scanResultPath} not found or unparseable`] };
  }

  const importMap = scan.importMap;
  if (!importMap || typeof importMap !== 'object') {
    return { recovered: 0, lines: [`  importMap recovery skipped — no importMap field`] };
  }

  const fileNodeIds = new Set();
  for (const node of assembled.nodes) {
    if (node.type === "file") fileNodeIds.add(node.id || "");
  }

  const existing = new Set();
  for (const edge of assembled.edges) {
    if (edge.type === "imports") existing.add(`${edge.source}|${edge.target}`);
  }

  let recovered = 0, skippedNoSrc = 0, skippedNoTgt = 0;
  for (const [srcPath, targets] of Object.entries(importMap)) {
    if (!Array.isArray(targets)) continue;
    const srcId = `file:${srcPath}`;
    if (!fileNodeIds.has(srcId)) {
      if (targets.length) skippedNoSrc++;
      continue;
    }
    for (const tgtPath of targets) {
      if (typeof tgtPath !== 'string' || !tgtPath) continue;
      const tgtId = `file:${tgtPath}`;
      if (!fileNodeIds.has(tgtId)) { skippedNoTgt++; continue; }
      if (srcId === tgtId) continue;
      const key = `${srcId}|${tgtId}`;
      if (existing.has(key)) continue;
      assembled.edges.push({
        source: srcId,
        target: tgtId,
        type: "imports",
        direction: "forward",
        weight: 0.7,
        recoveredFromImportMap: true,
      });
      existing.add(key);
      recovered++;
    }
  }

  const lines = [
    `  Recovered ${recovered} \`imports\` edges from importMap (${Object.keys(importMap).length} entries scanned)`,
  ];
  if (skippedNoSrc) lines.push(`  Skipped ${skippedNoSrc} importMap source files with no \`file:\` node in graph`);
  if (skippedNoTgt) lines.push(`  Skipped ${skippedNoTgt} importMap target paths with no \`file:\` node in graph`);

  return { recovered, lines };
}

// ── Main ──────────────────────────────────────────────────────────────────

function main() {
  if (!INTERMEDIATE_DIR) {
    console.error("Usage: node merge-batch-graphs.mjs <project-root>");
    process.exit(1);
  }

  // Discover batch files
  let files;
  try {
    files = readdirSync(INTERMEDIATE_DIR).filter(f => f.startsWith('batch-') && f.endsWith('.json'));
  } catch (e) {
    console.error(`Error: ${INTERMEDIATE_DIR} does not exist`);
    process.exit(1);
  }

  // Parse batch indices
  const byBatch = {};
  const unrecognizedFiles = [];
  for (const f of files) {
    const m = f.match(/^batch-(\d+)(?:-part-(\d+))?\.json$/);
    if (m) {
      const idx = parseInt(m[1]);
      if (!byBatch[idx]) byBatch[idx] = [];
      byBatch[idx].push({ name: f, part: m[2] ? parseInt(m[2]) : null });
    } else {
      unrecognizedFiles.push(f);
    }
  }

  if (unrecognizedFiles.length) {
    const preview = unrecognizedFiles.slice(0, 5).join(', ');
    console.error(`Warning: merge: ${unrecognizedFiles.length} batch file(s) with unrecognized filenames will be DROPPED — files: ${preview} — fix agent to use batch-<N>.json or batch-<N>-part-<K>.json`);
  }

  const logicalCount = Object.keys(byBatch).length;
  const multiPart = Object.values(byBatch).filter(v => v.length > 1).length;
  console.error(`Found ${files.length - unrecognizedFiles.length} batch files (${logicalCount} logical batches, ${multiPart} multi-part):`);

  // Missing part detection
  const missingPartWarnings = [];
  for (const [idx, entries] of Object.entries(byBatch)) {
    const partNums = entries.map(e => e.part).filter(p => p !== null);
    if (!partNums.length) continue;
    const present = new Set(partNums);
    const expected = new Set();
    for (let i = 1; i <= Math.max(...partNums); i++) expected.add(i);
    const missing = [...expected].filter(x => !present.has(x)).sort((a, b) => a - b);
    if (missing.length) {
      const msg = `batch ${idx} has parts ${[...present].sort((a,b)=>a-b)} but missing part ${missing} — possible truncated write — affected nodes/edges may be lost`;
      console.error(`Warning: merge: ${msg}`);
      missingPartWarnings.push(msg);
    }
  }

  // Load batches
  const unrecognizedSet = new Set(unrecognizedFiles);
  const batchFiles = files.filter(f => !unrecognizedSet.has(f));
  const batches = [];
  for (const f of batchFiles) {
    try {
      const data = JSON.parse(readFileSync(join(INTERMEDIATE_DIR, f), 'utf8'));
      if (!Array.isArray(data.nodes) || !Array.isArray(data.edges)) {
        console.error(`  Warning: skipping ${f}: missing or invalid nodes/edges array`);
        continue;
      }
      batches.push(data);
      console.error(`  ${f}: ${data.nodes.length} nodes, ${data.edges.length} edges`);
    } catch (e) {
      console.error(`  Warning: skipping ${f}: ${e.message}`);
    }
  }

  if (!batches.length) {
    console.error("Error: no valid batch files loaded");
    process.exit(1);
  }

  // Merge
  const { assembled, report } = mergeAndNormalize(batches);

  // Surface warnings
  if (missingPartWarnings.length) {
    report.push('');
    report.push(`Warning: ${missingPartWarnings.length} batch(es) with missing parts — some nodes/edges silently dropped:`);
    for (const w of missingPartWarnings) report.push(`  - ${w}`);
  }
  if (unrecognizedFiles.length) {
    const preview = unrecognizedFiles.slice(0, 5).join(', ');
    report.push('');
    report.push(`Warning: dropped ${unrecognizedFiles.length} batch file(s) with unrecognized filenames — files: ${preview} — fix agent to use only batch-<N>.json or batch-<N>-part-<K>.json patterns`);
  }

  // Recover imports
  const scanResultPath = join(INTERMEDIATE_DIR, 'scan-result.json');
  const { lines: recoveryLines } = recoverImportsFromScan(assembled, scanResultPath);
  report.push('');
  report.push('Imports edge recovery:');
  report.push(...recoveryLines);

  // Print report
  console.error('');
  for (const line of report) console.error(line);

  // Write output
  const outputPath = join(INTERMEDIATE_DIR, 'assembled-graph.json');
  writeFileSync(outputPath, JSON.stringify(assembled, null, 2), 'utf8');
  const sizeKb = (statSync(outputPath).size / 1024).toFixed(0);
  console.error(`\nWritten to ${outputPath} (${sizeKb} KB)`);
}

main();
