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

// Tauri forwards arguments after `--` to Cargo. The normal desktop default is
// Wry, so disable it while the test-only `e2e` feature selects CEF.
args.push("--", "--no-default-features");

const result = spawnSync(process.execPath, args, {
  cwd: new URL("..", import.meta.url),
  encoding: "utf8",
  stdio: "inherit",
});

if (result.error) throw result.error;
process.exitCode = result.status ?? 1;
