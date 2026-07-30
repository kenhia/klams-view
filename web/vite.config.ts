import { sveltekit } from "@sveltejs/kit/vite";
import tailwindcss from "@tailwindcss/vite";
import { defineConfig } from "vite";

export default defineConfig({
  plugins: [tailwindcss(), sveltekit()],
  server: {
    port: 5174,
    proxy: {
      "/api": {
        target: process.env.KLAMS_VIEW_API ?? "http://localhost:7778",
        changeOrigin: true,
      },
    },
  },
});
