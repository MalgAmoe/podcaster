import esbuild from "esbuild";
import { solidPlugin } from "esbuild-plugin-solid";

const args = process.argv.slice(2);
const watch = args.includes("--watch");

const ctx = await esbuild.context({
  entryPoints: ["js/app.js"],
  bundle: true,
  target: "es2022",
  outdir: "../priv/static/assets/js",
  external: ["/fonts/*", "/images/*"],
  plugins: [solidPlugin()],
  define: {
    "process.env.NODE_ENV": watch ? '"development"' : '"production"',
  },
  nodePaths: ["./node_modules", "../deps", "../_build/dev"],
});

if (watch) {
  await ctx.watch();
  console.log("Watching for changes...");
} else {
  await ctx.rebuild();
  await ctx.dispose();
}
