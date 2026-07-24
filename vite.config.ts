/// <reference types="vitest/config" />

import { fileURLToPath } from "node:url";
import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

const host = process.env.TAURI_DEV_HOST;

// https://vite.dev/config/
export default defineConfig(async () => ({
  plugins: [
    react(),
    tailwindcss(),
    // NumberFlow (scorecard/temp/clock digit-roll) ships two code paths
    // behind `esm-env`'s package-export conditions: a real
    // browser-custom-element path and an SSR-safe static-HTML fallback.
    // Vitest transforms test modules through Vite's SSR pipeline even
    // though `test.environment` is jsdom, so without an explicit
    // "browser" condition `esm-env` resolves BROWSER=false and NumberFlow
    // silently falls back to the static path — mounting fine but throwing
    // ("willUpdate is not a function") the instant a value prop changes,
    // since the SSR element is never upgraded to the real custom element.
    // A plain `resolve.conditions: ["browser"]` in this config's `resolve`
    // block does NOT fix this — Vitest on Vite 6+/7 ignores conditions set
    // that way for its own SSR dep resolution (vitest-dev/vitest#8431); a
    // `config()` hook mutating the resolved config is the documented
    // workaround, paired with `test.server.deps.inline` below (forces the
    // library through Vite's transform pipeline, which is what actually
    // honors the condition, rather than being pre-bundled/externalized).
    // Gated on `config.mode === "test"` (vitest's own mode) so the actual
    // app build/dev server — which already resolves the browser condition
    // correctly on its own — is untouched.
    {
      name: "numberflow-vitest-browser-condition",
      config(config, env) {
        if (env.command === "serve" && config.mode === "test") {
          config.resolve = config.resolve ?? {};
          config.resolve.conditions = ["browser"];
        }
      },
    },
  ],

  resolve: {
    alias: {
      // plan 112: shadcn-generated components import via "@/..."; resolve
      // ESM-safely with fileURLToPath rather than "/src" or bare
      // __dirname (unavailable under Vite's ESM config loading).
      "@": fileURLToPath(new URL("./src", import.meta.url)),
    },
  },

  build: {
    rollupOptions: {
      input: {
        main: "index.html",
        settings: "settings.html",
      },
    },
  },

  test: {
    environment: "jsdom",
    // Pin the timezone so date/time-formatting tests are deterministic
    // regardless of the runner's local zone. Without this, tests that assert
    // rendered `HH:MM` pass on the dev machine (IST) but fail in CI (UTC).
    // Assertions on formatted times are written against UTC.
    env: { TZ: "UTC" },
    // don't discover tests inside agent worktrees (.claude/worktrees/<name>
    // holds full repo copies whose tests would double-count or break ours)
    exclude: ["**/node_modules/**", "**/.claude/**", "**/dist/**"],
    server: {
      // Force NumberFlow's packages through Vite's transform pipeline
      // (rather than being externalized/pre-bundled) so the "browser"
      // condition forced above actually applies to them — see this
      // plugin's own doc comment for why both halves are needed.
      deps: { inline: ["number-flow", "@number-flow/react", "esm-env"] },
    },
  },

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent Vite from obscuring rust errors
  clearScreen: false,
  // 2. tauri expects a fixed port, fail if that port is not available
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      // 3. tell Vite to ignore watching `src-tauri`
      ignored: ["**/src-tauri/**"],
    },
  },
}));
