import type { PickaxeHit } from "@/hooks/use-pickaxe";
import type { PickaxeSearchOptions } from "@/lib/pickaxe-search-options";
import { PickaxeDiffPreview } from "./pickaxe-diff-preview";

export function PickaxeResultCard({
  hit,
  searchQuery,
  searchOptions,
}: {
  hit: PickaxeHit;
  searchQuery: string;
  searchOptions: PickaxeSearchOptions;
}) {
  return (
    <article className="overflow-hidden">
      <PickaxeDiffPreview
        hit={hit}
        searchQuery={searchQuery}
        searchOptions={searchOptions}
      />
    </article>
  );
}
