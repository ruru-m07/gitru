import { Button } from "@gitru/ui/components/button";
import { CommandPanel, CommandViewConfig } from "@gitru/ui/components/command";
import { Group, GroupSeparator, GroupText } from "@gitru/ui/components/group";
import { Input } from "@gitru/ui/components/input";
import { Kbd } from "@gitru/ui/components/kbd";
import { Label } from "@gitru/ui/components/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@gitru/ui/components/select";
import { cn } from "@gitru/ui/lib/utils";
import { open } from "@tauri-apps/plugin-dialog";
import {
  CircleAlertIcon,
  CornerDownLeftIcon,
  FolderOpenIcon,
  Loader2,
  X,
} from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import { toast } from "sonner";
import { useCloneProgress } from "@/hooks";
import { useRepositories } from "@/hooks/useRepositories";
import { useAppStore } from "@/store/useAppStore";

type RepoProtocol = "https://" | "http://" | "ssh://" | "git@";

const SUPPORTED_PROTOCOLS: RepoProtocol[] = [
  "https://",
  "http://",
  "ssh://",
  "git@",
];

function parseProtocolAndPath(value: string): {
  protocol: RepoProtocol;
  path: string;
} | null {
  const trimmed = value.trim();
  if (!trimmed) return null;

  for (const protocol of SUPPORTED_PROTOCOLS) {
    if (trimmed.toLowerCase().startsWith(protocol.toLowerCase())) {
      return {
        protocol,
        path: trimmed.slice(protocol.length),
      };
    }
  }

  return null;
}

function extractRepoNameFromUrl(value: string): string {
  const normalized = value
    .trim()
    .replace(/\.git$/, "")
    .replace(/\/$/, "");
  if (!normalized) return "";

  const lastSegment = normalized.split(/[/:]/).filter(Boolean).pop();
  return lastSegment ?? "";
}

function isLikelyValidClonePath(protocol: RepoProtocol, path: string): boolean {
  const trimmed = path.trim();
  if (!trimmed || /\s/.test(trimmed)) return false;

  if (protocol === "git@") {
    return /^[^:\s]+:[^/\s]+\/[^/\s]+(?:\.git)?\/?$/.test(trimmed);
  }

  try {
    const parsed = new URL(`${protocol}${trimmed}`);
    const segments = parsed.pathname.split("/").filter(Boolean);
    return Boolean(parsed.hostname) && segments.length >= 2;
  } catch {
    return false;
  }
}

function hashSeed(input: string): number {
  let hash = 2166136261;
  for (let i = 0; i < input.length; i += 1) {
    hash ^= input.charCodeAt(i);
    hash = Math.imul(hash, 16777619);
  }
  return hash >>> 0;
}

function createShuffledRanks(length: number, seedInput: string): number[] {
  if (length <= 0) return [];

  const indices = Array.from({ length }, (_, i) => i);
  let seed = hashSeed(seedInput) || 1;

  const next = () => {
    seed = (seed * 1664525 + 1013904223) >>> 0;
    return seed / 0x100000000;
  };

  for (let i = indices.length - 1; i > 0; i -= 1) {
    const j = Math.floor(next() * (i + 1));
    [indices[i], indices[j]] = [indices[j], indices[i]];
  }

  const rankByIndex = Array.from({ length }, () => 0);
  indices.forEach((cellIndex, rank) => {
    rankByIndex[cellIndex] = rank;
  });

  return rankByIndex;
}

function sleep(ms: number) {
  return new Promise<void>((resolve) => {
    setTimeout(resolve, ms);
  });
}

function formatDuration(seconds: number): string {
  const clamped = Math.max(0, Math.round(seconds));
  const mins = Math.floor(clamped / 60);
  const secs = clamped % 60;

  if (mins > 0) {
    return `${mins}m ${secs}s`;
  }

  return `${secs}s`;
}

export function useCloneRepositoryView(): CommandViewConfig<
  "clone-repository",
  undefined
