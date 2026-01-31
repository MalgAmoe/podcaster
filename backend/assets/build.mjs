import esbuild from "esbuild";
import { solidPlugin } from "esbuild-plugin-solid";
import { mkdirSync } from "fs";

// Ensure output directories exist (prevents silent build failures)
mkdirSync("../priv/static/assets/js", { recursive: true });
mkdirSync("../priv/static/assets/css", { recursive: true });

const args = process.argv.slice(2);
const watch = args.includes("--watch");
const deploy = process.env.MIX_ENV === "prod" || args.includes("--deploy");
const mixEnv = process.env.MIX_ENV || "dev";

const ctx = await esbuild.context({
  entryPoints: ["js/app.js"],
  bundle: true,
  target: "es2022",
  outdir: "../priv/static/assets/js",
  external: ["/fonts/*", "/images/*"],
  plugins: [solidPlugin()],
  minify: deploy,
  sourcemap: !deploy,
  define: {
    "process.env.NODE_ENV": deploy ? '"production"' : '"development"',
    "import.meta.env.DEV": deploy ? "false" : "true",
  },
  nodePaths: ["./node_modules", "../deps", `../_build/${mixEnv}`],
});

if (watch) {
  await ctx.watch();
  console.log("Watching for changes...");
} else {
  await ctx.rebuild();
  await ctx.dispose();
}
