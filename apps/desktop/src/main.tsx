import ReactDOM from "react-dom/client";
import { scan } from "react-scan";

import { AppRoot } from "./bootstrap/app-root";
import { initializeQueryBridge } from "./bootstrap/query-bridge";
import { enableDevDiagnostics } from "./bootstrap/runtime-utils";
import { redirectToLastPage } from "./bootstrap/session-restore";
import "./app.css";

if (import.meta.env.MODE === "e2e") {
  // Embedded WebDriver can retain Base UI transition nodes after they close
  // and treats a starting popup as hidden. Scope the workaround to those
  // portal lifecycle states so the rest of the app keeps its real motion.
  const style = document.createElement("style");
  style.textContent = `
    [data-base-ui-portal]:has(
      [data-slot="command-dialog-popup"][data-closed][data-ending-style]
    ),
    [data-base-ui-portal]:has(
      [data-slot="dialog-popup"][data-closed][data-ending-style]
    ) {
      display: none !important;
    }
    [data-base-ui-portal]
      [data-slot="command-dialog-popup"][data-open]:not([data-closed]),
    [data-base-ui-portal]
      [data-slot="dialog-popup"][data-open]:not([data-closed]) {
      opacity: 1 !important;
      scale: 1 !important;
      translate: none !important;
    }
  `;
  document.head.append(style);
  await import("@wdio/tauri-plugin");
}

await redirectToLastPage();
initializeQueryBridge();

const rootElement = document.getElementById("root");
if (rootElement && !rootElement.innerHTML) {
  const root = ReactDOM.createRoot(rootElement);
  root.render(<AppRoot />);
}

if (enableDevDiagnostics()) {
  scan({
    enabled: true,
  });
}
