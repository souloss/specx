import type { MessageKey } from "@/i18n";
import type { SpecificationKind } from "@/types/domain";
import type { SpecificationRefKind } from "@/types/api";
import type { ResourceTreeNode } from "@/types/navigation";
import type { Component } from "vue";

type SpecificationRequest = {
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

type SpecificationStepDefinition = {
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
  file: {
    extensions: readonly string[];
    mimeTypes: readonly string[];
    inspect?: (content: string, extension: string) => { title: string; format?: "json" | "yaml" };
  };
  buildNavigation?: (node: ResourceTreeNode, content: string) => ResourceTreeNode;
  createRequest: (draft: { id: string; serviceId: string; values: Record<string, string> }) => SpecificationRequest;
};

export function specificationCreationSteps(fields: readonly SpecificationFieldDefinition[]): readonly SpecificationStepDefinition[] {
  return [
    { id: "select-type", kind: "type-picker", labelKey: "specification.step1" },
    { id: "write-content", kind: "fields", labelKey: "specification.step2", fields },
    { id: "review", kind: "review", labelKey: "specification.step3", summaryFields: [{ id: "title", labelKey: "specification.title" }] },
  ];
}
