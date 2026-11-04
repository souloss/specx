import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";
import { SpecxLogo, SpecxMark } from "./index";

describe("specX SVG brand", () => {
  it("renders an accessible full wordmark with stable dimensions", () => {
    const wrapper = mount(SpecxLogo, { props: { size: 40 } });
    const svg = wrapper.get("svg");

    expect(svg.attributes("width")).toBe("140");
    expect(svg.attributes("height")).toBe("40");
    expect(svg.attributes("role")).toBe("img");
    expect(svg.get("title").text()).toBe("specX");
    expect(svg.findAll("linearGradient")).toHaveLength(5);
  });

  it("supports decorative marks and isolates generated SVG IDs", () => {
    const wrapper = mount(() => (
      <div>
        <SpecxMark size={24} decorative />
        <SpecxMark size={24} />
      </div>
    ));
    const svgs = wrapper.findAll("svg");
    const ids = svgs.flatMap((svg) => svg.findAll("linearGradient").map((gradient) => gradient.attributes("id")));

    expect(svgs[0]!.attributes("aria-hidden")).toBe("true");
    expect(svgs[0]!.attributes("role")).toBeUndefined();
    expect(svgs[1]!.get("title").text()).toBe("specX");
    expect(new Set(ids).size).toBe(ids.length);
  });
});
