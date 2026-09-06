import { openExternalUrl } from "@gitru/commands";
import { normalizeExternalHttpsUrl } from "./external-content";

export const openExternalUrlSafely = async (value: string) => {
  const url = normalizeExternalHttpsUrl(value);
  if (!url) return false;

  await openExternalUrl({ url });
  return true;
};
