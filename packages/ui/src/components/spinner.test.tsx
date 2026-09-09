import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { Spinner } from "./spinner.js";

describe("Spinner", () => {
  it("exposes an accessible loading status", () => {
    render(<Spinner className="test-size" />);

    const spinner = screen.getByRole("status", { name: "Loading" });
    expect(spinner.classList.contains("animate-spin")).toBe(true);
    expect(spinner.classList.contains("test-size")).toBe(true);
  });
});
