/** Vite entry: mounts the WAF Console React root. */
import { createRoot } from "react-dom/client";
import App from "./App";
import "@nerve/ui/style.css";
import "@logicflow/core/lib/style/index.css";
import "@logicflow/extension/lib/style/index.css";
import "./app-local.css";

// Skip StrictMode: dev double-mount of ensureAuth/Keycloak can cause full-page refresh loops
createRoot(document.getElementById("root")!).render(<App />);
