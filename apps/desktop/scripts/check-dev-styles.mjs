import assert from "node:assert/strict";
import { fileURLToPath } from "node:url";
import { createServer } from "vite";

// Exercise the development CSS transform, which the production build and
// jsdom component tests do not cover. A URL checks an already running server;
// otherwise use the real Vite config in middleware mode without occupying a port.
const root = fileURLToPath(new URL("../", import.meta.url));
let server;
try {
  const origin = process.argv[2];
  let css;
  if (!origin) {
    server = await createServer({
      root,
      configFile: fileURLToPath(new URL("../vite.config.ts", import.meta.url)),
      logLevel: "error",
      server: { middlewareMode: true, open: false },
    });
    const result = await server.transformRequest("/src/styles/globals.css?direct");
    assert.ok(result, "The development stylesheet must be transformed successfully");
    css = result.code;
  } else {
    const response = await fetch(new URL("/src/styles/globals.css?direct", origin), { signal: AbortSignal.timeout(15_000) });
    assert.equal(response.status, 200, "The development stylesheet must be served successfully");
    css = await response.text();
  }
  assert.doesNotMatch(css, /@(theme|tailwind|custom-variant|source)\b/, "Uncompiled Tailwind directives reached the browser; restart Vite to load the current plugins");
  assert.match(css, /\.flex\s*\{[^}]*display:\s*flex/, "The shell's flex layout utility is missing");
  assert.match(css, /\.h-10\s*\{/, "The toolbar height utility is missing");
  assert.match(css, /--font-sans:[^;]*Geist Variable/, "The shared font token was not compiled");
  console.log("Development CSS passed: compiled Tailwind utilities and shared font tokens are present.");
} finally {
  await server?.close();
}
