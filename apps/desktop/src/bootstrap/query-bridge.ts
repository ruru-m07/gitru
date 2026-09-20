import { initializeRepositoryChangeBridge } from "../state/core/repository-change-bridge";
import { initializeQueryFocusBridge } from "../state/core/state-manager";

export function initializeQueryBridge() {
  initializeQueryFocusBridge();
  initializeRepositoryChangeBridge();
}
