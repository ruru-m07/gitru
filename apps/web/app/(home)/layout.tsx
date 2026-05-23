import Navbar from "@/components/navbar";
import { cn } from "@/lib/cn";

export default function Layout({ children }: LayoutProps<"/">) {
  return (
    <main
      className={cn(
        "relative h-screen w-full flex flex-col bg-[#FDFDFD]",
        "[--container-width:72rem]",
        "[--container-gutter:10px] md:[--container-gutter:1rem] lg:[--container-gutter:14px]",
      )}
    >
      <Navbar />
      {children}
    </main>
  );
}
