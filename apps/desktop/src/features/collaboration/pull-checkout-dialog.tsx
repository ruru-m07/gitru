import {
  collaboration,
  collaborationErrorMessage,
  type LocalCloneRecord,
  type PullCheckoutPlan,
  type RemoteAccount,
  type RemoteItem,
  type ResourceMetadataSnapshot,
} from "@gitru/collaboration-client";
import { useLocalClones } from "@gitru/collaboration-client/react";
import { listRepositories } from "@gitru/commands";
import { Button } from "@gitru/ui/components/button";
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
import { Field, FieldLabel } from "@gitru/ui/components/field";
import { Input } from "@gitru/ui/components/input";
import { useRouter } from "@tanstack/react-router";
import { GitBranch, RefreshCw } from "lucide-react";
import { type FormEvent, useEffect, useId, useState } from "react";
import { useAppStore } from "@/store/use-app-store";
import {
  localLinkStateLabel,
  useLocalLinkIntent,
} from "./local-repository-links";

type CheckoutIdentity = {
  instanceId: string;
  itemId: string;
  repositoryId: string;
};

const noop = () => {};

export function PullRequestCheckoutButton({
  account,
  item,
  instanceId,
  metadata,
}: {
  account: RemoteAccount;
  item: RemoteItem;
  instanceId: string;
  metadata: ResourceMetadataSnapshot | null;
}) {
  const head = metadata?.fields.find((field) => field.field === "head");
  const repositoryId = item.repository_id;
  const available =
    item.kind === "pull_request" &&
    repositoryId !== null &&
    head?.saved_state === "known" &&
    metadata?.values.head !== null;
  const [open, setOpen] = useState(false);
  const [executionLocked, setExecutionLocked] = useState(false);
  if (!available || !repositoryId) return null;
  return (
    <Dialog
      open={open}
      onOpenChange={(next) => {
        if (!executionLocked) setOpen(next);
      }}
    >
      <DialogTrigger
        render={
          <Button type="button" size="sm" variant="outline" className="mt-4" />
        }
      >
        <GitBranch aria-hidden="true" />
        Check out locally
      </DialogTrigger>
      <DialogPopup className="sm:max-w-2xl" showCloseButton={!executionLocked}>
        <DialogHeader>
          <DialogTitle>Check out pull request</DialogTitle>
          <DialogDescription>
            Inspect an existing linked clone, then confirm the exact local Git
            operation. Selecting a clone never fetches or changes its worktree.
          </DialogDescription>
        </DialogHeader>
        <DialogPanel className="max-h-[70vh] overflow-y-auto">
          {open ? (
            <PullCheckoutDialogBody
              key={`${account.id}:${account.actor_id}:${account.authorization_epoch}:${item.id}`}
              account={account}
              instanceId={instanceId}
              itemId={item.id}
              repositoryId={repositoryId}
              onExecutionBusy={setExecutionLocked}
              onCompleted={() => setOpen(false)}
            />
          ) : null}
        </DialogPanel>
        <DialogFooter>
          <DialogClose
            render={
              <Button
                type="button"
                variant="ghost"
                disabled={executionLocked}
              />
            }
          >
            Cancel
          </DialogClose>
        </DialogFooter>
      </DialogPopup>
    </Dialog>
  );
}

