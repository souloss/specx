import { ref, watch } from "vue";
import { defineStore } from "pinia";
import { safeLocalStorageGet, safeLocalStorageSet } from "@/lib/backend/safeStorage";
import type { RequestProxyMode } from "@/lib/specifications/openapi/workbench/request/openapiRequestAgent";
export const useRequestAgentStore = defineStore("requestAgent", () => {
  const saved = safeLocalStorageGet("specx-request-proxy");
  const mode = ref<RequestProxyMode>(saved === "browser" || saved === "agent" ? saved : "auto");
  watch(mode, (value) => safeLocalStorageSet("specx-request-proxy", value));
  return { mode };
});
