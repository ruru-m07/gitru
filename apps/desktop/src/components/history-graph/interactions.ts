export function mountGraphInteractions(root: HTMLElement): () => void {
  let activeRowId: string | null = null;
  let tooltipTarget: HTMLElement | null = null;

  const tooltip = document.createElement("div");
  tooltip.setAttribute("role", "tooltip");
  tooltip.className =
    "pointer-events-none fixed z-50 flex max-w-72 origin-center scale-98 text-balance rounded-md border bg-popover not-dark:bg-clip-padding text-xs text-popover-foreground opacity-0 shadow-md/5 transition-[scale,opacity] duration-150 before:pointer-events-none before:absolute before:inset-0 before:rounded-[calc(var(--radius-md)-1px)] before:shadow-[0_1px_--theme(--color-black/4%)] dark:before:shadow-[0_-1px_--theme(--color-white/6%)]";
  const tooltipViewport = document.createElement("div");
  tooltipViewport.className = "relative size-full overflow-clip px-2 py-1";
  tooltip.appendChild(tooltipViewport);
  document.body.appendChild(tooltip);

  const setActiveRow = (rowId: string | null) => {
    if (rowId === activeRowId) return;
    for (const cell of root.querySelectorAll<HTMLElement>(
      "[data-row-hovered]",
    )) {
      delete cell.dataset.rowHovered;
    }
    for (const cell of root.querySelectorAll<HTMLElement>("[data-cell]")) {
      if (cell.dataset.cellId === rowId) {
        cell.dataset.rowHovered = "";
      }
    }
    activeRowId = rowId;
  };

  const hideTooltip = () => {
    tooltipTarget = null;
    tooltip.style.opacity = "0";
    tooltip.style.scale = "0.98";
  };

  const onPointerOver = (event: PointerEvent) => {
    const target = event.target as Element | null;
    const cell = target?.closest<HTMLElement>("[data-cell]");
    setActiveRow(cell?.dataset.cellId ?? null);

    const trigger = target?.closest<HTMLElement>("[data-tooltip]");
    if (!trigger || !trigger.dataset.tooltip) return;
    tooltipTarget = trigger;
    tooltipViewport.replaceChildren();
    if (trigger.dataset.tooltipAuthors) {
      const authors = JSON.parse(trigger.dataset.tooltipAuthors) as Array<{
        name: string;
        email: string;
      }>;
      const list = document.createElement("div");
      list.className = "flex flex-col gap-2";
      for (const author of authors) {
        const row = document.createElement("div");
        row.className = "flex items-center gap-1";
        const image = document.createElement("img");
        image.alt = author.name;
        image.className =
          "size-4.5 rounded-sm border object-cover ring-2 ring-background";
        image.src = `https://avatars.githubusercontent.com/u/e?email=${encodeURIComponent(author.email)}&s=64`;
        const name = document.createElement("span");
        name.textContent = author.name;
        row.append(image, name);
        list.appendChild(row);
      }
      tooltipViewport.appendChild(list);
    } else if (trigger.dataset.tooltipMutedLabel) {
      const content = document.createElement("div");
      content.className = "text-sm";
      const label = document.createElement("span");
      label.className = "text-muted-foreground";
      label.textContent = trigger.dataset.tooltipMutedLabel;
      content.append(label, ` ${trigger.dataset.tooltip}`);
      tooltipViewport.appendChild(content);
    } else {
      tooltipViewport.textContent = trigger.dataset.tooltip;
    }
    tooltip.style.opacity = "1";
    tooltip.style.scale = "1";
    const rect = trigger.getBoundingClientRect();
    const tooltipRect = tooltip.getBoundingClientRect();
    const left = Math.max(
      8,
      Math.min(
        window.innerWidth - tooltipRect.width - 8,
        rect.left + rect.width / 2 - tooltipRect.width / 2,
      ),
    );
    const preferredTop =
      trigger.dataset.tooltipSide === "bottom"
        ? rect.bottom + 4
        : rect.top - tooltipRect.height - 4;
    const top =
      preferredTop < 8
        ? rect.bottom + 4
        : preferredTop + tooltipRect.height > window.innerHeight - 8
          ? rect.top - tooltipRect.height - 4
          : preferredTop;
    tooltip.style.left = `${left}px`;
    tooltip.style.top = `${Math.max(8, top)}px`;
  };

  const onPointerOut = (event: PointerEvent) => {
    const related = event.relatedTarget as Node | null;
    if (tooltipTarget && !tooltipTarget.contains(related)) hideTooltip();
  };

  const onPointerLeave = () => {
    setActiveRow(null);
    hideTooltip();
  };

  root.addEventListener("pointerover", onPointerOver);
  root.addEventListener("pointerout", onPointerOut);
  root.addEventListener("pointerleave", onPointerLeave);
  root.addEventListener("scroll", onPointerLeave, { passive: true });

  return () => {
    root.removeEventListener("pointerover", onPointerOver);
    root.removeEventListener("pointerout", onPointerOut);
    root.removeEventListener("pointerleave", onPointerLeave);
    root.removeEventListener("scroll", onPointerLeave);
    tooltip.remove();
  };
}
