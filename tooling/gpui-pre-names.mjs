#!/usr/bin/env node
// Renames every crate to the name and version it is published under as gpui-pre,
// so `[patch.crates-io]` can put this fork in place of the gpui-pre crates that
// GPUI Kit pins. Run from the repository root after rebasing onto upstream:
//   node tooling/gpui-pre-names.mjs 0.3.10

import { readFileSync, writeFileSync } from "node:fs";

const version = process.argv[2];
if (!version) throw new Error("usage: gpui-pre-names.mjs <gpui-pre version>");

// Zed's `util` is unpublished and would take `gpui_util`'s name.
const RENAMED = { gpui: "gpui-pre", util: "gpui-pre-zed-util" };
const published = (name) =>
  RENAMED[name] ?? `gpui-pre-${name.replace(/^gpui_/, "").replaceAll("_", "-")}`;

const workspace = readFileSync("Cargo.toml", "utf8");
const members = [...workspace.matchAll(/^\s*"((?:crates|tooling)\/[^"]+)",$/gm)].map((m) => m[1]);
const names = new Set();

for (const member of members) {
  const manifest = `${member}/Cargo.toml`;
  let text = readFileSync(manifest, "utf8");
  const name = text.match(/^\[package\]\nname = "([^"]+)"/m)[1];
  names.add(name);
  text = text
    .replace(/^(\[package\]\nname = )"[^"]+"/m, `$1"${published(name)}"`)
    .replace(/^version = "[^"]+"/m, `version = "${version}"`);
  if (!new RegExp(`^\\[lib\\]\\nname = "${name}"`, "m").test(text)) {
    text = /^\[lib\]$/m.test(text)
      ? text.replace(/^\[lib\]$/m, `[lib]\nname = "${name}"`)
      : `${text.trimEnd()}\n\n[lib]\nname = "${name}"\n`;
  }
  writeFileSync(manifest, text);
}

writeFileSync(
  "Cargo.toml",
  workspace.replace(/^([a-z_]+) = \{ path = ("[^"]+")(?:, version = "[^"]+")?/gm, (line, name, path) =>
    names.has(name) ? `${name} = { package = "${published(name)}", path = ${path}` : line,
  ),
);
