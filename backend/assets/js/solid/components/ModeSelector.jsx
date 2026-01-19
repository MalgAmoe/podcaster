import { For } from "solid-js";
import { useProcess } from "../context/ProcessContext";

const MODES = [
  { id: "repair", label: "Repair" },
  { id: "natural", label: "Natural" },
  { id: "studio", label: "Studio" }
];

export function ModeSelector() {
  const { currentMode, setMode } = useProcess();

  return (
    <div class="form-control text-center">
      <p class="font-medium mb-3">Processing mode</p>
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
              {mode.label}
            </button>
          )}
        </For>
      </div>
    </div>
  );
}
