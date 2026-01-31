"use client";

import { cn } from "@gitru/ui/lib/utils";
import { CircleAlertIcon } from "lucide-react";
import * as React from "react";
import type { CommandViewConfig, CommandViewContext } from "./types.js";

class CommandViewErrorBoundary extends React.Component<
  {
    fallback?: (error: Error) => React.ReactNode;
    children: React.ReactNode;
  },
  { error: Error | null }
> {
  state = { error: null } as { error: Error | null };

  static getDerivedStateFromError(error: Error) {
    return { error };
  }

  componentDidCatch() {
    // Intentionally empty; error is rendered in panel.
  }

  render() {
    const { error } = this.state;
    if (error) {
      return this.props.fallback ? (
        this.props.fallback(error)
      ) : (
        <CommandPanelError message={error.message} />
      );
    }
    return this.props.children;
  }
}

function CommandPanelError({
  message = "Something went wrong.",
  className,
}: {
  message?: string;
  className?: string;
}) {
  return (
    <div
      aria-live="polite"
      className={cn(
        "flex items-start gap-2 text-destructive text-sm",
        className,
      )}
      role="alert"
    >
      <CircleAlertIcon className="mt-0.5 size-4" />
      <span>{message}</span>
    </div>
  );
}

interface CommandViewRendererProps<
  TConfig extends CommandViewConfig = CommandViewConfig,
> {
  view: TConfig;
  context: CommandViewContext;
}

function CommandViewRenderer<TConfig extends CommandViewConfig>({
  view,
  context,
}: CommandViewRendererProps<TConfig>) {
  const render = view?.render ?? (() => null);
  const fallback = view?.suspenseFallback;

  return (
    <CommandViewErrorBoundary fallback={view?.errorFallback}>
      <React.Suspense fallback={fallback ?? null}>
        {render(context)}
      </React.Suspense>
    </CommandViewErrorBoundary>
  );
}

export { CommandPanelError, CommandViewRenderer };
