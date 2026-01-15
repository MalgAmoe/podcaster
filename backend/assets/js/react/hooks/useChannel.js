import { useEffect, useRef, useCallback } from "react";
import { Socket } from "phoenix";

export function useChannel(jobId, onJobUpdate) {
  const socketRef = useRef(null);
  const channelRef = useRef(null);

  useEffect(() => {
    if (!jobId) return;

    // Get user token from window (set by root layout)
    const token = window.userToken;
    if (!token) {
      console.error("No userToken found - cannot connect to channel");
      return;
    }

    // Connect to socket
    const socket = new Socket("/socket", { params: { token } });
    socket.connect();
    socketRef.current = socket;

    // Join job channel
    const channel = socket.channel(`job:${jobId}`);
    channelRef.current = channel;

    channel
      .join()
      .receive("ok", ({ job }) => {
        // Initial job state
        if (onJobUpdate) onJobUpdate(job);
      })
      .receive("error", ({ reason }) => {
        console.error("Failed to join channel:", reason);
      });

    // Listen for job updates
    channel.on("job_updated", ({ job }) => {
      if (onJobUpdate) onJobUpdate(job);
    });

    // Cleanup on unmount or jobId change
    return () => {
      if (channelRef.current) {
        channelRef.current.leave();
        channelRef.current = null;
      }
      if (socketRef.current) {
        socketRef.current.disconnect();
        socketRef.current = null;
      }
    };
  }, [jobId, onJobUpdate]);

  const leave = useCallback(() => {
    if (channelRef.current) {
      channelRef.current.leave();
      channelRef.current = null;
    }
    if (socketRef.current) {
      socketRef.current.disconnect();
      socketRef.current = null;
    }
  }, []);

  return { leave };
}