> {
  const { cloneRepo, cancelClone, repositories } = useRepositories();
  const setSelectedRepository = useAppStore(
    (state) => state.setSelectedRepository,
  );
  const setRepoSelectIsOpen = useAppStore((state) => state.setRepoSelectIsOpen);
  const setOptimisticRepositoryCard = useAppStore(
    (state) => state.setOptimisticRepositoryCard,
  );

  const [repoProtocol, setRepoProtocol] = useState<RepoProtocol>("https://");
  const [urlPath, setUrlPath] = useState("");
  const [destinationPath, setDestinationPath] = useState("");
  const [repoClonePath, setRepoClonePath] = useState("");
  const [isCloning, setIsCloning] = useState(false);
  const [isCancelling, setIsCancelling] = useState(false);
  const [isCancelGrace, setIsCancelGrace] = useState(false);
  const [isFinishingScan, setIsFinishingScan] = useState(false);
  const [finishScanTick, setFinishScanTick] = useState(0);
  const [speedBytesPerSec, setSpeedBytesPerSec] = useState<number | null>(null);
  const [etaSeconds, setEtaSeconds] = useState<number | null>(null);
  const [didAttemptSubmit, setDidAttemptSubmit] = useState(false);
  const [operationId, setOperationId] = useState<string | null>(null);
  const { event } = useCloneProgress(operationId ?? undefined);

  // Smooth interpolated percent for dot animation
  const [displayPercent, setDisplayPercent] = useState(0);
  const displayPercentRef = useRef(0);
  const targetPercentRef = useRef(0);
  const speedSampleRef = useRef<{
    bytes: number;
    receivedObjects: number;
    totalObjects: number;
    at: number;
  } | null>(null);
  const etaSmoothingRef = useRef<number | null>(null);
  const lastEtaUpdateAtRef = useRef(0);

  useEffect(() => {
    if (event == null) return;

    const transferTotal = event.transfer?.totalObjects ?? 0;
    const transferDone = event.transfer?.receivedObjects ?? 0;
    const indexTotal = event.transfer?.totalObjects ?? 0; // same total
    const indexDone = event.transfer?.indexedObjects ?? 0;

    if (transferTotal > 0) {
      // Make perceived completion align with git clone behavior:
      // transfer is most of the visible work (0-90%), indexing is finalization (90-100%).
      const transferPercent = (transferDone / transferTotal) * 90;
      const indexPercent = (indexDone / indexTotal) * 10;
      targetPercentRef.current = transferPercent + indexPercent;
      return;
    }

    if (event.phase === "Finished") {
      targetPercentRef.current = 100;
      return;
    }

    targetPercentRef.current = Math.min(100, Math.max(0, event.percent ?? 0));
  }, [
    event?.transfer?.receivedObjects,
    event?.transfer?.indexedObjects,
    event?.transfer?.totalObjects,
    event?.percent,
    event?.phase,
  ]);

  useEffect(() => {
    if (!isCloning) {
      displayPercentRef.current = 0;
      targetPercentRef.current = 0;
      setDisplayPercent(0);
      speedSampleRef.current = null;
      etaSmoothingRef.current = null;
      lastEtaUpdateAtRef.current = 0;
      setSpeedBytesPerSec(null);
      setEtaSeconds(null);
      setIsCancelGrace(false);
      setIsFinishingScan(false);
      setFinishScanTick(0);
      return;
    }
    const interval = setInterval(() => {
      const current = displayPercentRef.current;
      const target = targetPercentRef.current;
      if (Math.abs(target - current) < 0.1) return;
      // Lerp: move 8% of the remaining distance each tick
      const next = current + (target - current) * 0.08;
      displayPercentRef.current = next;
      setDisplayPercent(next);
    }, 50);
    return () => clearInterval(interval);
  }, [isCloning]);

  useEffect(() => {
    if (!isCloning) return;
    if (event?.phase === "Finished" || event?.phase === "Cancelled") {
      etaSmoothingRef.current = 0;
      setEtaSeconds(0);
      return;
    }

    const bytes = event?.transfer?.receivedBytes ?? 0;
    const received = event?.transfer?.receivedObjects ?? 0;
    const total = event?.transfer?.totalObjects ?? 0;
    if (bytes <= 0) return;

    const now = Date.now();
    const previous = speedSampleRef.current;
    speedSampleRef.current = {
      bytes,
      receivedObjects: received,
      totalObjects: total,
      at: now,
    };

    if (!previous) return;

    const elapsedSec = Math.max(0.001, (now - previous.at) / 1000);
    const deltaBytes = bytes - previous.bytes;
    if (deltaBytes <= 0) return;

    const instantSpeed = deltaBytes / elapsedSec;
    setSpeedBytesPerSec((current) => {
      const next =
        current == null ? instantSpeed : current * 0.78 + instantSpeed * 0.22;

      if (total > 0 && received > 0) {
        const objectFraction = Math.min(1, received / total);
        if (objectFraction > 0.02 && next > 0) {
          const estimatedTotalBytes = bytes / objectFraction;
          const remainingBytes = Math.max(0, estimatedTotalBytes - bytes);
          const etaRaw = remainingBytes / next;
          const smoothedEta =
            etaSmoothingRef.current == null
              ? etaRaw
              : etaSmoothingRef.current * 0.86 + etaRaw * 0.14;
          etaSmoothingRef.current = smoothedEta;

          const nowMs = Date.now();
          if (nowMs - lastEtaUpdateAtRef.current >= 450) {
            lastEtaUpdateAtRef.current = nowMs;
            setEtaSeconds(smoothedEta);
          }
        }
      }

      return next;
    });
  }, [
    event?.phase,
    event?.transfer?.receivedBytes,
    event?.transfer?.receivedObjects,
    event?.transfer?.totalObjects,
    isCloning,
  ]);

  useEffect(() => {
    if (!isCloning) return;
    if (event?.phase !== "Finished") return;

    setIsFinishingScan(true);
    targetPercentRef.current = Math.max(95, targetPercentRef.current);
    const tickInterval = setInterval(() => {
      setFinishScanTick((tick) => tick + 1);
    }, 55);
    const settleTimeout = setTimeout(() => {
      targetPercentRef.current = 100;
      setIsFinishingScan(false);
    }, 520);

    return () => {
      clearInterval(tickInterval);
      clearTimeout(settleTimeout);
    };
  }, [event?.phase, isCloning]);

  // Reset smooth percent whenever a new clone starts
  useEffect(() => {
    displayPercentRef.current = 0;
    targetPercentRef.current = 0;
    setDisplayPercent(0);
  }, [operationId]);

  // Auto-append repo name to destination
  const repoName = extractRepoNameFromUrl(urlPath);

  // Auto-fill repoClonePath when repoName is derived from URL
  useEffect(() => {
    if (repoName && !repoClonePath) {
      setRepoClonePath(repoName);
    }
  }, [repoName]);

  const resolvedCloneFolderName = repoClonePath.trim() || repoName;
  const displayDestination =
    destinationPath && resolvedCloneFolderName
      ? `${destinationPath.replace(/\/$/, "")}/${resolvedCloneFolderName}`
      : destinationPath;

  // Format bytes
  const formatBytes = (bytes: number) => {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  };

  const transfer = event?.transfer;
  const receivedObjects = transfer?.receivedObjects ?? 0;
  const totalObjects = transfer?.totalObjects ?? event?.total ?? 0;
  const receivedBytes = transfer?.receivedBytes ?? 0;
  const indexedObjects = transfer?.indexedObjects ?? 0;
  const normalizedUrl = `${repoProtocol}${urlPath.trim()}`;
  const normalizedDestination = displayDestination.trim();
  const closeCommandPanelRef = useRef<(() => void) | null>(null);

  const overallProgressPercent = useMemo(() => {
    if (!event) return 0;
    if (event.phase === "Finished") return 100;

    const transferTotal = event.transfer?.totalObjects ?? 0;
    if (transferTotal > 0) {
      const transferDone = event.transfer?.receivedObjects ?? 0;
      const indexDone = event.transfer?.indexedObjects ?? 0;
      return Math.min(
        100,
        (transferDone / transferTotal) * 90 + (indexDone / transferTotal) * 10,
      );
    }

    return Math.min(100, Math.max(0, event.percent ?? 0));
  }, [
    event,
    event?.phase,
    event?.percent,
    event?.transfer?.indexedObjects,
    event?.transfer?.receivedObjects,
    event?.transfer?.totalObjects,
  ]);

  const cloneValidationError = useMemo(() => {
    if (!urlPath.trim()) return "Repository URL is required";
    if (!destinationPath.trim()) return "Destination folder is required";
    if (!resolvedCloneFolderName.trim())
      return "Repository folder name is required";
    if (!isLikelyValidClonePath(repoProtocol, urlPath)) {
      return "Enter a valid repository URL";
    }
    if (/[\\/:*?"<>|]/.test(resolvedCloneFolderName.trim())) {
      return "Repository folder name contains invalid characters";
    }
    if (repositories.find((r) => r.path === normalizedDestination)) {
      return "Repository already added";
    }
    return null;
  }, [
    destinationPath,
    normalizedDestination,
    repoProtocol,
    repositories,
    resolvedCloneFolderName,
    urlPath,
  ]);

  const speedLabel =
    speedBytesPerSec != null && speedBytesPerSec > 0
      ? `${formatBytes(speedBytesPerSec)}/s`
      : "—";

  const etaLabel =
    etaSeconds != null && Number.isFinite(etaSeconds) && etaSeconds > 0
      ? formatDuration(etaSeconds)
      : event?.phase === "Finished"
        ? "0s"
        : "—";

  const handleClone = async () => {
    setDidAttemptSubmit(true);

    if (cloneValidationError) {
      toast.error(cloneValidationError);
      return;
    }

    const nextOperationId = crypto.randomUUID();
    setOperationId(nextOperationId);
    setIsCloning(true);
    setOptimisticRepositoryCard({
      name: resolvedCloneFolderName,
      path: normalizedDestination,
    });
    displayPercentRef.current = 0;
    targetPercentRef.current = 0;
    setDisplayPercent(0);

    try {
      const repository = await cloneRepo({
        url: normalizedUrl,
        destinationPath: normalizedDestination,
        operationId: nextOperationId,
      });
      await sleep(560);
      await setSelectedRepository(repository);

      setRepoSelectIsOpen(false);
      setDidAttemptSubmit(false);
      closeCommandPanelRef.current?.();
      toast.success("Repository cloned successfully!");
    } catch {
      // handled by hook
    } finally {
      setOptimisticRepositoryCard(null);
      setIsCloning(false);
      setIsCancelling(false);
    }
  };

  const triggerCloneFromEnter = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key !== "Enter") return;
    e.preventDefault();
    void handleClone();
  };

  return {
    id: "clone-repository",
    input: {
      placeholder: "Clone repository",
      autoFocus: false,
    },
    header() {
      return <div />;
    },
    footer(context) {
      closeCommandPanelRef.current = context.close;

      return (
        <>
          <Button
            onClick={context.navigate.back}
            variant={"secondary"}
            className="flex items-center gap-2 cursor-pointer"
          >
            <span className="text-muted-foreground">Back</span>
            <Kbd className="bg-background">Esc</Kbd>
          </Button>
          {isCloning ? (
            <Button
              variant="destructive"
              disabled={isCancelling || !operationId}
              onClick={async () => {
                if (!operationId) return;
                setIsCancelling(true);
                setIsCancelGrace(true);
                try {
                  await sleep(240);
                  const cancelled = await cancelClone(operationId);
                  if (!cancelled) toast.error("No active clone to cancel");
                } catch {
                  // handled by hook
                } finally {
                  setIsCancelGrace(false);
                  setIsCancelling(false);
                }
              }}
            >
              {isCancelling ? (
                <Loader2
                  className={cn(
                    "size-4 animate-spin",
                    isCancelGrace && "animate-pulse",
                  )}
                />
              ) : (
                <X className="size-4" />
              )}
              {isCancelGrace ? "Stopping gracefully..." : "Cancel Clone"}
            </Button>
          ) : (
            <Button
              onClick={async () => {
                await handleClone();
              }}
            >
              <span>Clone</span>
              <Kbd className="bg-primary-foreground/20 text-primary-foreground">
                <CornerDownLeftIcon />
              </Kbd>
            </Button>
          )}
        </>
      );
    },
    render: () => {
      // Grid cols calculation
      const containerRef = useRef<HTMLDivElement>(null);
      const [cols, setCols] = useState(0);
      const [prepareTick, setPrepareTick] = useState(0);

      useEffect(() => {
        const calculate = () => {
          if (!containerRef.current) return;
          const width = containerRef.current.offsetWidth;
          const columns = Math.floor((width + 4) / 12);
          setCols(columns);
        };
        calculate();
        const ro = new ResizeObserver(calculate);
        if (containerRef.current) ro.observe(containerRef.current);
        return () => ro.disconnect();
      }, [containerRef]);

      const total = cols > 0 ? cols * Math.ceil(1000 / cols) : 1000;
      const filledCount = Math.round((displayPercent * total) / 100);

      const randomizedFillRanks = useMemo(
        () => createShuffledRanks(total, operationId ?? "clone-pixels"),
        [operationId, total],
      );

      const hasMaterialProgress =
        receivedObjects > 0 ||
        indexedObjects > 0 ||
        totalObjects > 0 ||
        (event?.percent ?? 0) > 1;

      const isPreparing =
        isCloning &&
        (!event ||
          event.phase === "Preparing" ||
          (!hasMaterialProgress &&
            (event.phase === "Started" ||
              event.phase === "Sideband" ||
              event.phase === "Message")));

      useEffect(() => {
        if (!isPreparing) {
          setPrepareTick(0);
          return;
        }

        const interval = setInterval(() => {
          setPrepareTick((tick) => tick + 1);
        }, 90);

        return () => clearInterval(interval);
      }, [isPreparing]);

      const rows = cols > 0 ? Math.ceil(total / cols) : 0;

      const preparingIsActiveCell = (index: number) => {
        if (cols === 0 || rows === 0) return false;

        const row = Math.floor(index / cols);
        const column = index % cols;
        const baseSeed = hashSeed(`${operationId ?? "prepare"}:${index}`);
        const sparkle = (baseSeed + prepareTick * 17) % 19 < 3;

        // Two drifting diagonals create a CRT-like scan dance.
        const diagonalA = (row + column + prepareTick) % 11 === 0;
        const diagonalB =
          (row - column + rows + ((prepareTick * 2) % (rows + cols || 1))) %
            13 ===
          0;

        return sparkle || diagonalA || diagonalB;
      };

      const finishingScanIsActiveCell = (index: number) => {
        if (!isFinishingScan || cols === 0 || rows === 0) return false;
        const row = Math.floor(index / cols);
        const column = index % cols;
        const scanner = (finishScanTick * 2) % Math.max(1, rows + cols);
        return row + column === scanner || row + column === scanner - 1;
      };

      return (
        <CommandPanel className="p-4">
          <div>
            {/* Inputs — hidden while cloning */}
            {!isCloning && (
              <div className="gap-4 flex flex-col">
                <div className="flex flex-col w-full">
                  <Label className="mb-1.5">Repository URL</Label>
                  <Group className="w-full">
                    <GroupText
                      render={
                        <Select
                          value={repoProtocol}
                          onValueChange={(value) =>
                            setRepoProtocol(value as RepoProtocol)
                          }
                        />
                      }
                    >
                      <SelectTrigger
                        className={cn(
                          "w-fit min-w-fit rounded-e-none border-e-0 bg-secondary",
                        )}
                        aria-label="Repository protocol"
                      >
                        <SelectValue className={"w-fit"} />
                      </SelectTrigger>
                      <SelectContent>
                        {SUPPORTED_PROTOCOLS.map((protocol) => (
                          <SelectItem key={protocol} value={protocol}>
                            {protocol}
                          </SelectItem>
                        ))}
                      </SelectContent>
                    </GroupText>
                    <GroupSeparator />
                    <Input
                      type="text"
                      value={urlPath}
                      onKeyDown={triggerCloneFromEnter}
                      onChange={(e) => {
                        const nextValue = e.target.value;
                        const parsed = parseProtocolAndPath(nextValue);

                        if (parsed) {
                          setRepoProtocol(parsed.protocol);
                          setUrlPath(parsed.path);
                          return;
                        }

                        setUrlPath(nextValue);
                      }}
                      onPaste={(e) => {
                        const pasted = e.clipboardData.getData("text");
                        const parsed = parseProtocolAndPath(pasted);
                        if (!parsed) return;

                        e.preventDefault();
                        setRepoProtocol(parsed.protocol);
                        setUrlPath(parsed.path);
                      }}
                      placeholder="github.com/owner/repository.git"
                      className="*:[input]:px-0! *:[input]:pl-2! w-full"
                    />
                  </Group>
                </div>
                <div className="flex flex-col w-full">
                  <Label className="mb-1.5">Destination Folder</Label>
                  {!destinationPath ? (
                    // No path selected yet — show just the Browse button
                    <Button
                      variant="outline"
                      className="w-fit"
                      onClick={async () => {
                        const folder = await open({
                          directory: true,
                          multiple: false,
                        });
                        if (typeof folder === "string") {
                          setDestinationPath(folder);
                        }
                      }}
                    >
                      <FolderOpenIcon className="size-4" />
                      Select destination
                    </Button>
                  ) : (
                    // Path selected — show InputGroup with path as prefix
                    <Group className="w-full">
                      <GroupText
                        render={<Label aria-label="Domain" htmlFor="domain" />}
                      >
                        {destinationPath.replace(/\/$/, "")}/
                      </GroupText>
                      <GroupSeparator />
                      <Input
                        placeholder={repoName || "repo-name"}
                        type="text"
                        value={repoClonePath}
                        onKeyDown={triggerCloneFromEnter}
                        onChange={(e) => setRepoClonePath(e.target.value)}
                        className="*:[input]:px-0! *:[input]:pl-2! w-full"
                      />
                      <GroupSeparator />
                      <GroupText
                        render={
                          <Button
                            type="button"
                            size={"icon"}
                            variant={"secondary"}
                            onClick={async () => {
                              const folder = await open({
                                directory: true,
                                multiple: false,
                              });
                              if (typeof folder === "string") {
                                setDestinationPath(folder);
                              }
                            }}
                          />
                        }
                      >
                        <FolderOpenIcon className="size-3.5" />
                      </GroupText>
                    </Group>
                  )}
                  {/* Show resolved path with repo name appended */}
                  {displayDestination && (
                    <p className="mt-1.5 text-xs text-muted-foreground">
                      Will clone into{" "}
                      <span className="font-mono text-foreground">
                        {destinationPath.replace(/\/$/, "")}/
                      </span>
                      <span className="font-mono font-semibold text-primary">
                        {repoClonePath || repoName || ""}
                      </span>
                    </p>
                  )}

                  {didAttemptSubmit && cloneValidationError && (
                    <p className="mt-1.5 text-xs text-destructive">
                      {cloneValidationError}
                    </p>
                  )}
                </div>
              </div>
            )}

            <div className="w-full" ref={containerRef}>
              {/* Progress view */}
              {isCloning && (
                <div className="space-y-3">
                  {/* Repo + destination summary */}
                  <div className="space-y-0.5">
                    <p className="text-sm font-semibold truncate">
                      {repoName || "Repository"}
                    </p>
                    <p className="text-xs text-muted-foreground font-mono truncate">
                      {displayDestination}
                    </p>
                  </div>

                  {isPreparing ? (
                    <>
                      {/* Preparing dot dance */}
                      <div
                        className="grid w-full"
                        style={{
                          gridTemplateColumns: `repeat(${cols}, 1fr)`,
                          gap: "3px",
                        }}
                      >
                        {cols > 0 &&
                          Array.from({ length: total }, (_, i) => (
                            <div
                              key={i}
                              className={cn(
                                "aspect-square rounded-[2px] transition-colors duration-150",
                                preparingIsActiveCell(i)
                                  ? "bg-primary"
                                  : "bg-muted-foreground/25",
                              )}
                            />
                          ))}
                      </div>

                      <div className="flex items-center gap-2.5 py-0.5 text-xs text-muted-foreground">
                        <Loader2 className="size-3.5 animate-spin shrink-0" />
                        <span>
                          {event?.status || "Preparing clone session..."}
                        </span>
                      </div>
                    </>
                  ) : (
                    <>
                      {/* Dot grid */}
                      <div
                        className="grid w-full"
                        style={{
                          gridTemplateColumns: `repeat(${cols}, 1fr)`,
                          gap: "3px",
                        }}
                      >
                        {cols > 0 &&
                          Array.from({ length: total }, (_, i) => (
                            <div
                              key={i}
                              className={cn(
                                "aspect-square rounded-[2px] transition-colors duration-300",
                                randomizedFillRanks[i] < filledCount ||
                                  finishingScanIsActiveCell(i)
                                  ? "bg-primary"
                                  : "bg-muted-foreground/25",
                              )}
                            />
                          ))}
                      </div>

                      {/* Stats row */}
                      <div className="grid grid-cols-3 gap-2 pt-0.5">
                        {[
                          {
                            label: "Phase",
                            value: event?.phase ?? "—",
                          },
                          {
                            label: "Progress",
                            value: `${Math.round(overallProgressPercent)}%`,
                          },
                          {
                            label: "Objects",
                            value:
                              totalObjects > 0
                                ? `${receivedObjects.toLocaleString()} / ${totalObjects.toLocaleString()}`
                                : "—",
                          },
                          {
                            label: "Received",
                            value:
                              receivedBytes > 0
                                ? formatBytes(receivedBytes)
                                : "—",
                          },
                          {
                            label: "Speed",
                            value: speedLabel,
                          },
                          {
                            label: "ETA",
                            value: etaLabel,
                          },
                        ].map(({ label, value }) => (
                          <div key={label} className="space-y-0.5">
                            <p className="text-[10px] uppercase tracking-wider text-muted-foreground">
                              {label}
                            </p>
                            <p className="text-xs font-mono font-medium tabular-nums truncate">
                              {value}
                            </p>
                          </div>
                        ))}
                      </div>

                      {/* Indexed objects sub-progress */}
                      {indexedObjects > 0 && totalObjects > 0 && (
                        <div className="space-y-1">
                          <div className="flex justify-between text-[10px] text-muted-foreground">
                            <span className="text-[10px] uppercase tracking-wider text-muted-foreground">
                              Finalize
                            </span>
                            <span className="tabular-nums font-mono">
                              {indexedObjects.toLocaleString()} /{" "}
                              {totalObjects.toLocaleString()}
                            </span>
                          </div>
                          <div className="h-0.5 w-full rounded-full bg-muted">
                            <div
                              className="h-full rounded-full bg-primary/50 transition-all duration-500"
                              style={{
                                width: `${Math.min(100, overallProgressPercent)}%`,
                              }}
                            />
                          </div>
                        </div>
                      )}
                      {/* Error / cancelled */}
                      {(event?.phase === "Error" ||
                        event?.phase === "Cancelled") && (
                        <div className="flex items-center gap-2 text-xs text-destructive">
                          <CircleAlertIcon className="size-3.5 shrink-0" />
                          <span>{event?.status ?? event?.phase}</span>
                        </div>
                      )}
                    </>
                  )}
                </div>
              )}
            </div>
          </div>
        </CommandPanel>
      );
    },
  };
}
