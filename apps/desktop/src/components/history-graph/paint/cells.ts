import type { Branch } from "@gitru/commands";
import type { ColumnSlot } from "@gitru/dom-virtual";
import { timeAgoFromUnixSeconds } from "@/lib/time";
import type { ProcessedRow } from "../helper";

const SVG_NS = "http://www.w3.org/2000/svg";
const CELL = "flex items-center min-w-0";

type IconName = "branch" | "cloud" | "current" | "file" | "push" | "tag";

type IconNode = readonly ["path" | "circle", Readonly<Record<string, string>>];

const ICON_NODES: Record<IconName, readonly IconNode[]> = {
  branch: [
    ["path", { d: "M15 6a9 9 0 0 0-9 9V3" }],
    ["circle", { cx: "18", cy: "6", r: "3" }],
    ["circle", { cx: "6", cy: "18", r: "3" }],
  ],
  cloud: [
    ["path", { d: "M17.5 19H9a7 7 0 1 1 6.71-9h1.79a4.5 4.5 0 1 1 0 9Z" }],
  ],
  current: [
    ["path", { d: "M10.1 2.18a9.93 9.93 0 0 1 3.8 0" }],
    ["path", { d: "M17.6 3.71a9.95 9.95 0 0 1 2.69 2.7" }],
    ["path", { d: "M21.82 10.1a9.93 9.93 0 0 1 0 3.8" }],
    ["path", { d: "M20.29 17.6a9.95 9.95 0 0 1-2.7 2.69" }],
    ["path", { d: "M13.9 21.82a9.94 9.94 0 0 1-3.8 0" }],
    ["path", { d: "M6.4 20.29a9.95 9.95 0 0 1-2.69-2.7" }],
    ["path", { d: "M2.18 13.9a9.93 9.93 0 0 1 0-3.8" }],
    ["path", { d: "M3.71 6.4a9.95 9.95 0 0 1 2.7-2.69" }],
    ["circle", { cx: "12", cy: "12", r: "1" }],
  ],
  file: [
    [
      "path",
      {
        d: "M6 22a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h8a2.4 2.4 0 0 1 1.704.706l3.588 3.588A2.4 2.4 0 0 1 20 8v12a2 2 0 0 1-2 2z",
      },
    ],
    ["path", { d: "M9 10h6" }],
    ["path", { d: "M12 13V7" }],
    ["path", { d: "M9 17h6" }],
  ],
  push: [
    ["path", { d: "m18 9-6-6-6 6" }],
    ["path", { d: "M12 3v14" }],
    ["path", { d: "M5 21h14" }],
  ],
  tag: [
    [
      "path",
      {
        d: "M12.586 2.586A2 2 0 0 0 11.172 2H4a2 2 0 0 0-2 2v7.172a2 2 0 0 0 .586 1.414l8.704 8.704a2.426 2.426 0 0 0 3.42 0l6.58-6.58a2.426 2.426 0 0 0 0-3.42z",
      },
    ],
    ["circle", { cx: "7.5", cy: "7.5", r: ".5", fill: "currentColor" }],
  ],
};

function icon(
  name: IconName,
  className = "size-3.5 shrink-0",
  strokeWidth = 1.2,
) {
  const svg = document.createElementNS(SVG_NS, "svg");
  svg.setAttribute("viewBox", "0 0 24 24");
  svg.setAttribute("fill", "none");
  svg.setAttribute("stroke", "currentColor");
  svg.setAttribute("stroke-width", String(strokeWidth));
  svg.setAttribute("stroke-linecap", "round");
  svg.setAttribute("stroke-linejoin", "round");
  svg.setAttribute("aria-hidden", "true");
  svg.setAttribute("class", className);
  for (const [tagName, attributes] of ICON_NODES[name]) {
    const node = document.createElementNS(SVG_NS, tagName);
    for (const [attribute, value] of Object.entries(attributes)) {
      node.setAttribute(attribute, value);
    }
    svg.appendChild(node);
  }
  return svg;
}

