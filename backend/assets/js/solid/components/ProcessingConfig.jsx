import { OptionsRow } from "./ModeSelector";
import { StrengthKnob } from "./StrengthKnob";

export function ProcessingConfig() {
  return (
    <div class="space-y-4">
      <StrengthKnob />
      <OptionsRow />
    </div>
  );
}
