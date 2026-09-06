import { GIT_PROVIDERS } from "@/types/app";
import {
  bitbucketWorkspaceAvatarUrl,
  githubAccountAvatarUrl,
  normalizeExternalHttpsUrl,
} from "./external-content";

interface ParseOriginResult {
  host: string;
  owner: string;
  repo: string;
  protocol: string;
  provider: "github" | "gitlab" | "bitbucket" | "unknown";
  avatarUrl?: string;
  href?: string;
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
  let href: string | undefined = undefined;

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
    let url: URL;
    try {
      url = new URL(origin);
    } catch {
      return undefined;
    }

    if (url.protocol !== "https:" && url.protocol !== "http:") {
      return undefined;
    }

    host = url.hostname;
    protocol = url.protocol.replace(":", "");
    const pathParts = url.pathname
      .replace(/^\//, "")
      .replace(/\.git$/, "")
      .split("/");
    if (pathParts.length >= 2) {
      owner = pathParts[0];
      // join remaining parts as repo to support subgroups (gitlab etc)
      repo = pathParts.slice(1).join("/");
    }
  }

  // ? Determine provider
  if (host === "github.com") {
    provider = "github";
    avatarUrl = githubAccountAvatarUrl(owner);
  } else if (host === "gitlab.com") {
    provider = "gitlab";
  } else if (host === "bitbucket.org") {
    provider = "bitbucket";
    avatarUrl = bitbucketWorkspaceAvatarUrl(owner);
  }

  if (host && owner && repo) {
    const hrefProtocol = protocol && protocol !== "ssh" ? protocol : "https";
    href =
      normalizeExternalHttpsUrl(`${hrefProtocol}://${host}/${owner}/${repo}`) ??
      undefined;
  }

  return { host, owner, repo, protocol, provider, avatarUrl, href };
}
