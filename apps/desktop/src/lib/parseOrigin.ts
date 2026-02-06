import { GIT_PROVIDERS } from "@/type";

interface ParseOriginResult {
  host: string;
  owner: string;
  repo: string;
  protocol: string;
  provider: "github" | "gitlab" | "bitbucket" | "unknown";
  avatarUrl?: string;
}

/**
 * if we get origin like
 * git@github.com:ruru-m07/gitru.git or
 * https://github.com/ruru-m07/gitru.git
 *
 * we need to parse origin to get github.com
 * @return {ParseOriginResult}
 */
export function parseOrigin(
  origin: string | undefined,
): ParseOriginResult | undefined {
  if (!origin) return undefined;

  let host = "";
  let owner = "";
  let repo = "";
  let protocol = "";
  let provider: GIT_PROVIDERS = "unknown";
  let avatarUrl: string | undefined = undefined;

  if (origin.startsWith("git@")) {
    // ? SSH format
    const match = origin.match(/^git@([^:]+):([^/]+)\/(.+?)(\.git)?$/);
    if (match) {
      host = match[1];
      owner = match[2];
      repo = match[3];
      protocol = "ssh";
    }
  } else {
    // ? HTTPS format
    const url = new URL(origin);
    host = url.hostname;
    protocol = url.protocol.replace(":", "");
    const pathParts = url.pathname.replace(/^\/|\.git$/g, "").split("/");
    if (pathParts.length >= 2) {
      owner = pathParts[0];
      repo = pathParts[1];
    }
  }

  // ? Determine provider
  if (host.includes("github.com")) {
    provider = "github";
    avatarUrl = owner ? `https://github.com/${owner}.png` : undefined;
  } else if (host.includes("gitlab.com")) {
    provider = "gitlab";
    // TODO: GitLab avatar URL pattern
    avatarUrl = owner ? `https://github.com/${owner}.png` : undefined;
  } else if (host.includes("bitbucket.org")) {
    provider = "bitbucket";
    avatarUrl = owner
      ? `https://bitbucket.org/workspaces/${owner}/avatar/64`
      : undefined;
  }

  return { host, owner, repo, protocol, provider, avatarUrl };
}
