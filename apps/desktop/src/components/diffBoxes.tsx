import { cn } from "@gitru/ui/lib/utils";

function getDiffBoxes(insertions: number, deletions: number) {
  const total = insertions + deletions;
  if (total === 0) return { green: 0, red: 0, empty: 5 };
  const filled = Math.min(total, 5);
  const green = Math.round((insertions / total) * filled);
  const red = filled - green;
  const empty = 5 - filled;
  return { green, red, empty };
}

type DiffStatProps = {
  stats: {
    insertions: number;
    deletions: number;
  };
};

export function DiffStat({ stats }: DiffStatProps) {
  const { green, red, empty } = getDiffBoxes(stats.insertions, stats.deletions);
  const boxes = [
    ...Array(green).fill("green"),
    ...Array(red).fill("red"),
    ...Array(empty).fill("empty"),
  ];

  return (
    <div style={{ display: "flex", gap: 1 }}>
      {boxes.map((type, i) => (
        <div
          key={i}
          className={cn(
            "size-3 border rounded-[4px]",
            type === "green" && "bg-green-600 border-green-700",
            type === "red" &&
              "bg-[repeating-linear-gradient(-45deg,var(--color-red-600)_0px,var(--color-red-600)_2px,color-mix(in_oklab,var(--color-red-600)_25%,transparent)_2px,color-mix(in_oklab,var(--color-red-600)_25%,transparent)_4px)] border-red-700",
            type === "empty" && "bg-secondary border-secondary-foreground/10",
          )}
        />
      ))}
    </div>
  );
}
