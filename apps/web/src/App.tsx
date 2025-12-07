import { Link } from "@tanstack/react-router";
import BackgroundShader from "./components/BackgroundShader";

function App() {
  return (
    <div className="relative h-screen w-full flex items-center justify-center">
      <BackgroundShader />
      <div className="absolute inset-0 pointer-events-none">
        <div className="max-w-[600px] mx-auto py-1 mt-20 pointer-events-auto">
          <img
            src="/logo192.png"
            alt="gitru logo"
            className="absolute size-7 -ml-8.5"
          />
          <div className="flex items-center">
            <h1 className="text-2xl font-mono font-[550]">Gitru</h1>
          </div>
          <p className="font-mono mt-2 text-justify">
            Gitru is a modern, lightweight, and powerful Git client designed to
            simplify and abstract away the complexity of Git.
            <br />
            <br />
            It simplifies complex Git workflows like rebasing and branch
            management, offers a clear visual interface.
            <br />
            <br />
            It's not for everyone — Gitru was designed for developers who are
            comfortable with Git concepts but want a cleaner and faster way to
            execute operations.
            <br />
            <br />
            As well it brings GitHub features like managing notifications, PR
            reviewing and issue tracking directly into the app.
            <br />
            <br />
            It started as a pet project, and now I'm trying to spend more time
            on it. Gitru will be available free and open-source soon. Join the{" "}
            <Link to="/waitlist" className="underline hover:text-primary">
              waitlist
            </Link>{" "}
            for early access and updates!
            <br />
            <br />
          </p>

          <div className="flex items-center gap-2 font-mono justify-between group">
            <span className="flex items-center gap-2">
              <Link
                to="/waitlist"
                className="hover:underline cursor-pointer hover:text-primary text-muted-foreground group-hover:text-foreground transition-colors"
              >
                Waitlist
              </Link>
              <p className="text-muted-foreground">·</p>
              <Link
                to="/roadmap"
                className="hover:underline cursor-pointer hover:text-primary text-muted-foreground group-hover:text-foreground transition-colors"
              >
                Roadmap
              </Link>
              <p className="text-muted-foreground">·</p>
              <Link
                to="/progress"
                className="hover:underline cursor-pointer hover:text-primary text-muted-foreground group-hover:text-foreground transition-colors"
              >
                Progress
              </Link>
            </span>{" "}
            <a
              target="_blank"
              rel="noopener noreferrer"
              className="hover:underline cursor-pointer hover:text-foreground text-muted-foreground group-hover:text-foreground transition-colors size-4"
              href="https://x.com/ruru_x"
            >
              <svg fill="none" viewBox="0 0 1200 1227">
                <path
                  fill="currentColor"
                  d="M714.163 519.284 1160.89 0h-105.86L667.137 450.887 357.328 0H0l468.492 681.821L0 1226.37h105.866l409.625-476.152 327.181 476.152H1200L714.137 519.284h.026ZM569.165 687.828l-47.468-67.894-377.686-540.24h162.604l304.797 435.991 47.468 67.894 396.2 566.721H892.476L569.165 687.854v-.026Z"
                />
              </svg>
              {/* <svg viewBox="0 0 256 209" preserveAspectRatio="xMidYMid">
                <path
                  d="M256 25.45c-9.42 4.177-19.542 7-30.166 8.27 10.845-6.5 19.172-16.793 23.093-29.057a105.183 105.183 0 0 1-33.351 12.745C205.995 7.201 192.346.822 177.239.822c-29.006 0-52.523 23.516-52.523 52.52 0 4.117.465 8.125 1.36 11.97-43.65-2.191-82.35-23.1-108.255-54.876-4.52 7.757-7.11 16.78-7.11 26.404 0 18.222 9.273 34.297 23.365 43.716a52.312 52.312 0 0 1-23.79-6.57c-.003.22-.003.44-.003.661 0 25.447 18.104 46.675 42.13 51.5a52.592 52.592 0 0 1-23.718.9c6.683 20.866 26.08 36.05 49.062 36.475-17.975 14.086-40.622 22.483-65.228 22.483-4.24 0-8.42-.249-12.529-.734 23.243 14.902 50.85 23.597 80.51 23.597 96.607 0 149.434-80.031 149.434-149.435 0-2.278-.05-4.543-.152-6.795A106.748 106.748 0 0 0 256 25.45"
                  fill="currentColor"
                />
              </svg> */}
            </a>
          </div>
        </div>
      </div>
    </div>
  );
}

export default App;
