import type { Jq } from "jq-wasm";
import { formatResponseJson } from "./openapiResponse";
import type { ResponseFilterResult } from "./openapiResponseFilter";

const MAX_RESULTS = 1_000;
const MAX_RESULT_BYTES = 1024 * 1024;
const limitError = "specx_response_filter_output_limit";

export function evaluateResponseFilter(jq: Pick<Jq, "raw">, body: string, query: string): ResponseFilterResult {
  if (query.length > 2_000 || query.includes("\0")) return { error: "syntax" };
  // Collect a bounded sequence of JSON literals without parsing numbers through JS.
  const program = `reduce limit(${MAX_RESULTS + 1}; (
${query}
)) as $item ({bytes: 0, values: []};
($item | tojson) as $value |
.bytes += ($value | utf8bytelength) + 1 |
if .bytes >= ${MAX_RESULT_BYTES} or (.values | length) >= ${MAX_RESULTS}
then error("${limitError}") else .values += [$value] end
) | .values[]`;
  const { stdout, stderr, exitCode } = jq.raw(body, program, ["-r", "--"]);
  if (exitCode !== 0) {
    const detail = stderr
      .split("\n", 1)[0]
      ?.replace(/ at <top-level>.*$/, "")
      .slice(0, 160);
    return stderr.endsWith(limitError) ? { error: "limit" } : exitCode === 3 ? { error: "syntax", detail } : { error: "evaluation" };
  }
  if (!stdout) {
    return { count: 0, text: "" };
  }
  const values = stdout.split("\n");
  if (values.length > MAX_RESULTS || new TextEncoder().encode(stdout).length >= MAX_RESULT_BYTES) {
    return { error: "limit" };
  }
  const json = values.length === 1 ? values[0]! : `[${values.join(",")}]`;
  const formatted = formatResponseJson(json);
  return formatted ? { count: values.length, text: formatted.text } : { error: "evaluation" };
}
