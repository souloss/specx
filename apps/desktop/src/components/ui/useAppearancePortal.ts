import { inject, type InjectionKey, type Ref } from "vue";

/** Floating UI stays inside the nearest preview's appearance boundary. */
export const appearancePortalKey: InjectionKey<Ref<HTMLElement | undefined>> = Symbol("appearance-portal");

export function useAppearancePortal() {
  return inject(appearancePortalKey, undefined);
}
