import { createFileRoute, Link, notFound } from "@tanstack/react-router";
import { DocsLayout } from "@/components/layout/notebook";
import { createServerFn } from "@tanstack/react-start";
import { source } from "@/lib/source";
import browserCollections from "fumadocs-mdx:collections/browser";
import {
  DocsBody,
  DocsDescription,
  DocsPage,
  DocsTitle,
} from "@/components/layout/notebook/page";
import defaultMdxComponents from "fumadocs-ui/mdx";
import { baseOptions } from "@/lib/layout.shared";
import { staticFunctionMiddleware } from "@tanstack/start-static-server-functions";
import { useFumadocsLoader } from "fumadocs-core/source/client";
import { Suspense } from "react";
import { ImageZoom } from 'fumadocs-ui/components/image-zoom';
import { cn } from "@/lib/utils";

export const Route = createFileRoute("/docs/$")({
  component: Page,
  loader: async ({ params }) => {
    const slugs = params._splat?.split("/") ?? [];
    const data = await loader({ data: slugs });
    await clientLoader.preload(data.path);
    return data;
  },
});

const loader = createServerFn({
  method: "GET",
})
  .inputValidator((slugs: string[]) => slugs)
  .middleware([staticFunctionMiddleware])
  .handler(async ({ data: slugs }) => {
    const page = source.getPage(slugs);
    if (!page) throw notFound();

    return {
      slugs: page.slugs,
      path: page.path,
      pageTree: await source.serializePageTree(source.getPageTree()),
    };
  });

const clientLoader = browserCollections.docs.createClientLoader({
  component({ toc, frontmatter, default: MDX }) {
    return (
      <DocsPage
        tableOfContent={{
          style: "clerk",
        }}
        toc={toc}
        className="gap-0"
      >
        <DocsTitle>{frontmatter.title}</DocsTitle>
        <DocsDescription>{frontmatter.description}</DocsDescription>
        <DocsBody>
          <MDX
            components={{
              ...defaultMdxComponents,
              h1: ({ className, ...props }) => <h1 className={cn("flex scroll-m-28 flex-row items-center gap-2 text-[2em] font-[350]", className)} {...props} />,
              p: ({ className, ...props }) => <p className={cn("mt-8 font-[350] tracking-[0.01em]", className)} {...props} />,
              img: ({className, ...props}) => <ImageZoom className={cn("rounded-md", className)} {...(props)} />,
            }}
          />
        </DocsBody>
      </DocsPage>
    );
  },
});

function Page() {
  const { pageTree, slugs, path } = useFumadocsLoader(Route.useLoaderData());
  const markdownUrl = `/llms.mdx/docs/${[...slugs, "index.mdx"].join("/")}`;

  const base = baseOptions();

  return (
    <DocsLayout
      {...base}
      nav={{ ...base.nav, mode: "top" }}
      tree={pageTree}
    >
      <Link to={markdownUrl} hidden />
      <Suspense>{clientLoader.useContent(path)}</Suspense>
    </DocsLayout>
  );
}
