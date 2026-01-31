// Standalone translation helper for use outside component tree
// (e.g., in context providers or utility functions)

import en from "../locales/en";
import es from "../locales/es";

const dictionaries = { en, es };

// Get the current locale from HTML lang attribute
function getLocale() {
  const locale = document.documentElement.lang || "en";
  return locale in dictionaries ? locale : "en";
}

// Flatten nested objects for translation lookup
function flattenDict(dict, prefix = "") {
  return Object.entries(dict).reduce((acc, [key, value]) => {
    const fullKey = prefix ? `${prefix}.${key}` : key;
    if (typeof value === "object" && value !== null) {
      Object.assign(acc, flattenDict(value, fullKey));
    } else {
      acc[fullKey] = value;
    }
    return acc;
  }, {});
}

const flatDictionaries = {
  en: flattenDict(en),
  es: flattenDict(es)
};

/**
 * Get a translation by key
 * @param {string} key - Translation key (e.g., "uploadFirst" or "stages.decoding")
 * @returns {string} - Translated string or key if not found
 */
export function t(key) {
  const locale = getLocale();
  const dict = flatDictionaries[locale];
  return dict[key] || key;
}

/**
 * Get a translation with parameter interpolation
 * @param {string} key - Translation key
 * @param {object} params - Parameters to interpolate (e.g., { count: 5 })
 * @returns {string} - Translated string with parameters replaced
 */
export function tt(key, params = {}) {
  let text = t(key);
  if (!text || text === key) return key;

  // Replace {param} placeholders
  Object.entries(params).forEach(([param, value]) => {
    text = text.replace(new RegExp(`\\{${param}\\}`, "g"), value);
  });
  return text;
}
