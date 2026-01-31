import { For, Show } from "solid-js";
import { useProcess } from "../context/ProcessContext";
import { useI18n } from "../context/I18nContext";

export function ModeSelector() {
  const { store, currentMode, setMode, currentAiClean, setAiClean } = useProcess();
  const { t } = useI18n();

  const MODES = [
    { id: "natural", labelKey: "natural" },
    { id: "studio", labelKey: "studio" }
  ];

  return (
    <div class="form-control text-center">
      <p class="font-medium mb-3">{t("processingMode")}</p>
      <div class="flex justify-center gap-2">
        <For each={MODES}>
          {(mode) => (
            <button
              type="button"
              onClick={() => setMode(mode.id)}
              class={`px-3 py-1.5 rounded-full transition-all text-sm ${
                mode.id === currentMode()
                  ? "bg-primary text-primary-content font-semibold"
                  : "bg-base-300 text-base-content/50"
              }`}
            >
              {t(mode.labelKey)}
            </button>
          )}
        </For>
      </div>

      {/* AI Clean toggle - only shown for voice category */}
      <Show when={store.processingConfig.category === "voice"}>
        <div class="mt-4 flex items-center justify-center gap-2">
          <label class="flex items-center gap-2 cursor-pointer">
            <input
              type="checkbox"
              class="checkbox checkbox-sm checkbox-primary"
              checked={currentAiClean()}
              onChange={(e) => setAiClean(e.target.checked)}
            />
            <span class="text-sm">{t("aiClean")}</span>
          </label>
          <div class="tooltip tooltip-right" data-tip={t("takesLonger")}>
            <svg xmlns="http://www.w3.org/2000/svg" class="h-4 w-4 opacity-50" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
            </svg>
          </div>
        </div>
      </Show>
    </div>
  );
}
