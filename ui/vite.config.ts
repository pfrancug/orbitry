import { sveltekit } from "@sveltejs/kit/vite";
import { defineConfig } from "vite";

export default defineConfig({
  plugins: [sveltekit()],
  server: {
    port: 4281,
    strictPort: true,
    proxy: {
      "/api": "http://127.0.0.1:4280",
    },
  },
  preview: {
    port: 4281,
    strictPort: true,
  },
});