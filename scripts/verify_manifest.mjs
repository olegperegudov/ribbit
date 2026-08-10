#!/usr/bin/env node
// Verify a Tauri updater manifest (latest.json / beta.json) before it is
// trusted by any channel. Shared by build.yml (right after the release is
// published) and release-control.yml (before a promote/rollback touches the
// stable channel) — one script, one definition of "safe to ship".
//
// Usage: node scripts/verify_manifest.mjs <manifest.json> <tag>
// Exit 0 when the manifest is good, 1 with ::error:: lines otherwise.

import { readFileSync } from "node:fs";

const [, , manifestPath, tag] = process.argv;
if (!manifestPath || !tag) {
  console.error("::error::usage: verify_manifest.mjs <manifest.json> <tag>");
  process.exit(1);
}

const REQUIRED_PLATFORMS = ["windows-x86_64", "darwin-aarch64", "darwin-x86_64"];

let manifest;
try {
  manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
} catch (e) {
  console.error(`::error::cannot read manifest ${manifestPath}: ${e.message}`);
  process.exit(1);
}

const platforms = manifest.platforms ?? {};
const have = Object.keys(platforms);
const missing = REQUIRED_PLATFORMS.filter((p) => !(p in platforms));
if (missing.length) {
  console.error(
    `::error::manifest missing platform keys: ${missing.join(", ")} (present: ${have.join(", ")})`,
  );
  process.exit(1);
}

if (`v${manifest.version}` !== tag) {
  console.error(`::error::manifest version ${JSON.stringify(manifest.version)} != tag ${JSON.stringify(tag)}`);
  process.exit(1);
}

// The macOS bundle must be per-arch, never universal: a lipo'd ad-hoc binary
// fails to anchor TCC grants, so the microphone silently stops working after
// an update. Catch a regression to universal here.
for (const key of ["darwin-aarch64", "darwin-x86_64"]) {
  const url = platforms[key]?.url ?? "";
  if (url.toLowerCase().includes("universal")) {
    console.error(
      `::error::${key} points at a universal bundle (${url.split("/").pop()}); macOS must ship per-arch or the mic breaks`,
    );
    process.exit(1);
  }
}

console.log(`manifest OK: version ${manifest.version}, platforms ${have.sort().join(", ")}`);
