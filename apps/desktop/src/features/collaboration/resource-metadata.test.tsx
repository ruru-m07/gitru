import { render, screen, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { fixtureItem } from "../../../tests/fixtures/collaboration";
import { fixtureMetadata } from "../../../tests/fixtures/resource-detail";
import { SelectedResourceHeader } from "./resource-metadata";

describe("authorized resource metadata presentation", () => {
  it("renders all duplicate/name-only labels without key collisions and handles empty title", () => {
    const error = vi.spyOn(console, "error").mockImplementation(() => {});
    const metadata = fixtureMetadata();
    metadata.values.title = "";
    metadata.values.labels = Array.from({ length: 100 }, (_, index) => ({
      provider_id: index < 50 ? "same-id" : null,
      name: "x".repeat(1024),
      color: null,
    }));
    render(<SelectedResourceHeader item={fixtureItem} metadata={metadata} />);
    expect(
      screen.getByRole("heading", {
        name: "This resource has an empty title.",
      }),
    ).toBeVisible();
    expect(screen.getAllByText("x".repeat(1024))).toHaveLength(100);
    expect(
      error.mock.calls.some((call) => String(call[0]).includes("same key")),
    ).toBe(false);
    error.mockRestore();
  });

  it("projects known metadata over summaries and keeps PR branch identities exact", () => {
    render(
      <SelectedResourceHeader
        item={{ ...fixtureItem, is_draft: true }}
        metadata={fixtureMetadata()}
      />,
    );
    expect(
      screen.getByRole("heading", { name: "Authoritative detail title" }),
    ).toBeVisible();
    expect(screen.queryByText(fixtureItem.title)).not.toBeInTheDocument();
    expect(screen.getByText("closed")).toBeVisible();
    expect(screen.getByText("Not a draft")).toBeVisible();
    expect(screen.queryByText("Ready for review")).not.toBeInTheDocument();
    expect(screen.queryByText("Draft")).not.toBeInTheDocument();
    expect(screen.getByText("detail-label")).toBeVisible();
    expect(screen.getByText("@detail-assignee")).toBeVisible();
    expect(screen.getByText("Detail milestone")).toBeVisible();
    expect(screen.getByText("a".repeat(40))).toBeVisible();
    expect(screen.getByText("b".repeat(40))).toBeVisible();
  });

  it("honors known null/empty fields instead of reviving a summary author or draft", () => {
    const metadata = fixtureMetadata();
    metadata.values = {
      ...metadata.values,
      title: null,
      author: null,
      state: null,
      web_url: null,
      labels: [],
      assignees: [],
      milestone: null,
      is_draft: false,
      head: null,
      base: null,
    };
    render(
      <SelectedResourceHeader
        item={{ ...fixtureItem, author: "old-summary-author", is_draft: true }}
        metadata={metadata}
      />,
    );
    expect(
      screen.getByRole("heading", { name: "Untitled resource" }),
    ).toBeVisible();
    expect(screen.getByText("State unavailable")).toBeVisible();
    expect(screen.getByText(/Author unavailable/)).toBeVisible();
    expect(screen.queryByText(/old-summary-author/)).not.toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: /Open/ }),
    ).not.toBeInTheDocument();
    expect(screen.getByText("No labels")).toBeVisible();
    expect(screen.getByText("No assignees")).toBeVisible();
    expect(screen.getByText("No milestone")).toBeVisible();
    expect(screen.queryByText("Draft")).not.toBeInTheDocument();
  });

  it("distinguishes retained omitted/oversized values and unknown fields with truthful validation", () => {
    const metadata = fixtureMetadata();
    metadata.fields = metadata.fields.map((field) =>
      field.field === "labels"
        ? {
            ...field,
            observed_state: "omitted",
            stale_at: "2000-01-01T00:00:00Z",
          }
        : field.field === "assignees"
          ? {
              ...field,
              saved_state: "not_loaded",
              observed_state: "oversized",
              validated_at: null,
            }
          : field,
    );
    render(<SelectedResourceHeader item={fixtureItem} metadata={metadata} />);
    expect(screen.getByText("detail-label")).toBeVisible();
    expect(screen.getByText("Latest provider value omitted")).toBeVisible();
    expect(
      screen.getByText("Latest provider value exceeds the local limit"),
    ).toBeVisible();
    expect(screen.getByText("May be stale")).toBeVisible();
    expect(screen.queryByText("@detail-assignee")).not.toBeInTheDocument();
    expect(screen.getAllByText("No saved value").length).toBeGreaterThan(0);
  });

  it("shares issue metadata while leaving PR-only branch and draft controls out", () => {
    const metadata = fixtureMetadata("issue");
    render(
      <SelectedResourceHeader
        item={{ ...fixtureItem, kind: "issue", is_draft: null }}
        metadata={metadata}
      />,
    );
    const header = within(
      screen.getByRole("banner", { name: "Selected resource metadata" }),
    );
    expect(header.getByText("Detail milestone")).toBeVisible();
    expect(header.queryByText("Head branch / SHA")).not.toBeInTheDocument();
    expect(header.queryByText("Base branch / SHA")).not.toBeInTheDocument();
  });
});
