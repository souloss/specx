import type { SpecificationKind } from "@/types/domain";
import type { MessageKey } from "@/i18n";

export type SpecificationDraft = { title: string; content: string };
export type SpecificationRequest = SpecificationDraft & { id: string; serviceId: string };

export type SpecificationFieldDefinition = {
  id: keyof SpecificationDraft;
  control: "text" | "markdown";
  labelKey: MessageKey;
  required: boolean;
};

export type SpecificationStepDefinition = {
  id: "type" | "content" | "confirm";
  labelKey: MessageKey;
  fields?: readonly SpecificationFieldDefinition[];
};

export type SpecificationTypeDefinition = {
  kind: SpecificationKind;
  category: "document";
  titleKey: "specification.types.markdown.title";
  descriptionKey: "specification.types.markdown.description";
  keywordsKey: "specification.types.markdown.keywords";
  icon: "markdown";
  enabled: boolean;
  steps: readonly SpecificationStepDefinition[];
  createRequest: (draft: SpecificationDraft & { id: string; serviceId: string }) => SpecificationRequest;
};

const markdownSteps = [
  { id: "type", labelKey: "specification.step1" },
  {
    id: "content",
    labelKey: "specification.step2",
    fields: [
      { id: "title", control: "text", labelKey: "specification.title", required: true },
      { id: "content", control: "markdown", labelKey: "specification.content", required: true },
    ],
  },
  { id: "confirm", labelKey: "specification.step3" },
] as const satisfies readonly SpecificationStepDefinition[];

/** The registry is the extension point for future API/schema/event documents. */
export const SPECIFICATION_TYPES: readonly SpecificationTypeDefinition[] = [
  {
    kind: "markdown",
    category: "document",
    titleKey: "specification.types.markdown.title",
    descriptionKey: "specification.types.markdown.description",
    keywordsKey: "specification.types.markdown.keywords",
    icon: "markdown",
    enabled: true,
    steps: markdownSteps,
    createRequest: (draft) => draft,
  },
];

export function specificationType(kind: SpecificationKind): SpecificationTypeDefinition {
  return SPECIFICATION_TYPES.find((definition) => definition.kind === kind) ?? SPECIFICATION_TYPES[0]!;
}
