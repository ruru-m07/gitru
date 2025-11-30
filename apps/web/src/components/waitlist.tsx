import { useEffect, useState } from "react";
import TypeWriter, { type TypeSegment, typeStyles } from "./type-writer";

const circle = `
      へ
  ૮  >  <)   ~meowOS
   /    |
乀(ˍ, ل ل
`;

const allSegments: TypeSegment[] = [
  { text: "$ soon", speed: 0, cursor: "primary" },
  // { text: "$ ", speed: 0 },
  // { text: "git clone ", className: typeStyles.muted, speed: 3, blink: false },
  // { text: "git@ruru.build:gitru.git", speed: 4, blink: false },
  // { text: "\n", speed: 0, delay: 2000 },
  // {
  //   text: "  Cloning into gitru...",
  //   className: typeStyles.muted,
  //   delay: 300,
  //   instant: true,
  // },
  // { text: "", speed: 0, delay: 2000 },
  // {
  //   text: "\n  ! [remote rejected] Access restricted to unauthorized users.",
  //   className: typeStyles.red,
  //   delay: 0,
  //   instant: true,
  // },
  // {
  //   text: "\n  hint: You're not authorized yet.",
  //   className: typeStyles.yellow,
  //   delay: 0,
  //   instant: true,
  // },
  // {
  //   text: "\n  hint: But you can join the early-access list.",
  //   className: typeStyles.yellow,
  //   delay: 0,
  //   instant: true,
  // },
  // { text: "\n", speed: 0, delay: 0 },
  // { text: "$ ", speed: 0, cursor: "primary" },
];

const bootingSegments: TypeSegment[] = [
  { text: "", cursor: "primary", delay: 2500 },
  // {
  //   text: "[0.000000] ",
  //   className: typeStyles.muted,
  //   instant: true,
  //   blink: false,
  //   delay: 0,
  // },
  // { text: "", delay: 200, blink: false },
  // {
  //   text: "meowOS",
  //   instant: true,
  //   blink: false,
  //   delay: 0,
  // },
  // {
  //   text: " version 6.9.0-zen (gitru@home) (gcc version 14.1.1)",
  //   className: typeStyles.muted,
  //   instant: true,
  //   blink: false,
  //   delay: 0,
  // },
  // {
  //   text: "\n[0.000421] ",
  //   className: typeStyles.muted,
  //   instant: true,
  //   blink: false,
  // },
  // { text: "", delay: 200, blink: false },
  // {
  //   text: "CPU0: Git-Optimized Kernel Extensions enabled",
  //   className: typeStyles.muted,
  //   instant: true,
  //   blink: false,
  // },
  // {
  //   text: "\n[0.002114] ",
  //   className: typeStyles.muted,
  //   instant: true,
  // },
  // { text: "", delay: 200, blink: false },
  // {
  //   text: "Mounting /usr/lib/",
  //   className: typeStyles.muted,
  //   instant: true,
  // },
  // {
  //   text: "gitru",
  //   instant: true,
  // },
  // {
  //   text: "\n[0.003500] ",
  //   className: typeStyles.muted,
  //   instant: true,
  //   cursor: "primary",
  //   blink: false,
  // },
  // { text: "", delay: 200, blink: false },
  // ! -------------------------------------
  // {
  //   text: "[0.004069] ",
  //   instant: true,
  // },
  // {
  //   text: "Welcome to waitlist terminal [Version 0.0.0]",
  //   cursor: "primary",
  //   blink: false,
  //   speed: 25,
  // },
  // {
  //   text: "\n[0.005100] ",
  //   className: typeStyles.muted,
  //   instant: true,
  //   cursor: "default",
  //   blink: false,
  // },
  // { text: "", delay: 200, blink: false },
  // {
  //   text: "\n[0.006901] ",
  //   instant: true,
  // },
  // {
  //   text: "System Notice:",
  //   cursor: "primary",
  // },
  // {
  //   text: "\n[0.006944] ",
  //   instant: true,
  // },
  // { text: "", delay: 200, blink: false },
  // {
  //   text: "  Gitru is a lightweight git client for humans.",
  //   cursor: "primary",
  //   blink: false,
  //   instant: true,
  // },
  // {
  //   text: "\n[0.006981] ",
  //   instant: true,
  // },
  // { text: "", delay: 200, blink: false },
  // {
  //   text: "  A Warapper around git, with just a better UX.",
  //   cursor: "primary",
  //   blink: false,
  //   instant: true,
  // },
  // {
  //   text: "\n[0.007001] ",
  //   instant: true,
  // },
  // { text: "", delay: 200, blink: false },
  // {
  //   text: "  Click. Commit. Continue.",
  //   cursor: "primary",
  //   blink: false,
  //   instant: true,
  // },
  // {
  //   text: "\n[0.010200] ",
  //   className: typeStyles.muted,
  //   instant: true,
  //   blink: false,
  // },
  // { text: "", delay: 1000, blink: false },
  // {
  //   text: "\n[0.012900] Press ENTER to continue... ",
  //   className: typeStyles.muted,
  //   instant: true,
  //   cursor: "primary",
  //   blink: true,
  // },
  // ! ------------------
  {
    text: "Welcome to waitlist terminal [Version 0.0.0]",
    // cursor: "primary",
    blink: false,
    speed: 2,
  },
  { text: "", delay: 200, blink: false, cursor: "primary" },
  {
    text: "\n\nSystem Notice:",
    cursor: "primary",
    speed: 3,
  },
  { text: "", delay: 100, cursor: "primary", blink: false },
  {
    text: "\n  - Gitru is a lightweight git client for humans.",
    cursor: "primary",
    blink: false,
    speed: 3,
  },
  { text: "", delay: 25, cursor: "primary", blink: false },
  {
    text: "\n  - A Warapper around git, with just a better UX.",
    cursor: "primary",
    blink: false,
    speed: 3,
  },
  { text: "", delay: 25, cursor: "primary", blink: false },
  {
    text: "\n  - Click. Commit. Continue.",
    cursor: "primary",
    blink: false,
    speed: 3,
  },
  {
    text: "\n\n",
    className: typeStyles.muted,
    instant: true,
    cursor: "primary",
    blink: true,
  },
  { text: "", delay: 2000 },
  {
    text: "Press ENTER to continue... ",
    className: typeStyles.muted,
    instant: true,
    cursor: "primary",
    blink: true,
  },
];

const Waitlist = () => {
  const [OSbootDone, setOSbootDone] = useState(false);
  const [openNextStep, setOpenNextStep] = useState(false);

  useEffect(() => {
    console.log(OSbootDone, openNextStep);
    if (!OSbootDone || openNextStep) return;

    const handleKeyPress = (e: KeyboardEvent) => {
      if (e.key === "Enter") {
        setOpenNextStep(true);
      }
    };
    window.addEventListener("keydown", handleKeyPress);
    return () => {
      window.removeEventListener("keydown", handleKeyPress);
    };
  }, [OSbootDone, openNextStep]);

  return (
    <div className="max-w-[600px] h-full w-full mx-1">
      {!openNextStep ? (
        <span className="mono whitespace-pre-wrap">
          <TypeWriter
            segments={bootingSegments}
            defaultSpeed={50}
            onComplete={() => setOSbootDone(true)}
          />
        </span>
      ) : (
        <>
          <pre>{circle}</pre>
          <br />
          <span className="mono whitespace-pre-wrap">
            <TypeWriter
              segments={allSegments}
              defaultSpeed={50}
              startDelay={500}
            />
          </span>
        </>
      )}
    </div>
  );
};

export default Waitlist;
