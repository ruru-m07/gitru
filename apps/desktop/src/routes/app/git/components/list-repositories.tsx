import { Button } from "@gitru/ui/components/button";
import {
  Avatar,
  AvatarFallback,
  AvatarImage,
} from "@gitru/ui/components/avatar";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@gitru/ui/components/menu";
import {
  InputGroup,
  InputGroupAddon,
  InputGroupInput,
} from "@gitru/ui/components/input-group";
import { useCommandNavigation } from "@gitru/ui/components/command";
import { open } from "@tauri-apps/plugin-dialog";
import { BadgeQuestionMark, ChevronDownIcon, SearchIcon } from "lucide-react";
import { memo, useState } from "react";
import { toast } from "sonner";
import { RepositoryListItem } from "@/components/RepositoryListItem";
import { useRepositories } from "@/hooks/useRepositories";
import { getAvatarByProvider } from "@/lib/getAvatarByGitProvider";
import { parseOrigin } from "@/lib/parseOrigin";
import { selectActiveRepository, useAppStore } from "@/store/useAppStore";
import type { GIT_PROVIDERS } from "@/types/app";
import { matchesSearchQuery } from "../lib/matches-search-query";

export const ListRepositories = memo(() => {
  const navigation = useCommandNavigation();
  const activeRepository = useAppStore(selectActiveRepository);
  const setSelectedRepository = useAppStore(
    (state) => state.setSelectedRepository,
  );
  const setRepoSelectIsOpen = useAppStore((state) => state.setRepoSelectIsOpen);

  const { repositories, addRepo, removeRepo } = useRepositories();
  const [query, setQuery] = useState("");

  const normalizedQuery = query.trim();
  const filteredRepositories = normalizedQuery
    ? repositories.filter((repo) => {
        const name = repo.name ?? "";
        const path = repo.path ?? "";
        return (
          matchesSearchQuery(name, normalizedQuery) ||
          matchesSearchQuery(path, normalizedQuery)
        );
      })
    : repositories;

  /* we are grouping repositories by owners */
  // const origin = parseOrigin(item.origin || "");
  // const icon = getAvatarByProvider(origin?.provider);
  const groupedByOwner = filteredRepositories.reduce(
    (acc, repo) => {
      const origin = parseOrigin(repo.origin || "");
      const owner = origin?.owner || "unknown";
      const provider = origin?.provider || "unknown";
      const key = `${provider}/${owner}`;

      if (!acc[key]) {
        acc[key] = {
          owner,
          provider,
          avatarUrl: origin?.avatarUrl,
          repos: [],
        };
      }
      acc[key].repos.push(repo);
      return acc;
    },
    {} as Record<
      string,
      {
        owner: string;
        provider: GIT_PROVIDERS;
        avatarUrl?: string;
        repos: typeof repositories;
      }
    >,
  );

  return (
    <div>
      <div className="w-full p-2 border-b flex justify-between items-center gap-2">
        <InputGroup>
          <InputGroupInput
            aria-label="Search"
            placeholder="Search"
            type="search"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
          />
          <InputGroupAddon>
            <SearchIcon aria-hidden="true" />
          </InputGroupAddon>
        </InputGroup>

        <DropdownMenu>
          <DropdownMenuTrigger render={<Button size={"sm"} />}>
            Add
            <ChevronDownIcon
              className="-me-1 opacity-60"
              size={16}
              aria-hidden="true"
            />
          </DropdownMenuTrigger>
          <DropdownMenuContent align="end">
            <DropdownMenuItem
              onClick={() => {
                navigation.setOpen(true);
                navigation.push("clone-repository");
              }}
            >
              Clone repository...
            </DropdownMenuItem>
            <DropdownMenuItem
              onClick={() => {
                navigation.setOpen(true);
                navigation.push("init-repository");
              }}
            >
              Initialize repository...
            </DropdownMenuItem>
            <DropdownMenuItem
              onClick={async () => {
                const folder = await open({
                  directory: true,
                  multiple: false,
                });

                if (folder) {
                  if (repositories.find((r) => r.path === folder)) {
                    toast.error("Repository already added");
                    return;
                  }

                  try {
                    const repo = await addRepo(folder);
                    if (repo) {
                      setSelectedRepository(repo);
                      setRepoSelectIsOpen(false);
                      toast.success("Repository added successfully!");
                    }
                  } catch (error) {
                    // Error already handled by the hook
                  }
                }
              }}
            >
              Add local repository
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
      </div>
      <div className="max-h-full _flex-1">
        <div className="">
          {groupedByOwner &&
            Object.entries(groupedByOwner)
              .filter(([, repos]) => repos.repos.length > 0)
              .map(([owner, repos]) => {
                const icon = getAvatarByProvider(repos?.provider || undefined);
                const hasKnownOrigin = repos.provider !== "unknown";

                return (
                  <div key={owner} className="border-b">
                    <div className="text-muted-foreground flex items-center px-2 py-1">
                      <div className="size-3.5 text-lg text-foreground mr-1">
                        {icon || <BadgeQuestionMark className="size-3.5" />}
                      </div>
                      {hasKnownOrigin ? (
                        <div>
                          <span>/</span>
                          <Avatar className="rounded-sm size-4 -translate-y-px mx-1">
                            <AvatarImage alt="User" src={repos.avatarUrl} />
                            <AvatarFallback>
                              {owner.charAt(0).toUpperCase()}
                            </AvatarFallback>
                          </Avatar>
                          <span>{repos.owner}</span>
                        </div>
                      ) : (
                        <div>
                          <span className="text-foreground">{owner}</span>
                        </div>
                      )}
                    </div>
                    {repos.repos.map((repo) => (
                      <RepositoryListItem
                        key={repo.id}
                        repo={repo}
                        dataRepoId={repo.id}
                        isSelected={activeRepository?.id === repo.id}
                        onSelect={() => {
                          setSelectedRepository(repo);
                          setRepoSelectIsOpen(false);
                        }}
                        onRemove={() => {
                          removeRepo(repo.id);
                          if (activeRepository?.id === repo.id) {
                            setSelectedRepository(null);
                          }
                        }}
                      />
                    ))}
                  </div>
                );
              })}
        </div>
      </div>
    </div>
  );
});
