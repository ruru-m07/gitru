import { DocsLayout } from "@/components/layout/notebook";
import { baseOptions } from "@/lib/layout.shared";
import { source } from "@/lib/source";

export default function Layout({ children }: LayoutProps<"/docs">) {
  const base = baseOptions();

  return (
    <DocsLayout
      {...base}
      nav={{ ...base.nav, mode: "top" }}
      tree={source.getPageTree()}
    >
      {children}
    </DocsLayout>
  );
}
