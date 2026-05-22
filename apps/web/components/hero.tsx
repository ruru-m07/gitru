"use client";

import { Mascot, MascotExpression } from "@gitru/mascot";
import { Dithering } from "@paper-design/shaders-react";
import { Clock } from "lucide-react";
import { delay, motion, useAnimate, useAnimation } from "motion/react";
import Link from "next/link";
import { useEffect, useState } from "react";
import { cn } from "@/lib/cn";
import { GithubIcon } from "./icons";
import { ImageZoom } from "./image-zoom";
import { buttonVariants } from "./ui/button";

const Hero = () => {
  const [scope, animate] = useAnimate();

  const [expression, setExpression] = useState<MascotExpression | undefined>(
    undefined,
  );

  useEffect(() => {
    const sequence = async () => {
      await animate(
        scope.current,
        { opacity: 1 },
        {
          delay: 1,
        },
      );
      await animate(
        scope.current,
        { top: "-52px" },
        {
          type: "spring",
          mass: 1,
          stiffness: 400,
          damping: 19,
        },
      );
      setExpression({
        eyes: "closed",
        mouth: "open",
      });
      await animate(
        scope.current,
        { top: ["-52px", "-120px"] },
        {
          delay: 0.5,
          ease: [0.22, 1, 0.36, 1],
        },
      );
      await animate(
        scope.current,
        { top: ["-120px", "-96px"] },
        {
          type: "spring",
          mass: 1,
          stiffness: 500,
          damping: 19,
        },
      );
      setExpression(undefined);
    };

    sequence();
  }, []);

  return (
    <div className="w-full h-full py-2 relative">
      <div className="max-w-(--container-width) mx-auto relative w-full z-10 mt-30 mb-15">
        <motion.h1
          initial={{ opacity: 0, y: 10, filter: "blur(10px)" }}
          animate={{ opacity: 1, y: 0, filter: "blur(0px)" }}
          transition={{ duration: 0.4 }}
          className="text-6xl font-normal mb-4"
        >
          A Git Client.
        </motion.h1>
        <motion.p
          initial={{ opacity: 0, y: 10, filter: "blur(10px)" }}
          animate={{ opacity: 1, y: 0, filter: "blur(0px)" }}
          transition={{ duration: 0.4, delay: 0.1 }}
          className="text-base text-black/80 mb-4 max-w-140"
        >
          Gitru is lightweight and powerful Git client designed to simplify and
          abstract away the complexity of Git.
        </motion.p>
        <motion.div
          initial={{ opacity: 0, y: 10, filter: "blur(10px)" }}
          animate={{ opacity: 1, y: 0, filter: "blur(0px)" }}
          transition={{ duration: 0.4, delay: 0.2 }}
          className="mb-8 flex items-center gap-2"
        >
          <Link
            href={"/download"}
            className={cn(
              buttonVariants({ variant: "default", size: "default" }),
              "shadow-[inset_-1px_-1px_2px_1px_rgba(0,0,0,0.1),inset_1px_1px_2px_1px_rgba(255,255,255,0.5),0px_0px_4px_2px_rgba(0,0,0,0.1)]",
            )}
          >
            <Clock className="size-4 text-[color-mix(in_oklab,var(--primary)_40%,#ffffff)]" />
            Join Waitlist
          </Link>

          <Link
            className={cn(
              buttonVariants({ variant: "secondary", size: "default" }),
            )}
            href={"https://github.com/ruru-m07/gitru"}
            target="_blank"
          >
            <GithubIcon />
            Github
          </Link>
        </motion.div>
      </div>
      <div className="relative">
        <motion.div
          initial={{ opacity: 0 }}
          animate={{ opacity: 0.2 }}
          transition={{ duration: 1, delay: 0.5 }}
          className="absolute inset-0 top-20 mask-[linear-gradient(0deg,rgba(255,255,255,0)_0%,rgba(255,255,255,1)_30%,rgba(255,255,255,1)_100%)]"
        >
          <Dithering
            width={"100%"}
            height={"100%"}
            colorBack="#ffffff"
            colorFront="#FF2200"
            shape="warp"
            type="2x2"
            size={11}
            speed={0.005}
          />
          <div className="pointer-events-none absolute inset-0 rounded-[12px] inset-ring-1 inset-ring-black/10 dark:inset-ring-white/10" />
        </motion.div>
        <motion.div className="max-w-(--container-width) relative mx-auto z-10">
          <motion.div
            ref={scope}
            className="absolute group opacity-0 right-4 top-0 inline-block **:data-[name='mascot-svg']:size-24 **:data-[name='heart-svg']:scale-75"
          >
            <Mascot
              expression={expression}
              expressionMap={{
                hover: {
                  eyes: "closed",
                  mouth: "open",
                },
              }}
            />
          </motion.div>
          <motion.div
            initial={{ opacity: 0, y: 10, filter: "blur(10px)" }}
            animate={{ opacity: 1, y: 0, filter: "blur(0px)" }}
            transition={{ duration: 0.4, delay: 0.3 }}
          >
            <ImageZoom
              width={2052}
              height={1212}
              src={"/preview.png"}
              alt={`preview image`}
              className="rounded-[14px]! [&_img]:rounded-[16px] [&_img]:ring! [&_img]:ring-black/12!"
            />
          </motion.div>
        </motion.div>
      </div>
    </div>
  );
};

export default Hero;
