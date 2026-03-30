// Standalone translation helper for use outside component tree

import en from "../locales/en";

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

const flatDict = flattenDict(en);

export function t(key) {
  return flatDict[key] || key;
}

export function tt(key, params = {}) {
  let text = t(key);
  if (!text || text === key) return key;

  Object.entries(params).forEach(([param, value]) => {
    text = text.replace(new RegExp(`\\{${param}\\}`, "g"), value);
  });
  return text;
}
