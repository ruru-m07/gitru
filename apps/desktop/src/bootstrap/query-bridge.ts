import { installCollaborationBridge } from "@gitru/collaboration-client";
import { initializeRepositoryChangeBridge } from "../state/core/repository-change-bridge";
import {
  initializeQueryFocusBridge,
  queryClient,
} from "../state/core/state-manager";

let stopCollaborationBridge: (() => void) | undefined;

export function initializeQueryBridge() {
  initializeQueryFocusBridge();
  initializeRepositoryChangeBridge();
  stopCollaborationBridge ??= installCollaborationBridge(queryClient);
}

if (import.meta.hot) {
  import.meta.hot.dispose(() => {
    stopCollaborationBridge?.();
    stopCollaborationBridge = undefined;
  });
}
