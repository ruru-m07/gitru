import { useSortable } from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";
import { type ReactNode } from "react";

import { SORTABLE_TAB_TRANSITION, TAB_RESIZE_TRANSITION } from "./constants";

type SortableTabShellProps = {
  id: string;
  className?: string;
  children: ReactNode;
  shouldAnimateWidth: boolean;
  width: number;
};

export const SortableTabShell = ({
  id,
  className,
  children,
  shouldAnimateWidth,
  width,
}: SortableTabShellProps) => {
  const {
    attributes,
    listeners,
    setNodeRef,
    transform,
    transition,
    isDragging,
  } = useSortable({
    id,
    transition: SORTABLE_TAB_TRANSITION,
  });
  const composedTransition = [
    transition,
    shouldAnimateWidth ? TAB_RESIZE_TRANSITION : null,
  ]
    .filter(Boolean)
    .join(", ");

  return (
    <div
      ref={setNodeRef}
      data-tab-id={id}
      style={{
        flex: "0 0 auto",
        transform: CSS.Transform.toString(transform),
        transition: composedTransition || undefined,
        width,
        willChange: shouldAnimateWidth ? "width" : undefined,
        zIndex: isDragging ? 30 : undefined,
      }}
      className={className}
      {...attributes}
      {...listeners}
    >
      {children}
    </div>
  );
};
