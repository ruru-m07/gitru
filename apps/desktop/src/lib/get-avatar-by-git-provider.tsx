import { JSX } from "react";
import { BitbucketIcon } from "@/components/svgs/bitbucket";
import { GithubIcon } from "@/components/svgs/github-icon";
import { GitlabIcon } from "@/components/svgs/gitlab-icon";
import { GIT_PROVIDERS } from "@/types/app";

export function getAvatarByProvider(
  provider: GIT_PROVIDERS | undefined,
  className?: string,
): JSX.Element | null {
  if (!provider) return null;

  switch (provider) {
    case "github":
      return <GithubIcon className={className} />;
    case "gitlab":
      return <GitlabIcon className={className} />;
    case "bitbucket":
      return <BitbucketIcon className={className} />;
    default:
      return null;
  }
}
