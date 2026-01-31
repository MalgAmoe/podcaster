import { useI18n } from "../context/I18nContext";

export function StepsIndicator(props) {
  const { t } = useI18n();

  const getStepClass = (step) => {
    const order = { upload: 1, process: 2, download: 3 };
    const currentOrder = order[props.currentStep] || 1;
    const stepOrder = order[step];
    return stepOrder <= currentOrder ? "step step-primary" : "step";
  };

  return (
    <ul class="steps steps-horizontal mt-8">
      <li class={getStepClass("upload")}>{t("feed")}</li>
      <li class={getStepClass("process")}>{t("munch")}</li>
      <li class={getStepClass("download")}>{t("enjoy")}</li>
    </ul>
  );
}

export function getCurrentStep(job) {
  if (!job) return "upload";
  if (job.status === "completed") return "download";
  if (job.status === "failed") return "process";
  return "process";
}
