import React, { createContext, useContext, useReducer, useCallback, useEffect } from "react";
import { api } from "../utils/api";
import { useUpload } from "../hooks/useUpload";
import { useChannel } from "../hooks/useChannel";

// Initial state
const initialState = {
  // Upload
  file: null,
  s3Key: null,
  filename: null,
  uploadProgress: 0,
  uploadState: "idle", // idle, uploading, ready, error

  // Presets
  presets: ["podcast", "broadcast", "gentle"],
  selectedPreset: "podcast",

  // Job
  job: null,

  // Error
  error: null,
};

// Action types
const ACTIONS = {
  SET_FILE: "SET_FILE",
  SET_UPLOAD_PROGRESS: "SET_UPLOAD_PROGRESS",
  SET_UPLOAD_STATE: "SET_UPLOAD_STATE",
  SET_UPLOAD_COMPLETE: "SET_UPLOAD_COMPLETE",
  SET_PRESETS: "SET_PRESETS",
  SELECT_PRESET: "SELECT_PRESET",
  SET_JOB: "SET_JOB",
  UPDATE_JOB: "UPDATE_JOB",
  SET_ERROR: "SET_ERROR",
  RESET: "RESET",
};

// Reducer
function reducer(state, action) {
  switch (action.type) {
    case ACTIONS.SET_FILE:
      return {
        ...state,
        file: action.file,
        filename: action.file?.name || null,
        uploadState: "idle",
        error: null,
      };

    case ACTIONS.SET_UPLOAD_PROGRESS:
      return {
        ...state,
        uploadProgress: action.progress,
        uploadState: "uploading",
      };

    case ACTIONS.SET_UPLOAD_STATE:
      return {
        ...state,
        uploadState: action.uploadState,
      };

    case ACTIONS.SET_UPLOAD_COMPLETE:
      return {
        ...state,
        s3Key: action.s3Key,
        filename: action.filename,
        uploadProgress: 100,
        uploadState: "ready",
      };

    case ACTIONS.SET_PRESETS:
      return {
        ...state,
        presets: action.presets,
      };

    case ACTIONS.SELECT_PRESET:
      return {
        ...state,
        selectedPreset: action.preset,
      };

    case ACTIONS.SET_JOB:
      return {
        ...state,
        job: action.job,
        error: null,
      };

    case ACTIONS.UPDATE_JOB:
      return {
        ...state,
        job: action.job,
      };

    case ACTIONS.SET_ERROR:
      return {
        ...state,
        error: action.error,
      };

    case ACTIONS.RESET:
      return {
        ...initialState,
        presets: state.presets,
        selectedPreset: state.selectedPreset,
      };

    default:
      return state;
  }
}

// Context
const ProcessContext = createContext(null);

// Provider
export function ProcessProvider({ children }) {
  const [state, dispatch] = useReducer(reducer, initialState);
  const { upload, progress, uploading, error: uploadError, reset: resetUpload } = useUpload();

  // Update upload progress
  useEffect(() => {
    if (uploading) {
      dispatch({ type: ACTIONS.SET_UPLOAD_PROGRESS, progress });
    }
  }, [progress, uploading]);

  // Handle upload errors
  useEffect(() => {
    if (uploadError) {
      dispatch({ type: ACTIONS.SET_ERROR, error: uploadError });
      dispatch({ type: ACTIONS.SET_UPLOAD_STATE, uploadState: "error" });
    }
  }, [uploadError]);

  // Job update handler for channel
  const handleJobUpdate = useCallback((job) => {
    dispatch({ type: ACTIONS.UPDATE_JOB, job });
  }, []);

  // Connect to job channel when job exists
  useChannel(state.job?.id, handleJobUpdate);

  // Load presets on mount
  useEffect(() => {
    api.getPresets()
      .then(({ presets }) => {
        dispatch({ type: ACTIONS.SET_PRESETS, presets });
      })
      .catch((err) => {
        console.error("Failed to load presets:", err);
      });
  }, []);

  // Actions
  const selectFile = useCallback((file) => {
    dispatch({ type: ACTIONS.SET_FILE, file });
  }, []);

  const uploadFile = useCallback(async (file) => {
    dispatch({ type: ACTIONS.SET_UPLOAD_STATE, uploadState: "uploading" });
    try {
      const { key, filename } = await upload(file);
      dispatch({ type: ACTIONS.SET_UPLOAD_COMPLETE, s3Key: key, filename });
      return { key, filename };
    } catch (err) {
      dispatch({ type: ACTIONS.SET_ERROR, error: err.message });
      dispatch({ type: ACTIONS.SET_UPLOAD_STATE, uploadState: "error" });
      throw err;
    }
  }, [upload]);

  const selectPreset = useCallback((preset) => {
    dispatch({ type: ACTIONS.SELECT_PRESET, preset });
  }, []);

  const submitJob = useCallback(async () => {
    if (!state.s3Key || !state.filename) {
      dispatch({ type: ACTIONS.SET_ERROR, error: "Please upload a file first" });
      return;
    }

    try {
      const job = await api.createJob(state.s3Key, state.filename, state.selectedPreset);
      dispatch({ type: ACTIONS.SET_JOB, job });
    } catch (err) {
      dispatch({ type: ACTIONS.SET_ERROR, error: err.message });
    }
  }, [state.s3Key, state.filename, state.selectedPreset]);

  const cancelJob = useCallback(async () => {
    if (state.job?.id) {
      try {
        await api.cancelJob(state.job.id);
      } catch (err) {
        console.error("Failed to cancel job:", err);
      }
    }
    resetUpload();
    dispatch({ type: ACTIONS.RESET });
  }, [state.job, resetUpload]);

  const clearError = useCallback(() => {
    dispatch({ type: ACTIONS.SET_ERROR, error: null });
  }, []);

  const reset = useCallback(() => {
    resetUpload();
    dispatch({ type: ACTIONS.RESET });
  }, [resetUpload]);

  const value = {
    state,
    selectFile,
    uploadFile,
    selectPreset,
    submitJob,
    cancelJob,
    clearError,
    reset,
  };

  return (
    <ProcessContext.Provider value={value}>
      {children}
    </ProcessContext.Provider>
  );
}

// Hook
export function useProcess() {
  const context = useContext(ProcessContext);
  if (!context) {
    throw new Error("useProcess must be used within a ProcessProvider");
  }
  return context;
}
