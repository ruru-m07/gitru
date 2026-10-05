//! Canonical filesystem identity spelling shared with Rust's Windows paths.
import { realpathSync } from "node:fs";
import { win32 } from "node:path";

/** A representation conversion, never a substitute for resolving the file. */
export function nativeCanonicalSpelling(
  canonical: string,
  platform: NodeJS.Platform = process.platform,
): string {
  return platform === "win32" ? win32.toNamespacedPath(canonical) : canonical;
}

/** Resolve OS identity, including DOS aliases, before restoring Rust's prefix.
 * The default Windows realpath walker preserves non-symlink short names. */
export function canonicalHarnessPath(path: string): string {
  return nativeCanonicalSpelling(realpathSync.native(path));
}
