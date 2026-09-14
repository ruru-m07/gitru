import { spawnSync } from "node:child_process";

const args = [
  "run",
  "tauri",
  "build",
  "--features",
  "e2e",
  "--config",
  "src-tauri/tauri.e2e.conf.json",
];

if (process.platform === "darwin") {
  // CEF resolves its framework and helper processes relative to an app bundle.
  args.push("--bundles", "app");
} else {
  args.push("--no-bundle");
}

const result = spawnSync(process.execPath, args, {
  cwd: new URL("..", import.meta.url),
  encoding: "utf8",
  stdio: "inherit",
});

if (result.error) throw result.error;
process.exitCode = result.status ?? 1;
