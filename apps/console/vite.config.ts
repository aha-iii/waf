import { defineConfig, loadEnv } from "vite";
import react from "@vitejs/plugin-react";
import path from "path";
import type { ProxyOptions } from "vite";

function apiProxy(target: string, apiKey: string): ProxyOptions {
  const proxy: ProxyOptions = { target, changeOrigin: true };
  if (apiKey) {
    proxy.configure = (server) => {
      server.on("proxyReq", (proxyReq, req) => {
        const hasAuth =
          req.headers.authorization ||
          req.headers["x-api-key"] ||
          proxyReq.getHeader("authorization") ||
          proxyReq.getHeader("x-api-key");
        if (!hasAuth) proxyReq.setHeader("x-api-key", apiKey);
      });
    };
  }
  return proxy;
}

/** Dev server on :5177 (see docs/PORTS.md). */
export default defineConfig(({ mode }) => {
  const envDir = path.resolve(__dirname, "../..");
  const env = loadEnv(mode, envDir, "");
  const devApiKey = env.NERVE_API_KEY || "";
  const wafApiPort = process.env.NERVE_WAF_CONTROLLER_PORT || env.NERVE_WAF_CONTROLLER_PORT || "9000";
  const wafApi = `http://127.0.0.1:${wafApiPort}`;
  const onWslDrvfs =
    __dirname.includes("/mnt/") || process.env.VITE_USE_POLLING === "1" || process.env.CHOKIDAR_USEPOLLING === "1";

  return {
    plugins: [react()],
    resolve: {
      alias: {
        "@nerve/ui/style.css": path.resolve(__dirname, "../../../../Nerve-ui/src/styles/index.css"),
        "@nerve/ui": path.resolve(__dirname, "../../../../Nerve-ui/src/index.ts"),
      },
    },
    server: {
      port: 5177,
      strictPort: true,
      // Bind all interfaces so the host can reach the app; do not casually change HMR or full-page refresh loops ensue
      host: "0.0.0.0",
      allowedHosts: true,
      watch: {
        // inotify on WSL /mnt/* often hits EMFILE; polling uses more CPU but is stable
        usePolling: onWslDrvfs,
        interval: onWslDrvfs ? 1000 : undefined,
        ignored: [
          "**/node_modules/**",
          "**/.git/**",
          "**/_legacy/**",
          "**/dist/**",
          "**/target/**",
          "**/data/**",
          "**/logs/**",
          "**/crates/**",
          "**/plugins/**",
          "**/deploy/**",
        ],
      },
      proxy: {
        "/api/alerts": { target: "http://127.0.0.1:8790", changeOrigin: true },
        "/api/topology": apiProxy("http://127.0.0.1:8780", devApiKey),
        "/api/np": apiProxy("http://127.0.0.1:8780", devApiKey),
        "/api/service-policies": apiProxy("http://127.0.0.1:8780", devApiKey),
        "/api/control": apiProxy("http://127.0.0.1:8780", devApiKey),
        "/api": apiProxy(wafApi, devApiKey),
      },
    },
    build: { outDir: "dist" },
  };
});
