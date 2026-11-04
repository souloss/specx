import { beforeEach, describe, expect, it, vi } from "vitest";
import { createPinia, setActivePinia } from "pinia";
import { useAppUpdateStore } from "./appUpdateStore";
import { checkAppUpdate, getAppReleaseInfo, restartApp, type AppUpdate } from "@/lib/backend/appRelease";
import { installMemoryStorage } from "@/test/browserStorage";
import { trackWrite } from "@/lib/backend/writeActivity";

vi.mock("@/lib/backend/runtime", () => ({ isTauriRuntime: () => true }));
vi.mock("@/lib/backend/appRelease", () => ({ getAppReleaseInfo: vi.fn(), checkAppUpdate: vi.fn(), restartApp: vi.fn() }));

function candidate(): AppUpdate {
  return { version: "0.2.0", download: vi.fn().mockResolvedValue(undefined), install: vi.fn().mockResolvedValue(undefined), close: vi.fn().mockResolvedValue(undefined) };
}

beforeEach(() => {
  installMemoryStorage();
  setActivePinia(createPinia());
  vi.resetAllMocks();
  vi.mocked(getAppReleaseInfo).mockResolvedValue({ version: "0.1.0", updaterConfigured: true });
});

describe("desktop update state", () => {
  it("does not check an unconfigured release source", async () => {
    vi.mocked(getAppReleaseInfo).mockResolvedValue({ version: "0.1.0", updaterConfigured: false });
    const store = useAppUpdateStore();
    await store.check();
    expect(checkAppUpdate).not.toHaveBeenCalled();
    expect(store.configured).toBe(false);
  });

  it("coalesces concurrent checks and retains a known update after a network failure", async () => {
    const update = candidate();
    vi.mocked(checkAppUpdate).mockResolvedValue(update);
    const store = useAppUpdateStore();
    await Promise.all([store.check(), store.check()]);
    expect(checkAppUpdate).toHaveBeenCalledTimes(1);
    vi.mocked(checkAppUpdate).mockRejectedValue(new Error("offline"));
    await store.check();
    expect(store.phase).toBe("available");
    expect(store.latestVersion).toBe("0.2.0");
    expect(store.errorKey).toBe("updates.checkFailed");
    expect(update.close).not.toHaveBeenCalled();
  });

  it("does not enable installation when a Finished event is followed by signature failure", async () => {
    const update = candidate();
    vi.mocked(update.download).mockImplementation(async (onEvent) => {
      onEvent({ event: "Started", data: { contentLength: 100 } });
      onEvent({ event: "Progress", data: { chunkLength: 100 } });
      onEvent({ event: "Finished" });
      throw new Error("signature mismatch");
    });
    vi.mocked(checkAppUpdate).mockResolvedValue(update);
    const store = useAppUpdateStore();
    await store.check();
    await store.download();
    await store.install();
    expect(store.phase).toBe("available");
    expect(store.errorKey).toBe("updates.downloadFailed");
    expect(update.install).not.toHaveBeenCalled();
    expect(restartApp).not.toHaveBeenCalled();
  });

  it("keeps a verified download when a check is requested and closes old resources when replacing a candidate", async () => {
    const first = candidate();
    const second = { ...candidate(), version: "0.3.0" };
    vi.mocked(checkAppUpdate).mockResolvedValueOnce(first).mockResolvedValueOnce(second);
    const store = useAppUpdateStore();
    await store.check();
    await store.check();
    expect(first.close).toHaveBeenCalledOnce();
    await store.download();
    await store.check();
    expect(checkAppUpdate).toHaveBeenCalledTimes(2);
    expect(store.phase).toBe("downloaded");
  });

  it("blocks installation while saves run and releases the write lock after failed installation", async () => {
    const update = candidate();
    vi.mocked(checkAppUpdate).mockResolvedValue(update);
    vi.mocked(update.install).mockRejectedValue(new Error("installer failed"));
    const store = useAppUpdateStore();
    await store.check();
    await store.download();
    let finish!: () => void;
    const write = trackWrite(
      () =>
        new Promise<void>((resolve) => {
          finish = resolve;
        }),
    );
    await store.install();
    expect(update.install).not.toHaveBeenCalled();
    expect(store.errorKey).toBe("updates.pendingWork");
    finish();
    await write;
    await store.install();
    expect(store.phase).toBe("downloaded");
    expect(store.errorKey).toBe("updates.installFailed");
    await expect(trackWrite(async () => "saved")).resolves.toBe("saved");
  });

  it("rejects new writes during installation and lets restart be retried without reinstalling", async () => {
    const update = candidate();
    vi.mocked(checkAppUpdate).mockResolvedValue(update);
    let finish!: () => void;
    vi.mocked(update.install).mockImplementation(
      () =>
        new Promise<void>((resolve) => {
          finish = resolve;
        }),
    );
    const store = useAppUpdateStore();
    await store.check();
    await store.download();
    vi.mocked(restartApp).mockRejectedValueOnce(new Error("blocked")).mockResolvedValueOnce(undefined);
    const installation = store.install();
    expect(store.phase).toBe("installing");
    await expect(trackWrite(async () => "saved")).rejects.toThrow();
    finish();
    await installation;
    expect(store.phase).toBe("restart");
    expect(store.errorKey).toBe("updates.restartFailed");
    await store.restart();
    expect(store.errorKey).toBeNull();
    expect(restartApp).toHaveBeenCalledTimes(2);
    expect(update.install).toHaveBeenCalledOnce();
  });
});
