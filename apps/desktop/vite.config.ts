import { fileURLToPath, URL } from "node:url";
import { readFileSync } from "node:fs";
import tailwindcss from "@tailwindcss/vite";
import vue from "@vitejs/plugin-vue";
import vueJsx from "@vitejs/plugin-vue-jsx";
import { defineConfig } from "vite";

const isDesktop = process.env.TAURI_ENV_PLATFORM !== undefined;
const apiTarget = process.env.SPECX_API_PROXY_TARGET ?? "http://127.0.0.1:4224";
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
    // Do not silently open a second port when an older frontend owns this one.
    strictPort: true,
    proxy: {
      "/api": { target: apiTarget, changeOrigin: false },
    },
    watch: { ignored: ["**/src-tauri/**"] },
  },
  build: { outDir: "dist", emptyOutDir: true },
  test: {
    environment: "jsdom",
    include: ["src/**/*.spec.{ts,tsx,js,jsx}"],
  },
});
