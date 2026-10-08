import {
  type ContextFacetCapability,
  collaboration,
  collaborationErrorMessage,
  type LocalCloneRecord,
  type PullFile,
  type PullFileArtifact,
  type PullFileCompleteness,
  type PullFileContext,
  type PullFileDiffRequest,
  type PullFileSnapshot,
  type RemoteAccount,
  type ReviewDraftAnchorSelection,
} from "@gitru/collaboration-client";
import {
  pullFileArtifactQueryOptions,
  pullFilesQueryOptions,
  useLocalClones,
  useVisibleDemand,
} from "@gitru/collaboration-client/react";
import { Badge } from "@gitru/ui/components/badge";
import { Button } from "@gitru/ui/components/button";
import {
  Collapsible,
  CollapsiblePanel,
  CollapsibleTrigger,
} from "@gitru/ui/components/collapsible";
import {
  Dialog,
  DialogClose,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogPanel,
  DialogPopup,
  DialogTitle,
  DialogTrigger,
} from "@gitru/ui/components/dialog";
import { Input } from "@gitru/ui/components/input";
import { cn } from "@gitru/ui/lib/utils";
import { type SelectedLineRange } from "@pierre/diffs";
import { PatchDiff } from "@pierre/diffs/react";
import { useQuery } from "@tanstack/react-query";
import { useVirtualizer } from "@tanstack/react-virtual";
import { ChevronDown, FileDiff, HardDrive } from "lucide-react";
import { useTheme } from "next-themes";
import {
  Component,
  type ComponentPropsWithRef,
  type KeyboardEvent,
  type ReactNode,
  useId,
  useRef,
  useState,
} from "react";
import { useDiffViewerSettings } from "@/components/diff/use-diff-view-setting-store";
import {
  CapabilityBoundary,
  ReadOnlyCapability,
  SynchronizationAvailability,
} from "./capability-boundary";
import {
  canMaintainDemand,
  canReadSaved,
  canSynchronize,
  dispatchCapabilityIntent,
} from "./capability-policy";
import { localLinkStateLabel } from "./local-repository-links";
import { useInlineReviewAuthoring } from "./review-submission";

const PAGE_LIMIT = 100;
const CURSOR_LIMIT = 100;
const VIRTUALIZE_AFTER = 40;
const FILE_ROW_HEIGHT = 64;

type Props = {
  account: RemoteAccount;
  subjectId: string;
  instanceId: string;
  repositoryId: string | null;
  policy: ContextFacetCapability | undefined;
};

