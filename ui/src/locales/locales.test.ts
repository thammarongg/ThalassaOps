// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import en from "./en";
import th from "./th";

const leaves = (value: unknown, prefix = ""): [string, unknown][] =>
  Object.entries(value as Record<string, unknown>).flatMap(([key, inner]) =>
    typeof inner === "object" && inner !== null
      ? leaves(inner, `${prefix}${key}.`)
      : [[`${prefix}${key}`, inner] as [string, unknown]]
  );

const keyPaths = (value: unknown): string[] => leaves(value).map(([key]) => key);

const missingFrom = (source: string[], target: string[]) => {
  const known = new Set(target);
  return source.filter((key) => !known.has(key));
};

describe("locale parity", () => {
  it("defines exactly the same key paths in en and th", () => {
    const enKeys = keyPaths(en).sort();
    const thKeys = keyPaths(th).sort();
    expect(missingFrom(enKeys, thKeys)).toEqual([]);
    expect(missingFrom(thKeys, enKeys)).toEqual([]);
  });

  /*
   * A few leaves are deliberately empty: `units.count` renders a bare number
   * where the other units append a suffix. Emptiness is therefore allowed, but
   * only in both catalogs at once, so a key stubbed on one side is still drift.
   */
  it("carries a string at every key path, blank in both catalogs or neither", () => {
    const thai = new Map(leaves(th));
    const wrong = leaves(en)
      .filter(([key, value]) => {
        const other = thai.get(key);
        if (typeof value !== "string" || typeof other !== "string") return true;
        return (value.trim() === "") !== (other.trim() === "");
      })
      .map(([key]) => key);
    expect(wrong).toEqual([]);
  });
});
