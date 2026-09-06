export type TabSwitchCycleMode = "MRU" | "Sequential";

// Temporary code-only setting until preferences UI is added.
export const TAB_SWITCH_CYCLE_MODE: TabSwitchCycleMode = "Sequential";

export const TAB_SWITCH_SHORTCUT_EVENT = "gitru:tab-switch-shortcut";

export type TabManagementShortcut = "create" | "close";
export type TabManagementShortcutModifier = "Control" | "Meta";

type TabManagementKeyboardEvent = Pick<
  KeyboardEvent,
  "altKey" | "ctrlKey" | "key" | "metaKey" | "repeat" | "shiftKey"
>;

const getPlatformShortcutModifier = (): TabManagementShortcutModifier =>
  typeof navigator !== "undefined" &&
  /Mac|iPhone|iPad|iPod/i.test(navigator.platform)
    ? "Meta"
    : "Control";

export const resolveTabManagementShortcut = (
  {
    altKey,
    ctrlKey,
    key,
    metaKey,
    repeat,
    shiftKey,
  }: TabManagementKeyboardEvent,
  shortcutModifier = getPlatformShortcutModifier(),
): TabManagementShortcut | null => {
  const hasShortcutModifier =
    shortcutModifier === "Meta" ? metaKey && !ctrlKey : ctrlKey && !metaKey;

  if (!hasShortcutModifier || altKey || shiftKey || repeat) {
    return null;
  }

  switch (key.toLowerCase()) {
    case "t":
      return "create";
    case "w":
      return "close";
    default:
      return null;
  }
};

export type TabSwitchShortcutPayload =
  | {
      phase: "advance";
      backward: boolean;
      modifier: "Control" | "Meta";
    }
  | {
      phase: "commit";
    }
  | {
      phase: TabManagementShortcut;
    };
