import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const script = fileURLToPath(new URL("./release-version.mjs", import.meta.url));
const run = (...args) => execFileSync(process.execPath, [script, ...args], { encoding: "utf8" }).trim();

test("automatic releases keep the configured channel and move forward", () => {
  assert.equal(run("next", "0.1.4-rc.1", "v0.1.3", "v0.1.4-rc.1"), "0.1.4-rc.2");
  assert.equal(run("next", "0.1.4-rc.1", "v0.1.4"), "0.1.5-rc.1");
  assert.equal(run("next", "0.1.4-rc.1", "v0.1.4-rc.9", "v0.1.4-rc.10"), "0.1.4-rc.11");
  assert.equal(run("next", "0.1.4", "v0.1.4", "v0.1.5-rc.1"), "0.1.5");
  assert.equal(run("next", "0.2.0", "v0.1.4"), "0.2.0");
});

test("rebuilding older releases preserves the current channel manifests", () => {
  assert.equal(run("can-promote", "0.1.4-rc.2", "v0.1.4-rc.10", "updater-beta"), "false");
  assert.equal(run("can-promote", "0.1.4-rc.10", "v0.1.4-rc.10", "v0.1.5"), "true");
  assert.equal(run("can-promote", "0.1.4", "v0.1.5"), "false");
  assert.equal(run("can-promote", "0.1.4", "v0.1.5-rc.1"), "true");
});