export function PullFilesPanel(props: Props) {
  const { account, subjectId, policy } = props;
  const [open, setOpen] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function synchronize(recheck = false) {
    setBusy(true);
    setError(null);
    try {
      await dispatchCapabilityIntent(
        policy,
        recheck ? "recheck_access" : "synchronize",
        () =>
          collaboration
            .forAccount(account)
            .hydrateDetail({ subject_id: subjectId, facet: "files" }),
      );
    } catch (failure) {
      setError(collaborationErrorMessage(failure));
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className="border-t pt-4" aria-label="Files">
      <Collapsible open={open} onOpenChange={setOpen}>
        <div className="flex flex-wrap items-center justify-between gap-2">
          <CollapsibleTrigger
            type="button"
            className="flex items-center gap-2 text-sm font-medium"
          >
            <ChevronDown
              aria-hidden="true"
              className={open ? "size-4 rotate-180" : "size-4"}
            />
            Files
          </CollapsibleTrigger>
          <ReadOnlyCapability policy={policy} />
        </div>
        <CollapsiblePanel className="motion-reduce:transition-none">
          {open ? (
            <div className="flex flex-col gap-3 pt-3">
              <SynchronizationAvailability policy={policy} />
              <CapabilityBoundary
                policy={policy}
                busy={busy}
                recheck={() => {
                  void synchronize(true);
                }}
              >
                {canReadSaved(policy) ? (
                  <>
                    <Button
                      type="button"
                      size="sm"
                      variant="ghost"
                      disabled={busy || !canSynchronize(policy)}
                      onClick={() => {
                        void synchronize();
                      }}
                    >
                      Sync files
                    </Button>
                    <ShownPullFiles {...props} />
                  </>
                ) : null}
              </CapabilityBoundary>
              {error ? (
                <p role="alert" className="text-xs text-destructive-foreground">
                  {error}
                </p>
              ) : null}
            </div>
          ) : null}
        </CollapsiblePanel>
      </Collapsible>
    </section>
  );
}

function ShownPullFiles(props: Props) {
  const { account, subjectId, policy } = props;
  const [pages, setPages] = useState<{
    cursors: Array<string | null>;
    position: number;
  }>({ cursors: [null], position: 0 });
  const cursor = pages.cursors[pages.position];
  const query = useQuery(
    pullFilesQueryOptions(account, {
      subject_id: subjectId,
      cursor,
      limit: PAGE_LIMIT,
    }),
  );
  const demandError = useVisibleDemand({
    account,
    target: {
      kind: "detail",
      repository_id: null,
      subject_id: subjectId,
      facet: "files",
    },
    enabled: account.state === "active" && canMaintainDemand(policy),
  });
  const next = query.data?.next_cursor ?? null;
  const capReached = pages.position + 1 >= CURSOR_LIMIT;
  const repeatedCursor =
    next !== null &&
    (next === cursor ||
      (pages.cursors.includes(next) &&
        pages.cursors[pages.position + 1] !== next));
  const canNext =
    !query.isPending &&
    !query.isError &&
    next !== null &&
    !capReached &&
    !repeatedCursor;

  function moveTo(position: number, continuation?: string) {
    if (continuation) {
      setPages({
        cursors: [...pages.cursors.slice(0, pages.position + 1), continuation],
        position,
      });
      return;
    }
    setPages({ ...pages, position });
  }

  return (
    <div className="flex flex-col gap-3">
      <p className="text-xs text-muted-foreground">
        Select a changed file to open its saved diff. Loading a missing selected
        diff from the provider or a linked clone is always explicit.
      </p>
      {query.isPending ? (
        <p role="status" className="text-xs text-muted-foreground">
          Loading saved files…
        </p>
      ) : query.isError ? (
        <div className="flex flex-col items-start gap-2">
          <p role="alert" className="text-xs text-destructive-foreground">
            {collaborationErrorMessage(query.error)}
          </p>
          {pages.position > 0 ? (
            <Button
              type="button"
              size="sm"
              variant="outline"
              onClick={() => setPages({ cursors: [null], position: 0 })}
            >
              Return to first saved page
            </Button>
          ) : null}
        </div>
      ) : query.data ? (
        <>
          <FileEvidence snapshot={query.data} />
          {query.data.context ? (
            <SavedFileContext context={query.data.context} />
          ) : null}
          {query.data.sync.error ? (
            <p role="alert" className="text-xs text-destructive-foreground">
              {collaborationErrorMessage(query.data.sync.error)}
            </p>
          ) : null}
          {query.data.sync.next_retry_at ? (
            <p role="status" className="text-xs text-muted-foreground">
              Sync can resume after{" "}
              <time dateTime={query.data.sync.next_retry_at}>
                {new Date(query.data.sync.next_retry_at).toLocaleString()}
              </time>
              . Saved files remain available.
            </p>
          ) : null}
          <ExactPullFilesView
            key={exactSnapshotKey(account, query.data)}
            {...props}
            snapshot={query.data}
          />
          <nav
            aria-label="Saved file pages"
            className="flex flex-wrap items-center gap-2"
          >
            <Button
              type="button"
              size="sm"
              variant="outline"
              disabled={pages.position === 0}
              onClick={() => moveTo(pages.position - 1)}
            >
              Previous saved files
            </Button>
            <p className="text-xs text-muted-foreground">
              Saved page {pages.position + 1}
            </p>
            <Button
              type="button"
              size="sm"
              variant="outline"
              disabled={!canNext}
              onClick={() => {
                if (next) moveTo(pages.position + 1, next);
              }}
            >
              Next saved files
            </Button>
          </nav>
          {next && capReached ? (
            <div className="flex flex-wrap items-center gap-2">
              <p className="text-xs text-muted-foreground">
                This view can browse up to 100 saved pages.
              </p>
              <Button
                type="button"
                size="sm"
                variant="outline"
                onClick={() => setPages({ cursors: [null], position: 0 })}
              >
                Return to first saved page
              </Button>
            </div>
          ) : null}
          {repeatedCursor ? (
            <div className="flex flex-wrap items-center gap-2">
              <p role="alert" className="text-xs text-destructive-foreground">
                The saved continuation did not advance.
              </p>
              <Button
                type="button"
                size="sm"
                variant="outline"
                onClick={() => setPages({ cursors: [null], position: 0 })}
              >
                Return to first saved page
              </Button>
            </div>
          ) : null}
        </>
      ) : null}
      {demandError ? (
        <p role="alert" className="text-xs text-destructive-foreground">
          {collaborationErrorMessage(demandError)}
        </p>
      ) : null}
    </div>
  );
}

function ExactPullFilesView({
  account,
  subjectId,
  instanceId,
  repositoryId,
  policy,
  snapshot,
}: Props & { snapshot: PullFileSnapshot }) {
  const [selectedKey, setSelectedKey] = useState<string | null>(null);
  const selected = snapshot.files.find((file) => file.file_key === selectedKey);
  const exact = snapshot.context && snapshot.facet_revision;

  if (!snapshot.files.length) {
    return (
      <p className="text-xs text-muted-foreground">
        {emptyMessage(snapshot.completeness)}
      </p>
    );
  }

  return (
    <div className="grid min-w-0 gap-3 lg:grid-cols-[minmax(14rem,0.75fr)_minmax(0,1.25fr)]">
      <PullFileList
        files={snapshot.files}
        selectedKey={selectedKey}
        onSelect={setSelectedKey}
        disabled={!exact}
      />
      <section
        aria-label="Selected file diff"
        className="min-w-0 rounded-md border bg-muted/10 p-3"
      >
        {!exact ? (
          <p className="text-xs text-muted-foreground">
            Refresh this changed-file list before opening its saved diffs.
          </p>
        ) : selected ? (
          <SelectedPullFile
            account={account}
            subjectId={subjectId}
            instanceId={instanceId}
            repositoryId={repositoryId}
            policy={policy}
            context={snapshot.context as PullFileContext}
            facetRevision={snapshot.facet_revision as string}
            file={selected}
          />
        ) : (
          <p className="text-xs text-muted-foreground">
            Choose a saved file to open its cached diff.
          </p>
        )}
      </section>
    </div>
  );
}

function PullFileList({
  files,
  selectedKey,
  onSelect,
  disabled,
}: {
  files: PullFile[];
  selectedKey: string | null;
  onSelect: (key: string) => void;
  disabled: boolean;
}) {
  const buttons = useRef<Array<HTMLButtonElement | null>>([]);
  if (files.length > VIRTUALIZE_AFTER) {
    return (
      <VirtualPullFileList
        files={files}
        selectedKey={selectedKey}
        onSelect={onSelect}
        disabled={disabled}
      />
    );
  }
  return (
    <ol
      aria-label="Saved pull request files"
      className="flex min-w-0 flex-col gap-1"
    >
      {files.map((file, index) => (
        <li key={file.file_key}>
          <PullFileButton
            ref={(node) => {
              buttons.current[index] = node;
            }}
            file={file}
            selected={file.file_key === selectedKey}
            tabIndex={
              selectedKey
                ? file.file_key === selectedKey
                  ? 0
                  : -1
                : index === 0
                  ? 0
                  : -1
            }
            disabled={disabled}
            onClick={() => onSelect(file.file_key)}
            onKeyDown={(event) =>
              navigateFiles(event, index, files, onSelect, (nextIndex) => {
                buttons.current[nextIndex]?.focus();
              })
            }
          />
        </li>
      ))}
    </ol>
  );
}

function VirtualPullFileList({
  files,
  selectedKey,
  onSelect,
  disabled,
}: {
  files: PullFile[];
  selectedKey: string | null;
  onSelect: (key: string) => void;
  disabled: boolean;
}) {
  const parent = useRef<HTMLDivElement>(null);
  const virtualizer = useVirtualizer({
    count: files.length,
    getScrollElement: () => parent.current,
    estimateSize: () => FILE_ROW_HEIGHT,
    overscan: 10,
    getItemKey: (index) => files[index].file_key,
  });
  const listId = useId();
  function focusAt(index: number) {
    virtualizer.scrollToIndex(index, { align: "auto" });
    requestAnimationFrame(() => {
      parent.current
        ?.querySelector<HTMLButtonElement>(`[data-file-index="${index}"]`)
        ?.focus();
    });
  }
  return (
    <div
      ref={parent}
      role="region"
      aria-label="Saved pull request files"
      className="h-80 min-w-0 overflow-auto rounded-md border"
    >
      <ol
        className="relative w-full"
        style={{ height: `${virtualizer.getTotalSize()}px` }}
      >
        {virtualizer.getVirtualItems().map((row) => {
          const file = files[row.index];
          return (
            <li
              key={row.key}
              aria-posinset={row.index + 1}
              aria-setsize={files.length}
              className="absolute left-0 top-0 w-full p-0.5"
              style={{
                height: `${row.size}px`,
                transform: `translateY(${row.start}px)`,
              }}
            >
              <PullFileButton
                id={`${listId}-${row.index}`}
                data-file-index={row.index}
                file={file}
                selected={file.file_key === selectedKey}
                tabIndex={
                  selectedKey
                    ? file.file_key === selectedKey
                      ? 0
                      : -1
                    : row.index === 0
                      ? 0
                      : -1
                }
                disabled={disabled}
                onClick={() => onSelect(file.file_key)}
                onKeyDown={(event) =>
                  navigateFiles(event, row.index, files, onSelect, focusAt)
                }
              />
            </li>
          );
        })}
      </ol>
    </div>
  );
}

const PullFileButton = function PullFileButton({
  file,
  selected,
  ...props
}: ComponentPropsWithRef<"button"> & {
  file: PullFile;
  selected: boolean;
}) {
  const oldPath = file.file.identity.old_path;
  const newPath = file.file.identity.new_path;
  const path = newPath ?? oldPath ?? "Unnamed file";
  const renamed = oldPath && newPath && oldPath !== newPath;
  return (
    <button
      type="button"
      aria-pressed={selected}
      className={cn(
        "flex min-h-14 w-full min-w-0 items-center justify-between gap-3 rounded-md border px-3 py-2 text-left outline-none transition-colors focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-1 disabled:pointer-events-none disabled:opacity-64",
        selected
          ? "border-primary bg-accent"
          : "bg-background hover:bg-accent/50",
      )}
      {...props}
    >
      <span className="min-w-0">
        <span className="block truncate text-sm font-medium" title={path}>
          {path}
        </span>
        {renamed ? (
          <span
            className="block truncate text-xs text-muted-foreground"
            title={oldPath}
          >
            from {oldPath}
          </span>
        ) : null}
      </span>
      <span className="flex shrink-0 items-center gap-2">
        <FileCounts file={file} />
        <Badge size="sm" variant="outline">
          {changeKindLabel(file.file.change_kind)}
        </Badge>
      </span>
    </button>
  );
};

function navigateFiles(
  event: KeyboardEvent<HTMLButtonElement>,
  index: number,
  files: PullFile[],
  onSelect: (key: string) => void,
  focusAt: (index: number) => void,
) {
  let next: number | null = null;
  if (event.key === "ArrowDown") next = Math.min(index + 1, files.length - 1);
  else if (event.key === "ArrowUp") next = Math.max(index - 1, 0);
  else if (event.key === "Home") next = 0;
  else if (event.key === "End") next = files.length - 1;
  else if (event.key === "Enter") next = index;
  if (next === null) return;
  event.preventDefault();
  onSelect(files[next].file_key);
  focusAt(next);
}

function SelectedPullFile({
  account,
  subjectId,
  instanceId,
  repositoryId,
  policy,
  context,
  facetRevision,
  file,
}: {
  account: RemoteAccount;
  subjectId: string;
  instanceId: string;
  repositoryId: string | null;
  policy: ContextFacetCapability | undefined;
  context: PullFileContext;
  facetRevision: string;
  file: PullFile;
}) {
  const request = {
    subject_id: subjectId,
    file_facet_revision: facetRevision,
    context,
    file_key: file.file_key,
  } satisfies Omit<PullFileDiffRequest, "account_id" | "authorization_epoch">;
  const query = useQuery(pullFileArtifactQueryOptions(account, request));
  const selectedSnapshot =
    query.data && matchesSelectedRequest(query.data.request, request, account)
      ? query.data
      : null;
  const snapshotMismatch =
    query.data !== undefined && selectedSnapshot === null;
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  async function hydrate() {
    setBusy(true);
    setMessage(null);
    setError(null);
    try {
      await collaboration.forAccount(account).hydratePullFile(request);
      setMessage(
        "Diff hydration was requested. The saved result will appear here when ready.",
      );
    } catch (failure) {
      setError(collaborationErrorMessage(failure));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="flex min-w-0 flex-col gap-3">
      <div className="flex min-w-0 flex-wrap items-center justify-between gap-2">
        <h4 className="min-w-0 break-all text-sm font-medium">
          {file.file.identity.new_path ??
            file.file.identity.old_path ??
            "Saved file"}
        </h4>
        {selectedSnapshot?.freshness === "stale" ? (
          <Badge size="sm" variant="outline">
            Saved diff may be stale
          </Badge>
        ) : null}
      </div>
      {query.isPending ? (
        <p role="status" className="text-xs text-muted-foreground">
          Loading the saved diff…
        </p>
      ) : query.isError ? (
        <p role="alert" className="text-xs text-destructive-foreground">
          {collaborationErrorMessage(query.error)}
        </p>
      ) : snapshotMismatch ? (
        <p role="alert" className="text-xs text-destructive-foreground">
          This saved file selection changed. Choose the file again.
        </p>
      ) : selectedSnapshot ? (
        <ArtifactBody
          artifact={selectedSnapshot.artifact}
          reviewSource={
            account.state === "active" &&
            account.provider === "github" &&
            account.host === "github.com" &&
            selectedSnapshot.freshness !== "stale" &&
            selectedSnapshot.artifact?.validation?.kind === "provider"
              ? {
                  file_facet_revision: facetRevision,
                  context,
                  file_key: file.file_key,
                }
              : undefined
          }
        />
      ) : null}
      <div className="flex flex-wrap gap-2">
        <Button
          type="button"
          size="sm"
          variant="outline"
          disabled={busy || !canSynchronize(policy)}
          onClick={() => {
            void hydrate();
          }}
        >
          <FileDiff aria-hidden="true" />
          Load from provider
        </Button>
        {repositoryId ? (
          <LoadLocalFileDialog
            account={account}
            instanceId={instanceId}
            repositoryId={repositoryId}
            sourceRepositoryProviderId={context.source_repository_provider_id}
            request={request}
            onLoaded={() => query.refetch()}
          />
        ) : null}
      </div>
      {message ? (
        <p role="status" className="text-xs text-muted-foreground">
          {message}
        </p>
      ) : null}
      {error ? (
        <p role="alert" className="text-xs text-destructive-foreground">
          {error}
        </p>
      ) : null}
    </div>
  );
}

type ReviewSource = Pick<
  ReviewDraftAnchorSelection,
  "file_facet_revision" | "context" | "file_key"
>;
function ArtifactBody({
  artifact,
  reviewSource,
}: {
  artifact: PullFileArtifact | null;
  reviewSource?: ReviewSource;
}) {
  if (!artifact) {
    return (
      <p className="text-xs text-muted-foreground">
        This file diff is not saved on this device yet.
      </p>
    );
  }
  const provenance =
    artifact.validation?.kind === "local_exact_range"
      ? "Read from the linked clone at the saved comparison"
      : artifact.validation?.kind === "provider"
        ? "Validated by the provider"
        : "No validation evidence is saved";
  return (
    <div className="flex min-w-0 flex-col gap-2">
      <p className="text-xs text-muted-foreground">{provenance}</p>
      <ArtifactContent artifact={artifact} reviewSource={reviewSource} />
    </div>
  );
}

function ArtifactContent({
  artifact,
  reviewSource,
}: {
  artifact: PullFileArtifact;
  reviewSource?: ReviewSource;
}) {
  switch (artifact.content_state) {
    case "text":
      return artifact.unified_text === "" ? (
        <p className="text-xs text-muted-foreground">
          The saved textual diff is empty.
        </p>
      ) : artifact.unified_text ? (
        <DiffRenderBoundary
          key={`${artifact.generation}:${artifact.file_key}:${artifact.last_access_revision}`}
        >
          <CachedPatch artifact={artifact} reviewSource={reviewSource} />
        </DiffRenderBoundary>
      ) : (
        <p className="text-xs text-muted-foreground">
          The saved text artifact has no renderable content.
        </p>
      );
    case "binary":
      return (
        <ArtifactNotice>
          Binary content is saved without a text diff.
        </ArtifactNotice>
      );
    case "image":
      return (
        <ArtifactNotice>
          An image artifact is saved; image preview is unavailable here.
        </ArtifactNotice>
      );
    case "omitted":
      return artifact.source?.strategy === "local_exact_range" &&
        artifact.binary_hint.state === "known" &&
        artifact.binary_hint.value ? (
        <ArtifactNotice>
          The linked clone reported binary content; no blob bytes were saved.
        </ArtifactNotice>
      ) : (
        <ArtifactNotice>
          The source omitted content for this file.
        </ArtifactNotice>
      );
    case "oversized":
      return (
        <ArtifactNotice>
          This diff exceeds Gitru&apos;s local text limit.
        </ArtifactNotice>
      );
    case "unsupported":
      return (
        <ArtifactNotice>
          This source cannot provide a safe diff for the file.
        </ArtifactNotice>
      );
    case "unavailable":
      return (
        <ArtifactNotice>The current file diff is unavailable.</ArtifactNotice>
      );
    case "not_loaded":
      return (
        <ArtifactNotice>This file diff has not been loaded.</ArtifactNotice>
      );
  }
}

function CachedPatch({
  artifact,
  reviewSource,
}: {
  artifact: PullFileArtifact;
  reviewSource?: ReviewSource;
}) {
  const authoring = useInlineReviewAuthoring();
  const [selected, setSelected] = useState<SelectedLineRange | null>(null);
  const id = useId();
  const canReview = authoring !== null && reviewSource !== undefined;
  const sameSide =
    selected?.side !== undefined &&
    (selected.endSide ?? selected.side) === selected.side;
  const canAdd =
    canReview &&
    selected !== null &&
    sameSide &&
    Number.isSafeInteger(selected.start) &&
    Number.isSafeInteger(selected.end) &&
    selected.start > 0 &&
    selected.end > 0;
  function addComment() {
    if (!canAdd || !selected || !authoring || !reviewSource) return;
    const side = selected.side === "deletions" ? "left" : "right";
    const start = Math.min(selected.start, selected.end);
    const end = Math.max(selected.start, selected.end);
    authoring.add({
      anchor: {
        ...reviewSource,
        line: end,
        side,
        start_line: start === end ? null : start,
        start_side: start === end ? null : side,
      },
      location: `${side === "left" ? artifact.identity.old_path : artifact.identity.new_path}, ${side} ${start === end ? `line ${end}` : `lines ${start}–${end}`}`,
    });
  }
  const { theme } = useTheme();
  const { diffStyle, overflow } = useDiffViewerSettings();
  const patch = renderablePatch(artifact);
  return (
    <div
      className={cn(
        "max-h-[42rem] w-full overflow-auto rounded-md",
        theme?.startsWith("dark-") ? "bg-black" : "bg-secondary",
      )}
    >
      {canReview ? (
        <div className="space-y-2 border-b p-3 text-xs">
          <p>
            Select lines in this provider diff to add a review comment. You can
            also enter a line below; Gitru verifies it against the saved diff
            when you save.
          </p>
          <div className="flex flex-wrap items-end gap-2">
            <label htmlFor={`${id}-line`}>
              Line
              <Input
                id={`${id}-line`}
                className="w-24"
                type="number"
                min={1}
                value={selected?.end ?? ""}
                onChange={(event) => {
                  const line = Number(event.target.value);
                  setSelected({
                    start: line,
                    end: line,
                    side: selected?.side ?? "additions",
                  });
                }}
              />
            </label>
            <Button
              type="button"
              size="sm"
              variant="outline"
              aria-pressed={selected?.side === "deletions"}
              onClick={() =>
                setSelected((value) => ({
                  start: value?.start ?? 1,
                  end: value?.end ?? 1,
                  side: "deletions",
                }))
              }
            >
              Old side
            </Button>
            <Button
              type="button"
              size="sm"
              variant="outline"
              aria-pressed={selected?.side === "additions"}
              onClick={() =>
                setSelected((value) => ({
                  start: value?.start ?? 1,
                  end: value?.end ?? 1,
                  side: "additions",
                }))
              }
            >
              New side
            </Button>
            <Button
              type="button"
              size="sm"
              disabled={!canAdd}
              onClick={addComment}
            >
              Add inline review comment
            </Button>
          </div>
          {selected && !sameSide ? (
            <p role="status">Choose a range on one side of the diff.</p>
          ) : null}
        </div>
      ) : null}
      <PatchDiff
        patch={patch}
        className="w-full"
        renderCustomHeader={() => (
          <div className="break-all border-b bg-muted/30 px-3 py-2 text-xs font-medium">
            {artifact.identity.new_path ??
              artifact.identity.old_path ??
              "Saved file"}
          </div>
        )}
        options={{
          themeType: theme?.startsWith("dark-") ? "dark" : "light",
          diffStyle,
          overflow,
          collapsedContextThreshold: 0,
          lineHoverHighlight: "both",
          enableLineSelection: canReview,
          onLineSelected: canReview ? setSelected : undefined,
          onLineNumberClick: canReview
            ? (line) =>
                setSelected({
                  start: line.lineNumber,
                  end: line.lineNumber,
                  side: line.annotationSide,
                })
            : undefined,
        }}
      />
    </div>
  );
}

/**
 * GitHub and GitLab can save an exact selected hunk without file headers.
 * Supply display headers from the already fenced membership identity; provider
 * patch headers remain untrusted text and never select a file or repository.
 */
function renderablePatch(artifact: PullFileArtifact) {
  const text = artifact.unified_text ?? "";
  if (text.startsWith("diff --git ")) return text;
  const oldPath = artifact.identity.old_path;
  const newPath = artifact.identity.new_path;
  const oldLabel = oldPath ? `a/${oldPath}` : "/dev/null";
  const newLabel = newPath ? `b/${newPath}` : "/dev/null";
  const identityHeader = `diff --git ${oldLabel} ${newLabel}\n`;
  if (text.startsWith("--- ")) return `${identityHeader}${text}`;
  return `${identityHeader}--- ${oldLabel}\n+++ ${newLabel}\n${text}`;
}

class DiffRenderBoundary extends Component<
  { children: ReactNode },
  { failed: boolean }
> {
  state = { failed: false };
  static getDerivedStateFromError() {
    return { failed: true };
  }
  render() {
    return this.state.failed ? (
      <p role="alert" className="text-xs text-destructive-foreground">
        The saved diff could not be rendered safely.
      </p>
    ) : (
      this.props.children
    );
  }
}

function ArtifactNotice({ children }: { children: ReactNode }) {
  return <p className="text-xs text-muted-foreground">{children}</p>;
}

function LoadLocalFileDialog({
  account,
  instanceId,
  repositoryId,
  sourceRepositoryProviderId,
  request,
  onLoaded,
}: {
  account: RemoteAccount;
  instanceId: string;
  repositoryId: string;
  sourceRepositoryProviderId: string;
  request: Omit<PullFileDiffRequest, "account_id" | "authorization_epoch">;
  onLoaded: () => Promise<unknown>;
}) {
  const [open, setOpen] = useState(false);
  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogTrigger
        render={<Button type="button" size="sm" variant="outline" />}
      >
        <HardDrive aria-hidden="true" />
        Read linked clone
      </DialogTrigger>
      <DialogPopup>
        <DialogHeader>
          <DialogTitle>Read file diff locally</DialogTitle>
          <DialogDescription>
            Choose an explicitly linked clone. Gitru only reads the saved pull
            request comparison and will not fetch or change the clone.
          </DialogDescription>
        </DialogHeader>
        <DialogPanel>
          {open ? (
            <LocalFileClonePicker
              account={account}
              instanceId={instanceId}
              repositoryId={repositoryId}
              sourceRepositoryProviderId={sourceRepositoryProviderId}
              request={request}
              onLoaded={async () => {
                await onLoaded();
                setOpen(false);
              }}
            />
          ) : null}
        </DialogPanel>
        <DialogFooter>
          <DialogClose render={<Button type="button" variant="ghost" />}>
            Cancel
          </DialogClose>
        </DialogFooter>
      </DialogPopup>
    </Dialog>
  );
}

function LocalFileClonePicker({
  account,
  instanceId,
  repositoryId,
  sourceRepositoryProviderId,
  request,
  onLoaded,
}: {
  account: RemoteAccount;
  instanceId: string;
  repositoryId: string;
  sourceRepositoryProviderId: string;
  request: Omit<PullFileDiffRequest, "account_id" | "authorization_epoch">;
  onLoaded: () => Promise<void>;
}) {
  const query = useLocalClones(
    account,
    instanceId,
    repositoryId,
    sourceRepositoryProviderId,
  );
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function choose(clone: LocalCloneRecord) {
    setBusy(true);
    setError(null);
    try {
      const snapshot = await collaboration
        .forAccount(account)
        .loadLocalPullFile({
          ...request,
          local_repository_id: clone.local_repository_id,
          link_id: clone.link_id,
          link_generation: clone.generation,
        });
      if (!matchesSelectedRequest(snapshot.request, request, account)) {
        throw { code: "stale_view" };
      }
      await onLoaded();
    } catch (failure) {
      setError(collaborationErrorMessage(failure));
    } finally {
      setBusy(false);
    }
  }

  return (
    <section
      className="flex flex-col gap-3"
      aria-label="Linked clones for saved file"
    >
      {query.isPending ? (
        <p role="status" className="text-sm text-muted-foreground">
          Inspecting linked local clones…
        </p>
      ) : null}
      {query.isError ? (
        <p role="alert" className="text-sm text-destructive-foreground">
          {collaborationErrorMessage(query.error)}
        </p>
      ) : null}
      {error ? (
        <p role="alert" className="text-sm text-destructive-foreground">
          {error}
        </p>
      ) : null}
      {query.data?.clones.map((clone) => (
        <article
          key={clone.link_id}
          className="flex flex-col gap-2 rounded-md border p-3"
        >
          <p className="break-all text-sm">
            {clone.local_repository_name ?? "Missing local registration"}
          </p>
          <p className="text-xs text-muted-foreground">
            {localLinkStateLabel[clone.state]}
          </p>
          <Button
            type="button"
            size="sm"
            disabled={busy || clone.state !== "linked"}
            onClick={() => {
              void choose(clone);
            }}
          >
            Read this clone
          </Button>
        </article>
      ))}
      {query.data && !query.data.clones.length ? (
        <p className="text-sm text-muted-foreground">
          No saved local clone is linked. Open the local Git repository and
          choose Linked collaboration.
        </p>
      ) : null}
      <Button
        type="button"
        size="sm"
        variant="outline"
        disabled={busy || query.isFetching}
        onClick={() => {
          void query.refetch();
        }}
      >
        Inspect clones again
      </Button>
    </section>
  );
}

function FileEvidence({ snapshot }: { snapshot: PullFileSnapshot }) {
  const capRemoteHasMore = snapshot.completeness.cap?.remote_has_more;
  return (
    <div className="flex flex-wrap gap-2" aria-label="Saved file evidence">
      {snapshot.freshness === "stale" ? (
        <Badge size="sm" variant="outline">
          Saved list may be stale
        </Badge>
      ) : null}
      {snapshot.sync.state === "syncing" ? (
        <Badge size="sm" variant="outline">
          Updating
        </Badge>
      ) : null}
      {snapshot.sync.state === "offline" ? (
        <Badge size="sm" variant="outline">
          Offline · showing saved files
        </Badge>
      ) : null}
      {snapshot.sync.state === "rate_limited" ? (
        <Badge size="sm" variant="outline">
          Update paused by the provider
        </Badge>
      ) : null}
      {snapshot.sync.state === "auth_required" ? (
        <Badge size="sm" variant="outline">
          Reconnect to update files
        </Badge>
      ) : null}
      {snapshot.completeness.state === "partial" ? (
        <Badge size="sm" variant="outline">
          Partial file list
        </Badge>
      ) : null}
      {snapshot.completeness.state === "capped" ? (
        <Badge size="sm" variant="outline">
          {snapshot.completeness.cap?.provenance === "provider"
            ? "Provider file limit reached"
            : "Local file limit reached"}
        </Badge>
      ) : null}
      {snapshot.coverage.remote_has_more ||
      (capRemoteHasMore?.state === "known" && capRemoteHasMore.value) ? (
        <Badge size="sm" variant="outline">
          More files exist remotely
        </Badge>
      ) : snapshot.completeness.state === "capped" &&
        capRemoteHasMore?.state === "unknown" ? (
        <Badge size="sm" variant="outline">
          More files may exist remotely
        </Badge>
      ) : null}
    </div>
  );
}

function SavedFileContext({ context }: { context: PullFileContext }) {
  return (
    <section
      aria-label="Saved file context"
      className="rounded-md border bg-muted/30 p-3"
    >
      <dl className="grid grid-cols-[auto_minmax(0,1fr)] gap-x-4 gap-y-2 text-xs">
        <ContextValue label="Base commit" value={context.base_oid} shorten />
        <ContextValue label="Head commit" value={context.head_oid} shorten />
      </dl>
    </section>
  );
}

function ContextValue({
  label,
  value,
  shorten = false,
}: {
  label: string;
  value: string;
  shorten?: boolean;
}) {
  return (
    <div className="contents">
      <dt className="text-muted-foreground">{label}</dt>
      <dd className="min-w-0 break-all">
        <code
          aria-label={shorten ? `${label}: ${value}` : undefined}
          title={shorten ? value : undefined}
        >
          {shorten ? value.slice(0, 12) : value}
        </code>
      </dd>
    </div>
  );
}

function FileCounts({ file }: { file: PullFile }) {
  const additions = knownCount(file.file.additions);
  const deletions = knownCount(file.file.deletions);
  if (additions === null && deletions === null) return null;
  return (
    <span
      className="text-xs"
      aria-label={`${additions ?? "unknown"} additions, ${deletions ?? "unknown"} deletions`}
    >
      {additions !== null ? (
        <span className="text-success-foreground">+{additions}</span>
      ) : null}{" "}
      {deletions !== null ? (
        <span className="text-destructive-foreground">−{deletions}</span>
      ) : null}
    </span>
  );
}

function knownCount(count: PullFile["file"]["additions"]) {
  return count.state === "known" ? count.value : null;
}

function changeKindLabel(kind: PullFile["file"]["change_kind"]) {
  return kind.replace(/_/g, " ");
}

function emptyMessage(completeness: PullFileCompleteness) {
  switch (completeness.state) {
    case "complete":
      return "The provider returned no files for this saved range.";
    case "syncing":
      return "File synchronization is in progress.";
    case "missing":
      return "Files have not been saved on this device yet.";
    default:
      return "No files are available in this saved partial view.";
  }
}

function exactSnapshotKey(account: RemoteAccount, snapshot: PullFileSnapshot) {
  return JSON.stringify([
    account.id,
    account.authorization_epoch,
    snapshot.subject_id,
    snapshot.facet_revision,
    snapshot.context,
  ]);
}

function matchesSelectedRequest(
  returned: PullFileDiffRequest,
  expected: Omit<PullFileDiffRequest, "account_id" | "authorization_epoch">,
  account: RemoteAccount,
) {
  return (
    returned.account_id === account.id &&
    returned.authorization_epoch === account.authorization_epoch &&
    returned.subject_id === expected.subject_id &&
    returned.file_facet_revision === expected.file_facet_revision &&
    returned.file_key === expected.file_key &&
    returned.context.base_oid === expected.context.base_oid &&
    returned.context.head_oid === expected.context.head_oid &&
    returned.context.merge_base_oid === expected.context.merge_base_oid &&
    returned.context.base_repository_provider_id ===
      expected.context.base_repository_provider_id &&
    returned.context.source_repository_provider_id ===
      expected.context.source_repository_provider_id &&
    returned.context.body_metadata_facet_revision ===
      expected.context.body_metadata_facet_revision
  );
}
