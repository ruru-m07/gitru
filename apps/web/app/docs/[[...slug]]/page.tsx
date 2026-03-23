import type { TOCItemType } from "fumadocs-core/toc";
import { createRelativeLink } from "fumadocs-ui/mdx";
import type { Metadata } from "next";
import { notFound } from "next/navigation";
import type { ComponentType } from "react";
import { ImageZoom } from "@/components/image-zoom";
import {
  DocsBody,
  DocsDescription,
  DocsPage,
  DocsTitle,
} from "@/components/layout/notebook/page";
import { cn } from "@/lib/cn";
import { getPageImage, source } from "@/lib/source";
import { getMDXComponents } from "@/mdx-components";

type MDXRenderable = {
  body: ComponentType<Record<string, unknown>>;
  toc?: TOCItemType[];
  full?: boolean;
  title?: string;
  description?: string;
};

type LazyMDXRenderable = {
  load: () => Promise<MDXRenderable>;
};

function hasBody(value: unknown): value is MDXRenderable {
  return typeof value === "object" && value !== null && "body" in value;
}

function hasLoad(value: unknown): value is LazyMDXRenderable {
  return typeof value === "object" && value !== null && "load" in value;
}

export default async function Page(props: PageProps<"/docs/[[...slug]]">) {
  const params = await props.params;
  const page = source.getPage(params.slug);
  if (!page) notFound();

  // Fumadocs can expose MDX either eagerly (`data.body`) or lazily (`data.load()`).
  const pageData = hasBody(page.data)
    ? page.data
    : hasLoad(page.data)
      ? // @ts-ignore
        await page.data.load()
      : null;
  if (!pageData) notFound();

  const MDX = pageData.body;
  const markdownUrl = `/llms.mdx/docs/${[...page.slugs, "index.mdx"].join("/")}`;

  return (
    <DocsPage
      tableOfContent={{
        style: "clerk",
      }}
      toc={pageData.toc}
      full={pageData.full}
      className="gap-0"
    >
      <DocsTitle>{pageData.title ?? page.data.title}</DocsTitle>
      <DocsDescription className="mb-0">
        {pageData.description ?? page.data.description}
      </DocsDescription>
      <DocsBody>
        <MDX
          components={getMDXComponents({
            // this allows you to link to other pages with relative file paths
            a: createRelativeLink(source, page),
            h1: ({ className, ...props }) => (
              <h1
                className={cn(
                  "flex scroll-m-28 flex-row items-center gap-2 text-[2em] font-[350]",
                  className,
                )}
                {...props}
              />
            ),
            p: ({ className, ...props }) => (
              <p
                className={cn("mt-8 font-[350] tracking-[0.01em]", className)}
                {...props}
              />
            ),
            img: ({ className, ...props }) => (
              <ImageZoom className={cn("rounded-md")} {...props} />
            ),
          })}
        />
      </DocsBody>
    </DocsPage>
  );
}

export async function generateStaticParams() {
  return source.generateParams();
}

export async function generateMetadata(
  props: PageProps<"/docs/[[...slug]]">,
): Promise<Metadata> {
  const params = await props.params;
  const page = source.getPage(params.slug);
  if (!page) notFound();

  return {
    title: page.data.title,
    description: page.data.description,
    openGraph: {
      images: getPageImage(page).url,
    },
  };
}
