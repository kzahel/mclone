import { defineConfig } from "vite";
import { repositoryRoot } from "./src/vite-figure-path";

export default defineConfig({
  server: {
    fs: {
      allow: [repositoryRoot],
    },
  },
});
