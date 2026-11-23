import type { SpecificationKind } from "@/types/domain";
import { markdownSpecification } from "./markdown/definition";
import { openApiSpecification } from "./openapi/definition";
import type { SpecificationTypeDefinition } from "./definition";

export type { SpecificationFieldDefinition, SpecificationTypeDefinition } from "./definition";

export const SPECIFICATION_TYPES: readonly SpecificationTypeDefinition[] = [markdownSpecification, openApiSpecification];

export function specificationType(kind: SpecificationKind): SpecificationTypeDefinition {
  return SPECIFICATION_TYPES.find((definition) => definition.kind === kind) ?? markdownSpecification;
}
