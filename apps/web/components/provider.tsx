"use client";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { RootProvider } from "fumadocs-ui/provider/next";
import { type ReactNode, useState } from "react";
import SearchDialog from "@/components/search";

export function Provider({ children }: { children: ReactNode }) {
  const [queryClient] = useState(() => new QueryClient());

  return (
    <QueryClientProvider client={queryClient}>
      <RootProvider search={{ SearchDialog }}>{children}</RootProvider>
    </QueryClientProvider>
  );
}
