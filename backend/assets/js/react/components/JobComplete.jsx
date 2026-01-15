import React from "react";
import { useProcess } from "../context/ProcessContext";

export function JobComplete() {
  const { state, reset } = useProcess();
  const { job } = state;

  if (!job) return null;

  const handleDownload = () => {
    if (job.download_url) {
      window.location.href = job.download_url;
    }
  };

  return (
    <div className="flex flex-col items-center py-8">
      <div className="w-20 h-20 rounded-full bg-primary/10 flex items-center justify-center mb-6">
        <svg className="w-10 h-10 text-primary" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M5 13l4 4L19 7" />
        </svg>
      </div>

      <h3 className="text-2xl font-bold">
        MOOO! <span className="inline-block animate-bounce-soft">🐄</span>
      </h3>
      <p className="text-base-content/60 mt-2">The cow is satisfied. Your audio is now delicious.</p>
      <p className="text-base-content/40 text-sm mt-1 truncate max-w-full">{job.filename}</p>

      <div className="flex gap-3 mt-8">
        <button onClick={handleDownload} className="btn btn-primary btn-lg gap-2">
          <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2"
                  d="M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-4l-4 4m0 0l-4-4m4 4V4" />
          </svg>
          Grab it!
        </button>
        <button onClick={reset} className="btn btn-ghost">
          🐄 Feed me more!
        </button>
      </div>
    </div>
  );
}
