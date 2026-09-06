const ALLOWED_REMOTE_IMAGE_HOSTS = new Set([
  "avatars.githubusercontent.com",
  "bitbucket.org",
  "github.com",
]);

const GITHUB_ACCOUNT_PATTERN = /^[a-z\d](?:[a-z\d-]{0,37}[a-z\d])?$/i;

export const normalizeExternalHttpsUrl = (value: string) => {
  if (
    !value.startsWith("https://") ||
    value.length > 4_096 ||
    /[\u0000-\u001f\u007f]/.test(value)
  ) {
    return null;
  }

  try {
    const url = new URL(value);
    if (
      url.protocol !== "https:" ||
      !url.hostname ||
      url.username !== "" ||
      url.password !== ""
    ) {
      return null;
    }

    return url.toString();
  } catch {
    return null;
  }
};

export const normalizeRemoteImageUrl = (value: string | undefined) => {
  if (!value) return undefined;

  const normalized = normalizeExternalHttpsUrl(value);
  if (!normalized) return undefined;

  const url = new URL(normalized);
  if (
    !ALLOWED_REMOTE_IMAGE_HOSTS.has(url.hostname) ||
    (url.port !== "" && url.port !== "443")
  ) {
    return undefined;
  }

  if (url.hostname === "github.com") {
    const account = url.pathname.match(/^\/([^/]+)\.png$/)?.[1];
    if (!account || !GITHUB_ACCOUNT_PATTERN.test(account)) return undefined;
  }

  if (url.hostname === "avatars.githubusercontent.com") {
    if (
      url.pathname !== "/u/e" ||
      Array.from(url.searchParams.keys()).some(
        (key) => key !== "email" && key !== "s",
      )
    ) {
      return undefined;
    }
  }

  if (
    url.hostname === "bitbucket.org" &&
    !/^\/workspaces\/[a-z\d_-]+\/avatar\/64$/i.test(url.pathname)
  ) {
    return undefined;
  }

  return normalized;
};

export const githubAccountAvatarUrl = (account: string | undefined) => {
  if (!account || !GITHUB_ACCOUNT_PATTERN.test(account)) return undefined;
  return normalizeRemoteImageUrl(`https://github.com/${account}.png`);
};

export const githubCommitterAvatarUrl = (email: string | undefined) => {
  if (!email) return undefined;

  const url = new URL("https://avatars.githubusercontent.com/u/e");
  url.searchParams.set("email", email);
  url.searchParams.set("s", "64");
  return normalizeRemoteImageUrl(url.toString());
};

export const bitbucketWorkspaceAvatarUrl = (workspace: string | undefined) => {
  if (!workspace || !/^[a-z\d_-]+$/i.test(workspace)) return undefined;
  return normalizeRemoteImageUrl(
    `https://bitbucket.org/workspaces/${workspace}/avatar/64`,
  );
};
