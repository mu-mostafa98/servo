import fs from 'fs';

const data = JSON.parse(fs.readFileSync('d:/Projects/servo/.understand-anything/intermediate/batch-31.json', 'utf-8'));
const { nodes, edges } = data;

// Sort files alphabetically by path
const fileNodes = nodes.filter(n => n.type === 'file');
fileNodes.sort((a, b) => a.filePath.localeCompare(b.filePath));

const allPaths = fileNodes.map(n => n.filePath);
console.log('File paths in order:', allPaths);

// Compute parts
const nodeCount = nodes.length;
const edgeCount = edges.length;
const parts = Math.ceil(Math.max(nodeCount / 60, edgeCount / 120));
console.log(`Node count: ${nodeCount}, Edge count: ${edgeCount}. Splitting into ${parts} parts.`);

// Chunk files
const filesPerPart = Math.ceil(allPaths.length / parts);
const chunks = [];
for (let i = 0; i < parts; i++) {
    const start = i * filesPerPart;
    const end = Math.min(start + filesPerPart, allPaths.length);
    chunks.push(allPaths.slice(start, end));
}
console.log('File chunks per part:', chunks.map(c => c.length));

// For each part, collect:
// - File nodes whose filePath is in this chunk
// - Sub-nodes (function/class) whose filePath is in this chunk
// - Edges whose source is a node in this chunk

function collectPart(chunkPaths) {
    const pathSet = new Set(chunkPaths);
    const partNodes = [];
    const nodeIds = new Set();

    for (const n of nodes) {
        if (n.type === 'file') {
            if (pathSet.has(n.filePath)) {
                partNodes.push(n);
                nodeIds.add(n.id);
            }
        } else {
            // function/class nodes
            if (pathSet.has(n.filePath)) {
                partNodes.push(n);
                nodeIds.add(n.id);
            }
        }
    }

    const partEdges = [];
    const edgeSet = new Set();
    for (const e of edges) {
        // Only include edges whose source is in this part
        if (nodeIds.has(e.source)) {
            const key = e.source + '|' + e.target + '|' + e.type;
            if (!edgeSet.has(key)) {
                partEdges.push(e);
                edgeSet.add(key);
            }
        }
    }

    return { nodes: partNodes, edges: partEdges };
}

// Write each part
const outDir = 'd:/Projects/servo/.understand-anything/intermediate';
for (let i = 0; i < parts; i++) {
    const part = collectPart(chunks[i]);
    const partFile = `${outDir}/batch-31-part-${i + 1}.json`;
    fs.writeFileSync(partFile, JSON.stringify(part, null, 2));
    console.log(`Part ${i + 1}: ${part.nodes.length} nodes, ${part.edges.length} edges -> ${partFile}`);

    // Validate
    const valid = part.edges.every(e => {
        const srcExists = part.nodes.some(n => n.id === e.source);
        const tgtExists = part.nodes.some(n => n.id === e.target) ||
                          e.target.startsWith('file:') ||
                          e.target.startsWith('function:') ||
                          e.target.startsWith('class:');
        return srcExists && tgtExists;
    });
    if (!valid) {
        console.log(`WARNING: Part ${i + 1} has edges with source not in this part!`);
        for (const e of part.edges) {
            const srcIn = part.nodes.some(n => n.id === e.source);
            const tgtIn = part.nodes.some(n => n.id === e.target);
            if (!srcIn) console.log(`  Source missing: ${e.source}`);
        }
    }
}

// Delete the combined file
fs.unlinkSync(`${outDir}/batch-31.json`);
console.log('Deleted combined batch-31.json');
