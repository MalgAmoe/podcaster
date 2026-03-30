import { createContext, useContext } from "solid-js";
import * as i18n from "@solid-primitives/i18n";
import en from "../locales/en";

const I18nContext = createContext();

export function I18nProvider(props) {
  const dict = i18n.flatten(en);
  const t = i18n.translator(() => dict);

  const tt = (key, params = {}) => {
    let text = t(key);
    if (!text) return key;

    Object.entries(params).forEach(([param, value]) => {
      text = text.replace(new RegExp(`\\{${param}\\}`, "g"), value);
    });
    return text;
  };

  const value = { t, tt, locale: "en" };

  return (
    <I18nContext.Provider value={value}>
      {props.children}
    </I18nContext.Provider>
  );
}

export function useI18n() {
  const ctx = useContext(I18nContext);
  if (!ctx) {
    return { t: (key) => key, tt: (key) => key, locale: "en" };
  }
  return ctx;
}
