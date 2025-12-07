import { cn } from "@gitru/ui/lib/utils";
import { useEffect, useRef, useState } from "react";
import { api } from "@/lib/api";
import TypeWriter, { type TypeSegment, typeStyles } from "./type-writer";

// export const cat = `
//       へ
//   ૮  >  <)   ~meowOS
//    /    |
// 乀(ˍ, ل ل
// `;

const allSegments: TypeSegment[] = [
  { text: "$ ", speed: 0, cursor: "primary" },
  {
    text: "git config ",
    className: typeStyles.muted,
    delay: 500,
    speed: 30,
    blink: false,
  },
  {
    text: "--global user.name ",
    speed: 30,
    blink: false,
  },
  { text: '"', speed: 0 },
  {
    text: "",
    input: true,
    inputId: "username",
    cursor: "primary",
    blink: true,
  },
  { text: '"', speed: 0, instant: true },
  { text: "\n", speed: 0, delay: 0 },
  { text: "$ ", speed: 0, cursor: "primary" },
  {
    text: "git config ",
    className: typeStyles.muted,
    delay: 500,
    speed: 30,
    blink: false,
  },
  {
    text: "--global user.email ",
    speed: 30,
    blink: false,
  },
  { text: '"', speed: 0 },
  {
    text: "",
    input: true,
    inputId: "email",
    cursor: "primary",
    blink: true,
  },
  { text: '"', speed: 0, instant: true },
];

// const _bootingSegments: TypeSegment[] = [
// { text: "", cursor: "primary", delay: 2500 },
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
// // ! ------------------
// {
//   text: "Welcome to waitlist terminal [Version 0.0.0]",
//   // cursor: "primary",
//   blink: false,
//   speed: 2,
// },
// { text: "", delay: 200, blink: false, cursor: "primary" },
// {
//   text: "\n\nSystem Notice:",
//   cursor: "primary",
//   speed: 3,
// },
// { text: "", delay: 100, cursor: "primary", blink: false },
// {
//   text: "\n  - Gitru is a lightweight git client for humans.",
//   cursor: "primary",
//   blink: false,
//   speed: 3,
// },
// { text: "", delay: 25, cursor: "primary", blink: false },
// {
//   text: "\n  - A Warapper around git, with just a better UX.",
//   cursor: "primary",
//   blink: false,
//   speed: 3,
// },
// { text: "", delay: 25, cursor: "primary", blink: false },
// {
//   text: "\n  - Click. Commit. Continue.",
//   cursor: "primary",
//   blink: false,
//   speed: 3,
// },
// {
//   text: "\n\n",
//   className: typeStyles.muted,
//   instant: true,
//   cursor: "primary",
//   blink: true,
// },
// { text: "", delay: 2000 },
// {
//   text: "Press ENTER to continue... ",
//   className: typeStyles.muted,
//   instant: true,
//   cursor: "primary",
//   blink: true,
// },
// ! ------------------
//   {
//     text: `Welcome to waitlist terminal [Version 0.0.0]

// System Notice:
//   - Gitru is a lightweight git client for humans.
//   - A Warapper around git, with just a better UX.
//   - Click. Commit. Continue.

// `,
//     blink: false,
//     instant: true,
//   },
// { text: "", delay: 2000 },
// {
//   text: "Press ENTER to continue... ",
//   className: typeStyles.muted,
//   instant: true,
//   cursor: "primary",
//   blink: true,
// },
// ];

const bootingSegmentsText = `Welcome to waitlist terminal [Version 0.0.0]

System Notice:
  - Gitru is a lightweight git client for humans.
  - A Warapper around git, with just a better UX.
  - Click. Commit. Continue.

`;

