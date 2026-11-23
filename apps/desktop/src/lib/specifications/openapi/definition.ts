import { Braces } from "@lucide/vue";
import { specificationCreationSteps, type SpecificationTypeDefinition } from "../definition";
import { isRecord, parseOpenApiContent, type OpenApiObject } from "./contract/openapi";
import { buildOpenApiNavigation } from "./navigation";

function openApiTitle(document: OpenApiObject | null): string {
  const info = document?.info;
  return isRecord(info) && typeof info.title === "string" ? info.title.trim() : "";
}

export const openApiSpecification: SpecificationTypeDefinition = {
  kind: "openapi",
  category: "api",
  categoryKey: "specification.categories.api",
  directoryKey: "specification.kinds.openapi",
  titleKey: "specification.types.openapi.title",
  descriptionKey: "specification.types.openapi.description",
  keywordsKey: "specification.types.openapi.keywords",
  icon: Braces,
  enabled: true,
  steps: specificationCreationSteps([
    { id: "title", control: "text", labelKey: "specification.title", required: false },
    { id: "content", control: "textarea", labelKey: "specification.openapiContent", required: true, defaultValueKey: "specification.defaultOpenapiContent", rows: 18 },
  ]),
  file: {
    extensions: [".json", ".yaml", ".yml"],
    mimeTypes: ["application/json", "application/yaml", "text/yaml"],
    inspect(content, extension) {
      const document = parseOpenApiContent(content);
      const version = document?.openapi;
      if (typeof version !== "string" || !(version.startsWith("3.0.") || version.startsWith("3.1."))) throw new Error("invalid_openapi");
      return { title: openApiTitle(document), format: extension === ".json" ? "json" : "yaml" };
    },
  },
  buildNavigation: buildOpenApiNavigation,
  createRequest: ({ id, serviceId, values }) => ({
    id,
    serviceId,
    kind: "openapi",
    title: values.title?.trim() || openApiTitle(parseOpenApiContent(values.content ?? "")),
    content: values.content ?? "",
  }),
};
