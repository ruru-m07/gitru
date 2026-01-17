import { ShikiWorkerClient } from "@gitru/diff/worker";
import React from "react";

const Progress = () => {
  const shiki = new ShikiWorkerClient();

  const [html, setHtml] = React.useState<string | null>(null);

  return (
    <div className="relative h-screen w-full flex items-center justify-center pt-20">
      <div className="max-w-[600px] h-full w-full">
        <h1 className="text-xl font-mono text-center">soon...</h1>
        <button
          onClick={async () => {
            const html = await shiki.highlight(
              `{
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
    "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
    "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
    "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
    "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
    "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
    "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
    "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
    "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
    "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
  "name": "@gitru/commands",
  "module": "index.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./index.ts"
    }
  },
  "devDependencies": {
    "@tauri-apps/api": "catalog:",
    "zod": "catalog:",
    "@types/bun": "^1.3.6"
  },
}`,
              "json",
              "vesper",
            );
            setHtml(html.html);
          }}
        >
          highlight
        </button>
        <div dangerouslySetInnerHTML={{ __html: html ?? "" }}></div>
      </div>
    </div>
  );
};

export default Progress;
