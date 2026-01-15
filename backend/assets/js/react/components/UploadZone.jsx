import React, { useCallback, useRef, useState } from "react";
import { useProcess } from "../context/ProcessContext";

export function UploadZone() {
  const { state, uploadFile, reset } = useProcess();
  const fileInputRef = useRef(null);
  const [isDragging, setIsDragging] = useState(false);

  const handleFile = useCallback(async (file) => {
    if (!file) return;

    // Validate file type
    if (!file.type.startsWith("audio/")) {
      return;
    }

    // Validate file size (100MB)
    if (file.size > 100 * 1024 * 1024) {
      return;
    }

    try {
      await uploadFile(file);
    } catch (err) {
      // Error is handled in context
    }
  }, [uploadFile]);

  const handleDrop = useCallback((e) => {
    e.preventDefault();
    setIsDragging(false);

    const file = e.dataTransfer.files[0];
    handleFile(file);
  }, [handleFile]);

  const handleDragOver = useCallback((e) => {
    e.preventDefault();
    setIsDragging(true);
  }, []);

  const handleDragLeave = useCallback((e) => {
    e.preventDefault();
    setIsDragging(false);
  }, []);

  const handleFileSelect = useCallback((e) => {
    const file = e.target.files[0];
    handleFile(file);
  }, [handleFile]);

  const handleClick = useCallback(() => {
    fileInputRef.current?.click();
  }, []);

  // Show file info if uploaded or uploading
  if (state.uploadState === "uploading" || state.uploadState === "ready") {
    return (
      <div className="flex items-center gap-4 p-4 bg-base-300 rounded-2xl">
        <div className="w-12 h-12 rounded-full bg-primary/10 flex items-center justify-center shrink-0">
          {state.uploadState === "ready" ? (
            <svg className="w-6 h-6 text-primary" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2"
                    d="M9 19V6l12-3v13M9 19c0 1.105-1.343 2-3 2s-3-.895-3-2 1.343-2 3-2 3 .895 3 2zm12-3c0 1.105-1.343 2-3 2s-3-.895-3-2 1.343-2 3-2 3 .895 3 2zM9 10l12-3" />
            </svg>
          ) : (
            <span className="loading loading-spinner loading-sm text-primary"></span>
          )}
        </div>
        <div className="flex-1 min-w-0">
          <p className="font-medium truncate">{state.filename}</p>
          {state.uploadState === "ready" ? (
            <p className="text-sm text-primary">Ready to munch!</p>
          ) : (
            <div className="flex items-center gap-2 mt-1">
              <progress
                className="progress progress-primary flex-1 h-2"
                value={state.uploadProgress}
                max="100"
              />
              <span className="text-xs text-base-content/60 w-8">{state.uploadProgress}%</span>
            </div>
          )}
        </div>
        <button
          type="button"
          onClick={reset}
          className="btn btn-ghost btn-sm btn-circle"
        >
          <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M6 18L18 6M6 6l12 12" />
          </svg>
        </button>
      </div>
    );
  }

  // Show upload zone
  return (
    <>
      <input
        ref={fileInputRef}
        type="file"
        accept="audio/*"
        onChange={handleFileSelect}
        className="hidden"
      />
      <label
        onClick={handleClick}
        onDrop={handleDrop}
        onDragOver={handleDragOver}
        onDragLeave={handleDragLeave}
        className={`border-2 border-dashed rounded-3xl p-12 text-center
                   hover:border-primary hover:bg-primary/5 transition-all duration-200
                   cursor-pointer group block
                   ${isDragging ? "border-primary bg-primary/5" : "border-base-300"}`}
      >
        <div className="flex flex-col items-center gap-4">
          <div className="w-16 h-16 rounded-full bg-primary/10 flex items-center justify-center
                          group-hover:bg-primary/20 transition-colors">
            <svg className="w-8 h-8 text-primary" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2"
                    d="M7 16a4 4 0 01-.88-7.903A5 5 0 1115.9 6L16 6a5 5 0 011 9.9M15 13l-3-3m0 0l-3 3m3-3v12" />
            </svg>
          </div>
          <div>
            <p className="font-medium text-base-content text-lg">Feed the cow!</p>
            <p className="text-sm text-base-content/60 mt-1">She's VERY hungry for your audio</p>
          </div>
          <p className="text-xs text-base-content/40">nom nom nom - WAV, MP3, FLAC</p>
        </div>
      </label>
    </>
  );
}
