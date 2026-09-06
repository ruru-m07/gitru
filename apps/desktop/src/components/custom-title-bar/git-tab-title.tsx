import { type RepositoryInfo } from "@gitru/commands";
import { Git } from "@gitru/icon";
import {
  Avatar,
  AvatarFallback,
  AvatarImage,
} from "@gitru/ui/components/avatar";
import { cn } from "@gitru/ui/lib/utils";

import { getAvatarByProvider } from "@/lib/get-avatar-by-git-provider";
import { parseOrigin } from "@/lib/parse-origin";

export const GitTabTitle = ({
  repository,
  isActive,
}: {
  repository: RepositoryInfo | null;
  isActive: boolean;
}) => {
  if (!repository) {
    return (
      <div className="flex min-w-0 items-center gap-1.5 truncate font-medium">
        <Git className="size-4 shrink-0" />
        <span className="truncate font-medium">Git Repository</span>
      </div>
    );
  }

  const origin = parseOrigin(repository.origin);

  if (!origin) {
    return (
      <span className="truncate font-medium">
        {repository.name}
        {repository.current_branch ? ` > ${repository.current_branch}` : ""}
      </span>
    );
  }

  const providerIcon = getAvatarByProvider(
    origin.provider,
    cn("size-2.5 rounded-full", isActive ? "" : "bg-secondary"),
  );
  const textClass = isActive ? "text-foreground" : "text-muted-foreground";

  return (
    <div className="flex min-w-0 items-center">
      <div className="relative shrink-0">
        <Avatar className="size-4.5 rounded-sm">
          <AvatarImage alt={origin.owner} src={origin.avatarUrl} />
          <AvatarFallback>
            {origin.owner.charAt(0).toUpperCase()}
          </AvatarFallback>
        </Avatar>
        {providerIcon ? (
          <span
            className={cn(
              "absolute -inset-e-1 -bottom-1 rounded-full p-0.5",
              isActive ? "bg-background" : "bg-secondary",
            )}
          >
            {providerIcon}
          </span>
        ) : null}
      </div>

      <span className="ml-2 flex min-w-0 items-center gap-1 text-sm">
        <span className="truncate text-muted-foreground">{origin.owner}</span>
        <span className="text-muted-foreground">/</span>
        <span className={cn("truncate font-medium", textClass)}>
          {origin.repo}
        </span>
        <span className="text-muted-foreground mx-1">&gt;</span>
        <span className={cn("flex min-w-0 items-center gap-1", textClass)}>
          <span className="truncate">
            {repository.current_branch ?? "detached"}
          </span>
        </span>
      </span>
    </div>
  );
};
