import { JSX } from "react";
import { BitbucketIcon } from "@/components/svgs/bitbucket";
import { GithubIcon } from "@/components/svgs/githubIcon";
import { GitlabIcon } from "@/components/svgs/gitlabIcon";
import { GIT_PROVIDERS } from "@/type";

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
