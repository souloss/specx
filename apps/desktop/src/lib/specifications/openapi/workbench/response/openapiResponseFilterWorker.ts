import { loadJq } from "jq-wasm";
import wasmURL from "jq-wasm/jq.wasm?url";
import { evaluateResponseFilter } from "./openapiResponseFilterEngine";
import type { ResponseFilterResult } from "./openapiResponseFilter";

self.onmessage = async (event: MessageEvent<{ body: string; query: string }>) => {
  const send = (result: ResponseFilterResult) => self.postMessage(result);
  try {
    const jq = await loadJq({ wasmURL });
    send(evaluateResponseFilter(jq, event.data.body, event.data.query));
  } catch {
    send({ error: "unavailable" });
  }
};
