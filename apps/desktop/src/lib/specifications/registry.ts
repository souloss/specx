import type { MessageKey } from "@/i18n";
import type { SpecificationKind } from "@/types/domain";
import type { SpecificationRefKind } from "@/types/api";
import { Braces, FileText } from "@lucide/vue";
import type { Component } from "vue";
import { parseOpenApiContent } from "@/lib/openapi";

export type SpecificationDraft = Record<string, string>;
export type SpecificationRequest = {
  id: string;
  serviceId: string;
  title: string;
  content: string;
  kind: SpecificationKind;
  source?: { path: string; refKind: SpecificationRefKind; refName: string };
};

export type SpecificationFieldDefinition = {
  id: string;
  control: "text" | "textarea";
  labelKey: MessageKey;
  required: boolean;
  defaultValueKey?: MessageKey;
  rows?: number;
};

export type SpecificationStepDefinition = {
  id: string;
  kind: "type-picker" | "fields" | "review";
  labelKey: MessageKey;
  fields?: readonly SpecificationFieldDefinition[];
  summaryFields?: readonly { id: string; labelKey: MessageKey }[];
};

export type SpecificationTypeDefinition = {
  kind: SpecificationKind;
  category: string;
  categoryKey: MessageKey;
  directoryKey: MessageKey;
  titleKey: MessageKey;
  descriptionKey: MessageKey;
  keywordsKey: MessageKey;
  icon: Component;
  enabled: boolean;
  steps: readonly SpecificationStepDefinition[];
  createRequest: (draft: { id: string; serviceId: string; values: SpecificationDraft }) => SpecificationRequest;
};

function openApiTitle(content: string): string {
  const info = parseOpenApiContent(content)?.info;
  if (!info || typeof info !== "object" || Array.isArray(info) || !("title" in info) || typeof info.title !== "string") return "";
  return info.title.trim();
}

const markdownSteps = [
  { id: "select-type", kind: "type-picker", labelKey: "specification.step1" },
  {
    id: "write-content",
    kind: "fields",
    labelKey: "specification.step2",
    fields: [
      { id: "title", control: "text", labelKey: "specification.title", required: true },
      { id: "content", control: "textarea", labelKey: "specification.content", required: true, defaultValueKey: "specification.defaultContent", rows: 12 },
    ],
  },
  {
    id: "review",
    kind: "review",
    labelKey: "specification.step3",
    summaryFields: [{ id: "title", labelKey: "specification.title" }],
  },
] as const satisfies readonly SpecificationStepDefinition[];

/** The registry is the extension point for future API/schema/event documents. */
export const SPECIFICATION_TYPES: readonly SpecificationTypeDefinition[] = [
  {
    kind: "markdown",
    category: "document",
    categoryKey: "specification.categories.document",
    directoryKey: "specification.kinds.markdown",
    titleKey: "specification.types.markdown.title",
    descriptionKey: "specification.types.markdown.description",
    keywordsKey: "specification.types.markdown.keywords",
    icon: FileText,
    enabled: true,
    steps: markdownSteps,
    createRequest: ({ id, serviceId, values }) => ({
      id,
      serviceId,
      kind: "markdown",
      title: values.title?.trim() ?? "",
      content: values.content ?? "",
    }),
  },
  {
    kind: "openapi",
    category: "api",
    categoryKey: "specification.categories.api",
    directoryKey: "specification.kinds.openapi",
    titleKey: "specification.types.openapi.title",
    descriptionKey: "specification.types.openapi.description",
    keywordsKey: "specification.types.openapi.keywords",
    icon: Braces,
    enabled: true,
    steps: markdownSteps.map((step) =>
      step.id === "write-content"
        ? {
            ...step,
            fields: [
              { id: "title", control: "text", labelKey: "specification.title", required: false },
              { id: "content", control: "textarea", labelKey: "specification.openapiContent", required: true, defaultValueKey: "specification.defaultOpenapiContent", rows: 18 },
            ],
          }
        : step,
    ),
    createRequest: ({ id, serviceId, values }) => ({
      id,
      serviceId,
      kind: "openapi",
      title: values.title?.trim() || openApiTitle(values.content ?? ""),
      content: values.content ?? "",
    }),
  },
];

export function specificationType(kind: SpecificationKind): SpecificationTypeDefinition {
  return SPECIFICATION_TYPES.find((definition) => definition.kind === kind) ?? SPECIFICATION_TYPES[0]!;
}
