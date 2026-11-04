import { spawn } from "node:child_process";
import { root, writeUpdaterConfiguration } from "./release.mjs";

const [command, ...args] = process.argv.slice(2);
if (!["dev", "build"].includes(command)) throw new Error("Use desktop.mjs dev or build.");
const pnpmCli = process.env.npm_execpath;
if (!pnpmCli) throw new Error("Run this wrapper with pnpm dev:desktop or pnpm build:desktop.");
const config = await writeUpdaterConfiguration();
const child = spawn(process.execPath, [pnpmCli, "tauri", command, "--config", config, ...args], { cwd: root, stdio: "inherit" });
child.on("exit", (code) => {
  process.exitCode = code ?? 1;
});
child.on("error", (error) => {
  console.error(error.message);
  process.exitCode = 1;
});
