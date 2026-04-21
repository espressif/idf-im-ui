/**
 * i18n helper for GUI tests.
 *
 * Reads the same vue-i18n JSON locale files the production GUI uses
 * (src/locales/{en,cn}.json) so tests assert against a single source of
 * truth: change a string in the locale file and the test follows
 * automatically.
 *
 * Exports:
 *   - tGui(path, vars?, locale?) — resolve a dotted i18n path to its
 *     rendered string, substituting {var} placeholders from `vars`.
 *     Throws if the path is missing, if the path does not resolve to a
 *     string, or if any {var} placeholder has no matching entry in
 *     `vars`.
 *   - matchable(path, locale?) — return the portion of the translated
 *     string that precedes the first {var} placeholder, trimmed. Useful
 *     for substring assertions (findByText / .include(...)) when the
 *     dynamic value is unknown or noisy.
 *
 * Uses fs.readFileSync + JSON.parse (instead of `import ... with
 * { type: "json" }`) so the helper works on every Node 20.x release our
 * CI currently pins to, without relying on the JSON import attributes
 * stabilization (Node 20.10+).
 */

import fs from "fs";
import path from "path";
import { fileURLToPath } from "url";

const HERE = path.dirname(fileURLToPath(import.meta.url));
const LOCALES_DIR = path.resolve(HERE, "..", "..", "src", "locales");

function loadLocale(name) {
  const file = path.join(LOCALES_DIR, `${name}.json`);
  const raw = fs.readFileSync(file, "utf8");
  return JSON.parse(raw);
}

const LOCALES = {
  en: loadLocale("en"),
  cn: loadLocale("cn"),
};

function walk(obj, dottedPath) {
  const parts = dottedPath.split(".");
  let node = obj;
  for (const key of parts) {
    if (node == null || typeof node !== "object" || !(key in node)) {
      throw new Error(`i18n key missing: "${dottedPath}"`);
    }
    node = node[key];
  }
  return node;
}

/**
 * Resolve a vue-i18n dotted path to a rendered string.
 *
 * @param {string} keyPath - e.g. "welcome.title"
 * @param {Object<string, string|number>} [vars] - values for {placeholders}
 * @param {"en"|"cn"} [locale="en"]
 * @returns {string}
 */
export function tGui(keyPath, vars = {}, locale = "en") {
  if (!(locale in LOCALES)) {
    throw new Error(`i18n locale not loaded: "${locale}"`);
  }
  const raw = walk(LOCALES[locale], keyPath);
  if (typeof raw !== "string") {
    throw new Error(
      `i18n path "${keyPath}" did not resolve to a string (got ${typeof raw})`
    );
  }
  return raw.replace(/\{(\w+)\}/g, (_, name) => {
    if (!(name in vars)) {
      throw new Error(
        `i18n missing var "${name}" for "${keyPath}" (locale=${locale})`
      );
    }
    return String(vars[name]);
  });
}

/**
 * Return the portion of a translation that precedes the first `{var}`
 * placeholder or HTML tag, with trailing whitespace trimmed. Useful
 * for substring assertions (findByText / .include(...)) when the
 * dynamic value is unknown or when the rendered DOM strips inline
 * tags from the locale value (e.g. `<strong>{name}</strong>`).
 *
 * @param {string} keyPath
 * @param {"en"|"cn"} [locale="en"]
 * @returns {string}
 */
export function matchable(keyPath, locale = "en") {
  if (!(locale in LOCALES)) {
    throw new Error(`i18n locale not loaded: "${locale}"`);
  }
  const raw = walk(LOCALES[locale], keyPath);
  if (typeof raw !== "string") {
    throw new Error(
      `i18n path "${keyPath}" did not resolve to a string (got ${typeof raw})`
    );
  }
  const brace = raw.indexOf("{");
  const tag = raw.indexOf("<");
  const candidates = [brace, tag].filter((i) => i !== -1);
  if (candidates.length === 0) return raw;
  return raw.slice(0, Math.min(...candidates)).trimEnd();
}

/**
 * Return an XPath-safe substring of a translation.
 *
 * GUITestRunner uses `contains(text(), '...')` XPath expressions, whose
 * single-quoted literal cannot embed a real apostrophe. Many English
 * copies contain apostrophes (e.g. "Don't show..."), so we return the
 * longest apostrophe-free segment of the translated value. The segment
 * still comes from the locale file, so the key remains the source of
 * truth.
 *
 * Throws if the segment is empty (nothing useful to match on).
 *
 * @param {string} keyPath
 * @param {"en"|"cn"} [locale="en"]
 * @returns {string}
 */
export function xpathText(keyPath, locale = "en") {
  if (!(locale in LOCALES)) {
    throw new Error(`i18n locale not loaded: "${locale}"`);
  }
  const raw = walk(LOCALES[locale], keyPath);
  if (typeof raw !== "string") {
    throw new Error(
      `i18n path "${keyPath}" did not resolve to a string (got ${typeof raw})`
    );
  }
  const segments = raw.split("'").map((s) => s.trim()).filter(Boolean);
  if (segments.length === 0) {
    throw new Error(`i18n path "${keyPath}" has no XPath-safe text`);
  }
  return segments.reduce((a, b) => (a.length >= b.length ? a : b));
}
