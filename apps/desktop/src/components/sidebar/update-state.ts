import {
  checkForUpdateByChannel,
  downloadAndInstallUpdateByChannel,
} from "@gitru/commands";
import { listen } from "@tauri-apps/api/event";
import { relaunch } from "@tauri-apps/plugin-process";
import { useCallback, useEffect, useRef, useState } from "react";
import { UpdateChannel } from "@/types/store";

export type UpdateStatus =
  | "idle"
  | "checking"
  | "available"
  | "downloading"
  | "downloaded"
  | "error";

export type AvailableUpdate = {
  version?: string;
  notes?: string;
  pubDate?: string;
  channel: UpdateChannel;
};

export type DownloadProgress = {
  percent: number;
  downloadedBytes: number;
  totalBytes: number;
};

type UpdaterDownloadProgressEvent = {
  phase: "Started" | "Progress" | "Finished";
  contentLength?: number;
  chunkLength?: number;
  downloaded?: number;
  percent?: number;
  channel: UpdateChannel;
  version?: string;
};

export function useUpdateState(channel: UpdateChannel) {
  const [updateStatus, setUpdateStatus] = useState<UpdateStatus>("idle");
  const [availableUpdate, setAvailableUpdate] =
    useState<AvailableUpdate | null>(null);
  const [downloadProgress, setDownloadProgress] =
    useState<DownloadProgress | null>(null);

  const isCheckingRef = useRef(false);
  const mountedRef = useRef(false);
  const lastChannelRef = useRef<UpdateChannel | null>(null);

  const checkForUpdates = useCallback(async () => {
    if (isCheckingRef.current || updateStatus === "downloading") {
      return;
    }

    isCheckingRef.current = true;
    setUpdateStatus("checking");
    setAvailableUpdate(null);
    setDownloadProgress(null);

    try {
      const update = await checkForUpdateByChannel({ channel });

      if (!update.available) {
        setUpdateStatus("idle");
        return;
      }

      setAvailableUpdate({
        channel,
        version: update.version,
        notes: update.notes,
        pubDate: update.pub_date,
      });
      setUpdateStatus("available");
    } catch (error) {
      setUpdateStatus("error");
      setAvailableUpdate(null);
      setDownloadProgress(null);
      console.debug("[updater] failed to check for updates", error);
    } finally {
      isCheckingRef.current = false;
    }
  }, [channel, updateStatus]);

  const startDownloadAndInstall = useCallback(async () => {
    if (updateStatus === "downloading" || updateStatus === "downloaded") {
      return;
    }

    setUpdateStatus("downloading");
    setDownloadProgress({
      percent: 0,
      downloadedBytes: 0,
      totalBytes: 0,
    });

    try {
      await downloadAndInstallUpdateByChannel({ channel });
      setUpdateStatus("downloaded");
      setDownloadProgress((prev) => ({
        percent: 100,
        downloadedBytes: prev?.downloadedBytes ?? 0,
        totalBytes: prev?.totalBytes ?? 0,
      }));
    } catch (error) {
      setUpdateStatus("error");
      console.debug("[updater] failed to download/install update", error);
    }
  }, [channel, updateStatus]);

  const restartApp = useCallback(async () => {
    await relaunch();
  }, []);

  useEffect(() => {
    let unlisten: (() => void) | undefined;

    void listen<UpdaterDownloadProgressEvent>(
      "updater://download-progress",
      (event) => {
        const payload = event.payload;

        if (payload.phase === "Started") {
          setUpdateStatus("downloading");
          setDownloadProgress({
            percent: 0,
            downloadedBytes: 0,
            totalBytes: payload.contentLength ?? 0,
          });
          return;
        }

        if (payload.phase === "Progress") {
          setUpdateStatus("downloading");
          setDownloadProgress({
            percent: Math.max(
              0,
              Math.min(100, Math.round(payload.percent ?? 0)),
            ),
            downloadedBytes: payload.downloaded ?? 0,
            totalBytes: payload.contentLength ?? 0,
          });
          return;
        }

        if (payload.phase === "Finished") {
          setDownloadProgress((prev) => ({
            percent: 100,
            downloadedBytes: payload.downloaded ?? prev?.downloadedBytes ?? 0,
            totalBytes: payload.contentLength ?? prev?.totalBytes ?? 0,
          }));
        }
      },
    ).then((dispose) => {
      unlisten = dispose;
    });

    return () => {
      if (unlisten) {
        unlisten();
      }
    };
  }, []);

  useEffect(() => {
    if (mountedRef.current) {
      return;
    }

    mountedRef.current = true;
    lastChannelRef.current = channel;
    void checkForUpdates();
  }, [channel, checkForUpdates]);

  useEffect(() => {
    if (!mountedRef.current) {
      return;
    }

    if (lastChannelRef.current === channel) {
      return;
    }

    lastChannelRef.current = channel;
    if (updateStatus === "downloading") {
      return;
    }

    void checkForUpdates();
  }, [channel, checkForUpdates, updateStatus]);

  return {
    updateStatus,
    availableUpdate,
    downloadProgress,
    checkForUpdates,
    startDownloadAndInstall,
    restartApp,
  };
}
