import { useProcess } from "../context/ProcessContext";
import { useI18n } from "../context/I18nContext";

function InfoTip(props) {
  return (
    <div class="tooltip tooltip-top" data-tip={props.tip}>
      <svg xmlns="http://www.w3.org/2000/svg" class="h-3.5 w-3.5 opacity-40" fill="none" viewBox="0 0 24 24" stroke="currentColor">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
      </svg>
    </div>
  );
}

export function OptionsRow() {
  const { currentAiClean, setAiClean, currentMono, setMono } = useProcess();
  const { t } = useI18n();

  return (
    <div class="flex justify-center mt-6">
      <div class="flex flex-col gap-2 px-6">
        <label class="flex items-center gap-1.5 cursor-pointer">
          <input
            type="checkbox"
            class="checkbox checkbox-xs checkbox-primary"
            checked={currentAiClean()}
            onChange={(e) => setAiClean(e.target.checked)}
          />
          <span class="text-xs opacity-70">{t("aiClean")}</span>
          <InfoTip tip={t("takesLonger")} />
        </label>
        <label class="flex items-center gap-1.5 cursor-pointer">
          <input
            type="checkbox"
            class="checkbox checkbox-xs checkbox-primary"
            checked={currentMono()}
            onChange={(e) => setMono(e.target.checked)}
          />
          <span class="text-xs opacity-70">{t("centerAudio")}</span>
          <InfoTip tip={t("centerAudioTooltip")} />
        </label>
      </div>
    </div>
  );
}
