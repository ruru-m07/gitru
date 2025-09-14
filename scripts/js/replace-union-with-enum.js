#!/usr/bin/env node
/**
 * Replace `z.union([...])` with `z.enum([...])` in TypeScript files.
 * Usage: node replace-union-with-enum.js input.ts [output.ts]
 */

const fs = require("node:fs");
const path = require("node:path");

// Regex to match z.union([...]) including multiline arrays
const UNION_REGEX = /z\.union\s*\(\s*\[([\s\S]*?)\]\s*\)/g;

function replaceUnions(code) {
	return code.replace(UNION_REGEX, (_, inner) => {
		return `z.enum([${inner.trim()}])`;
	});
}

function processFile(inputPath, outputPath) {
	const code = fs.readFileSync(inputPath, "utf8");
	const newCode = replaceUnions(code);
	fs.writeFileSync(outputPath, newCode, "utf8");
	console.log(`✅ Replaced unions saved to ${outputPath}`);
}

// CLI usage
const [, , input, output] = process.argv;
if (!input) {
	console.error("Usage: node replace-union-with-enum.js input.ts [output.ts]");
	process.exit(1);
}
const out = output || input.replace(/\.ts$/, ".enum.ts");
processFile(path.resolve(input), path.resolve(out));
