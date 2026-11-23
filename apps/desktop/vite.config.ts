import { fileURLToPath, URL } from "node:url";
import { readFileSync } from "node:fs";
import tailwindcss from "@tailwindcss/vite";
import vue from "@vitejs/plugin-vue";
import vueJsx from "@vitejs/plugin-vue-jsx";
import { defineConfig } from "vite";

const isDesktop = process.env.TAURI_ENV_PLATFORM !== undefined;
const apiTarget = process.env.SPECX_API_PROXY_TARGET ?? "http://127.0.0.1:4224";
const apiOrigin = new URL(apiTarget).origin;
// When the API is configured with a public reverse-proxy origin, browser
// writes through the dev proxy must carry that same origin as well.
const publicOrigin = process.env.SPECX_PUBLIC_ORIGIN?.trim() || apiOrigin;
const packageInfo = JSON.parse(readFileSync(new URL("../../package.json", import.meta.url), "utf8"));
const releaseConfig = JSON.parse(readFileSync(new URL("../../release.config.json", import.meta.url), "utf8"));

export default defineConfig({
  plugins: [vue(), vueJsx(), tailwindcss()],
  define: {
    __SPECX_BUILD_INFO__: JSON.stringify({ version: packageInfo.version, repository: process.env.SPECX_RELEASE_REPOSITORY || releaseConfig.repository || "", commit: (process.env.SPECX_BUILD_COMMIT || "").slice(0, 12) }),
  },
  resolve: {
    alias: { "@": fileURLToPath(new URL("./src", import.meta.url)) },
  },
  clearScreen: false,
  server: {
    host: "127.0.0.1",
    port: isDesktop ? 1420 : 5173,
    allowedHosts: [".localhost"],
    // Do not silently open a second port when an older frontend owns this one.
    strictPort: true,
    proxy: {
      "/api": {
        target: apiTarget,
        changeOrigin: false,
        // The browser Origin is the dev-server address, while the API sees a
        // same-process proxy. Align it with the configured public API origin
        // so write-origin protection does not reject local development calls.
        configure(proxy) {
          proxy.on("proxyReq", (proxyRequest, request) => {
            if (request.headers.origin) proxyRequest.setHeader("origin", publicOrigin);
          });
        },
      },
    },
    watch: { ignored: ["**/src-tauri/**"] },
  },
  build: { outDir: "dist", emptyOutDir: true },
  worker: { format: "es" },
  test: {
    environment: "jsdom",
    include: ["src/**/*.spec.{ts,tsx,js,jsx}"],
  },
});
