// Build the injected Windows helper and stage it where the app expects it.
//
// The helper is only meaningful on Windows; on other hosts this is a no-op so
// `pnpm tauri dev/build` keeps working unchanged.

import { execFileSync } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

if (process.platform !== "win32") {
  process.exit(0);
}

const here = dirname(fileURLToPath(import.meta.url));
const desktop = resolve(here, "..");
const root = resolve(desktop, "../..");
const srcTauri = join(desktop, "src-tauri");
const triple = process.env.TAURI_ENV_TARGET_TRIPLE;

console.log("veil: building the Windows hook helper");
const cargoArgs = ["build", "-p", "veil-hook-win", "--release"];
if (triple) {
  cargoArgs.push("--target", triple);
}
execFileSync("cargo", cargoArgs, {
  cwd: root,
  stdio: "inherit",
});

const built = triple
  ? join(root, "target", triple, "release", "veil_hook_win.dll")
  : join(root, "target", "release", "veil_hook_win.dll");

if (!existsSync(built)) {
  throw new Error(`hook DLL was not produced at ${built}`);
}

const stageDirs = [
  join(srcTauri, "resources"),
  join(srcTauri, "target", "debug"),
  join(srcTauri, "target", "release"),
  join(root, "target", "debug"),
  join(root, "target", "release"),
];

if (triple) {
  stageDirs.push(join(srcTauri, "target", triple, "debug"));
  stageDirs.push(join(srcTauri, "target", triple, "release"));
  stageDirs.push(join(root, "target", triple, "debug"));
  stageDirs.push(join(root, "target", triple, "release"));
}

for (const dir of stageDirs) {
  mkdirSync(dir, { recursive: true });
  const destination = join(dir, "veil_hook_win.dll");
  if (resolve(built).toLowerCase() !== resolve(destination).toLowerCase()) {
    copyFileSync(built, destination);
  }
}

console.log(`veil: hook helper staged across ${stageDirs.length} directories`);
