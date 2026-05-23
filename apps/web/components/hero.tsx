"use client";

import { Mascot, MascotExpression } from "@gitru/mascot";
import { Clock } from "lucide-react";
import { motion, useAnimate } from "motion/react";
import Image from "next/image";
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
      <div className="max-w-(--container-width) px-(--container-gutter) mx-auto relative w-full z-10 mt-30 mb-15">
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
        <motion.div className="max-w-(--container-width) px-(--container-gutter) relative mx-auto z-10">
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
              transition={{
                duration: 0.3,
              }}
            />
          </motion.div>
          <motion.div
            initial={{ opacity: 0, y: 10, filter: "blur(10px)" }}
            animate={{ opacity: 1, y: 0, filter: "blur(0px)" }}
            transition={{ duration: 0.4, delay: 0.3 }}
            className={cn(
              "relative inline-block",
              "[--outer-border:10px] md:[--outer-border:16px] lg:[--outer-border:28px]",
              "[--inner-border:4px] md:[--inner-border:10px] lg:[--inner-border:16px]",
              "[--spaceing-between:6px] md:[--spaceing-between:8px] lg:[--spaceing-between:16px]",
            )}
          >
            {/* Background layer */}
            <div className="absolute inset-0">
              <Image
                fill
                src="/preview-background.webp"
                alt="preview background image"
                className="rounded-(--outer-border) object-cover"
                fetchPriority="high"
                priority
              />
              <div className="absolute inset-0 rounded-(--outer-border) ring-1 ring-inset ring-black/12!" />
            </div>

            {/* Content (drives height) */}
            <div className="relative p-(--spaceing-between)">
              <ImageZoom
                width={2052}
                height={1212}
                src="/preview.webp"
                alt="preview image"
                className="rounded-(--inner-border) [&_img]:rounded-[16px] [&_img]:ring! [&_img]:ring-black/12!"
                fetchPriority="high"
                priority
              />
            </div>
          </motion.div>
        </motion.div>
      </div>
    </div>
  );
};

export default Hero;
