import { defineConfig, loadEnv } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import path from "node:path";
import process from "node:process";

const host = process.env.TAURI_DEV_HOST;
const wwwRoot = path.resolve(__dirname, "apps/www");

function buildProcessEnv(mode: string) {
  const env = loadEnv(mode, wwwRoot, "");
  const processEnv: Record<string, string> = {
    NODE_ENV: mode,
  };
  for (const [key, value] of Object.entries(env)) {
    if (key.startsWith("NEXT_PUBLIC_")) {
      processEnv[key] = value;
    }
  }
  return processEnv;
}

export default defineConfig(({ mode }) => ({
  plugins: [react(), tailwindcss()],
  envDir: wwwRoot,
  envPrefix: ["NEXT_PUBLIC_"],
  define: {
    "process.env": JSON.stringify(buildProcessEnv(mode)),
  },
  resolve: {
    alias: {
      "@": wwwRoot,
      "@desktop": path.resolve(__dirname, "./src"),
      "next/link": path.resolve(__dirname, "./src/shims/next-link.tsx"),
      "next/navigation": path.resolve(
        __dirname,
        "./src/shims/next-navigation.ts",
      ),
    },
  },
  test: {
    environment: "node",
    include: [
      "src/**/*.test.ts",
      "apps/www/lib/**/*.test.ts",
      "apps/www/hooks/**/*.test.ts",
      "apps/www/components/**/*.hook.test.ts",
      "apps/www/app/**/*.test.ts",
      "crates/sprite-computer/guest/**/*.test.mjs",
    ],
  },
  clearScreen: false,
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
      ignored: ["**/src-tauri/**"],
    },
    proxy: {
      "/api": {
        target: "http://127.0.0.1:3000",
        changeOrigin: true,
      },
    },
  },
}));
