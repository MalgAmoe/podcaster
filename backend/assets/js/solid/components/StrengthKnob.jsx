import { For } from "solid-js";
import { useProcess } from "../context/ProcessContext";

const STRENGTH_LABELS = ["Gentle", "Light", "Moderate", "Strong", "Aggressive"];

export function StrengthKnob() {
  const { currentStrength, setStrength } = useProcess();

  return (
    <div class="form-control text-center">
      <p class="font-medium mb-3">Strength: {STRENGTH_LABELS[currentStrength() - 1]}</p>
      <div class="flex justify-center gap-2">
        <For each={[1, 2, 3, 4, 5]}>
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
