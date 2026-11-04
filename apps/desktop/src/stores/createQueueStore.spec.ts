import { beforeEach, describe, expect, it, vi } from "vitest";
import { createPinia, setActivePinia } from "pinia";
import { nextTick } from "vue";
import { useCreateQueueStore } from "./createQueueStore";
import { useWorkspaceStore } from "./workspace";
import { specxApi } from "@/lib/backend/client";
import { catalogFixture } from "@/test/catalogFixture";
import { installMemoryStorage } from "@/test/browserStorage";

vi.mock("@/lib/backend/client", async (importOriginal) => ({
  ...(await importOriginal<typeof import("@/lib/backend/client")>()),
  specxApi: { createGroup: vi.fn(), createService: vi.fn() },
}));
beforeEach(() => {
  installMemoryStorage();
  vi.resetAllMocks();
  setActivePinia(createPinia());
});
const input = { id: "new-group", workspaceId: "workspace", kind: "group" as const, name: "New group", parentId: "platform" };
const confirmed = { id: input.id, name: input.name, parentId: input.parentId, position: 3 };
function setup() {
  const workspace = useWorkspaceStore();
  workspace.catalog = catalogFixture();
  workspace.authenticated = true;
  return { workspace, queue: useCreateQueueStore() };
}
function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: Error) => void;
  const promise = new Promise<T>((done, fail) => {
    resolve = done;
    reject = fail;
  });
  return { promise, resolve, reject };
}
describe("recoverable create queue", () => {
  it("shows a pending item immediately, deduplicates submissions and merges the server result", async () => {
    const { workspace, queue } = setup();
    const response = deferred<typeof confirmed>();
    vi.mocked(specxApi.createGroup).mockReturnValueOnce(response.promise);
    queue.enqueue(input);
    queue.enqueue(input);
    const waiting = queue.retry(input.id);
    expect(queue.current[0]?.status).toBe("saving");
    expect(workspace.catalog!.groups).toHaveLength(2);
    expect(specxApi.createGroup).toHaveBeenCalledOnce();
    response.resolve({ ...confirmed, name: "Canonical name" });
    await waiting;
    expect(queue.current).toEqual([]);
    expect(workspace.catalog!.groups.at(-1)).toEqual({ ...confirmed, name: "Canonical name" });
  });
  it("keeps failed input across reload and retries using the same idempotency key", async () => {
    const { queue } = setup();
    vi.mocked(specxApi.createGroup).mockRejectedValueOnce(new Error("offline"));
    queue.enqueue(input);
    await queue.retry(input.id);
    await nextTick();
    expect(queue.current[0]).toMatchObject({ name: input.name, status: "failed", error: "offline" });
    const firstArguments = vi.mocked(specxApi.createGroup).mock.calls[0];
    setActivePinia(createPinia());
    const restored = setup();
    expect(restored.queue.current[0]).toMatchObject({ name: input.name, status: "failed" });
    vi.mocked(specxApi.createGroup).mockResolvedValueOnce(confirmed);
    await restored.queue.retry(input.id);
    expect(vi.mocked(specxApi.createGroup).mock.calls[1]).toEqual(firstArguments);
    expect(restored.workspace.catalog!.groups.at(-1)?.id).toBe(input.id);
  });
  it("preserves a service validator and canonical fields without a catalog refresh", async () => {
    const { workspace, queue } = setup();
    vi.mocked(specxApi.createService).mockResolvedValueOnce({ id: "new-service", name: "API", slug: "new-service", groupId: null, description: "", tagIds: [], updatedAt: "2026-11-04T00:00:00Z", archived: false, etag: '"server-version"' });
    queue.enqueue({ ...input, kind: "resource", id: "new-service", name: "API", parentId: null });
    await queue.retry("new-service");
    expect(workspace.catalog!.services.at(-1)).toMatchObject({ id: "new-service", etag: '"server-version"', createdAt: "2026-11-04T00:00:00Z" });
    expect(queue.current).toHaveLength(0);
  });
  it("does not apply a late create to another workspace", async () => {
    const { workspace, queue } = setup();
    const response = deferred<typeof confirmed>();
    vi.mocked(specxApi.createGroup).mockReturnValueOnce(response.promise);
    queue.enqueue(input);
    const waiting = queue.retry(input.id);
    workspace.catalog = { ...catalogFixture(), workspace: { id: "other", name: "Other", tenantName: "tenant" } };
    response.resolve(confirmed);
    await waiting;
    expect(workspace.catalog.groups).toHaveLength(2);
    expect(queue.current).toEqual([]);
  });
  it("invalidates late responses after sign-out and retains a recoverable draft", async () => {
    const { workspace, queue } = setup();
    const response = deferred<typeof confirmed>();
    vi.mocked(specxApi.createGroup).mockReturnValueOnce(response.promise);
    queue.enqueue(input);
    const waiting = queue.retry(input.id);
    workspace.authenticated = false;
    workspace.catalog = null;
    response.resolve(confirmed);
    await waiting;
    workspace.catalog = catalogFixture();
    expect(workspace.catalog.groups).toHaveLength(2);
    expect(queue.current[0]?.status).toBe("failed");
  });
  it("reconciles a failed response when a later catalog confirms the write", async () => {
    const { workspace, queue } = setup();
    vi.mocked(specxApi.createGroup).mockRejectedValueOnce(new Error("response lost"));
    queue.enqueue(input);
    await queue.retry(input.id);
    workspace.catalog = { ...workspace.catalog!, groups: [...workspace.catalog!.groups, confirmed] };
    await nextTick();
    expect(queue.current).toEqual([]);
    expect(specxApi.createGroup).toHaveBeenCalledOnce();
  });
  it("never rolls back unrelated confirmed creates when another request fails", async () => {
    const { workspace, queue } = setup();
    const response = deferred<typeof confirmed>();
    vi.mocked(specxApi.createGroup)
      .mockReturnValueOnce(response.promise)
      .mockResolvedValueOnce({ ...confirmed, id: "second" });
    queue.enqueue(input);
    queue.enqueue({ ...input, id: "second" });
    await queue.retry("second");
    response.reject(new Error("offline"));
    await queue.retry(input.id);
    expect(workspace.catalog!.groups.some((item) => item.id === "second")).toBe(true);
    expect(queue.current[0]).toMatchObject({ id: input.id, status: "failed" });
    queue.discard(input.id);
    expect(queue.current).toEqual([]);
  });
});
