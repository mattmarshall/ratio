// Bundle the console.
//
// esbuild's JS API rather than its CLI: the `esbuild` bin entry is a NATIVE
// binary, and rules_js runs bin entries with node — which greets it with
// "SyntaxError: Invalid or unexpected token" on the first byte. Importing the
// library instead lets esbuild find and spawn its own binary the way it
// expects to, and keeps this to one bazel_dep fewer than rules_esbuild.
import * as esbuild from "esbuild";

const out = process.argv[2];
if (!out) throw new Error("usage: build.mjs <outfile>");

await esbuild.build({
  entryPoints: ["dist/src/main.js"],
  bundle: true,
  format: "esm",
  minify: true,
  target: "es2022",
  outfile: out,
  // React reads this and drops its development-only warnings and prop-type
  // machinery. Without it the bundle ships the dev build, which is roughly
  // twice the size and slower for no benefit a demo can see.
  define: { "process.env.NODE_ENV": '"production"' },
  logLevel: "warning",
});
