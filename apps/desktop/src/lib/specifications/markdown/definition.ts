import { FileText } from "@lucide/vue";
import { specificationCreationSteps, type SpecificationTypeDefinition } from "../definition";

export const markdownSpecification: SpecificationTypeDefinition = {
  kind: "markdown",
  category: "document",
  categoryKey: "specification.categories.document",
  directoryKey: "specification.kinds.markdown",
  titleKey: "specification.types.markdown.title",
  descriptionKey: "specification.types.markdown.description",
  keywordsKey: "specification.types.markdown.keywords",
  icon: FileText,
  enabled: true,
  steps: specificationCreationSteps([
    { id: "title", control: "text", labelKey: "specification.title", required: true },
    { id: "content", control: "textarea", labelKey: "specification.content", required: true, defaultValueKey: "specification.defaultContent", rows: 12 },
  ]),
  file: { extensions: [".md", ".markdown"], mimeTypes: ["text/markdown"] },
  createRequest: ({ id, serviceId, values }) => ({
    id,
    serviceId,
    kind: "markdown",
    title: values.title?.trim() ?? "",
    content: values.content ?? "",
  }),
};
