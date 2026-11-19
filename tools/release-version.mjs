import { readFileSync, writeFileSync } from "node:fs";

const pattern = /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?$/;
const isNumber = (part) => /^\d+$/.test(part);

function parse(version) {
  const match = pattern.exec(version);
  if (!match) throw new Error(`Invalid release version: ${version}`);
  const prerelease = match[4]?.split(".") ?? [];
  if (prerelease.some((part) => isNumber(part) && part.length > 1 && part.startsWith("0"))) {
    throw new Error(`Invalid release version: ${version}`);
  }
  return { version, core: match.slice(1, 4).map(BigInt), prerelease };
}

function compare(left, right) {
  for (const [index, number] of left.core.entries()) {
    if (number !== right.core[index]) return number > right.core[index] ? 1 : -1;
  }
  if (!left.prerelease.length || !right.prerelease.length) {
    return Number(!left.prerelease.length) - Number(!right.prerelease.length);
  }
  for (const [index, part] of left.prerelease.entries()) {
    const other = right.prerelease[index];
    if (other === undefined) return 1;
    if (part === other) continue;
    if (isNumber(part) && isNumber(other)) return BigInt(part) > BigInt(other) ? 1 : -1;
    if (isNumber(part) !== isNumber(other)) return isNumber(part) ? -1 : 1;
    return part > other ? 1 : -1;
  }
  return left.prerelease.length - right.prerelease.length;
}

function versions(tags) {
  return tags.flatMap((tag) => {
    if (!tag.startsWith("v")) return [];
    try {
      return [parse(tag.slice(1))];
    } catch {
      return []; // Ignore tags outside the app's version scheme.
    }
  });
}

function nextVersion(current, tags) {
  const beta = current.prerelease.length > 0;
  const existing = versions(tags).filter((version) => beta || !version.prerelease.length);
  const latest = existing.sort(compare).at(-1);
  if (!latest || compare(current, latest) > 0) return current.version;

  const [major, minor, patch] = latest.core;
  if (!latest.prerelease.length) {
    return `${major}.${minor}.${patch + 1n}${beta ? "-rc.1" : ""}`;
  }
  const prerelease = [...latest.prerelease];
  const last = prerelease.at(-1);
  if (isNumber(last)) prerelease[prerelease.length - 1] = String(BigInt(last) + 1n);
  else prerelease.push("1");
  return `${latest.core.join(".")}-${prerelease.join(".")}`;
}

function stampVersion(version) {
  const config = "apps/desktop/src-tauri/tauri.conf.json";
  const current = JSON.parse(readFileSync(config, "utf8")).version;

  const files = new Map();
  for (const path of ["Cargo.toml", "apps/desktop/src-tauri/Cargo.toml"]) {
    const contents = readFileSync(path, "utf8");
    const line = `\nversion = "${current}"\n`;
    if (!contents.includes(line)) throw new Error(`Version mismatch in ${path}`);
    files.set(path, contents.replace(line, `\nversion = "${version}"\n`));
  }
  for (const path of [config, "apps/desktop/package.json"]) {
    const contents = readFileSync(path, "utf8");
    const line = `\n  "version": "${current}",`;
    if (!contents.includes(line)) throw new Error(`Version mismatch in ${path}`);
    files.set(path, contents.replace(line, `\n  "version": "${version}",`));
  }
  for (const path of ["Cargo.lock", "apps/desktop/src-tauri/Cargo.lock"]) {
    const contents = readFileSync(path, "utf8").split("[[package]]");
    const updated = contents.map((entry) => {
      if (!/^\s*name = "(?:veil-[^"]+|omniscreenveil-desktop)"\n/.test(entry) || /^source = /m.test(entry)) {
        return entry;
      }
      const line = `\nversion = "${current}"\n`;
      if (!entry.includes(line)) throw new Error(`Version mismatch in ${path}`);
      return entry.replace(line, `\nversion = "${version}"\n`);
    });
    files.set(path, updated.join("[[package]]"));
  }
  for (const [path, contents] of files) writeFileSync(path, contents);
}

const [command, version, ...tags] = process.argv.slice(2);
const current = parse(version ?? "");
if (command === "next") {
  console.log(nextVersion(current, tags));
} else if (command === "stamp") {
  stampVersion(current.version);
} else if (command === "can-promote") {
  const sameChannel = versions(tags).filter((tag) => Boolean(tag.prerelease.length) === Boolean(current.prerelease.length));
  console.log(sameChannel.every((tag) => compare(current, tag) >= 0));
} else {
  throw new Error(`Unknown release version command: ${command}`);
}
