import { useSortable } from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";
import { type ReactNode } from "react";

import { SORTABLE_TAB_TRANSITION } from "./constants";

type SortableTabShellProps = {
  id: string;
  className?: string;
  children: ReactNode;
};

export const SortableTabShell = ({
  id,
  className,
  children,
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

  return (
    <div
      ref={setNodeRef}
      data-tab-id={id}
      style={{
        transform: CSS.Transform.toString(transform),
        transition,
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
