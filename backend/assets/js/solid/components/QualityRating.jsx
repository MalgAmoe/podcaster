import { createSignal, Show } from "solid-js";
import { api } from "../utils/api";

export function QualityRating(props) {
  const [rated, setRated] = createSignal(false);
  const [showTextInput, setShowTextInput] = createSignal(false);
  const [text, setText] = createSignal("");
  const [submitting, setSubmitting] = createSignal(false);

  async function submitRating(rating) {
    if (rating === "not_good") {
      setShowTextInput(true);
      try {
        await api.submitFeedback({ job_id: props.jobId, prompt_key: "quality", rating });
      } catch { /* silent */ }
      return;
    }

    try {
      await api.submitFeedback({ job_id: props.jobId, prompt_key: "quality", rating });
    } catch { /* silent */ }

    setRated(true);
    if (props.onComplete) props.onComplete();
  }

  async function submitText() {
    setSubmitting(true);
    try {
      await api.submitFeedback({ job_id: props.jobId, prompt_key: "quality_detail", value: text() });
    } catch { /* silent */ }
    setRated(true);
    setSubmitting(false);
    if (props.onComplete) props.onComplete();
  }

  function skip() {
    setRated(true);
    if (props.onComplete) props.onComplete();
  }

  return (
    <Show when={!rated()}>
      <div class="mt-6 text-center">
        <Show when={!showTextInput()}>
          <p class="text-sm text-base-content/50 mb-3">How does it sound?</p>
          <div class="flex justify-center gap-3">
            <button
              onClick={() => submitRating("great")}
              class="px-4 py-2 rounded-full text-sm bg-base-300 hover:bg-primary hover:text-primary-content transition-all cursor-pointer"
            >
              Sounds great
            </button>
            <button
              onClick={() => submitRating("ok")}
              class="px-4 py-2 rounded-full text-sm bg-base-300 hover:bg-base-content/20 transition-all cursor-pointer"
            >
              It's OK
            </button>
            <button
              onClick={() => submitRating("not_good")}
              class="px-4 py-2 rounded-full text-sm bg-base-300 hover:bg-error/20 hover:text-error transition-all cursor-pointer"
            >
              Not good
            </button>
          </div>
        </Show>
        <Show when={showTextInput()}>
          <p class="text-sm text-base-content/50 mb-3">What could be better?</p>
          <textarea
            value={text()}
            onInput={(e) => setText(e.target.value)}
            class="textarea textarea-bordered w-full text-sm"
            rows="2"
            placeholder="e.g. too much compression, voice sounds thin..."
          />
          <div class="flex justify-center gap-2 mt-3">
            <button
              onClick={submitText}
              class="btn btn-primary btn-sm"
              disabled={submitting()}
            >
              Send
            </button>
            <button onClick={skip} class="btn btn-ghost btn-sm text-xs text-base-content/40">Skip</button>
          </div>
        </Show>
      </div>
    </Show>
  );
}
