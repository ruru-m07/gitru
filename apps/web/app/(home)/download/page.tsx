"use client";

import { Clock } from "lucide-react";
import Image from "next/image";
import { useEffect, useRef, useState } from "react";
import { Button } from "@/components/ui/button";
import { api } from "@/lib/api";
import { cn } from "@/lib/cn";

const DownloadPage = () => {
  const [isLoading, setIsLoading] = useState(false);
  const [isSuccess, setIsSuccess] = useState(false);
  const [isAlreadyExists, setIsAlreadyExists] = useState(false);
  const [isError, setIsError] = useState(false);
  const [errorMessage, setErrorMessage] = useState("");
  const [mobileForm, setMobileForm] = useState({ username: "", email: "" });

  const [totalJoined, setTotalJoined] = useState<number>(0);

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

  return (
    <div className="h-screen w-screen flex flex-col items-center">
      <div className="flex flex-col gap-10 max-w-(--container-width) px-(--container-gutter) w-full mt-30 h-full">
        <h2 className="text-4xl font-light">Join Waitlist</h2>
        <div className="flex-1 flex flex-col gap-3 h-full">
          <div
            className={cn(
              "relative inline-block",
              "[--outer-border:10px] md:[--outer-border:16px] lg:[--outer-border:28px]",
              "[--inner-border:4px] md:[--inner-border:10px] lg:[--inner-border:16px]",
              "[--spaceing-between:6px] md:[--spaceing-between:8px] lg:[--spaceing-between:16px]",
            )}
          >
            {/* Background layer */}
            <div className="absolute inset-0 overflow-hidden rounded-(--outer-border) bg-black/50">
              <Image
                fill
                src="/waitlist-background.webp"
                alt="Waitlist Illustration"
                className="object-cover blur-xs"
                fetchPriority="high"
                priority
              />
              <div className="absolute inset-0 rounded-(--outer-border) ring-1 ring-inset ring-black/12!" />
            </div>

            {/* Content (drives height) */}
            <div className="relative p-(--spaceing-between) my-30 flex flex-col items-center">
              <form
                onSubmit={async (event) => {
                  event.preventDefault();
                  await handleAllInputsSubmit({
                    username: mobileForm.username,
                    email: mobileForm.email,
                  });
                }}
                className="w-full max-w-md space-y-4 rounded-xl "
              >
                <Input
                  label="Name"
                  name="username"
                  placeholder="nickname"
                  value={mobileForm.username}
                  onChange={(event) =>
                    setMobileForm((prev) => ({
                      ...prev,
                      username: event.target.value,
                    }))
                  }
                />

                <Input
                  label="Email"
                  name="email"
                  type="email"
                  placeholder="yo@example.com"
                  value={mobileForm.email}
                  onChange={(event) =>
                    setMobileForm((prev) => ({
                      ...prev,
                      email: event.target.value,
                    }))
                  }
                />

                <Button
                  size={"lg"}
                  type="submit"
                  disabled={isLoading}
                  className={cn(
                    "w-full h-10 focus-visible:ring-2",
                    "shadow-[inset_-1px_-1px_2px_1px_rgba(0,0,0,0.1),inset_1px_1px_2px_1px_rgba(255,255,255,0.1),0px_0px_4px_2px_rgba(0,0,0,0.1)]",
                  )}
                >
                  <Clock className="size-4 text-[color-mix(in_oklab,var(--primary)_40%,#ffffff)]" />
                  {isLoading ? `Joining...` : "Join waitlist"}
                </Button>

                <div className="text-sm font-mono whitespace-pre-wrap">
                  {isError && (
                    <p className="text-red-400">error: {errorMessage}</p>
                  )}
                  {isAlreadyExists && (
                    <p className="text-yellow-400">
                      You are already on the waitlist
                    </p>
                  )}
                  {isSuccess && (
                    <p className="text-green-400">
                      Successfully joined the waitlist!
                    </p>
                  )}
                  {(isAlreadyExists || isSuccess) && (
                    <p className="text-white">
                      You will receive an email when you are in.
                    </p>
                  )}
                </div>
              </form>
            </div>
          </div>
          {totalJoined > 0 && (
            <div className="text-xs text-right text-muted-foreground font-mono">
              {totalJoined} humans look-in
            </div>
          )}
        </div>
      </div>
    </div>
  );
};

export default DownloadPage;

const Input = ({
  label,
  name,
  type = "text",
  placeholder,
  value,
  onChange,
  disabled,
}: {
  label: string;
  name: string;
  type?: string;
  placeholder?: string;
  value: string;
  onChange: (event: React.ChangeEvent<HTMLInputElement>) => void;
  disabled?: boolean;
}) => (
  <label className="space-y-1 flex flex-col">
    <span className="text-sm font-medium text-white">{label}</span>
    <input
      type={type}
      name={name}
      required
      className="w-full rounded-lg bg-white/70 backdrop-blur-sm h-10 border border-black/20 px-3 py-2 focus:outline-none focus:ring-2 focus:ring-white/40"
      placeholder={placeholder}
      value={value}
      onChange={onChange}
      disabled={disabled}
    />
  </label>
);
