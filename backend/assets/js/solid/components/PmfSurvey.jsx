import { createSignal, Show } from "solid-js";
import { api } from "../utils/api";

function hasAnsweredPmf() {
  try {
    return localStorage.getItem("pmf_answered") === "true";
  } catch { return false; }
}

function markPmfAnswered() {
  try { localStorage.setItem("pmf_answered", "true"); } catch {}
}

export function PmfSurvey(props) {
  const [pmfAnswer, setPmfAnswer] = createSignal(null);
  const [improvementText, setImprovementText] = createSignal("");
  const [submitted, setSubmitted] = createSignal(false);
  const [submitting, setSubmitting] = createSignal(false);

  if (window.isGuest) return null;
  const count = window.completedJobsCount || 0;
  if (count < 3 || hasAnsweredPmf()) return null;

  async function selectPmf(value) {
    setPmfAnswer(value);
    try {
      await api.submitFeedback({ job_id: props.jobId, prompt_key: "pmf", value });
    } catch { /* silent */ }
  }

  async function submitImprovement() {
    setSubmitting(true);
    if (improvementText().trim()) {
      try {
        await api.submitFeedback({ job_id: props.jobId, prompt_key: "improvement", value: improvementText() });
      } catch { /* silent */ }
    }
    markPmfAnswered();
    setSubmitted(true);
    setSubmitting(false);
  }

  function skip() {
    markPmfAnswered();
    setSubmitted(true);
  }

  return (
    <Show when={!submitted()}>
      <div class="fixed inset-0 bg-base-100/80 flex items-center justify-center z-50 p-4">
        <div class="card bg-base-200 border border-base-300 shadow-xl w-full max-w-sm rounded-3xl">
          <div class="card-body">
            <div class="flex items-center gap-3 mb-2">
              <img src="/images/munchy_cow_head.svg" alt="" class="w-10 h-10" />
              <div>
                <p class="text-xs text-primary font-medium">You're one of our first users</p>
                <p class="text-sm text-base-content/50">Your input shapes what we build</p>
              </div>
            </div>

            <Show when={!pmfAnswer()}>
              <p class="text-sm text-base-content mt-2 mb-4">
                How would you feel if you could no longer use Munchy Cow?
              </p>
              <div class="flex flex-col gap-2">
                <button
                  onClick={() => selectPmf("very_disappointed")}
                  class="px-4 py-2 rounded-full text-sm bg-base-200 hover:bg-primary hover:text-primary-content transition-all cursor-pointer text-left"
                >
                  Very disappointed
                </button>
                <button
                  onClick={() => selectPmf("somewhat_disappointed")}
                  class="px-4 py-2 rounded-full text-sm bg-base-200 hover:bg-base-content/20 transition-all cursor-pointer text-left"
                >
                  Somewhat disappointed
                </button>
                <button
                  onClick={() => selectPmf("not_disappointed")}
                  class="px-4 py-2 rounded-full text-sm bg-base-200 hover:bg-base-content/20 transition-all cursor-pointer text-left"
                >
                  Not disappointed
                </button>
              </div>
              <button onClick={skip} class="btn btn-ghost btn-sm text-xs text-base-content/40 mt-3">
                Not now
              </button>
            </Show>

            <Show when={pmfAnswer()}>
              <p class="text-sm text-base-content mt-2 mb-3">
                What would make this more useful for you?
              </p>
              <textarea
                value={improvementText()}
                onInput={(e) => setImprovementText(e.target.value)}
                class="textarea textarea-bordered w-full text-sm"
                rows="3"
                placeholder="I wish it could..."
                autofocus
              />
              <div class="flex gap-2 mt-3">
                <button
                  onClick={submitImprovement}
                  class="btn btn-primary btn-sm flex-1"
                  disabled={submitting()}
                >
                  Send
                </button>
                <button onClick={skip} class="btn btn-ghost btn-sm text-base-content/40">
                  Skip
                </button>
              </div>
            </Show>
          </div>
        </div>
      </div>
    </Show>
  );
}
