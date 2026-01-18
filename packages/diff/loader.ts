import { registerCustomTheme } from "@pierre/diffs";
import { vesperLight } from "./themes/vesper-light";

export function themeLoader() {
  registerCustomTheme("vesper-light", async () => vesperLight as any);
}