function initials(name: string) {
  return name
    .split(/\s+/)
    .filter(Boolean)
    .map((part) => part[0])
    .join("")
    .slice(0, 2)
    .toUpperCase();
}

function avatar(name: string, email: string, extraClass = "") {
  const root = document.createElement("span");
  root.className = `relative inline-flex size-4.5 shrink-0 select-none items-center justify-center overflow-hidden rounded-sm border bg-background align-middle text-xs font-medium ring-2 ring-background ${extraClass}`;
  root.textContent = initials(name);
  root.dataset.tooltip = name;
  root.dataset.tooltipSide = "bottom";

  const image = document.createElement("img");
  image.alt = name;
  image.loading = "lazy";
  image.decoding = "async";
  image.className = "absolute inset-0 size-full object-cover";
  image.src = `https://avatars.githubusercontent.com/u/e?email=${encodeURIComponent(email)}&s=64`;
  image.addEventListener("error", () => image.remove(), { once: true });
  root.appendChild(image);
  return root;
}

function bindCell(element: HTMLElement, row: ProcessedRow) {
  element.dataset.cell = "";
  element.dataset.cellId = row.row.oid;
  delete element.dataset.rowHovered;
}

function branchBadge(
  row: ProcessedRow,
  label: string,
  kind: "local" | "remote" | "tag",
  current: boolean,
) {
  const badge = document.createElement("span");
  badge.className =
    "relative z-10 flex min-w-0 w-fit items-center justify-center gap-1 rounded-sm px-1.5 py-1";
  badge.style.backgroundColor = `color-mix(in oklab, ${row.color} 20%, var(--color-background))`;
  badge.style.setProperty(
    "--icon-color",
    `color-mix(in oklab, ${row.color} 50%, var(--color-foreground))`,
  );
  badge.style.color = "var(--icon-color)";
  badge.appendChild(
    icon(
      current
        ? "current"
        : kind === "local"
          ? "branch"
          : kind === "remote"
            ? "cloud"
            : "tag",
    ),
  );
  const text = document.createElement("span");
  text.className = "min-w-0 truncate text-xs";
  text.textContent = label;
  badge.appendChild(text);
  return badge;
}

