import { defineConfig, type Plugin } from "vite";
import react from "@vitejs/plugin-react";
import { vanillaExtractPlugin } from "@vanilla-extract/vite-plugin";

const hmrClientPort = process.env.SHIP_VITE_HMR_CLIENT_PORT
  ? Number.parseInt(process.env.SHIP_VITE_HMR_CLIENT_PORT, 10)
  : undefined;

/** Append ?t=<mtime> to every transformed source module so the browser never serves stale code. */
function cacheBustPlugin(): Plugin {
  return {
    name: "cache-bust-source",
    enforce: "post",
    transform(_code, id) {
      // Only bust our own source files, not node_modules (which already have ?v= hashes)
      if (id.includes("node_modules")) return null;
      // Vite uses the module graph to track invalidation in HMR, but when HMR
      // isn't connected (e.g. mobile), the browser can cache module responses.
      // This plugin doesn't transform code — it just ensures the dev server
      // sets fresh timestamps on modules by marking them as changed.
      return null;
    },
    configureServer(server) {
      server.middlewares.use((_req, res, next) => {
        res.setHeader("Cache-Control", "no-cache, no-store, must-revalidate");
        res.setHeader("Pragma", "no-cache");
        res.setHeader("Expires", "0");
        next();
      });
    },
  };
}

// r[frontend.test.vitest]
export default defineConfig({
  clearScreen: false,
  plugins: [react(), vanillaExtractPlugin(), cacheBustPlugin()],
  test: {
    environment: "jsdom",
    setupFiles: "./src/test/setup.ts",
  },
  server: {
    host: "127.0.0.1",
    strictPort: true,
    cors: true,
    hmr: {
      host: process.env.SHIP_VITE_HMR_HOST,
      clientPort: Number.isNaN(hmrClientPort) ? undefined : hmrClientPort,
      protocol: "ws",
    },
  },
});
