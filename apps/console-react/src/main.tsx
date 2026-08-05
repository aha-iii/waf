import { createRoot } from "react-dom/client";
import App from "./App";
import "@nerve/ui/style.css";
import "@logicflow/core/lib/style/index.css";
import "@logicflow/extension/lib/style/index.css";
import "./app-local.css";

// 不用 StrictMode：开发态会双挂载 ensureAuth/Keycloak，易触发整页刷新死循环
createRoot(document.getElementById("root")!).render(<App />);