export function createBranchSlot(
  getCurrentBranch: () => Branch | null,
  getPushEnabled: () => boolean,
): ColumnSlot<ProcessedRow> {
  const element = document.createElement("div");
  element.className = `${CELL} relative max-w-full gap-1 overflow-hidden pl-1 pr-2 text-nowrap`;

  return {
    element,
    paint(row) {
      bindCell(element, row);
      element.replaceChildren();
      const currentBranch = getCurrentBranch();
      const currentBranchRef =
        row.row.refs.find((ref) => ref.kind === "Local")?.display_name ===
        currentBranch?.name;
      const isCurrent = row.row.refs.some(
        (ref) => ref.display_name === currentBranch?.name,
      );

      for (const ref of row.localBranchRefs) {
        element.appendChild(
          branchBadge(row, ref.display_name, "local", currentBranchRef),
        );
      }
      for (const ref of row.remoteRefs) {
        element.appendChild(
          branchBadge(row, ref.display_name, "remote", currentBranchRef),
        );
      }
      for (const ref of row.tags) {
        element.appendChild(
          branchBadge(row, ref.display_name, "tag", currentBranchRef),
        );
      }

      if (row.row.refs.length === 0 && row.branchRefs.length > 0) {
        const hidden = document.createElement("div");
        hidden.dataset.hiddenBranchRefs = "";
        hidden.className =
          "hidden min-w-0 items-center gap-1 overflow-hidden opacity-70";
        for (const ref of row.branchRefs) {
          hidden.appendChild(
            branchBadge(
              row,
              ref.display_name,
              ref.kind === "Local" ? "local" : "remote",
              currentBranchRef,
            ),
          );
        }
        element.appendChild(hidden);
      }

      if (row.row.refs.length > 0) {
        const railWrapper = document.createElement("span");
        railWrapper.className = "absolute right-0 top-0 z-10 h-full py-0.5";
        const rail = document.createElement("span");
        rail.className = "block h-full w-0.5";
        rail.style.background = isCurrent
          ? `repeating-linear-gradient(0deg,color-mix(in oklab,${row.color} 90%,var(--color-background)) 0 2px,var(--color-background) 2px 4px)`
          : `color-mix(in oklab,${row.color} 90%,var(--color-background))`;
        railWrapper.appendChild(rail);
        element.appendChild(railWrapper);

        const connector = document.createElement("span");
        connector.className = "absolute left-1 right-0 z-0 h-0.5";
        connector.style.top = "15px";
        connector.style.background = `repeating-linear-gradient(to right,color-mix(in oklab,${row.color} 30%,var(--color-background)) 0 6px,transparent 4px 8px)`;
        element.appendChild(connector);
      }

      if (isCurrent && getPushEnabled()) {
        const button = document.createElement("button");
        button.type = "button";
        button.ariaLabel = "Push current branch";
        button.className =
          "relative z-10 inline-flex size-7 min-w-0 w-fit shrink-0 cursor-pointer! select-none items-center justify-center gap-1 whitespace-nowrap rounded-sm border border-dashed border-(--icon-color)/45! bg-popover px-1.5 py-1 font-medium text-foreground outline-none shadow-xs/5 transition-shadow hover:bg-[color-mix(in_oklab,var(--icon-color)_20%,var(--color-background))]! focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-1";
        button.style.setProperty(
          "--icon-color",
          `color-mix(in oklab, ${row.color} 50%, var(--color-foreground))`,
        );
        button.style.color = "var(--icon-color)";
        button.style.backgroundColor = `color-mix(in oklab, ${row.color} 20%, var(--color-background))`;
        button.appendChild(icon("push"));
        element.appendChild(button);
      }
    },
  };
}

export function createSummarySlot(): ColumnSlot<ProcessedRow> {
  const element = document.createElement("div");
  element.className = `${CELL} relative gap-2 px-2`;
  const wrapper = document.createElement("span");
  wrapper.className = "flex min-w-0 items-center";
  const text = document.createElement("span");
  text.className = "min-w-0 truncate text-sm";
  wrapper.appendChild(text);

  const railWrapper = document.createElement("span");
  railWrapper.className = "absolute left-0 top-0 h-full py-0.5";
  const rail = document.createElement("span");
  rail.className = "block h-full w-0.5";
  railWrapper.appendChild(rail);
  element.append(wrapper, railWrapper);

  return {
    element,
    paint(row) {
      bindCell(element, row);
      text.textContent = row.row.commit.summary;
      rail.style.background = `repeating-linear-gradient(0deg,color-mix(in oklab,${row.color} 90%,var(--color-background)) 0 1.5px,transparent 1.5px 3px)`;
    },
  };
}

export function createCommittersSlot(): ColumnSlot<ProcessedRow> {
  const element = document.createElement("div");
  element.className = `${CELL} gap-2 border-l px-2`;
  return {
    element,
    paint(row) {
      bindCell(element, row);
      element.replaceChildren();
      const authors = row.row.commit.authors;
      const stack = document.createElement("div");
      stack.className = "group flex items-center";
      const authorAvatar = avatar(authors.author.name, authors.author.email);
      authorAvatar.style.zIndex = String(authors.co_authors.length + 1);
      stack.appendChild(authorAvatar);
      for (const [index, coAuthor] of authors.co_authors
        .slice(0, 2)
        .entries()) {
        const coAuthorAvatar = avatar(
          coAuthor.name,
          coAuthor.email,
          "ml-[-0.2rem] will-change-auto transition-all duration-100 group-hover:ml-0.5",
        );
        coAuthorAvatar.style.zIndex = String(authors.co_authors.length - index);
        stack.appendChild(coAuthorAvatar);
      }
      if (authors.co_authors.length > 2) {
        const more = document.createElement("span");
        more.className =
          "-ml-1 flex h-4.5 items-center rounded-sm bg-secondary px-1 font-mono text-xs text-nowrap tabular-nums ring-2 ring-background transition-all duration-100 will-change-auto";
        more.textContent = `+${authors.co_authors.length - 2}`;
        more.dataset.tooltip = `${authors.co_authors.length - 2} more co-authors`;
        more.dataset.tooltipAuthors = JSON.stringify(
          authors.co_authors.slice(2),
        );
        more.dataset.tooltipSide = "bottom";
        stack.appendChild(more);
      }
      element.appendChild(stack);
    },
  };
}

