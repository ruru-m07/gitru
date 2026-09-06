import type { TestingLibraryMatchers } from "@testing-library/jest-dom/matchers";
import "vitest";

declare module "vitest" {
  interface Assertion<T = unknown> extends TestingLibraryMatchers<void, T> {}

  interface AsymmetricMatchersContaining
    extends TestingLibraryMatchers<void, unknown> {}
}
