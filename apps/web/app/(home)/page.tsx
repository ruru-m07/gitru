import { Metadata } from "next";
import HomePage from "@/components/homePage";

export const metadata: Metadata = {
  title: "Gitru",
  description: "Gitru - A Git client",
  icons: new URL("/favicon.ico", "https://gitru.app"),
};

export default function Page() {
  return <HomePage />;
}
