import {
  restrictToHorizontalAxis,
  restrictToParentElement,
} from "@dnd-kit/modifiers";

export const DEFAULT_TAB_ROUTE = "/app/git";
export const DND_MODIFIERS = [
  restrictToHorizontalAxis,
  restrictToParentElement,
];
export const SORTABLE_TAB_TRANSITION = {
  duration: 170,
  easing: "cubic-bezier(0.22, 1, 0.36, 1)",
};
export const TAB_SWITCHER_HEADER_HEIGHT_PX = 240;
export const MAIN_HEADER_HEIGHT_CSS_VAR = "--main-custom-header-height";

export type TabSwitchModifier = "Control" | "Meta";

export type CustomTitleBarProps = {
  restrictedPaths: string[];
};
