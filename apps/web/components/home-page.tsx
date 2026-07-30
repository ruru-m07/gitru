"use client";

import { useTheme } from "next-themes";
import { useLayoutEffect } from "react";
import Hero from "./hero";

export default function HomePage() {
  const { setTheme } = useTheme();

  useLayoutEffect(() => {
    setTheme("light");
  }, []);

  return (
    <div>
      <Hero />
      {/* bottom spaceing */}
      <div className="min-h-6"></div>
    </div>
  );
}
