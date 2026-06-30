export type GIT_PROVIDERS = "github" | "gitlab" | "bitbucket" | "unknown";

export type CSSVars = React.CSSProperties & {
  [key: `--${string}`]: string | number;
};
