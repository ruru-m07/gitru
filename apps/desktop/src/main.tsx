import ReactDOM from "react-dom/client";
import { scan } from "react-scan";

import { AppRoot } from "./bootstrap/app-root";
import { initializeQueryBridge } from "./bootstrap/query-bridge";
import { enableDevDiagnostics } from "./bootstrap/runtime-utils";
import { redirectToLastPage } from "./bootstrap/session-restore";
import "./app.css";

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
