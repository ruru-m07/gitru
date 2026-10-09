import { Button } from "@gitru/ui/components/button";
import { ExternalLink } from "lucide-react";
import { useState } from "react";
import { normalizeExternalHttpsUrl } from "@/lib/external-content";
import { openExternalUrlSafely } from "@/lib/open-external-url";

/** Render provider content as text; only validated HTTPS URLs reach the opener. */
export function ProviderLink({ url }: { url: string | null | undefined }) {
  const normalized = url ? normalizeExternalHttpsUrl(url) : null;
  const [failed, setFailed] = useState(false);
  if (!normalized) return null;

  return (
    <div>
      <Button
        variant="outline"
        size="sm"
        onClick={() => {
          setFailed(false);
          void openExternalUrlSafely(normalized).then(
            (opened) => setFailed(!opened),
            () => setFailed(true),
          );
        }}
      >
        <ExternalLink aria-hidden="true" />
        Open on provider
      </Button>
      {failed ? (
        <p role="alert" className="mt-2 text-xs text-destructive-foreground">
          Could not open your browser. Try again.
        </p>
      ) : null}
    </div>
  );
}