const Waitlist = () => {
  const [openNextStep, setOpenNextStep] = useState(false);
  const [_formData, setFormData] = useState<Record<string, string>>({});
  const [isLoading, setIsLoading] = useState(false);
  const [isSuccess, setIsSuccess] = useState(false);
  const [isAlreadyExists, setIsAlreadyExists] = useState(false);
  const [isError, setIsError] = useState(false);
  const [errorMessage, setErrorMessage] = useState("");
  const [loaderFrame, setLoaderFrame] = useState(0);

  const [totalJoined, setTotalJoined] = useState<number>(0);

  const loaderFrames = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

  const fetched = useRef(false);

  useEffect(() => {
    if (fetched.current) return;
    fetched.current = true;
    (async () => {
      const { data } = await api.waitlist.count.get();
      setTotalJoined(data?.count ?? 0);
    })();
  }, []);

  const handleAllInputsSubmit = async (values: Record<string, string>) => {
    console.log("Form submitted:", values);
    setFormData(values);
    setIsError(false);
    setErrorMessage("");

    // Validate name is not empty
    if (!values["username"] || values["username"].trim() === "") {
      setIsError(true);
      setErrorMessage("Name cannot be empty");
      return;
    }

    // Validate email is not empty
    if (!values["email"] || values["email"].trim() === "") {
      setIsError(true);
      setErrorMessage("Email cannot be empty");
      return;
    }

    // Validate email format on frontend
    const emailRegex = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;
    if (!emailRegex.test(values["email"])) {
      setIsError(true);
      setErrorMessage("Invalid email format");
      return;
    }

    setIsLoading(true);

    try {
      const { data, error } = await api.waitlist.post({
        email: values["email"],
        name: values["username"],
      });

      if (error) {
        console.error("API error:", error);
        setIsLoading(false);
        setIsError(true);
        setErrorMessage("Failed to connect to server");
        return;
      }

      if (data?.success) {
        console.log(data);

        // Check if user already existed
        const alreadyExisted =
          data.message === "You are already on the waitlist";

        setTimeout(() => {
          console.log("Success! User registered:", values);
          setIsLoading(false);
          if (alreadyExisted) {
            setIsAlreadyExists(true);
          } else {
            setIsSuccess(true);
          }
        }, 1500);
      } else {
        setIsLoading(false);
        setIsError(true);
        setErrorMessage(data?.message || "Something went wrong");
      }
    } catch (err) {
      console.error("Network error:", err);
      setIsLoading(false);
      setIsError(true);
      setErrorMessage("Network error. Please try again.");
    }
  };

  // Loader animation effect
  useEffect(() => {
    if (!isLoading) return;
    const interval = setInterval(() => {
      setLoaderFrame((prev) => (prev + 1) % loaderFrames.length);
    }, 80);
    return () => clearInterval(interval);
  }, [isLoading, loaderFrames.length]);

  useEffect(() => {
    if (openNextStep) return;

    const handleKeyPress = (e: KeyboardEvent) => {
      if (e.key === "Enter") {
        setOpenNextStep(true);
      }
    };
    window.addEventListener("keydown", handleKeyPress);
    return () => {
      window.removeEventListener("keydown", handleKeyPress);
    };
  }, [openNextStep]);

  let [showBlinkingCursor, setShowBlinkingCursor] = useState(true);

  useEffect(() => {
    const interval = setInterval(() => {
      setShowBlinkingCursor((prev) => !prev);
    }, 530);
    return () => clearInterval(interval);
  }, []);

  const [carEyes, setCarEyes] = useState(true);

  useEffect(() => {
    if (!openNextStep) return;
    const interval = setInterval(
      () => {
        setCarEyes((prev) => !prev);
      },
      carEyes ? 4000 : 3000,
    );

    return () => clearInterval(interval);
  }, [carEyes, openNextStep]);

  return (
    <div className="relative h-screen w-full flex items-center justify-center pt-20">
      <div className="max-w-[600px] h-full w-full mx-1">
        {!openNextStep ? (
          <span className="mono whitespace-pre-wrap">
            {bootingSegmentsText}
            {/* <TypeWriter
            segments={bootingSegments}
            defaultSpeed={50}
            onComplete={() => setOSbootDone(true)}
          /> */}
            <span className="text-muted-foreground">
              Press ENTER to continue...
              <span
                className={cn(
                  "h-full px-[5px] transition-opacity relative bg-primary",
                  showBlinkingCursor
                    ? "opacity-100"
                    : "bg-transparent _opacity-0",
                )}
              >
                {showBlinkingCursor ? (
                  ""
                ) : (
                  <span
                    className={cn(
                      "absolute w-full left-0 right-0 bottom-0 h-1",
                      "bg-primary",
                    )}
                  />
                )}
              </span>
            </span>
          </span>
        ) : (
          <>
            {carEyes ? (
              <pre className="font-mono">
                {"      へ\n"}
                {"  ૮  "}
                <span className="text-primary">{">"}</span>
                {"  "}
                <span className="text-primary">{"<"}</span>
                {")    ~Gitru UwU\n"}
                {"   /    |\n"}
                {"乀(ˍ, ل ل"}
              </pre>
            ) : (
              <pre className="font-mono">
                {"      へ\n"}
                {"  ૮  "}
                <span className="text-primary">{"o"}</span>
                {"  "}
                <span className="text-primary">{"o"}</span>
                {")    ~Gitru UwU \n"}
                {"   /    |\n"}
                {"乀(ˍ, ل ل"}
              </pre>
            )}
            <br />
            <span className="mono whitespace-pre-wrap">
              <TypeWriter
                segments={allSegments}
                defaultSpeed={50}
                startDelay={500}
                onAllInputsSubmit={handleAllInputsSubmit}
                showCursor={
                  !isLoading && !isSuccess && !isAlreadyExists && !isError
                }
              />
              {isLoading && (
                <>
                  {"\n  "}
                  <span className="text-muted-foreground">
                    <span className="text-primary">
                      {loaderFrames[loaderFrame]}
                    </span>{" "}
                    Connecting to origin...
                  </span>
                </>
              )}
              {isError && (
                <>
                  {"\n  "}
                  <span className="text-red-500">✗ error: {errorMessage}</span>
                  {"\n  "}
                  <span className="text-muted-foreground">
                    hint: Please check your input and try again.
                  </span>
                </>
              )}
              {isAlreadyExists && (
                <>
                  {"\n  "}
                  <span className="text-yellow-500">
                    ! You are already on the waitlist
                  </span>
                  {"\n  "}
                  <span className="text-muted-foreground">
                    {`hint: You'll receive an email when you're in!`}
                  </span>
                </>
              )}
              {isSuccess && (
                <>
                  {"\n  "}
                  <span className="text-green-600">
                    ✓ Successfully joined the waitlist!
                  </span>
                  {"\n  "}
                  <span className="text-muted-foreground">
                    {`next: You'll receive an email when you're in!`}
                  </span>
                </>
              )}
              {(isSuccess || isAlreadyExists) && (
                <TypeWriter
                  segments={[
                    {
                      text: "\n$ ",
                      delay: 0,
                      speed: 0,
                      cursor: "primary",
                    },
                  ]}
                  defaultSpeed={50}
                  startDelay={0}
                  showCursor={true}
                />
              )}
            </span>
          </>
        )}
      </div>
      <div className="absolute bottom-2 right-2">
        <pre>
          <span className="text-primary font-mono tabular-nums">
            {totalJoined} people look in
          </span>
        </pre>
      </div>
    </div>
  );
};

export default Waitlist;
