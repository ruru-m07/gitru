"use client";

import { useTheme } from "next-themes";
import { useLayoutEffect } from "react";
import Angru from "@/components/mascot/angru";
import Blueprint from "@/components/mascot/blueprint";
import CriMascot from "@/components/mascot/cri";
import Flyru from "@/components/mascot/flyru";
import Wru from "@/components/mascot/wru";

const MascotPage = () => {
  const { setTheme } = useTheme();

  useLayoutEffect(() => {
    setTheme("light");
  }, []);

  return (
    <div className="w-full">
      <div className="relative max-w-(--container-width) px-(--container-gutter) mx-auto">
        <div className="mt-10 lg:mt-0 lg:absolute top-20 left-0 flex items-center justify-center gap-10">
          <Angru />
          <CriMascot />
        </div>
        <Flyru className="absolute bottom-10 translate-y-full lg:translate-y-0 lg:top-0 right-40 lg:right-0 rotate-20" />
        <div className="flex justify-center w-full">
          <Blueprint className="w-full h-fit" />
        </div>
        <Wru className="lg:absolute bottom-30 left-0" />
      </div>
    </div>
  );
};

export default MascotPage;
