import { readFileSync } from "node:fs";
import { defineConfig } from "vite";
import { viteSingleFile } from "vite-plugin-singlefile";

// Single self-contained index.html (JS + CSS inlined) so a static deploy is one file plus element-index.json.
export default defineConfig({
  base: "./",
  build: { outDir: "dist", emptyOutDir: true },
  plugins: [
    viteSingleFile(),
    {
      name: "element-index",
      configureServer(server) {
        server.middlewares.use("/element-index.json", (_req, res) => {
          res.setHeader("Content-Type", "application/json");
          res.end(readFileSync("element-index.json"));
        });
      },
      generateBundle() {
        this.emitFile({ type: "asset", fileName: "element-index.json", source: readFileSync("element-index.json", "utf8") });
      },
    },
  ],
});
