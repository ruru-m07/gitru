import Link from "next/link";
import { cn } from "@/lib/cn";
import { GithubIcon, XIcon } from "./icons";
import Logo from "./logo";
import { buttonVariants } from "./ui/button";

const Navbar = () => {
  return (
    <div className="max-w-(--container-width) px-(--container-gutter) mx-auto w-full py-2 flex items-center justify-between">
      <Link href="/" className="flex items-center gap-2">
        <Logo className="size-7" />
        <span className="text-2xl font-bold">Gitru</span>
      </Link>

      <div className="flex items-center justify-center gap-4">
        {/* <div className="flex items-center gap-4">
              <span className="text-sm hover:underline text-muted-foreground hover:text-foreground cursor-pointer">
                Docs
              </span>
              <span className="text-sm hover:underline text-muted-foreground hover:text-foreground cursor-pointer">
                About
              </span>
              <span className="text-sm hover:underline text-muted-foreground hover:text-foreground cursor-pointer">
                Changelog
              </span>
            </div> */}
        {/* <div className="h-6 w-px bg-border" /> */}
        <div className="flex items-center justify-center gap-1">
          <Link
            className={cn(buttonVariants({ variant: "ghost", size: "icon" }))}
            href={"https://x.com/ruru_1x"}
            target="_blank"
            aria-label="Visit Gitru on X"
          >
            <XIcon className="size-3.5" />
          </Link>
          <Link
            className={cn(buttonVariants({ variant: "ghost", size: "icon" }))}
            href={"https://github.com/ruru-m07/gitru"}
            target="_blank"
            aria-label="View Gitru on GitHub"
          >
            <GithubIcon />
          </Link>
        </div>
      </div>
    </div>
  );
};

export default Navbar;
