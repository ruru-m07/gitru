#!/usr/bin/env node
/**
 * Fix "used before declaration" errors in TS files
 * Usage: node fix-used-before-declaration.js input.ts [output.ts]
 */

const fs = require("node:fs");
const path = require("node:path");

// Simple regex patterns for exported constants and types
const EXPORT_CONST = /export\s+const\s+(\w+)\s*=\s*[^;]+;/g;
const EXPORT_TYPE = /export\s+type\s+(\w+)\s*=\s*[^;]+;/g;

/**
 * Parse all exported symbols and their full definitions.
 */
function extractExports(code) {
  const matches = [];
  const combined = [
    ...code.matchAll(EXPORT_CONST),
    ...code.matchAll(EXPORT_TYPE),
  ];
  for (const m of combined) {
    matches.push({ name: m[1], full: m[0], index: m.index });
  }
  return matches.sort((a, b) => a.index - b.index);
}

/**
 * Analyze references between definitions.
 */
function buildDependencyGraph(definitions) {
  const graph = {};
  for (const def of definitions) {
    const deps = [];
    for (const other of definitions) {
      if (def === other) continue;
      const regex = new RegExp(`\\b${other.name}\\b`);
      if (regex.test(def.full)) deps.push(other.name);
    }
    graph[def.name] = deps;
  }
  return graph;
}

/**
 * Topologically sort definitions based on dependencies.
 */
function topoSort(definitions, graph) {
  const sorted = [];
  const visited = new Set();
  const visiting = new Set();

  function visit(name) {
    if (visited.has(name)) return;
    if (visiting.has(name))
      throw new Error(`Cyclic dependency detected: ${name}`);
    visiting.add(name);
    for (const dep of graph[name] || []) visit(dep);
    visiting.delete(name);
    visited.add(name);
    sorted.push(name);
  }

  for (const def of definitions) visit(def.name);

  return sorted;
}

function fixFile(inputPath, outputPath) {
  const code = fs.readFileSync(inputPath, "utf8");
  const definitions = extractExports(code);

  if (!definitions.length) {
    console.error("No exports found to reorder.");
    return;
  }

  const graph = buildDependencyGraph(definitions);
  const sortedNames = topoSort(definitions, graph);

  // Map name -> definition
  const defMap = Object.fromEntries(definitions.map((d) => [d.name, d.full]));

  // Reconstruct the file
  const header = code.split(definitions[0].full)[0]; // Preserve imports/comments at top
  const body = sortedNames.map((n) => defMap[n]).join("\n\n");
  const tail = code.slice(
    Math.max(...definitions.map((d) => d.index + d.full.length)),
  );

  const fixed = `${header.trim()}\n\n${body}\n\n${tail.trim()}\n`;

  fs.writeFileSync(outputPath, fixed, "utf8");
  console.log(`✅ Fixed file saved to ${outputPath}`);
}

// CLI
const [, , input, output] = process.argv;
if (!input) {
  console.error(
    "Usage: node fix-used-before-declaration.js input.ts [output.ts]",
  );
  process.exit(1);
}
const out = output || input.replace(/\.ts$/, ".fixed.ts");
fixFile(path.resolve(input), path.resolve(out));
