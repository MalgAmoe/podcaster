import { createSignal, Show } from "solid-js";
import { api } from "../utils/api";

// Micro-questions, shown one at a time, rotated based on what's been answered
const PROMPTS = [
  { key: "use_case", question: "What are you using this for?", options: ["Podcast", "YouTube", "Voiceover", "Music", "Other"] },
  { key: "used_before", question: "What did you use before?", options: ["Nothing", "Audacity", "Auphonic", "Descript", "Adobe Podcast", "Other"] },
  { key: "would_pay", question: "Would you pay for this?", options: ["Yes", "Maybe", "No"] },
];

function getAnsweredKeys() {
  try {
    return JSON.parse(localStorage.getItem("feedback_answered") || "[]");
  } catch { return []; }
}

function markAnswered(key) {
  const answered = getAnsweredKeys();
  if (!answered.includes(key)) {
    answered.push(key);
    localStorage.setItem("feedback_answered", JSON.stringify(answered));
  }
}

function getNextPrompt() {
  const answered = getAnsweredKeys();
  return PROMPTS.find(p => !answered.includes(p.key)) || null;
}

function shouldShowFeedback() {
  // Show feedback every 3rd job, starting from job 1
  const count = window.completedJobsCount || 0;
  return count > 0 && count % 3 === 1;
}

export function FeedbackPrompt(props) {
  const [submitted, setSubmitted] = createSignal(false);
  const [rating, setRating] = createSignal(null);
  const prompt = getNextPrompt();

  if (!shouldShowFeedback() && !props.alwaysShow) return null;

  async function submitRating(value) {
    setRating(value);
    try {
      await api.submitFeedback({ job_id: props.jobId, rating: value });
    } catch { /* silent */ }
  }

  async function submitAnswer(promptKey, value) {
    markAnswered(promptKey);
    setSubmitted(true);
    try {
      await api.submitFeedback({ job_id: props.jobId, prompt_key: promptKey, value });
    } catch { /* silent */ }
  }

  return (
    <Show when={!submitted()}>
      <div class="mt-4 p-4 rounded-xl bg-base-200 border border-base-300">
        {/* Thumbs up/down */}
        <Show when={!rating()}>
          <div class="flex items-center gap-3">
            <span class="text-sm text-base-content/60">How does it sound?</span>
            <button
              onClick={() => submitRating("up")}
              class="btn btn-ghost btn-sm text-lg"
              title="Sounds good"
            >
              👍
            </button>
            <button
              onClick={() => submitRating("down")}
              class="btn btn-ghost btn-sm text-lg"
              title="Needs work"
            >
              👎
            </button>
          </div>
        </Show>

        {/* After rating, show micro-question if available */}
        <Show when={rating() && prompt}>
          <div class="mt-2">
            <p class="text-sm text-base-content/60 mb-2">{prompt.question}</p>
            <div class="flex flex-wrap gap-1">
              {prompt.options.map(opt => (
                <button
                  onClick={() => submitAnswer(prompt.key, opt)}
                  class="btn btn-ghost btn-xs"
                >
                  {opt}
                </button>
              ))}
            </div>
          </div>
        </Show>

        {/* After rating with no prompt, say thanks */}
        <Show when={rating() && !prompt}>
          <p class="text-sm text-base-content/50">Thanks for the feedback!</p>
        </Show>
      </div>
    </Show>
  );
}
