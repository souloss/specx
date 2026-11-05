import { describe, expect, it } from "vitest";
import { SPECIFICATION_TYPES, specificationType } from "./registry";

describe("specification registry", () => {
  it("describes Markdown as a document with picker, fields, and review steps", () => {
    const markdown = specificationType("markdown");

    expect(SPECIFICATION_TYPES).toHaveLength(1);
    expect(markdown.category).toBe("document");
    expect(markdown.steps.map((step) => step.kind)).toEqual(["type-picker", "fields", "review"]);
    expect(markdown.steps[1]?.fields?.map((field) => field.id)).toEqual(["title", "content"]);
    expect(markdown.steps[2]?.summaryFields?.map((field) => field.id)).toEqual(["title"]);
  });

  it("keeps request mapping inside the registered specification type", () => {
    const request = specificationType("markdown").createRequest({
      id: "spec-1",
      serviceId: "service-1",
      values: { title: "  API guide  ", content: "# API" },
    });

    expect(request).toEqual({ id: "spec-1", serviceId: "service-1", title: "API guide", content: "# API" });
  });
});
