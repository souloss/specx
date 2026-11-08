import { describe, expect, it } from "vitest";
import { Braces, FileText } from "@lucide/vue";
import { SPECIFICATION_TYPES, specificationType } from "./registry";

describe("specification registry", () => {
  it("describes Markdown as a document with picker, fields, and review steps", () => {
    const markdown = specificationType("markdown");

    expect(SPECIFICATION_TYPES).toHaveLength(2);
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

    expect(request).toEqual({ id: "spec-1", serviceId: "service-1", kind: "markdown", title: "API guide", content: "# API" });
  });

  it("uses the OpenAPI info title when the user leaves the document title blank", () => {
    const request = specificationType("openapi").createRequest({
      id: "spec-openapi",
      serviceId: "service-1",
      values: { title: "", content: "openapi: 3.1.0\ninfo:\n  title: Accounts API\n  version: '1'\npaths: {}\n" },
    });
    expect(request.title).toBe("Accounts API");
    expect(request.kind).toBe("openapi");
  });

  it("publishes a provider icon for every registered specification type", () => {
    expect(specificationType("markdown").icon).toBe(FileText);
    expect(specificationType("openapi").icon).toBe(Braces);
    expect(specificationType("markdown").directoryKey).toBe("specification.kinds.markdown");
    expect(specificationType("openapi").directoryKey).toBe("specification.kinds.openapi");
  });
});
