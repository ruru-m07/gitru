import { Button } from "@gitru/ui/components/button";
import { Inbox, RefreshCw, WifiOff } from "lucide-react";
import type { ReactNode } from "react";

export function CollaborationStatePanel({
  title,
  children,
  action,
  onAction,
  busy = false,
  offline = false,
}: {
  title: string;
  children: ReactNode;
  action?: string;
  onAction?: () => void;
  busy?: boolean;
  offline?: boolean;
}) {
  const Icon = offline ? WifiOff : Inbox;
  return (
    <div className="flex min-h-60 flex-1 flex-col items-center justify-center gap-3 p-6 text-center">
      <div className="rounded-xl border bg-muted/40 p-3 text-muted-foreground">
        <Icon className="size-6" aria-hidden="true" />
      </div>
      <h2 className="text-sm font-medium">{title}</h2>
      <div className="max-w-sm text-sm leading-relaxed text-muted-foreground">
        {children}
      </div>
      {action && onAction ? (
        <Button variant="outline" onClick={onAction} disabled={busy}>
          <RefreshCw className="size-4" aria-hidden="true" />
          {action}
        </Button>
      ) : null}
    </div>
  );
}
