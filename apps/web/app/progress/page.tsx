"use client";

import { motion } from "motion/react";
import Link from "next/link";
import { ImageZoom } from "@/components/image-zoom";

const Page = () => {
  return (
    <div className="relative h-screen">
      <div className="h-screen flex flex-col items-center min-w-screen overflow-y-auto pb-40">
        <div className="max-w-[600px] w-full pt-10 md:pt-20 px-4 md:py-6 gap-6">
          <div className="flex flex-col gap-4">
            {[
              {
                src: "/assets/i1.png",
                width: 2052,
                height: 1212,
              },
              {
                src: "/assets/i2.png",
                width: 2049,
                height: 1382,
              },
              {
                src: "/assets/i3.png",
                width: 2052,
                height: 1220,
              },
              {
                src: "/assets/i4.png",
                width: 2042,
                height: 1292,
              },
            ].map((image, index) => {
              return (
                <div className="relative">
                  <ImageZoom
                    key={index}
                    width={image.width}
                    height={image.height}
                    src={image.src}
                    alt={`Image ${index + 1}`}
                    className="rounded-lg"
                  />
                  <div className="pointer-events-none absolute inset-0 rounded-lg ring-1 ring-black/10 ring-inset dark:ring-white/10" />
                </div>
              );
            })}
          </div>
        </div>
      </div>

      <Link href="/">
        <motion.img
          src="/logo192.png"
          alt="gitru logo"
          className="absolute size-8 md:size-7 left-3 md:left-4 top-4 md:top-20"
          layoutId="logo"
        />
      </Link>
    </div>
  );
};

export default Page;
