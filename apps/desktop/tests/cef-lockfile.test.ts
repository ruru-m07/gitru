import { describe, expect, it } from "vitest";
import { cefBinaryVersionFromCargoLock } from "../scripts/cef-lockfile";

const cargoLock = `version = 4

[[package]]
name = "cef"
version = "151.8.1+151.3.24"
source = "registry+https://github.com/rust-lang/crates.io-index"
`;

describe("CEF Cargo.lock parsing", () => {
  it("reads the binary version from LF lockfiles", () => {
    expect(cefBinaryVersionFromCargoLock(cargoLock)).toBe("151.3.24");
  });

  it("reads the binary version from CRLF lockfiles", () => {
    expect(
      cefBinaryVersionFromCargoLock(cargoLock.replace(/\n/g, "\r\n")),
    ).toBe("151.3.24");
  });

  it("fails closed when the CEF package is absent", () => {
    expect(() => cefBinaryVersionFromCargoLock("version = 4\n")).toThrow(
      "Cargo.lock has no cef package",
    );
  });
});
