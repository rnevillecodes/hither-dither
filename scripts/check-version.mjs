// Checks that the app version agrees across package.json, src-tauri/Cargo.toml and
// src-tauri/tauri.conf.json, and (given a tag such as v0.2.0) that it matches the tag.
// Usage: node scripts/check-version.mjs [tag]
import { readFileSync } from "node:fs";

const root = new URL("..", import.meta.url);
const read = (path) => readFileSync(new URL(path, root), "utf8");

function cargoVersion(toml) {
  const pkg = toml.match(/^\[package\]\s*$([\s\S]*?)(?=^\[|(?![\s\S]))/m)?.[1] ?? "";
  return pkg.match(/^version\s*=\s*"([^"]+)"/m)?.[1] ?? null;
}

const versions = {
  "package.json": JSON.parse(read("package.json")).version ?? null,
  "src-tauri/Cargo.toml": cargoVersion(read("src-tauri/Cargo.toml")),
  "src-tauri/tauri.conf.json": JSON.parse(read("src-tauri/tauri.conf.json")).version ?? null,
};

const tag = process.argv[2];
// WiX (the Windows .msi bundler) accepts only numeric pre-release identifiers.
if (tag && !/^(refs\/tags\/)?v\d+\.\d+\.\d+(-\d+)?$/.test(tag)) {
  console.error(
    `Tag ${tag} is not vX.Y.Z or vX.Y.Z-N. Pre-release suffixes must be numeric (for example v0.2.0-1), ` +
      "because the Windows .msi installer rejects others.",
  );
  process.exit(1);
}
const expected = tag ? tag.replace(/^refs\/tags\//, "").replace(/^v/, "") : versions["package.json"];
const mismatched = Object.entries(versions).filter(([, v]) => v !== expected);

if (mismatched.length > 0) {
  console.error(tag ? `Tag ${tag} expects version ${expected}, but:` : "The app version differs between files:");
  for (const [file, v] of Object.entries(versions)) console.error(`  ${file}: ${v ?? "(no version found)"}`);
  process.exit(1);
}
console.log(`Version ${expected} matches in ${Object.keys(versions).join(", ")}.`);
