import { Instrument_Serif, Inter } from "next/font/google";
import { Provider } from "@/components/provider";
import "./global.css";
import { Metadata } from "next";
import { cn } from "@/lib/cn";

const inter = Inter({
  subsets: ["latin"],
});

const instrumentSerif = Instrument_Serif({
  subsets: ["latin-ext"],
  weight: "400",
  display: "swap",
  variable: "--font-instrument-serif",
});

export async function generateMetadata({
  params,
}: LayoutProps<"/">): Promise<Metadata> {
  return {
    title: "Gitru",
    description: "A modern, lightweight, and powerful Git client.",
    openGraph: {
      title: "Gitru",
      description: "A modern, lightweight, and powerful Git client.",
      url: "https://gitru.app",
      locale: "en_US",
      images: {
        url: "https://gitru.app/og.png",
        width: 1200,
        height: 630,
        alt: "Gitru",
      },
    },
    twitter: {
      card: "summary_large_image",
      title: "Gitru",
      description: "A modern, lightweight, and powerful Git client.",
      images: {
        url: "https://gitru.app/og.png",
        alt: "Gitru",
        width: 1200,
        height: 630,
      },
    },
  };
}

export default function Layout({ children }: LayoutProps<"/">) {
  return (
    <html
      lang="en"
      className={cn(inter.className, instrumentSerif.variable)}
      suppressHydrationWarning
    >
      <head>
        <link
          rel="apple-touch-icon"
          sizes="180x180"
          href="/apple-touch-icon.png"
        />
        <link
          rel="icon"
          type="image/png"
          sizes="32x32"
          href="/favicon-32x32.png"
        />
        <link
          rel="icon"
          type="image/png"
          sizes="16x16"
          href="/favicon-16x16.png"
        />
        <link rel="manifest" href="/site.webmanifest" />

        <meta charSet="UTF-8" />
        <meta name="viewport" content="width=device-width, initial-scale=1.0" />
        <link rel="icon" href="/favicon.ico" />
        <meta name="theme-color" content="#ffffff" />
        <meta
          name="description"
          content="A modern, lightweight, and powerful Git client."
        />

        <meta property="og:type" content="website" />
        <meta property="og:site_name" content="Gitru" />
        <meta property="og:title" content="Gitru" />
        <meta
          property="og:description"
          content="A modern, lightweight, and powerful Git client."
        />
        <meta property="og:image" content="https://gitru.app/og.png" />
        <meta property="og:image:width" content="1200" />
        <meta property="og:image:height" content="630" />
        <meta property="og:image:alt" content="Gitru - Waitlist" />
        <meta property="og:url" content="https://gitru.app" />
        <meta property="og:locale" content="en_US" />
        <meta property="og:type" content="website" />

        <meta name="twitter:card" content="summary_large_image" />
        <meta name="twitter:title" content="Gitru" />
        <meta
          name="twitter:description"
          content="Gitru - A modern git client for humans."
        />
        <meta name="twitter:image" content="https://gitru.app/og.png" />
        <meta name="twitter:image:title" content="Gitru" />
        <meta name="twitter:image:alt" content="Gitru" />
        <meta name="twitter:image:width" content="1200" />
        <meta name="twitter:image:height" content="630" />
        <meta name="twitter:image:content_type" content="image/png" />

        <title>Gitru</title>
      </head>
      <body className="flex flex-col min-h-screen">
        <Provider>{children}</Provider>
      </body>
    </html>
  );
}
