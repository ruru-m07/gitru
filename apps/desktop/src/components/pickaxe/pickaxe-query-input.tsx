import { Button } from "@gitru/ui/components/button";
import {
  InputGroup,
  InputGroupAddon,
  InputGroupInput,
} from "@gitru/ui/components/input-group";
import { cn } from "@gitru/ui/lib/utils";
import { CaseSensitive, Pickaxe, Regex, WholeWord } from "lucide-react";
import type { PickaxeSearchOptions } from "@/lib/pickaxe-search-options";

type PickaxeQueryInputProps = {
  value: string;
  onChange: (value: string) => void;
  onKeyDown?: React.KeyboardEventHandler<HTMLInputElement>;
  searchOptions: PickaxeSearchOptions;
  onSearchOptionsChange: (options: PickaxeSearchOptions) => void;
};

function SearchOptionToggle({
  label,
  ariaLabel,
  pressed,
  onToggle,
  className,
}: {
  label: React.ReactNode;
  ariaLabel: string;
  pressed: boolean;
  onToggle: () => void;
  className?: string;
}) {
  return (
    <Button
      type="button"
      size="icon-xs"
      variant={pressed ? "secondary" : "ghost"}
      aria-label={ariaLabel}
      aria-pressed={pressed}
      className={cn(
        "h-7 min-w-7 px-1 font-mono text-xs text-muted-foreground",
        pressed && "text-foreground",
        className,
      )}
      onClick={onToggle}
    >
      {label}
    </Button>
  );
}

export function PickaxeQueryInput({
  value,
  onChange,
  onKeyDown,
  searchOptions,
  onSearchOptionsChange,
}: PickaxeQueryInputProps) {
  const updateOption = <K extends keyof PickaxeSearchOptions>(
    key: K,
    nextValue: PickaxeSearchOptions[K],
  ) => {
    onSearchOptionsChange({
      ...searchOptions,
      [key]: nextValue,
    });
  };

  return (
    <InputGroup className="flex-1">
      <InputGroupInput
        aria-label="Pickaxe search query"
        placeholder="Search across commits and files..."
        value={value}
        onChange={(event) => onChange(event.target.value)}
        onKeyDown={onKeyDown}
        size={"sm"}
      />
      <InputGroupAddon className="pr-0.75 ml-0.5">
        <Pickaxe
          className="text-muted-foreground/85 size-3.5"
          aria-hidden="true"
        />
      </InputGroupAddon>

      <InputGroupAddon align="inline-end" className="gap-0">
        <SearchOptionToggle
          label={<CaseSensitive />}
          ariaLabel="Match case"
          pressed={searchOptions.matchCase}
          onToggle={() => updateOption("matchCase", !searchOptions.matchCase)}
        />
        <SearchOptionToggle
          label={<WholeWord />}
          ariaLabel="Match whole word"
          pressed={searchOptions.matchWholeWord}
          onToggle={() =>
            updateOption("matchWholeWord", !searchOptions.matchWholeWord)
          }
        />
        <SearchOptionToggle
          label={<Regex />}
          ariaLabel="Use regular expression"
          pressed={searchOptions.isRegex}
          onToggle={() => updateOption("isRegex", !searchOptions.isRegex)}
        />
      </InputGroupAddon>
    </InputGroup>
  );
}
