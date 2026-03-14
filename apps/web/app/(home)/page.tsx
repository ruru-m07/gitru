import { Metadata } from "next";
import HomePage from "@/components/homePage";

export const metadata: Metadata = {
  title: "Gitru",
  description: "A modern Git client for humans.",
  icons: new URL("/favicon.ico", "https://gitru.app"),
};

export default function Page() {
  return <HomePage />;
}
