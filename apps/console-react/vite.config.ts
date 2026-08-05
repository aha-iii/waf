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

/** React Console：5177，避免与原 Console :5174 冲突 */
export default defineConfig(({ mode }) => {
  const envDir = path.resolve(__dirname, "../..");
  const env = loadEnv(mode, envDir, "");
  const devApiKey = env.NERVE_API_KEY || "";
  const wafApiPort = process.env.NERVE_WAF_API_PORT || env.NERVE_WAF_API_PORT || "8781";
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
      // 绑定所有网卡，宿主机可访问；不要乱改 HMR，否则会整页死循环刷新
      host: "0.0.0.0",
      allowedHosts: true,
      watch: {
        // WSL /mnt/* 上 inotify 易 EMFILE；polling 略耗 CPU 但稳定
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
