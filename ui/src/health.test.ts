import { describe, expect, it, vi } from "vitest";
import { requestHealth } from "./health";

describe("requestHealth", () => {
  it("sends the Sprint 1 command envelope and returns the Rust health value", async () => {
    const invoke = vi.fn().mockResolvedValue({ ok: true, value: { status: "healthy" } });

    await expect(requestHealth(invoke)).resolves.toEqual({ status: "healthy" });
    expect(invoke).toHaveBeenCalledWith("system_health", {
      envelope: expect.objectContaining({
        command: "system.health",
        capability: "WorkspaceRead",
        scope: { resource_ids: [] }
      })
    });
  });
});
