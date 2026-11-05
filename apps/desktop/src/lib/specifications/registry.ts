import type { SpecificationKind } from "@/types/domain";

export type SpecificationTypeDefinition = {
  kind: SpecificationKind;
  category: "document";
  titleKey: "specification.types.markdown.title";
  descriptionKey: "specification.types.markdown.description";
  keywordsKey: "specification.types.markdown.keywords";
  icon: "markdown";
  enabled: boolean;
};

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
  },
];

export function specificationType(kind: SpecificationKind): SpecificationTypeDefinition {
  return SPECIFICATION_TYPES.find((definition) => definition.kind === kind) ?? SPECIFICATION_TYPES[0]!;
}
