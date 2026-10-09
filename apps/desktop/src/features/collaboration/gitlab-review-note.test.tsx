import type { GitlabDiscussionNoteV1 } from "@gitru/collaboration-client";
import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { GitlabReviewNote } from "./gitlab-review-note";

const base: GitlabDiscussionNoteV1 = {
  note_type: null,
  system: false,
  individual_note: false,
  resolvable: null,
  resolved_at: null,
  resolved_by: null,
  position: null,
  observed_note_count: 1,
  retained_note_count: 1,
};
describe("GitLab native discussion evidence", () => {
  it("retains unknown resolution and absent position for general notes", () => {
    render(<GitlabReviewNote note={base} />);
    expect(screen.getByText("Resolution support unknown")).toBeTruthy();
    expect(
      screen.getByText("No diff position supplied by GitLab."),
    ).toBeTruthy();
    expect(screen.queryByText("Resolved")).toBeNull();
  });
  it("keeps divergent GitLab base/start/head and marks truncated system notes", () => {
    render(
      <GitlabReviewNote
        note={{
          ...base,
          system: true,
          note_type: "DiffNote",
          resolvable: true,
          observed_note_count: 51,
          retained_note_count: 50,
          position: {
            position_type: "text",
            base_oid: "a".repeat(40),
            start_oid: "b".repeat(40),
            head_oid: "c".repeat(40),
            old_path: "before Δ.rs",
            new_path: "after Δ.rs",
            old_line: null,
            new_line: 8,
            line_range: null,
            width: null,
            height: null,
            x: null,
            y: null,
          },
        }}
      />,
    );
    expect(screen.getByText("System note")).toBeTruthy();
    expect(screen.getByText(/Saved 50 of 51 observed notes/)).toBeTruthy();
    for (const letter of ["a", "b", "c"])
      expect(screen.getByTitle(letter.repeat(40))).toHaveTextContent(
        letter.repeat(12),
      );
    expect(screen.getByText(/before Δ.rs → after Δ.rs/)).toBeTruthy();
    expect(
      screen.getByText(
        /do not establish an approval or resolution for the current head/,
      ),
    ).toBeTruthy();
  });
  it("displays native decimal image positions without inventing a common line anchor", () => {
    render(
      <GitlabReviewNote
        note={{
          ...base,
          individual_note: true,
          resolvable: false,
          position: {
            position_type: "image",
            base_oid: null,
            start_oid: null,
            head_oid: null,
            old_path: null,
            new_path: "image.png",
            old_line: null,
            new_line: null,
            line_range: null,
            width: 1920,
            height: 1080,
            x: "12.75",
            y: "-0.5",
          },
        }}
      />,
    );
    expect(screen.getByText("Individual note")).toBeTruthy();
    expect(screen.getByText("Not resolvable in GitLab")).toBeTruthy();
    expect(
      screen.getByText("Image position: x 12.75, y -0.5; size 1920 × 1080"),
    ).toBeTruthy();
    expect(screen.queryByText(/Old line:/)).toBeNull();
  });
});
