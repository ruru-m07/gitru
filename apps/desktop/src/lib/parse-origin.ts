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

/** Present native-sanitized origins; this does not resolve repository identity. */
export function parseOrigin(
  origin: string | undefined,
): ParseOriginResult | undefined {
  if (
    !origin ||
    origin.length > 4_096 ||
    /[\u0000-\u001f\u007f]/.test(origin)
  ) {
    return undefined;
  }

  let host = "";
  let owner = "";
  let repo = "";
  let protocol = "";
  let provider: GIT_PROVIDERS = "unknown";
  let avatarUrl: string | undefined = undefined;
  let href: string | undefined = undefined;

  let path = "";
  const scp = !origin.includes("://")
    ? origin.match(/^(?:[^@/\s]+@)?([^:/?#\s]+):([^?#\s]+)(?:[?#].*)?$/)
    : null;
  if (scp) {
    // The native sanitizer removes the optional user from SCP origins.
    host = scp[1].toLowerCase();
    path = scp[2];
    protocol = "ssh";
  } else {
    let url: URL;
    try {
      url = new URL(origin);
    } catch {
      return undefined;
    }

    if (!["https:", "http:", "ssh:"].includes(url.protocol)) {
      return undefined;
    }

    host = url.hostname;
    protocol = url.protocol.replace(":", "");
    path = url.pathname;
  }

  const pathParts = path
    .replace(/^\/+/, "")
    .replace(/\.git$/, "")
    .split("/");
  if (pathParts.length >= 2) {
    owner = pathParts[0];
    // Keep subgroup display paths without interpreting provider identity.
    repo = pathParts.slice(1).join("/");
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
