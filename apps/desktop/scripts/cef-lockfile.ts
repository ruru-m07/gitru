export function cefBinaryVersionFromCargoLock(cargoLock: string): string {
  const normalizedCargoLock = cargoLock.replace(/\r\n?/g, "\n");
  const cefPackage = normalizedCargoLock
    .split("[[package]]")
    .find((block) => /\nname = "cef"\n/.test(`\n${block}`));

  if (!cefPackage) {
    throw new Error("Cargo.lock has no cef package");
  }

  const version = cefPackage.match(/\nversion = "[^"]+\+([^"]+)"/)?.[1];
  if (!version) {
    throw new Error("Cargo.lock cef version has no binary version");
  }

  return version;
}
