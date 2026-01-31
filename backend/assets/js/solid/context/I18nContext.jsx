import { createContext, useContext } from "solid-js";
import * as i18n from "@solid-primitives/i18n";
import en from "../locales/en";
import es from "../locales/es";

const dictionaries = { en, es };

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

const I18nContext = createContext();

export function I18nProvider(props) {
  // Get locale from HTML lang attribute (set by Phoenix)
  const locale = document.documentElement.lang || "en";
  const validLocale = locale in dictionaries ? locale : "en";

  // Create translator
  const dict = i18n.flatten(dictionaries[validLocale]);
  const t = i18n.translator(() => dict);

  // Helper for template strings with interpolation
  // Usage: tt("key", { name: "value" })
  const tt = (key, params = {}) => {
    let text = t(key);
    if (!text) return key;

    // Replace {param} placeholders
    Object.entries(params).forEach(([param, value]) => {
      text = text.replace(new RegExp(`\\{${param}\\}`, "g"), value);
    });
    return text;
  };

  const value = {
    t,      // Simple translation: t("key") or t("stages.decoding")
    tt,     // With interpolation: tt("key", { param: "value" })
    locale: validLocale
  };

  return (
    <I18nContext.Provider value={value}>
      {props.children}
    </I18nContext.Provider>
  );
}

export function useI18n() {
  const ctx = useContext(I18nContext);
  if (!ctx) {
    // Fallback if used outside provider (shouldn't happen but safe)
    return {
      t: (key) => key,
      tt: (key) => key,
      locale: "en"
    };
  }
  return ctx;
}
