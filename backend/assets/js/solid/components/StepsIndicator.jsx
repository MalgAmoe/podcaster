export function StepsIndicator(props) {
  const getStepClass = (step) => {
    const order = { upload: 1, process: 2, download: 3 };
    const currentOrder = order[props.currentStep] || 1;
    const stepOrder = order[step];
    return stepOrder <= currentOrder ? "step step-primary" : "step";
  };

  return (
    <ul class="steps steps-horizontal mt-8">
      <li class={getStepClass("upload")}>Feed</li>
      <li class={getStepClass("process")}>Munch</li>
      <li class={getStepClass("download")}>Enjoy</li>
    </ul>
  );
}

export function getCurrentStep(job) {
  if (!job) return "upload";
  if (job.status === "completed") return "download";
  if (job.status === "failed") return "process";
  return "process";
}
