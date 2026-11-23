import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";
import OpenApiSnippetSelector from "./OpenApiSnippetSelector.vue";
import type { SnippetLanguage } from "@/lib/specifications/openapi/snippets/openapiCodegen";

describe("shared request client selector", () => {
  it("switches families to a valid client and exposes only compatible clients", async () => {
    const wrapper = mount(OpenApiSnippetSelector, { props: { modelValue: "curl" as SnippetLanguage, "onUpdate:modelValue": (value: SnippetLanguage) => wrapper.setProps({ modelValue: value }) } });
    await wrapper.get('select[name="openapi-snippet-language"]').setValue("JavaScript");
    expect(wrapper.props("modelValue")).toBe("fetch");
    expect(wrapper.get('select[name="openapi-snippet-client"]').text()).toContain("XHR");
    expect(wrapper.get('select[name="openapi-snippet-client"]').text()).not.toContain("cURL");
    await wrapper.get('select[name="openapi-snippet-client"]').setValue("xhr");
    expect(wrapper.props("modelValue")).toBe("xhr");
    await wrapper.get('select[name="openapi-snippet-language"]').setValue("Java");
    expect(wrapper.props("modelValue")).toBe("java");
    expect(wrapper.get('select[name="openapi-snippet-client"]').text()).toContain("OkHttp");
    wrapper.unmount();
  });
});
