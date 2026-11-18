#!/usr/bin/env node
// Build the Tauri updater manifest (`latest.json`) from signed bundle output.
//
// The release workflow downloads every platform's artifacts into one folder,
// then runs this script. Channel selection lives in the workflow: stable
// publishes this manifest on the version tag (and GitHub's `releases/latest`
// points at it), beta additionally copies it to the rolling `updater-beta`
// release.
import {
  existsSync,
  readdirSync,
  readFileSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { basename, join } from "node:path";

const artifactsDir = process.argv[2] ?? "artifacts";
const outputPath = process.argv[3] ?? join(artifactsDir, "latest.json");
const tag = process.env.RELEASE_TAG;
const version = process.env.RELEASE_VERSION;
const channel = process.env.RELEASE_CHANNEL;

if (!tag || !version || !channel) {
  console.error(
    "RELEASE_TAG, RELEASE_VERSION and RELEASE_CHANNEL must be set",
  );
  process.exit(1);
}

if (tag.replace(/^v/, "") !== version) {
  console.error(`tag ${tag} does not match version ${version}`);
  process.exit(1);
}

const expectedChannel = version.includes("-") ? "beta" : "stable";
if (channel !== expectedChannel) {
  console.error(
    `version ${version} belongs to the ${expectedChannel} channel, not ${channel}`,
  );
  process.exit(1);
}

function walk(directory) {
  const entries = [];
  for (const name of readdirSync(directory)) {
    const path = join(directory, name);
    if (statSync(path).isDirectory()) entries.push(...walk(path));
    else entries.push(path);
  }
  return entries;
}

if (!existsSync(artifactsDir)) {
  console.error(`artifact directory not found: ${artifactsDir}`);
  process.exit(1);
}

const files = walk(artifactsDir);
const targets = [
  { platform: "linux-x86_64", match: (name) => name.endsWith(".AppImage") },
  { platform: "windows-x86_64", match: (name) => name.endsWith("-setup.exe") },
  { platform: "darwin-aarch64", match: (name) => name.endsWith(".app.tar.gz") },
];

const platforms = {};
for (const { platform, match } of targets) {
  const file = files.find((path) => match(basename(path)));
  if (!file) {
    // A platform can legitimately be absent from a partial release. The
    // updater reports that target as unavailable instead of installing the
    // wrong bundle.
    console.warn(`latest.json: no ${platform} updater artifact found; skipped`);
    continue;
  }

  const signaturePath = `${file}.sig`;
  if (!existsSync(signaturePath)) {
    console.error(`missing signature next to ${file}`);
    process.exit(1);
  }
  const signature = readFileSync(signaturePath, "utf8").trim();
  if (!signature) {
    console.error(`empty signature next to ${file}`);
    process.exit(1);
  }

  const asset = encodeURIComponent(basename(file));
  platforms[platform] = {
    signature,
    url: `https://github.com/j1seki/omniscreenveil/releases/download/${encodeURIComponent(tag)}/${asset}`,
  };
}

if (Object.keys(platforms).length === 0) {
  console.error("no updater artifacts found");
  process.exit(1);
}

const manifest = {
  version,
  notes: `OmniScreenVeil v${version} (${channel} channel). Release notes: https://github.com/j1seki/omniscreenveil/releases/tag/${encodeURIComponent(tag)}`,
  pub_date: new Date().toISOString(),
  platforms,
};

writeFileSync(outputPath, `${JSON.stringify(manifest, null, 2)}\n`);
console.log(`wrote ${outputPath}`);
console.log(JSON.stringify(manifest, null, 2));