export function PullCheckoutDialogBody({
  account,
  instanceId,
  itemId,
  repositoryId,
  onExecutionBusy = noop,
  onCompleted = noop,
  onNavigate,
}: {
  account: RemoteAccount;
  onExecutionBusy?: (busy: boolean) => void;
  onCompleted?: () => void;
  onNavigate?: (localRepositoryId: string) => Promise<void>;
} & CheckoutIdentity) {
  const query = useLocalClones(account, instanceId, repositoryId);
  const registrations = useAppStore((state) => state.repositories);
  const registeredById = new Map(
    registrations.map((repository) => [repository.id, repository]),
  );
  const client = collaboration.forAccount(account);
  const begin = useLocalLinkIntent();
  const router = useRouter({ warn: false });
  const branchId = useId();
  const [selected, setSelected] = useState<LocalCloneRecord | null>(null);
  const [branch, setBranch] = useState("");
  const [plan, setPlan] = useState<PullCheckoutPlan | null>(null);
  const [planning, setPlanning] = useState(false);
  const [executing, setExecuting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState<string | null>(null);
  useEffect(
    () => () => {
      onExecutionBusy(false);
    },
    [onExecutionBusy],
  );

  async function inspect(
    clone: LocalCloneRecord,
    localBranch?: string,
  ): Promise<void> {
    const current = begin();
    setSelected(clone);
    setPlan(null);
    setPlanning(true);
    setError(null);
    setSuccess(null);
    try {
      const next = await client.planPullCheckout({
        instance_id: instanceId,
        subject_id: itemId,
        local_repository_id: clone.local_repository_id,
        link_id: clone.link_id,
        link_generation: clone.generation,
        ...(localBranch ? { local_branch: localBranch } : {}),
      });
      if (!current()) return;
      if (next.local_repository_id !== clone.local_repository_id)
        throw { code: "stale_view" };
      setBranch(next.local_branch);
      setPlan(next);
    } catch (failure) {
      if (current()) setError(checkoutErrorMessage(failure));
    } finally {
      if (current()) setPlanning(false);
    }
  }

  async function execute(): Promise<void> {
    if (!plan?.plan_id || !selected) return;
    const current = begin();
    setExecuting(true);
    onExecutionBusy(true);
    setError(null);
    try {
      let receipt: Awaited<ReturnType<typeof client.executePullCheckout>>;
      try {
        receipt = await client.executePullCheckout(plan.plan_id);
        if (!current()) return;
        if (
          receipt.local_repository_id !== selected.local_repository_id ||
          receipt.branch !== plan.local_branch ||
          receipt.oid !== plan.expected_oid
        )
          throw { code: "stale_view" };
      } catch (failure) {
        if (current()) {
          setPlan(null);
          setError(checkoutErrorMessage(failure));
        }
        return;
      }

      // A verified native receipt is authoritative: the branch is already
      // changed locally. Retire the single-use plan before attempting the
      // separate convenience step of opening that repository.
      setPlan(null);
      if (receipt.git_reported_failure) {
        setSuccess(
          `Checked out ${receipt.branch} at ${shortOid(receipt.oid)}, but Git reported a local checkout warning. Review repository hooks and worktree state before continuing.`,
        );
        return;
      }
      setSuccess(
        `Checked out ${receipt.branch} at ${shortOid(receipt.oid)} successfully.`,
      );
      try {
        if (onNavigate) await onNavigate(receipt.local_repository_id);
        else {
          const repositories = await listRepositories({ refreshStale: false });
          if (!current()) return;
          const repository = repositories.find(
            (candidate) => candidate.id === receipt.local_repository_id,
          );
          if (!repository || !router) throw new Error("repository unavailable");
          const store = useAppStore.getState();
          store.setRepositories(repositories);
          store.syncActiveTab({
            repositoryId: repository.id,
            routePath: "/app/git",
            title: repository.name,
          });
          await router.navigate({ to: "/app/git" });
        }
      } catch {
        if (current()) {
          setError(
            "Checkout succeeded, but Gitru could not open the repository. Open it from the repository list.",
          );
        }
        return;
      }
      if (current()) onCompleted();
    } finally {
      if (current()) {
        setExecuting(false);
        onExecutionBusy(false);
      }
    }
  }

  function replan(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const localBranch = branch.trim();
    if (selected && localBranch) void inspect(selected, localBranch);
  }

  return (
    <section className="space-y-4" aria-label="Pull request checkout plan">
      {query.isPending ? (
        <p role="status" className="text-sm text-muted-foreground">
          Inspecting linked local clones…
        </p>
      ) : null}
      {query.isError ? (
        <p role="alert" className="text-sm text-destructive-foreground">
          {checkoutErrorMessage(query.error)}
        </p>
      ) : null}
      {error ? (
        <p role="alert" className="text-sm text-destructive-foreground">
          {error}
        </p>
      ) : null}
      {success ? (
        <p role="status" className="text-sm">
          {success}
        </p>
      ) : null}
      {!query.isError
        ? query.data?.clones.map((clone, index) => {
            const registration =
              clone.state === "local_repository_missing"
                ? undefined
                : registeredById.get(clone.local_repository_id);
            const descriptionId = `${branchId}-clone-${index}`;
            return (
              <article
                key={clone.link_id}
                className="flex flex-wrap items-center justify-between gap-3 rounded-md border p-3"
                aria-label={`Checkout clone ${clone.local_repository_id}`}
              >
                <div className="min-w-0">
                  <p className="truncate text-sm font-medium">
                    {clone.local_repository_name ??
                      registration?.name ??
                      "Missing local registration"}
                  </p>
                  <p
                    id={descriptionId}
                    className="break-all text-xs text-muted-foreground"
                  >
                    {registration
                      ? `Local path: ${registration.path}`
                      : `Registration: ${clone.local_repository_id}`}
                  </p>
                  <p className="text-xs text-muted-foreground">
                    {localLinkStateLabel[clone.state]}
                  </p>
                </div>
                <Button
                  type="button"
                  size="sm"
                  aria-describedby={descriptionId}
                  variant={
                    selected?.link_id === clone.link_id
                      ? "secondary"
                      : "outline"
                  }
                  disabled={planning || executing || clone.state !== "linked"}
                  onClick={() => {
                    setBranch("");
                    void inspect(clone);
                  }}
                >
                  Inspect {clone.local_repository_name ?? "clone"}
                </Button>
              </article>
            );
          })
        : null}
      {query.data && !query.data.clones.length ? (
        <p className="text-sm text-muted-foreground">
          No saved local clone is linked. Open the local Git repository and
          choose Linked collaboration first.
        </p>
      ) : null}
      {selected ? (
        <form className="space-y-3 rounded-md border p-3" onSubmit={replan}>
          <Field>
            <FieldLabel htmlFor={branchId}>Local branch</FieldLabel>
            <Input
              id={branchId}
              type="text"
              maxLength={1024}
              spellCheck={false}
              value={branch}
              disabled={planning || executing}
              onChange={(event) => {
                setBranch(event.target.value);
                setPlan(null);
                setError(null);
                setSuccess(null);
              }}
            />
          </Field>
          <Button
            type="submit"
            size="sm"
            variant="outline"
            disabled={planning || executing || branch.trim().length === 0}
          >
            {planning ? "Inspecting…" : "Inspect branch"}
          </Button>
        </form>
      ) : null}
      {plan ? <CheckoutPlanSummary plan={plan} /> : null}
      {plan ? (
        <Button
          type="button"
          disabled={executing || !plan.plan_id || !plan.inspection.action}
          onClick={() => {
            void execute();
          }}
        >
          {executing ? "Checking out…" : confirmationLabel(plan)}
        </Button>
      ) : null}
      <Button
        type="button"
        size="sm"
        variant="ghost"
        disabled={planning || executing || query.isFetching}
        onClick={() => {
          setSelected(null);
          setPlan(null);
          setBranch("");
          setError(null);
          setSuccess(null);
          void query.refetch();
        }}
      >
        <RefreshCw aria-hidden="true" />
        Inspect clones again
      </Button>
    </section>
  );
}

function CheckoutPlanSummary({ plan }: { plan: PullCheckoutPlan }) {
  const current = plan.inspection.detached
    ? `Detached at ${shortOid(plan.inspection.current_head_oid)}`
    : (plan.inspection.current_branch ?? "Branch unavailable");
  const action = plan.inspection.action
    ? actionDescription[plan.inspection.action]
    : null;
  const blocker = plan.inspection.blocker
    ? blockerDescription[plan.inspection.blocker]
    : null;
  return (
    <section
      className="space-y-3 rounded-md border bg-muted/30 p-3"
      aria-label="Checkout inspection"
    >
      <dl className="grid gap-3 text-sm sm:grid-cols-2">
        <PlanValue label="Clone" value={plan.local_repository_name} />
        <PlanValue label="Current worktree" value={current} />
        <PlanValue
          label="Pull request source"
          value={`${plan.source_repository}:${plan.source_branch}`}
        />
        <PlanValue label="Fetch remote" value={plan.source_remote} />
        <PlanValue label="Local branch" value={plan.local_branch} />
        <PlanValue
          label="Exact commit"
          value={shortOid(plan.expected_oid)}
          title={plan.expected_oid}
        />
        <PlanValue
          label="Saved head"
          value={
            plan.metadata_stale
              ? "Saved provider value may be stale"
              : plan.metadata_validated_at
                ? `Validated ${formatDate(plan.metadata_validated_at)}`
                : "Validation time unavailable"
          }
        />
      </dl>
      {action ? <p className="text-sm">{action}</p> : null}
      {blocker ? (
        <p role="status" className="text-sm text-destructive-foreground">
          {blocker}
        </p>
      ) : null}
    </section>
  );
}

function PlanValue({
  label,
  value,
  title,
}: {
  label: string;
  value: string;
  title?: string;
}) {
  return (
    <div className="min-w-0">
      <dt className="text-xs font-medium text-muted-foreground">{label}</dt>
      <dd className="break-all" title={title}>
        {value}
      </dd>
    </div>
  );
}

const actionDescription: Record<
  NonNullable<PullCheckoutPlan["inspection"]["action"]>,
  string
> = {
  already_checked_out: "Confirmation verifies the branch and commit again.",
  switch_existing:
    "Confirmation switches to the existing branch at this exact commit.",
  create_branch:
    "Confirmation creates the local branch at this exact local commit and checks it out.",
  fetch_and_create_branch:
    "Confirmation fetches the saved source branch, verifies its exact commit, then creates and checks out the local branch.",
};

const blockerDescription: Record<
  NonNullable<PullCheckoutPlan["inspection"]["blocker"]>,
  string
> = {
  dirty_worktree:
    "This worktree has uncommitted changes. Commit, stash, or discard them, then inspect again.",
  active_operation:
    "Finish the active Git operation in this worktree, then inspect again.",
  existing_branch_diverged:
    "This local branch points to another commit. Choose a different branch name; Gitru will not reset it.",
};

function confirmationLabel(plan: PullCheckoutPlan): string {
  switch (plan.inspection.action) {
    case "fetch_and_create_branch":
      return "Fetch and check out";
    case "create_branch":
      return "Create and check out";
    case "switch_existing":
      return "Switch branch";
    case "already_checked_out":
      return "Verify checkout";
    default:
      return "Checkout blocked";
  }
}

function checkoutErrorMessage(error: unknown): string {
  const code =
    typeof error === "object" && error !== null && "code" in error
      ? error.code
      : undefined;
  switch (code) {
    case "not_found":
      return "The saved pull request or a matching source remote is no longer available. Refresh the pull request and linked clones, then inspect again.";
    case "not_ready":
      return "The linked worktree or saved pull request source is unavailable. Refresh it or choose another clone.";
    case "stale_view":
      return "The pull request head, link, remotes, or worktree changed. Inspect again before checking out.";
    case "busy":
      return "The worktree is busy. Finish its current Git operation, then inspect again.";
    case "network":
      return "Git could not fetch the pull request branch. Check network and Git credentials, then inspect again.";
    case "invalid_input":
      return "Choose a valid local branch name and inspect again.";
    case "local_state_changed":
      return "Git may have changed this worktree without completing the requested checkout. Inspect the local repository before retrying.";
    default:
      return collaborationErrorMessage(error);
  }
}

function shortOid(oid: string): string {
  return oid.slice(0, 12);
}

function formatDate(value: string): string {
  const parsed = new Date(value);
  return Number.isNaN(parsed.getTime())
    ? "at an unknown time"
    : parsed.toLocaleString();
}
