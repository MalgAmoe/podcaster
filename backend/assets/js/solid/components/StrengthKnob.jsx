import { For } from "solid-js";
import { useProcess } from "../context/ProcessContext";
import { useI18n } from "../context/I18nContext";

export function StrengthKnob() {
  const { currentStrength, setStrength } = useProcess();
  const { t } = useI18n();

  const STRENGTH_KEYS = ["subtle", "balanced", "intense"];

  return (
    <div class="form-control text-center">
      <p class="font-medium mb-3">{t("strength")}: {t(STRENGTH_KEYS[currentStrength() - 1])}</p>
      <div class="flex justify-center gap-2">
        <For each={[1, 2, 3]}>
          {(level) => (
            <button
              type="button"
              onClick={() => setStrength(level)}
              class={`w-8 h-8 rounded-full transition-all flex items-center justify-center text-sm font-medium ${
                level <= currentStrength()
                  ? "bg-primary text-primary-content"
                  : "bg-base-300 text-base-content/50"
              }`}
            >
              {level}
            </button>
          )}
        </For>
      </div>
    </div>
  );
}
