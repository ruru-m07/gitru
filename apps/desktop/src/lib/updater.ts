import { invoke } from "@tauri-apps/api/core";

export type UpdateChannel = "stable" | "beta";

export type UpdateCheckResponse = {
  available: boolean;
  channel: UpdateChannel;
  current_version: string;
  version?: string | null;
  notes?: string | null;
  pub_date?: string | null;
};

export const checkForUpdateByChannel = async (
  channel: UpdateChannel,
): Promise<UpdateCheckResponse> => {
  return invoke<UpdateCheckResponse>("check_for_update_by_channel", { channel });
};

export const installUpdateByChannel = async (
  channel: UpdateChannel,
): Promise<string> => {
  return invoke<string>("install_update_by_channel", { channel });
};
