import React from "react";
import { useProcess } from "../context/ProcessContext";
import { getFriendlyJobError } from "../utils/errors";

export function JobFailed() {
  const { state, reset } = useProcess();
  const { job } = state;

  if (!job) return null;

  return (
    <div className="flex flex-col items-center py-8">
      <div className="w-20 h-20 rounded-full bg-error/10 flex items-center justify-center mb-6">
        <svg className="w-10 h-10 text-error" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M6 18L18 6M6 6l12 12" />
        </svg>
      </div>

      <h3 className="text-2xl font-bold text-error">The cow choked!</h3>
      <p className="text-base-content/60 mt-2 text-center px-4">
        {getFriendlyJobError(job.error)}
      </p>

      <button onClick={reset} className="btn btn-primary mt-8">
        Feed her again
      </button>
    </div>
  );
}
