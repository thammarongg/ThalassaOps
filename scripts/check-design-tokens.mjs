#!/usr/bin/env node
// Token discipline guard.
//
// Visual values flow through the custom properties in the `:root` block of
// ui/src/styles.css and nowhere else. This fails the build when a colour
// literal appears anywhere but there: in another stylesheet, further down
// styles.css, or inline in a component.
//
// Why it exists: the palette swap on 2026-09-10 reskinned every screen by
// editing one block, and that only works while the block is the only place
// colour lives. See docs/design/ux-ui-change-playbook.md.
//
// No dependencies on purpose — CI gains a step, not a toolchain.

import { readdirSync, readFileSync, statSync } from "node:fs";
import { dirname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const scanRoot = join(repoRoot, "ui", "src");

const namedColours = [
  "aqua",
  "beige",
  "black",
  "blue",
  "brown",
  "cyan",
  "fuchsia",
  "gold",
  "gray",
  "green",
  "grey",
  "indigo",
  "ivory",
  "lime",
  "magenta",
  "maroon",
  "navy",
  "olive",
  "orange",
  "pink",
  "purple",
  "red",
  "salmon",
  "silver",
  "teal",
  "violet",
  "white",
  "yellow"
];

const hex = /#[0-9a-fA-F]{3,8}\b/;
const colourFunction = /\b(?:rgba?|hsla?|hwb|lab|lch|oklab|oklch)\(/;
const named = new RegExp(`(?:^|[\\s:,(])(?:${namedColours.join("|")})(?=[\\s;,)]|$)`, "i");

const files = [];
const walk = (dir) => {
  for (const entry of readdirSync(dir)) {
    const full = join(dir, entry);
    if (statSync(full).isDirectory()) walk(full);
    else if (/\.(css|ts|tsx)$/.test(entry)) files.push(full);
  }
};
walk(scanRoot);
files.sort();

const findings = [];

for (const file of files) {
  const shown = relative(repoRoot, file);
  const isStylesheet = file.endsWith(".css");
  const isTokenFile = shown === "ui/src/styles.css";
  let inComment = false;
  let inRoot = false;

  readFileSync(file, "utf8")
    .split("\n")
    .forEach((raw, index) => {
      // Strip comments so a hex in prose never trips the guard.
      let line = "";
      let rest = raw;
      while (rest.length > 0) {
        if (inComment) {
          const end = rest.indexOf("*/");
          if (end === -1) return;
          rest = rest.slice(end + 2);
          inComment = false;
          continue;
        }
        const start = rest.indexOf("/*");
        const lineComment = isStylesheet ? -1 : rest.indexOf("//");
        if (lineComment !== -1 && (start === -1 || lineComment < start)) {
          line += rest.slice(0, lineComment);
          return check(line, index);
        }
        if (start === -1) {
          line += rest;
          break;
        }
        line += rest.slice(0, start);
        rest = rest.slice(start + 2);
        inComment = true;
      }
      check(line, index);
    });

  function check(line, index) {
    if (isTokenFile) {
      if (/^:root\s*\{/.test(line)) inRoot = true;
      else if (inRoot && /^\}/.test(line)) inRoot = false;
      if (inRoot) return;
    }
    const literal = hex.test(line) || colourFunction.test(line) || named.test(line);
    if (!literal) return;
    findings.push({ file: shown, line: index + 1, text: line.trim() });
  }
}

if (findings.length === 0) {
  console.log(`token guard: ${files.length} files, no colour literal outside :root`);
  process.exit(0);
}

console.error("token guard: colour literals must live in the :root block of ui/src/styles.css\n");
for (const finding of findings) {
  console.error(`  ${finding.file}:${finding.line}  ${finding.text}`);
}
console.error(`\n${findings.length} literal(s). Add a token, then use it.`);
process.exit(1);
