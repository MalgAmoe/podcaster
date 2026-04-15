import { For } from "solid-js";
import { useProcess } from "../context/ProcessContext";
import { useI18n } from "../context/I18nContext";

export function StrengthKnob() {
  const { currentStrength, setStrength } = useProcess();
  const { t } = useI18n();

  const STRENGTH_KEYS = ["subtle", "balanced", "intense"];

  return (
    <div class="form-control text-center">
      <p class="font-medium mb-3">{t("strength")}: <span class="text-primary">{t(STRENGTH_KEYS[currentStrength() - 1])}</span></p>
      <div class="flex justify-center gap-3">
        <For each={[1, 2, 3]}>
          {(level) => (
            <button
              type="button"
              onClick={() => setStrength(level)}
              class={`w-10 h-10 rounded-full transition-all duration-200 flex items-center justify-center text-sm font-semibold ${
                level <= currentStrength()
                  ? "bg-primary text-primary-content shadow-[0_0_12px_rgba(147,51,234,0.4)]"
                  : "bg-base-300 text-base-content/40 hover:bg-base-content/10"
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
