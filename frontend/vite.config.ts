import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import { vanillaExtractPlugin } from "@vanilla-extract/vite-plugin";

const hmrClientPort = process.env.SHIP_VITE_HMR_CLIENT_PORT
  ? Number.parseInt(process.env.SHIP_VITE_HMR_CLIENT_PORT, 10)
  : undefined;

// r[frontend.test.vitest]
export default defineConfig({
  clearScreen: false,
  plugins: [react(), vanillaExtractPlugin()],
  test: {
    environment: "jsdom",
    setupFiles: "./src/test/setup.ts",
  },
  server: {
    host: "127.0.0.1",
    strictPort: true,
    hmr: {
      host: process.env.SHIP_VITE_HMR_HOST,
      clientPort: Number.isNaN(hmrClientPort) ? undefined : hmrClientPort,
      protocol: "ws",
    },
  },
});
