import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";
import OpenApiResponseViewer from "./OpenApiResponseViewer.vue";

describe("response visualization preview", () => {
  it("preserves template styles while isolating and sanitizing response HTML", async () => {
    const wrapper = mount(OpenApiResponseViewer, {
      props: { body: "{}", contentType: "application/json", visualize: '<style>.field { display: grid; }</style><dl class="field"><dt>name</dt><dd>SpecX</dd></dl><script>parent.alert(1)</script><img src="https://example.test/pixel" onerror="alert(1)">' },
      global: { stubs: { OpenApiResponseBody: { template: '<div><slot name="leading" /></div>' } } },
    });
    await wrapper
      .findAll("button")
      .find((button) => button.text() === "Visualize")!
      .trigger("click");
    const frame = wrapper.get("iframe");
    const document = new DOMParser().parseFromString(frame.attributes("srcdoc")!, "text/html");
    expect(document.querySelector("style")?.textContent).toContain("display: grid");
    expect(document.querySelector("dd")?.textContent).toBe("SpecX");
    expect(document.querySelector("script")).toBeNull();
    expect(document.querySelector("img")?.hasAttribute("onerror")).toBe(false);
    expect(frame.attributes("sandbox")).toBe("");
    expect(frame.attributes("referrerpolicy")).toBe("no-referrer");
    expect(document.querySelector('meta[http-equiv="Content-Security-Policy"]')?.getAttribute("content")).toBe("default-src 'none'; img-src data: blob:; style-src 'unsafe-inline'");
    wrapper.unmount();
  });
});
