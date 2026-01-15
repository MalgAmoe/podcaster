import { For } from "solid-js";
import { useProcess } from "../context/ProcessContext";

export function PresetSelector() {
  const { store, selectPreset } = useProcess();

  return (
    <div class="form-control text-center">
      <p class="font-medium mb-3">How hard should the cow chew?</p>
      <div class="flex flex-wrap gap-2 justify-center mt-2">
        <For each={store.presets}>
          {(preset) => (
            <button
              type="button"
              onClick={() => selectPreset(preset)}
              class={`btn btn-sm ${preset === store.selectedPreset ? "btn-primary" : "btn-outline"}`}
            >
              {preset.charAt(0).toUpperCase() + preset.slice(1)}
            </button>
          )}
        </For>
      </div>
    </div>
  );
}
