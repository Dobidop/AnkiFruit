import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// The app is served by the Rust backend from the same origin, so asset paths
// must be relative and API calls can stay origin-relative.
export default defineConfig({
  base: "./",
  plugins: [react()],
  build: { outDir: "dist", emptyOutDir: true },
});
