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
    metadataBase: new URL("https://gitru.app"),
    title: "Gitru",
    description: "Gitru - A Git client",
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
        <meta name="description" content="Gitru - A Git client" />

        <title>Gitru</title>
      </head>
      <body className="flex flex-col min-h-screen">
        <Provider>{children}</Provider>
      </body>
    </html>
  );
}
