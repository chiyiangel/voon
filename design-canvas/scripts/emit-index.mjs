import { createRequire } from "node:module";
import { mkdirSync, writeFileSync } from "node:fs";
import { pathToFileURL } from "node:url";

const require = createRequire(import.meta.url);
const esbuild = require("esbuild");
mkdirSync("node_modules/.cache", { recursive: true });
const outfile = "node_modules/.cache/voon-index.mjs";
await esbuild.build({
  entryPoints: ["src/index-data.ts"],
  bundle: true,
  format: "esm",
  platform: "node",
  outfile,
});
const mod = await import(`${pathToFileURL(outfile).href}?t=${Date.now()}`);
const json = mod.emitIndex();
writeFileSync("element-index.json", json);
const parsed = JSON.parse(json);
console.log(`element-index.json  frames=${parsed.frameCount}  elements=${parsed.elementCount}`);
