import {
  CommandListView,
  CommandViewConfig,
} from "@gitru/ui/components/command";
import { Kbd } from "@gitru/ui/components/kbd";
import { CheckIcon, CornerDownLeftIcon } from "lucide-react";
import { toast } from "sonner";
import { type UpdateChannel, useAppStore } from "@/store/useAppStore";

export type UpdateChannelItem = {
  id: UpdateChannel;
  label: string;
  keywords: string[];
};

const CHANNELS: UpdateChannelItem[] = [
  { id: "stable", label: "Stable", keywords: ["stable", "release", "production"] },
  { id: "beta", label: "Beta", keywords: ["beta", "test", "preview"] },
];

export function useSwitchUpdateChannelView(): CommandViewConfig<
  "switch-update-channel",
  UpdateChannelItem
> {
  const currentChannel = useAppStore((s) => s.updateChannel);
  const setUpdateChannel = useAppStore((s) => s.setUpdateChannel);

  return {
    id: "switch-update-channel",
    input: {
      placeholder: "Search update channels...",
      autoFocus: true,
    },
    footer() {
      return (
        <>
          <div className="flex items-center gap-2">
            <span>Back</span>
            <Kbd>Esc</Kbd>
          </div>
          <div className="flex items-center gap-2">
            <span>Select</span>
            <Kbd>
              <CornerDownLeftIcon />
            </Kbd>
          </div>
        </>
      );
    },
    command: {
      items: () => CHANNELS,
      getItemValue: (item) =>
        `${item.label} ${item.id} ${item.keywords.join(" ")}`.trim(),
      filter: (value: unknown, query: string) => {
        if (!query) return true;
        const q = query.toLowerCase();
        return String(value).toLowerCase().includes(q);
      },
    },
    render: (context) => {
      const channels =
        (context.filteredCommandItems as UpdateChannelItem[] | undefined) ??
        CHANNELS;

      return (
        <CommandListView<UpdateChannelItem>
          items={channels}
          itemsArePreFiltered={context.filteredCommandItems !== undefined}
          showSeparators={false}
          getItemKey={(item) => item.id}
          getItemValue={(item) =>
            `${item.label} ${item.id} ${item.keywords.join(" ")}`.trim()
          }
          renderItemContent={(item) => (
            <div className="w-full flex items-center justify-between">
              <div className="flex items-center gap-2">
                <span className="text-sm font-medium">{item.label}</span>
                <span className="text-xs text-muted-foreground uppercase">
                  {item.id}
                </span>
              </div>
              {currentChannel === item.id ? <CheckIcon className="size-4" /> : null}
            </div>
          )}
          onSelect={(item) => {
            setUpdateChannel(item.id);
            toast.success(
              `Update channel set to ${item.label}. Restart to apply automatic checks.`,
            );
            context.close();
          }}
        />
      );
    },
  };
}
