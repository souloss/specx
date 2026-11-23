import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";
import OpenApiDocumentMetadata from "./OpenApiDocumentMetadata.vue";

describe("document metadata", () => {
  it("shows contact, SPDX license and external documentation while rejecting executable links", () => {
    const wrapper = mount(OpenApiDocumentMetadata, {
      props: { info: { contact: { name: "Support", url: "javascript:alert(1)", email: "help@example.com" }, license: { identifier: "MIT", url: "https://example.com/license" }, termsOfService: "data:text/html,unsafe" }, externalDocs: { description: "Guide", url: "https://example.com/guide" } },
    });
    expect(wrapper.text()).toContain("Support");
    expect(wrapper.text()).toContain("MIT");
    expect(wrapper.findAll("a").map((a) => a.attributes("href"))).toEqual(["mailto:help%40example.com", "https://example.com/license", "https://example.com/guide"]);
    wrapper.unmount();
  });
});
