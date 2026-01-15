import React from "react";
import { useProcess } from "../context/ProcessContext";

export function PresetSelector() {
  const { state, selectPreset } = useProcess();

  return (
    <div className="form-control text-center">
      <p className="font-medium mb-3">How hard should the cow chew?</p>
      <div className="flex flex-wrap gap-2 justify-center mt-2">
        {state.presets.map((preset) => (
          <button
            key={preset}
            type="button"
            onClick={() => selectPreset(preset)}
            className={`btn btn-sm ${
              preset === state.selectedPreset ? "btn-primary" : "btn-outline"
            }`}
          >
            {preset.charAt(0).toUpperCase() + preset.slice(1)}
          </button>
        ))}
      </div>
    </div>
  );
}
