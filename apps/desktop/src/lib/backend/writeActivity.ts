import { readonly, ref } from "vue";
import { translate } from "@/i18n";

const pending = ref(0);
let installationLocked = false;
export const pendingWrites = readonly(pending);

export async function trackWrite<T>(operation: () => Promise<T>): Promise<T> {
  if (installationLocked) throw new Error(translate("updates.installing"));
  pending.value++;
  try {
    return await operation();
  } finally {
    pending.value--;
  }
}

/** Refuse a restart while a write is running, and prevent new writes from
 * slipping in while the native installer is preparing to stop this process. */
export function lockWritesForInstallation(): (() => void) | undefined {
  if (pending.value || installationLocked) return undefined;
  installationLocked = true;
  return () => {
    installationLocked = false;
  };
}
