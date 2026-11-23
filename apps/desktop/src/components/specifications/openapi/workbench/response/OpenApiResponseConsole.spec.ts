import { describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import OpenApiResponseConsole from "./OpenApiResponseConsole.vue";
describe("script console", () => {
  it("shows assertion results alongside logs and a step error", () => {
    const wrapper = mount(OpenApiResponseConsole, {
      props: {
        logs: ["received response"],
        error: "post-response step 2 failed",
        tests: [
          { name: "HTTP 200", passed: true },
          { name: "payload", passed: false, error: "expected 1 to equal 2" },
        ],
      },
    });
    expect(wrapper.text()).toContain("1/2");
    expect(wrapper.findAll(".response-script-test")).toHaveLength(2);
    expect(wrapper.get(".response-script-test.failed").text()).toContain("expected 1 to equal 2");
    expect(wrapper.get("[role=alert]").text()).toContain("step 2");
    expect(wrapper.text()).toContain("received response");
  });
});
