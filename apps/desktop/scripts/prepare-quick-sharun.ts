import { createHash } from "node:crypto";
import { chmodSync, existsSync, readFileSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import { resolve } from "node:path";

const upstreamCommit = "3e280d1b2270fecfcb2c2c823b490c782ecf1277";
const upstreamSha256 =
  "9704dbcc9c75a9e77d8ffffe00f480b72d7a7fa91ea9d11698043f88f9fecdaf";

function fail(message: string): never {
  throw new Error(`Cannot prepare CEF AppImage bundler: ${message}`);
}

if (process.platform !== "linux") {
  console.log("Skipping the Linux-only CEF AppImage bundler preparation.");
} else {
  const cacheRoot = process.env.XDG_CACHE_HOME ?? resolve(homedir(), ".cache");
  const destination = resolve(cacheRoot, "tauri/quick-sharun.sh");
  if (!existsSync(destination)) {
    fail(`Tauri did not download quick-sharun to ${destination}`);
  }

  const source = readFileSync(destination, "utf8");
  const sourceHash = createHash("sha256").update(source).digest("hex");
  if (sourceHash !== upstreamSha256) {
    fail(
      `quick-sharun no longer matches upstream ${upstreamCommit}: ${sourceHash}`,
    );
  }

  const unpatched = `\tfor s do
\t\tif ! head -c 20 "$s" | grep -q '#!.*sh'; then`;
  const patched = `\tfor s do
\t\t[ -f "$s" ] || continue
\t\tif ! head -c 20 "$s" | grep -q '#!.*sh'; then`;
  const occurrences = source.split(unpatched).length - 1;
  if (occurrences !== 1) {
    fail(`expected one quick-sharun script loop, found ${occurrences}`);
  }

  writeFileSync(destination, source.replace(unpatched, patched), {
    mode: 0o755,
  });
  chmodSync(destination, 0o755);

  console.log(
    `Patched pinned quick-sharun ${upstreamCommit} with CEF directory handling.`,
  );
}
