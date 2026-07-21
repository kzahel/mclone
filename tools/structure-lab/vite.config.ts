import { defineConfig } from "vite";
import { repositoryRoot } from "./src/paths";

export default defineConfig({
  server: { fs: { allow: [repositoryRoot] } },
});
