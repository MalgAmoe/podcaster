import React from "react";
import { useProcess } from "../context/ProcessContext";
import { getFriendlyStage } from "../utils/stages";

export function JobProgress() {
  const { state, cancelJob } = useProcess();
  const { job } = state;

  if (!job) return null;

  const percentComplete = job.progress?.percent_complete || 0;
  const stage = job.progress?.stage;

  return (
    <div className="flex flex-col items-center py-8">
      <div
        className="radial-progress text-primary text-2xl font-bold"
        style={{
          "--value": percentComplete,
          "--size": "10rem",
          "--thickness": "0.5rem",
        }}
        role="progressbar"
      >
        {percentComplete}%
      </div>

      <p className="mt-6 text-xl font-bold">
        <span className="inline-block animate-munch">🐄</span> *munch munch munch*
      </p>
      <p className="text-base-content/60 mt-2">{getFriendlyStage(stage)}</p>
      <p className="text-base-content/40 text-sm mt-1 truncate max-w-full">{job.filename}</p>

      <button
        onClick={cancelJob}
        className="btn btn-ghost btn-sm mt-6 text-error"
      >
        Cancel
      </button>
    </div>
  );
}
