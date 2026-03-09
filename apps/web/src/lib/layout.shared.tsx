import Wordmark from "@/components/wordmark";
import type { BaseLayoutProps } from "fumadocs-ui/layouts/shared";
import { Info } from "lucide-react";

export const gitConfig = {
  user: "ruru-m07",
  repo: "gitru",
};

export function baseOptions(): BaseLayoutProps {
  return {
    nav: {
      title: (
        <div className="flex items-center justify-center gap-2">
          <Wordmark />
        </div>
      ),
      transparentMode: "top",
    },
    links: [
      {
        type: "main",
        text: "Download",
        url: "/download",
      },
      {
        type: "main",
        text: "Blog",
        url: "/blog",
      },
      {
        type: "main",
        text: "Changelog",
        url: "/changelog",
      },
    ],
    githubUrl: "https://github.com/ruru-m07/gitru",
    themeSwitch: {
      mode: "light-dark",
    },
    // githubUrl: `https://github.com/${gitConfig.user}/${gitConfig.repo}`,
  };
}
