/**
 * i18n helper for GUI tests.
 *
 * Resolves UI strings through the same `vue-i18n` library the production
 * GUI uses, fed from the same locale files (`src/locales/{en,cn}.json`),
 * so tests assert against a single source of truth: change a string in
 * the locale file and the tests follow automatically.
 *
 * Exports:
 *   - tGui(path, vars?, locale?) — translate a dotted i18n path via
 *     `vue-i18n`'s `t()`. Throws if the key is missing (i.e. `t()`
 *     returned the path verbatim).
 *   - matchable(path, locale?) — returns the portion of the *raw*
 *     locale value that precedes the first `{var}` placeholder or
 *     `<tag>`, trimmed. Used for substring assertions when Selenium's
 *     `getText()` strips inline HTML or when the dynamic value is
 *     unknown at assert time.
 *   - xpathText(path, locale?) — returns the longest apostrophe-free
 *     segment of the raw locale value. `GUITestRunner` builds XPath 1.0
 *     `contains(text(), '...')` expressions, whose single-quoted literal
 *     cannot embed a real `'`.
 *
 * Notes:
 *   - Lookup + interpolation are delegated to `vue-i18n`. We do not
 *     reimplement them.
 *   - Locale JSON is loaded with `fs.readFileSync` + `JSON.parse` (not
 *     `import ... with { type: "json" }`) so the helper works on every
 *     Node 20.x release CI pins, without depending on JSON-import
 *     attributes (Node 20.10+).
 *   - We retain a reference to the parsed objects (`RAW`) for
 *     `matchable` / `xpathText`, which need the un-interpolated template
 *     string. `vue-i18n` may compile messages internally, so we don't
 *     read them back through `getLocaleMessage`.
 */

import fs from "fs";
import path from "path";
import { fileURLToPath } from "url";
import { createI18n } from "vue-i18n";

const HERE = path.dirname(fileURLToPath(import.meta.url));
const LOCALES_DIR = path.resolve(HERE, "..", "..", "src", "locales");

function loadLocale(name) {
  const file = path.join(LOCALES_DIR, `${name}.json`);
  return JSON.parse(fs.readFileSync(file, "utf8"));
}

const RAW = {
  en: loadLocale("en"),
  cn: loadLocale("cn"),
};

const i18n = createI18n({
  legacy: false,
  locale: "en",
  fallbackLocale: false,
  missingWarn: false,
  fallbackWarn: false,
  messages: { en: RAW.en, cn: RAW.cn },
});

/**
 * Resolve a vue-i18n dotted path to a rendered string.
 *
 * @param {string} keyPath - e.g. "welcome.title"
 * @param {Object<string, string|number>} [vars] - values for {placeholders}
 * @param {"en"|"cn"} [locale="en"]
 * @returns {string}
 */
export function tGui(keyPath, vars = {}, locale = "en") {
  const out = i18n.global.t(keyPath, vars, { locale });
  if (out === keyPath) {
    throw new Error(`i18n key missing: "${keyPath}" (locale=${locale})`);
  }
  return out;
}

function rawMessage(keyPath, locale) {
  if (!(locale in RAW)) {
    throw new Error(`i18n locale not loaded: "${locale}"`);
  }
  const node = keyPath
    .split(".")
    .reduce(
      (acc, k) =>
        acc != null && typeof acc === "object" && k in acc ? acc[k] : undefined,
      RAW[locale]
    );
  if (typeof node !== "string") {
    throw new Error(
      `i18n path "${keyPath}" did not resolve to a string (got ${typeof node})`
    );
  }
  return node;
}

/**
 * Return the portion of a translation that precedes the first `{var}`
 * placeholder or HTML tag, with trailing whitespace trimmed. Useful for
 * substring assertions (findByText / .include(...)) when the dynamic
 * value is unknown or when the rendered DOM strips inline tags from the
 * locale value (e.g. `<strong>{name}</strong>`).
 *
 * @param {string} keyPath
 * @param {"en"|"cn"} [locale="en"]
 * @returns {string}
 */
export function matchable(keyPath, locale = "en") {
  const raw = rawMessage(keyPath, locale);
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
  const raw = rawMessage(keyPath, locale);
  const segments = raw
    .split("'")
    .map((s) => s.trim())
    .filter(Boolean);
  if (segments.length === 0) {
    throw new Error(`i18n path "${keyPath}" has no XPath-safe text`);
  }
  return segments.reduce((a, b) => (a.length >= b.length ? a : b));
}
