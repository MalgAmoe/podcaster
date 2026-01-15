import React from "react";
import { useProcess } from "../context/ProcessContext";
import { UploadZone } from "./UploadZone";
import { PresetSelector } from "./PresetSelector";
import { JobProgress } from "./JobProgress";
import { JobComplete } from "./JobComplete";
import { JobFailed } from "./JobFailed";
import { ErrorAlert } from "./ErrorAlert";
import { StepsIndicator, getCurrentStep } from "./StepsIndicator";

export function ProcessPage() {
  const { state, submitJob } = useProcess();
  const { job, error, uploadState } = state;

  const handleSubmit = (e) => {
    e.preventDefault();
    submitJob();
  };

  const renderContent = () => {
    if (job) {
      switch (job.status) {
        case "queued":
        case "processing":
          return <JobProgress />;
        case "completed":
          return <JobComplete />;
        case "failed":
          return <JobFailed />;
        default:
          return <JobProgress />;
      }
    }

    // No job - show upload form
    return (
      <form onSubmit={handleSubmit} className="space-y-6">
        <UploadZone />
        <PresetSelector />
        <button
          type="submit"
          disabled={uploadState !== "ready"}
          className="btn btn-primary w-full btn-lg"
        >
          🐄 MUNCH IT!
        </button>
      </form>
    );
  };

  return (
    <div className="min-h-[calc(100vh-4rem)] flex flex-col items-center justify-center p-4">
      {/* Main card */}
      <div className="card bg-base-200 w-full max-w-lg border border-base-300 rounded-3xl">
        <div className="card-body">
          <ErrorAlert message={error} />
          {renderContent()}
        </div>
      </div>

      {/* Steps indicator */}
      <StepsIndicator currentStep={getCurrentStep(job)} />
    </div>
  );
}
