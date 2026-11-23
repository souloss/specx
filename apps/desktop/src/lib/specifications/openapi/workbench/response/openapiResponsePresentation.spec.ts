import { describe, expect, it } from "vitest";
import { decodeResponseBytes, responseEncodings, responseEncodingForContentType } from "./openapiResponsePresentation";
describe("response byte presentation", () => {
  it("detects response charset aliases and UTF16 byte order markers", () => {
    expect(responseEncodingForContentType('text/plain; charset="Shift_JIS"')).toBe("Shift_JIS");
    expect(responseEncodingForContentType("text/plain; charset=UTF-16BE")).toBe("utf16-be");
    expect(responseEncodingForContentType("text/plain; charset=gb2312")).toBe("GB2312");
    expect(responseEncodingForContentType("text/plain; charset=unsupported")).toBe("utf8");
    expect(decodeResponseBytes(new Uint8Array([0xfe, 0xff, 0x4e, 0x2d]), "utf16")).toBe("中");
    expect(decodeResponseBytes(new Uint8Array([0xff, 0xfe, 0x2d, 0x4e]), "utf16")).toBe("中");
  });
  it("renders hex/base64 from the original bytes, not a decoded string", () => {
    const bytes = new Uint8Array([0, 255, 97, 98]);
    expect(decodeResponseBytes(bytes, "hex")).toBe("00ff6162");
    expect(decodeResponseBytes(bytes, "base64")).toBe("AP9hYg==");
    expect(decodeResponseBytes(bytes, "binary")).toBe("\x00\xffab");
  });
  it("decodes GBK, UTF16, single-byte encodings and ASCII without changing captured data", () => {
    expect(decodeResponseBytes(new Uint8Array([0xd6, 0xd0, 0xce, 0xc4]), "GBK")).toBe("中文");
    expect(decodeResponseBytes(new Uint8Array([0x2d, 0x4e]), "utf16-le")).toBe("中");
    expect(decodeResponseBytes(new Uint8Array([0x4e, 0x2d]), "utf16-be")).toBe("中");
    expect(decodeResponseBytes(new Uint8Array([0xe9]), "ISO-8859-1")).toBe("é");
    expect(decodeResponseBytes(new Uint8Array([0xa4]), "ISO-8859-16")).toBe("€");
    expect(decodeResponseBytes(new Uint8Array(1024 * 1024).fill(65), "binary")).toHaveLength(1024 * 1024);
    for (const encoding of responseEncodings) expect(() => decodeResponseBytes(new Uint8Array([65]), encoding)).not.toThrow();
  });
});
