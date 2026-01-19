import { useProcess } from "../context/ProcessContext";

export function CategoryToggle() {
  const { store, setCategory } = useProcess();
  const category = () => store.processingConfig.category;

  return (
    <div class="form-control text-center">
      <p class="font-medium mb-3">What are you processing?</p>
      <div class="flex gap-2 justify-center">
        <button
          type="button"
          onClick={() => setCategory("voice")}
          class={`px-3 py-1.5 rounded-full transition-all text-sm ${
            category() === "voice"
              ? "bg-primary text-primary-content font-semibold"
              : "bg-base-300 text-base-content/50"
          }`}
        >
          Voice
        </button>
        <button
          type="button"
          onClick={() => setCategory("mixed")}
          class={`px-3 py-1.5 rounded-full transition-all text-sm ${
            category() === "mixed"
              ? "bg-primary text-primary-content font-semibold"
              : "bg-base-300 text-base-content/50"
          }`}
        >
          Mixed Audio
        </button>
      </div>
    </div>
  );
}
