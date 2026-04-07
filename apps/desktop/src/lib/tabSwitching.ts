export type TabSwitchCycleMode = "MRU" | "Sequential";

// Temporary code-only setting until preferences UI is added.
export const TAB_SWITCH_CYCLE_MODE: TabSwitchCycleMode = "Sequential";

export const TAB_SWITCH_SHORTCUT_EVENT = "gitru:tab-switch-shortcut";

export type TabSwitchShortcutPayload =
  | {
      phase: "advance";
      backward: boolean;
      modifier: "Control" | "Meta";
    }
  | {
      phase: "commit";
    };
