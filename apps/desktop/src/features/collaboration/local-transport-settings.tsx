import {
  collaboration,
  collaborationErrorMessage,
  collaborationKeys,
  type LocalTransportBinding,
  type RemoteAccount,
  type TransportBindingRequest,
} from "@gitru/collaboration-client";
import {
  useCollaborationAccounts,
  useLocalRepositoryLinks,
} from "@gitru/collaboration-client/react";
import { Button } from "@gitru/ui/components/button";
import { Field, FieldLabel } from "@gitru/ui/components/field";
import { Input } from "@gitru/ui/components/input";
import {
  Select,
  SelectItem,
  SelectPopup,
  SelectTrigger,
  SelectValue,
} from "@gitru/ui/components/select";
import { skipToken, useQuery } from "@tanstack/react-query";
import { useId, useState } from "react";
import { useAppStore } from "@/store/use-app-store";
import { isTrustedAccountWindow } from "./account-manager";
import { useLocalLinkIntent } from "./local-repository-links";

/** Mounted only by the already suspended main settings host. */
export function LocalTransportSettings() {
  const [expanded, setExpanded] = useState(false);
  const [repositoryId, setRepositoryId] = useState<string | null>(null);
  const repositories = useAppStore((state) => state.repositories);
  if (!isTrustedAccountWindow()) return null;
  const registered = repositories.find(
    (repository) => repository.id === repositoryId,
  );
  const names = new Set<string>();
  const duplicateNames = new Set<string>();
  for (const repository of repositories) {
    if (names.has(repository.name)) duplicateNames.add(repository.name);
    names.add(repository.name);
  }
  const repositoryItems = repositories.map((repo) => ({
    value: repo.id,
    label: duplicateNames.has(repo.name)
      ? `${repo.name} · ${repo.path}`
      : repo.name,
  }));
  return (
    <section
      className="mt-5 space-y-3 border-t pt-4"
      aria-label="Repository transports"
    >
      <Button
        size="sm"
        variant="outline"
        aria-expanded={expanded}
        onClick={() => setExpanded((value) => !value)}
      >
        Repository transports
      </Button>
      {expanded ? (
        <>
          <p className="text-xs text-muted-foreground">
            Map an exact Git transport host to an existing provider
            installation. This changes Gitru’s mapping only.
          </p>
          <Select
            items={repositoryItems}
            value={repositoryId}
            onValueChange={setRepositoryId}
          >
            <SelectTrigger aria-label="Registered local repository">
              <SelectValue>
                {registered
                  ? undefined
                  : "Choose a registered local repository"}
              </SelectValue>
            </SelectTrigger>
            <SelectPopup alignItemWithTrigger={false}>
              {repositoryItems.map((repo) => (
                <SelectItem key={repo.value} value={repo.value}>
                  {repo.label}
                </SelectItem>
              ))}
            </SelectPopup>
          </Select>
          {registered ? (
            <TransportBindingEditor
              key={registered.id}
              localRepositoryId={registered.id}
            />
          ) : (
            <p className="text-xs text-muted-foreground">
              Choose a registered local repository to inspect current mappings.
            </p>
          )}
        </>
      ) : null}
    </section>
  );
}
function TransportBindingEditor({
  localRepositoryId,
}: {
  localRepositoryId: string;
}) {
  const query = useLocalRepositoryLinks(localRepositoryId);
  const accounts = useCollaborationAccounts();
  const [accountId, setAccountId] = useState<string | null>(null);
  const active =
    accounts.data?.accounts.filter((account) => account.state === "active") ??
    [];
  const account = active.find((value) => value.id === accountId);
  const [editing, setEditing] = useState<LocalTransportBinding | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const begin = useLocalLinkIntent();
  const snapshot = !query.isError ? query.data?.snapshot : undefined;
  async function remove(binding: LocalTransportBinding) {
    if (!snapshot) return;
    const current = begin();
    setBusy(true);
    setError(null);
    try {
      await collaboration.removeTransportBinding(
        { id: binding.id, generation: binding.generation },
        snapshot.bindings_generation,
      );
    } catch (failure) {
      if (current()) setError(collaborationErrorMessage(failure));
    } finally {
      if (current()) setBusy(false);
    }
  }
  return (
    <div className="space-y-3">
      {query.isPending ? (
        <p role="status">Inspecting transport mappings…</p>
      ) : null}
      {query.isError ? (
        <p role="alert">{collaborationErrorMessage(query.error)}</p>
      ) : null}
      {error ? <p role="alert">{error}</p> : null}
      {snapshot?.bindings.map((binding) => (
        <div key={binding.id} className="space-y-2 rounded-md border p-3">
          <p className="break-all text-xs">
            {binding.transport} · {binding.host}:{binding.port}/
            {binding.path_prefix} → {binding.instance_id} · {binding.layout}
          </p>
          <div className="flex gap-2">
            <Button
              size="sm"
              variant="outline"
              disabled={busy}
              onClick={() => setEditing(binding)}
            >
              Edit mapping
            </Button>
            <Button
              size="sm"
              variant="ghost"
              disabled={busy}
              onClick={() => {
                void remove(binding);
              }}
            >
              Remove mapping
            </Button>
          </div>
        </div>
      ))}
      <Select
        items={active.map((value) => ({
          value: value.id,
          label: `@${value.login} (${value.host})`,
        }))}
        value={accountId}
        onValueChange={(value) => {
          setEditing(null);
          setAccountId(value);
        }}
      >
        <SelectTrigger aria-label="Provider installation account">
          <SelectValue>
            {account ? undefined : "Choose an existing provider installation"}
          </SelectValue>
        </SelectTrigger>
        <SelectPopup alignItemWithTrigger={false}>
          {active.map((value) => (
            <SelectItem key={value.id} value={value.id}>
              @{value.login} ({value.host})
            </SelectItem>
          ))}
        </SelectPopup>
      </Select>
      {snapshot && (account || editing) ? (
        <BindingForm
          key={`${account?.id ?? ""}:${account?.authorization_epoch ?? ""}:${editing?.id ?? "new"}:${editing?.generation ?? ""}`}
          account={account}
          editing={editing}
          bindingsGeneration={snapshot.bindings_generation}
          onSaved={() => setEditing(null)}
        />
      ) : null}
      <Button
        size="sm"
        variant="ghost"
        disabled={query.isFetching}
        onClick={() => {
          void query.refetch();
        }}
      >
        Reload mappings
      </Button>
    </div>
  );
}
function BindingForm({
  account,
  editing,
  bindingsGeneration,
  onSaved,
}: {
  account: RemoteAccount | undefined;
  editing: LocalTransportBinding | null;
  bindingsGeneration: string;
  onSaved(): void;
}) {
  const profile = useQuery({
    queryKey: account
      ? collaborationKeys.capabilities(account)
      : ["collaboration", "binding-no-account"],
    queryFn: account
      ? ({ signal }) => collaboration.forAccount(account).capabilities(signal)
      : skipToken,
    networkMode: "always",
    retry: false,
    staleTime: Infinity,
    refetchOnWindowFocus: false,
    refetchOnReconnect: false,
  });
  const instanceId = editing?.instance_id ?? profile.data?.instance.id;
  const [host, setHost] = useState(editing?.host ?? "");
  const [port, setPort] = useState(String(editing?.port ?? 443));
  const [prefix, setPrefix] = useState(editing?.path_prefix ?? "");
  const [transport, setTransport] = useState<
    TransportBindingRequest["transport"]
  >(editing?.transport ?? "https");
  const [layout, setLayout] = useState<TransportBindingRequest["layout"]>(
    editing?.layout ?? "owner_repository",
  );
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const id = useId();
  const begin = useLocalLinkIntent();
  return (
    <form
      className="space-y-3 rounded-md border p-3"
      onSubmit={(event) => {
        event.preventDefault();
        if (!instanceId) return;
        const current = begin();
        setBusy(true);
        setError(null);
        void collaboration
          .saveTransportBinding({
            instance_id: instanceId,
            transport,
            host,
            port: Number(port),
            path_prefix: prefix,
            layout,
            expected_bindings_generation: bindingsGeneration,
            replace: editing
              ? { id: editing.id, generation: editing.generation }
              : null,
          })
          .then(() => {
            if (current()) onSaved();
          })
          .catch((failure) => {
            if (current()) setError(collaborationErrorMessage(failure));
          })
          .finally(() => {
            if (current()) setBusy(false);
          });
      }}
    >
      <p className="break-all text-xs text-muted-foreground">
        Installation: {instanceId ?? "Reading the saved installation…"}
      </p>
      <Field>
        <FieldLabel htmlFor={`${id}-host`}>Exact transport host</FieldLabel>
        <Input
          id={`${id}-host`}
          value={host}
          onChange={(event) => setHost(event.currentTarget.value)}
          maxLength={1024}
          required
          disabled={busy}
          placeholder="github-work"
        />
      </Field>
      <Field>
        <FieldLabel htmlFor={`${id}-port`}>Transport port</FieldLabel>
        <Input
          id={`${id}-port`}
          type="number"
          min={1}
          max={65535}
          value={port}
          onChange={(event) => setPort(event.currentTarget.value)}
          required
          disabled={busy}
        />
      </Field>
      <Field>
        <FieldLabel htmlFor={`${id}-prefix`}>Path prefix</FieldLabel>
        <Input
          id={`${id}-prefix`}
          value={prefix}
          onChange={(event) => setPrefix(event.currentTarget.value)}
          maxLength={1024}
          disabled={busy}
          placeholder="Optional segment prefix"
        />
      </Field>
      <Select
        items={[
          { value: "https", label: "HTTPS" },
          { value: "ssh", label: "SSH" },
          { value: "scp", label: "SCP-style SSH" },
        ]}
        value={transport}
        onValueChange={(value) => {
          if (value === "https" || value === "ssh" || value === "scp") {
            setTransport(value);
            setPort(value === "https" ? "443" : "22");
          }
        }}
        disabled={busy}
      >
        <SelectTrigger aria-label="Transport kind">
          <SelectValue />
        </SelectTrigger>
        <SelectPopup alignItemWithTrigger={false}>
          <SelectItem value="https">HTTPS</SelectItem>
          <SelectItem value="ssh">SSH</SelectItem>
          <SelectItem value="scp">SCP-style SSH</SelectItem>
        </SelectPopup>
      </Select>
      <Select
        items={[
          {
            value: "owner_repository",
            label: "Owner or workspace / repository",
          },
          { value: "subgroups", label: "Namespace subgroups / repository" },
        ]}
        value={layout}
        onValueChange={(value) => {
          if (value === "owner_repository" || value === "subgroups")
            setLayout(value);
        }}
        disabled={busy}
      >
        <SelectTrigger aria-label="Repository path layout">
          <SelectValue />
        </SelectTrigger>
        <SelectPopup alignItemWithTrigger={false}>
          <SelectItem value="owner_repository">
            Owner or workspace / repository
          </SelectItem>
          <SelectItem value="subgroups">
            Namespace subgroups / repository
          </SelectItem>
        </SelectPopup>
      </Select>
      {profile.isError ? (
        <p role="alert">{collaborationErrorMessage(profile.error)}</p>
      ) : null}
      {error ? (
        <p role="alert">{error} Reload mappings before retrying.</p>
      ) : null}
      <Button
        type="submit"
        size="sm"
        disabled={
          busy ||
          !instanceId ||
          !host ||
          !Number.isInteger(Number(port)) ||
          Number(port) < 1 ||
          Number(port) > 65535
        }
      >
        {editing ? "Save mapping changes" : "Add transport mapping"}
      </Button>
    </form>
  );
}
