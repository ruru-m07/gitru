import {
  CommandListGroup,
  CommandListView,
  CommandViewConfig,
} from "@gitru/ui/components/command";
import { Kbd } from "@gitru/ui/components/kbd";
import { cn } from "@gitru/ui/lib/utils";
import { CheckIcon, CornerDownLeftIcon } from "lucide-react";
import { useTheme } from "next-themes";
import { useEffect, useState } from "react";
import { Colors, type ColorsType } from "@/lib/colors";

export type ThemeItem = (typeof Colors)[number];

export function useSwitchThemeView(): CommandViewConfig<
  "switch-theme",
  ThemeItem
> {
  const { theme, setTheme } = useTheme();
  const [mounted, setMounted] = useState(false);

  useEffect(() => {
    setMounted(true);
  }, []);

  return {
    id: "switch-theme",
    input: {
      placeholder: "Search themes...",
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
      items: () => [...Colors],
      getItemValue: (item) => `${item.name} ${item.key}`.trim(),
      filter: (value: unknown, query: string) => {
        if (!query) return true;
        const q = query.toLowerCase();
        return String(value).toLowerCase().includes(q);
      },
    },
    render: (context) => {
      const themes = (context.filteredCommandItems as
        | ThemeItem[]
        | undefined) ?? [...Colors];

      const groups = [
        {
          id: "light-themes",
          label: "Light",
          items: themes.filter((item) => !item.key.startsWith("dark-")),
        },
        {
          id: "dark-themes",
          label: "Dark",
          items: themes.filter((item) => item.key.startsWith("dark-")),
        },
      ].filter(
        (group) => group.items.length > 0,
      ) satisfies CommandListGroup<ThemeItem>[];

      return (
        <CommandListView<ThemeItem>
          groups={groups}
          itemsArePreFiltered={context.filteredCommandItems !== undefined}
          showSeparators={false}
          estimateItemSize={72}
          getItemKey={(item) => item.key}
          getItemValue={(item) => `${item.name} ${item.key}`.trim()}
          renderItemContent={(item) => {
            const selected = mounted && theme === item.key;

            return (
              <div
                className={cn(
                  item.key,
                  "w-full flex items-center justify-between",
                )}
              >
                <div className="flex items-center gap-2">
                  {["primary"].map((colorName) => (
                    <div
                      key={colorName}
                      className="h-5 w-5 rounded-sm ring-1 ring-inset ring-white/15"
                      style={{
                        backgroundColor: `var(--${colorName})`,
                      }}
                    />
                  ))}
                  <span className="text-sm font-medium">{item.name}</span>
                </div>
                {selected ? <CheckIcon className="size-4" /> : null}
              </div>
            );
          }}
          onSelect={(item) => {
            setTheme(item.key as ColorsType);
            context.close();
          }}
        />
      );
    },
  };
}
