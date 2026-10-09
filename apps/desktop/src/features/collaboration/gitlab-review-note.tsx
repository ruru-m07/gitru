import type { GitlabDiscussionNoteV1 } from "@gitru/collaboration-client";
import { Badge } from "@gitru/ui/components/badge";

/** Native GitLab evidence stays separate from a common current/original anchor. */
export function GitlabReviewNote({ note }: { note: GitlabDiscussionNoteV1 }) {
  const position = note.position;
  return (
    <div
      aria-label="GitLab discussion details"
      className="min-w-0 space-y-2 text-xs text-muted-foreground"
    >
      <div className="flex flex-wrap gap-1.5">
        {note.system ? <Badge variant="secondary">System note</Badge> : null}
        {note.individual_note ? (
          <Badge variant="outline">Individual note</Badge>
        ) : null}
        {note.note_type ? <span>{note.note_type}</span> : null}
        <span>
          {note.resolvable === null
            ? "Resolution support unknown"
            : note.resolvable
              ? "Resolvable in GitLab"
              : "Not resolvable in GitLab"}
        </span>
      </div>
      {note.retained_note_count < note.observed_note_count ? (
        <p>
          Saved {note.retained_note_count} of {note.observed_note_count}{" "}
          observed notes in this discussion. The saved discussion is partial.
        </p>
      ) : null}
      {note.resolved_by || note.resolved_at ? (
        <p>
          Resolution reported
          {note.resolved_by
            ? ` by ${note.resolved_by.login ?? note.resolved_by.display_name ?? note.resolved_by.provider_id}`
            : ""}
          {note.resolved_at ? ` at ${note.resolved_at}` : ""}.
        </p>
      ) : null}
      {position ? (
        <div className="min-w-0 space-y-1.5" aria-label="GitLab diff position">
          <p>GitLab {position.position_type} position</p>
          {position.old_path || position.new_path ? (
            <p className="break-all font-mono">
              {position.old_path ?? "Old path unknown"}
              {position.old_path !== position.new_path
                ? ` → ${position.new_path ?? "New path unknown"}`
                : ""}
            </p>
          ) : null}
          {position.old_line !== null || position.new_line !== null ? (
            <p>
              Old line: {position.old_line ?? "unknown"} · New line:{" "}
              {position.new_line ?? "unknown"}
            </p>
          ) : null}
          {position.line_range ? (
            <p className="break-all">
              Native range: {position.line_range.start.kind} (old{" "}
              {position.line_range.start.old_line ?? "unknown"}, new{" "}
              {position.line_range.start.new_line ?? "unknown"}) →{" "}
              {position.line_range.end.kind} (old{" "}
              {position.line_range.end.old_line ?? "unknown"}, new{" "}
              {position.line_range.end.new_line ?? "unknown"})
            </p>
          ) : null}
          {position.width !== null ||
          position.height !== null ||
          position.x !== null ||
          position.y !== null ? (
            <p>
              Image position: x {position.x ?? "unknown"}, y{" "}
              {position.y ?? "unknown"}; size {position.width ?? "unknown"} ×{" "}
              {position.height ?? "unknown"}
            </p>
          ) : null}
          <dl className="grid min-w-0 grid-cols-[auto_minmax(0,1fr)] gap-x-2 gap-y-1">
            {(
              [
                ["Base", position.base_oid],
                ["Start", position.start_oid],
                ["Head", position.head_oid],
              ] as const
            ).map(([name, oid]) => (
              <div className="contents" key={name}>
                <dt>{name}</dt>
                <dd
                  className="min-w-0 truncate font-mono"
                  title={oid ?? undefined}
                >
                  {oid ? oid.slice(0, 12) : "unknown"}
                </dd>
              </div>
            ))}
          </dl>
          <p>
            These references identify GitLab’s saved diff version. They do not
            establish an approval or resolution for the current head.
          </p>
        </div>
      ) : (
        <p>No diff position supplied by GitLab.</p>
      )}
    </div>
  );
}
