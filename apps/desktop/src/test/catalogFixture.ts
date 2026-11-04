import type { Catalog } from "@/types/domain";

export function catalogFixture(): Catalog {
  return {
    workspace: { id: "workspace", name: "团队", tenantName: "tenant" },
    groups: [
      { id: "platform", name: "平台", parentId: null, position: 0 },
      { id: "auth", name: "身份", parentId: "platform", position: 0 },
    ],
    tags: [{ id: "critical", name: "核心", color: "#2563eb" }],
    services: [
      { id: "svc-b", name: "Service 10", slug: "billing", description: "结算", groupId: "auth", tagIds: ["critical"], updatedAt: "2026-11-03", createdAt: "2026-11-03", etag: '"v1"' },
      { id: "svc-a", name: "Service 2", slug: "accounts", description: "账号", groupId: "auth", tagIds: [], updatedAt: "2026-11-03", createdAt: "2026-11-03", etag: '"v2"' },
    ],
    specifications: [{ id: "spec", serviceId: "svc-a", title: "访问约定", content: "# 约定", kind: "markdown", updatedAt: "2026-11-03" }],
  };
}
