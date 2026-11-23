import { Layers3 } from "@lucide/vue";
import { translate } from "@/i18n";
import type { ResourceTreeNode } from "@/types/navigation";
import { getOpenApiOperations, parseOpenApiContent } from "./contract/openapi";
import { getOpenApiModels } from "./contract/openapiModels";

export function buildOpenApiNavigation(node: ResourceTreeNode, content: string): ResourceTreeNode {
  const { id, serviceId, specificationId, providerKind, providerIcon } = node;
  const context = { serviceId, specificationId, providerKind, providerIcon };
  const tags = new Map<string, ResourceTreeNode & { children: ResourceTreeNode[] }>();
  for (const operation of getOpenApiOperations(content)) {
    for (const tag of operation.tags) {
      const tagId = `${id}:tag:${tag}`;
      const tagNode = tags.get(tag) ?? { ...context, id: tagId, label: tag, type: "directory", parentId: id, children: [] };
      tagNode.children.push({
        ...context,
        id: `${id}:operation:${operation.identityKey}:${tag}`,
        label: `${operation.method.toUpperCase()} ${operation.path}`,
        type: "leaf",
        parentId: tagId,
        operationKey: operation.identityKey,
        searchText: `${operation.operation.summary ?? ""} ${operation.operation.operationId ?? ""}`,
      });
      tags.set(tag, tagNode);
    }
  }

  const modelsId = `${id}:models`;
  const modelContext = { ...context, providerIcon: Layers3 };
  const models: ResourceTreeNode = {
    ...modelContext,
    id: modelsId,
    label: translate("specification.openapiModels.directory"),
    type: "directory",
    parentId: id,
    modelsDirectory: true,
    children: getOpenApiModels(parseOpenApiContent(content)).map((model) => ({
      ...modelContext,
      id: `${modelsId}:${model.reference}`,
      label: model.name,
      type: "leaf",
      parentId: modelsId,
      modelRef: model.reference,
      searchText: `${model.description} ${JSON.stringify(model.schema)}`,
    })),
  };
  return { ...node, type: "directory", children: [...[...tags.values()].sort((a, b) => a.label.localeCompare(b.label)), models] };
}