export function createTimestampSlot(): ColumnSlot<ProcessedRow> {
  const element = document.createElement("div");
  element.className = `${CELL} gap-2 border-l px-2`;
  const wrapper = document.createElement("div");
  wrapper.className =
    "flex min-w-0 items-center justify-center gap-1 text-muted-foreground";
  const text = document.createElement("span");
  text.className =
    "min-w-0 shrink-0 truncate text-nowrap text-sm text-muted-foreground";
  wrapper.appendChild(text);
  element.appendChild(wrapper);
  return {
    element,
    paint(row) {
      bindCell(element, row);
      text.textContent = timeAgoFromUnixSeconds(row.row.commit.timestamp);
    },
  };
}

export function createHashSlot(): ColumnSlot<ProcessedRow> {
  const element = document.createElement("span");
  element.className = `${CELL} border-l px-2 font-mono text-sm text-muted-foreground`;
  return {
    element,
    paint(row) {
      bindCell(element, row);
      element.textContent = row.row.commit.id.slice(0, 7);
    },
  };
}

function diffBoxes(insertions: number, deletions: number) {
  const total = insertions + deletions;
  const filled = Math.min(total, 5);
  const green = total === 0 ? 0 : Math.round((insertions / total) * filled);
  return { green, red: filled - green, empty: 5 - filled };
}

export function createStatsSlot(): ColumnSlot<ProcessedRow> {
  const element = document.createElement("div");
  element.className = `${CELL} justify-between gap-2 border-l px-2`;
  return {
    element,
    paint(row) {
      bindCell(element, row);
      element.replaceChildren();
      const stats = row.row.commit.stats;

      const files = document.createElement("span");
      files.className =
        "flex items-center gap-1 font-mono text-sm tabular-nums text-muted-foreground";
      files.append(
        icon("file", "size-4 opacity-75", 2),
        String(stats.files_changed),
      );

      const values = document.createElement("span");
      values.className = "flex items-center gap-2 font-mono text-sm";
      const plus = document.createElement("span");
      plus.className = "tabular-nums text-green-600";
      plus.textContent = `+${stats.insertions}`;
      const minus = document.createElement("span");
      minus.className = "tabular-nums text-red-600";
      minus.textContent = `-${stats.deletions}`;
      values.append(plus, minus);

      const boxes = document.createElement("span");
      boxes.className = "flex gap-px";
      const counts = diffBoxes(stats.insertions, stats.deletions);
      for (const [kind, count] of Object.entries(counts)) {
        for (let index = 0; index < count; index++) {
          const box = document.createElement("span");
          box.className =
            kind === "green"
              ? "size-3 rounded-[4px] border border-green-700 bg-green-600"
              : kind === "red"
                ? "size-3 rounded-[4px] border border-red-700 bg-[repeating-linear-gradient(-45deg,var(--color-red-600)_0px,var(--color-red-600)_2px,color-mix(in_oklab,var(--color-red-600)_25%,transparent)_2px,color-mix(in_oklab,var(--color-red-600)_25%,transparent)_4px)]"
                : "size-3 rounded-[4px] border border-secondary-foreground/10 bg-secondary";
          boxes.appendChild(box);
        }
      }

      const right = document.createElement("span");
      right.className = "flex items-center justify-end gap-2";
      right.append(values, boxes);
      element.append(files, right);
    },
  };
}
