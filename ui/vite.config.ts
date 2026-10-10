import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// Relative base: the built app opens from any path (a tailnet address, a subpath, a file server).
export default defineConfig({ base: "./", plugins: [react()], build: { outDir: "dist", emptyOutDir: true } });
