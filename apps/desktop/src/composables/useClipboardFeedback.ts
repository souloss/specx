import { onBeforeUnmount, ref } from "vue";

/** Clipboard feedback belongs to the active view and expires with it. */
export function useClipboardFeedback() {
  const copied = ref(false);
  const failed = ref(false);
  let timer: ReturnType<typeof setTimeout> | undefined;
  let active = true;
  async function copy(value: string) {
    clearTimeout(timer);
    copied.value = false;
    failed.value = false;
    try {
      await navigator.clipboard.writeText(value);
      if (!active) return;
      copied.value = true;
      timer = setTimeout(() => (copied.value = false), 1600);
    } catch {
      if (active) failed.value = true;
    }
  }
  onBeforeUnmount(() => {
    active = false;
    clearTimeout(timer);
  });
  return { copy, copied, failed };
}
