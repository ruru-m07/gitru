import { resolve } from "node:path";
import react from "@vitejs/plugin-react";
import { configDefaults, defineConfig } from "vitest/config";

const repositoryRoot = __dirname;
const sharedSetup = resolve(repositoryRoot, "tests/setup.ts");
const reactTestFiles = ["**/*.{test,spec}.{ts,tsx}"];

const reactProject = ({
  alias,
  name,
  path,
  setupFile = sharedSetup,
}: {
  alias?: Record<string, string>;
  name: string;
  path: string;
  setupFile?: string;
}) => ({
  extends: true as const,
  resolve: alias ? { alias } : undefined,
  test: {
    include: reactTestFiles,
    name,
    root: resolve(repositoryRoot, path),
    setupFiles: [setupFile],
  },
});

export default defineConfig({
  plugins: [react()],
  test: {
    clearMocks: true,
    environment: "jsdom",
    environmentOptions: {
      jsdom: {
        pretendToBeVisual: true,
        url: "http://localhost/",
      },
    },
    env: {
      TZ: "UTC",
    },
    exclude: [
      ...configDefaults.exclude,
      "**/.next/**",
      "**/.source/**",
      "**/.turbo/**",
      "**/out/**",
    ],
    projects: [
      reactProject({
        alias: {
          "@": resolve(repositoryRoot, "apps/desktop/src"),
        },
        name: "desktop",
        path: "apps/desktop",
        setupFile: resolve(repositoryRoot, "apps/desktop/tests/setup.ts"),
      }),
      reactProject({
        alias: {
          "@": resolve(repositoryRoot, "apps/web"),
        },
        name: "web",
        path: "apps/web",
      }),
      reactProject({
        alias: {
          "@gitru/ui": resolve(repositoryRoot, "packages/ui/src"),
        },
        name: "ui",
        path: "packages/ui",
      }),
      reactProject({
        alias: {
          "@gitru/mascot": resolve(repositoryRoot, "packages/mascot/src"),
        },
        name: "mascot",
        path: "packages/mascot",
      }),
      reactProject({
        name: "icon",
        path: "packages/icon",
      }),
      {
        extends: true,
        test: {
          environment: "node",
          include: ["**/*.{test,spec}.ts"],
          name: "commands",
          root: resolve(repositoryRoot, "packages/commands"),
          setupFiles: [],
        },
      },
    ],
    reporters: process.env.GITHUB_ACTIONS
      ? ["default", "github-actions"]
      : ["default"],
    restoreMocks: true,
    unstubEnvs: true,
    unstubGlobals: true,
  },
});
