import React from "react";

export function StepsIndicator({ currentStep }) {
  const getStepClass = (step) => {
    const order = { upload: 1, process: 2, download: 3 };
    const currentOrder = order[currentStep] || 1;
    const stepOrder = order[step];

    if (stepOrder <= currentOrder) {
      return "step step-primary";
    }
    return "step";
  };

  return (
    <ul className="steps steps-horizontal mt-8">
      <li className={getStepClass("upload")}>Feed</li>
      <li className={getStepClass("process")}>Munch</li>
      <li className={getStepClass("download")}>Enjoy</li>
    </ul>
  );
}

export function getCurrentStep(job) {
  if (!job) return "upload";
  if (job.status === "completed") return "download";
  if (job.status === "failed") return "process";
  return "process";
}
