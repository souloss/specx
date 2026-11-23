import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";
import MarkdownPreview from "./MarkdownPreview.vue";

describe("MarkdownPreview", () => {
  it("removes executable HTML and unsafe links from rendered content", () => {
    const wrapper = mount(MarkdownPreview, {
      props: { content: "hello <script>alert(1)</script> [run](javascript:alert(1))" },
    });

    expect(wrapper.find("script").exists()).toBe(false);
    expect(wrapper.find("a").attributes("href")).toBeUndefined();
    expect(wrapper.text()).toContain("hello");
  });

  it("renders the common first-version Markdown structures", () => {
    const wrapper = mount(MarkdownPreview, {
      props: { content: "| Name | Value |\n| --- | --- |\n| mode | safe |\n\n- [x] shipped" },
    });

    expect(wrapper.find("table").exists()).toBe(true);
    expect(wrapper.find('input[type="checkbox"]').exists()).toBe(true);
  });
});
