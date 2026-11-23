import { describe, expect, it } from "vitest";
import { describeExecutedRequest } from "./openapiExecutedRequest";

describe("actual request snapshot", () => {
  it("shows the final UTF-8 upload bytes and the transmitted content type", () => {
    const body = new TextEncoder().encode('{"title":"猫"}');
    expect(describeExecutedRequest({ method: "PUT", url: "https://example.test/posts/1", headers: [["content-type", "application/json; charset=utf-8"]], body })).toMatchObject({ method: "PUT", body: '{"title":"猫"}', bodyBytes: body.length, mediaType: "application/json; charset=utf-8" });
  });
  it("preserves serialized form and multipart content instead of substituting an empty body", () => {
    for (const [mediaType, text] of [
      ["application/x-www-form-urlencoded", "title=hello+world"],
      ["multipart/form-data; boundary=xyz", '--xyz\r\nContent-Disposition: form-data; name="title"\r\n\r\nhello\r\n--xyz--\r\n'],
    ]) {
      expect(describeExecutedRequest({ method: "POST", url: "https://example.test", headers: [["Content-Type", mediaType!]], body: new TextEncoder().encode(text) }).body).toBe(text);
    }
  });
  it("keeps binary uploads visible and distinguishes them from absent request bodies", () => {
    expect(describeExecutedRequest({ method: "PUT", url: "https://example.test", headers: [["content-type", "application/octet-stream"]], body: new Uint8Array([0, 255, 7]) })).toMatchObject({ bodyBytes: 3, body: "00 ff 07", binary: true });
    expect(describeExecutedRequest({ method: "GET", url: "https://example.test", headers: [], body: new Uint8Array() })).toMatchObject({ body: "", bodyBytes: 0, binary: false });
  });
});
