import { Inter } from "next/font/google";
import { Provider } from "@/components/provider";
import "./global.css";

const inter = Inter({
  subsets: ["latin"],
});

export default function Layout({ children }: LayoutProps<"/">) {
  return (
    <html lang="en" className={inter.className} suppressHydrationWarning>
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
          content="Gitru - A modern Git client for humans."
        />

        <meta property="og:type" content="website" />
        <meta property="og:site_name" content="Gitru" />
        <meta property="og:title" content="Gitru" />
        <meta
          property="og:description"
          content="Gitru - A modern git client for humans."
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
