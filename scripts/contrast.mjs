#!/usr/bin/env node
// Computes the WCAG contrast ratio of every colour pair the product renders.
//
// The frontend suite cannot do this: vitest-axe runs in jsdom, which skips
// the colour-contrast rule entirely, so a green suite says nothing about
// whether a screen is readable. This is the "compute the ratios; jsdom does
// not" step of docs/design/ux-ui-change-playbook.md, made repeatable.
//
// Three sections, because WCAG asks different things of each:
//
//   text        4.5:1, WCAG 1.4.3.
//   controls    1.4.11 asks that the information needed to identify a
//               control reach 3:1 — not that its border does. A control
//               with a distinguishable fill passes on the fill; one drawn
//               as bare text plus a hairline has only that hairline, and
//               the hairline has to carry it. Each control is measured
//               both ways and passes on the better of the two.
//   decorative  card hairlines, dividers, grouping edges. Losing one costs
//               no information, so WCAG sets no threshold. Measured and
//               printed so a change to them stays visible.
//
// Colour specs: a token name, or a color-mix, written as it is in the CSS:
//   mix:--fg:30          color-mix(in srgb, var(--fg) 30%, transparent)
//   mix:--surface:72:--bg  color-mix(in srgb, var(--surface) 72%, var(--bg))
// A mix with transparent is composited over whatever sits behind it.

import { readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const stylesheet = join(repoRoot, "ui", "src", "styles.css");

const source = readFileSync(stylesheet, "utf8");
const rootStart = source.indexOf(":root");
const rootBlock = source.slice(rootStart, source.indexOf("\n}", rootStart));

const tokens = new Map();
for (const [, name, value] of rootBlock.matchAll(/(--[a-z0-9-]+):\s*(#[0-9a-fA-F]{3,8})\s*;/g)) {
  tokens.set(name, value);
}

const TEXT = 4.5;
const BOUNDARY = 3;

const text = [
  ["--fg", "--bg", "body text on the page"],
  ["--fg", "--surface", "body text on a card"],
  ["--fg", "--surface2", "body text on a raised row"],
  ["--fg", "--surface3", "body text on a selected row"],
  ["--fg2", "--bg", "secondary text on the page"],
  ["--fg2", "--surface", "secondary text on a card"],
  ["--fg2", "--surface2", "secondary text on a raised row"],
  ["--fg3", "--bg", "meta text on the page"],
  ["--fg3", "--surface", "meta text on a card"],
  ["--accent", "--bg", "accent text on the page"],
  ["--accent", "--surface", "accent text on a card"],
  ["--info", "--surface", "link text on a card"],
  ["--crit", "--surface", "critical text on a card"],
  ["--warn", "--surface", "warning text on a card"],
  ["--ok", "--surface", "healthy text on a card"],
  ["--ai", "--surface", "AI text on a card"],
  ["--crit", "--critbg", "critical text in its own pill"],
  ["--warn", "--warnbg", "warning text in its own pill"],
  ["--ok", "--okbg", "healthy text in its own pill"],
  ["--info", "--infobg", "info text in its own pill"],
  ["--ai", "--aibg", "AI text in its own panel"],
  ["--navfg", "--nav", "sidebar label"],
  ["--navsub", "--nav", "sidebar sub-label"]
];

// [selector, edge, fill (null when transparent), what it sits on]
const controls = [
  [".connector-actions button", "--accent", null, "--surface"],
  [".command-surface input", "--accent", null, "--surface"],
  [".logs-panel__query input", "--accent", null, "--surface"],
  [".notification-center button", "mix:--fg:30", null, "--surface"],
  [".ai-button", "mix:--fg:30", null, "--surface"],
  [".ai-button--primary", "--accent", "--accent", "--surface"],
  [".ai-fallback-order__actions button", "mix:--fg:30", null, "--surface"],
  [".ai-provider-form select", "mix:--fg:30", "mix:--bg:80:--surface", "--surface"],
  [".operations-widget-settings__main select", "mix:--fg:30", "--bg", "--surface"],
  [".topology-filters__field select", "mix:--fg:30", "--surface", "--bg"],
  [".topology-graph__node-select", "mix:--fg:25", "mix:--surface:72:--bg", "--bg"],
  [".operations-critical-number__button", "mix:--fg:18", "mix:--bg:54:--surface", "--surface"],
  [".incident-queue__row", "--border", "--surface2", "--surface"]
];

const decorative = [
  ["mix:--fg:20", "--bg", "card edge against the page"],
  ["mix:--fg:20", "--surface2", "card edge against a raised row"],
  ["--border", "--bg", "grouping edge on the page"],
  ["--border", "--surface", "divider inside a card"],
  ["--border", "--surface2", "row edge on a raised surface"],
  ["--border2", "--surface", "defined but unused today"]
];

const channel = (value) => {
  const c = value / 255;
  return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
};

const rgb = (hex) => {
  const full =
    hex.length === 4
      ? hex
          .slice(1)
          .split("")
          .map((c) => c + c)
          .join("")
      : hex.slice(1, 7);
  return [0, 2, 4].map((i) => parseInt(full.slice(i, i + 2), 16));
};

const luminance = ([r, g, b]) => 0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(b);

const ratio = (a, b) => {
  const [hi, lo] = [luminance(a), luminance(b)].sort((x, y) => y - x);
  return (hi + 0.05) / (lo + 0.05);
};

const blend = (colour, alpha, behind) =>
  colour.map((c, i) => Math.round(c * alpha + behind[i] * (1 - alpha)));

const resolveColour = (spec, behind) => {
  if (spec === null) return null;
  if (!spec.startsWith("mix:")) {
    const value = tokens.get(spec);
    return value ? rgb(value) : null;
  }
  const [, token, percent, second] = spec.split(":");
  const value = tokens.get(token);
  if (!value) return null;
  const alpha = Number(percent) / 100;
  // color-mix with transparent keeps the colour and scales its alpha, so
  // what lands on screen is that colour composited over what is behind.
  const over = second ? resolveColour(second, behind) : behind;
  return over ? blend(rgb(value), alpha, over) : null;
};

let failures = 0;
let missing = 0;
const say = (cells, widths) =>
  console.log(
    `  ${cells.map((cell, i) => (i < 2 ? cell.padEnd(widths[i]) : cell.padStart(widths[i]))).join("  ")}`
  );

const render = (title, rows) => {
  console.log(`\n${title}\n`);
  const widths = [0, 1, 2, 3, 4].map((i) => Math.max(...rows.map((row) => row[i].length)));
  for (const row of rows) say(row, widths);
};

const textRows = text.map(([fg, bg, label]) => {
  const background = resolveColour(bg, null);
  const foreground = resolveColour(fg, background);
  if (!foreground || !background) {
    missing += 1;
    return [`${fg} on ${bg}`, label, "—", `${TEXT}:1`, "MISSING TOKEN"];
  }
  const measured = ratio(foreground, background);
  if (measured < TEXT) failures += 1;
  return [
    `${fg} on ${bg}`,
    label,
    `${measured.toFixed(2)}:1`,
    `${TEXT}:1`,
    measured >= TEXT ? "pass" : "FAIL"
  ];
});

const controlRows = controls.map(([selector, edgeSpec, fillSpec, onSpec]) => {
  const on = resolveColour(onSpec, null);
  const edge = resolveColour(edgeSpec, on);
  const fill = resolveColour(fillSpec, on);
  if (!on || !edge) {
    missing += 1;
    return [selector, `on ${onSpec}`, "—", `${BOUNDARY}:1`, "MISSING TOKEN"];
  }
  const edgeRatio = ratio(edge, on);
  const fillRatio = fill ? ratio(fill, on) : 0;
  const best = Math.max(edgeRatio, fillRatio);
  if (best < BOUNDARY) failures += 1;
  const how = fillRatio > edgeRatio ? "fill" : "edge";
  return [
    selector,
    fill ? `edge ${edgeRatio.toFixed(2)} · fill ${fillRatio.toFixed(2)}` : "edge only",
    `${best.toFixed(2)}:1`,
    `${BOUNDARY}:1`,
    best >= BOUNDARY ? `pass on ${how}` : "FAIL"
  ];
});

const decorativeRows = decorative.map(([fg, bg, label]) => {
  const background = resolveColour(bg, null);
  const foreground = resolveColour(fg, background);
  if (!foreground || !background) return [`${fg} on ${bg}`, label, "—", "—", "MISSING TOKEN"];
  return [
    `${fg} on ${bg}`,
    label,
    `${ratio(foreground, background).toFixed(2)}:1`,
    "—",
    "not gated"
  ];
});

render("Text — WCAG 1.4.3, 4.5:1", textRows);
render("Controls — WCAG 1.4.11, 3:1 on the edge or the fill", controlRows);
render("Decorative edges — no threshold, measured only", decorativeRows);

console.log(
  `\n${textRows.length + controlRows.length} required, ${failures} below threshold` +
    `${missing > 0 ? `, ${missing} missing` : ""}; ${decorativeRows.length} decorative, not gated.`
);

process.exit(failures + missing > 0 ? 1 : 0);
