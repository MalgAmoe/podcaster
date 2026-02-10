import { ModeSelector } from "./ModeSelector";
import { StrengthKnob } from "./StrengthKnob";

export function ProcessingConfig() {
  return (
    <div class="space-y-6">
      <StrengthKnob />
      <ModeSelector />
    </div>
  );
}
