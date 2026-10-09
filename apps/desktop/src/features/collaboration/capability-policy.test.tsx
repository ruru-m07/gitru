import type { ContextFacetCapability } from "@gitru/collaboration-client";
import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { CapabilityBoundary, ReadOnlyCapability } from "./capability-boundary";
import {
  canMaintainDemand,
  canReadSaved,
  dispatchCapabilityIntent,
  inboxPresentation,
} from "./capability-policy";

const supported: ContextFacetCapability = {
  facet: "issues",
  saved_read: { state: "supported", reason: null },
  synchronize: { state: "supported", reason: null },
  remote_write: { state: "unsupported", reason: "not_implemented" },
  observation: "not_loaded",
  sync: {
    state: "idle",
    last_success_at: null,
    next_retry_at: null,
    error: null,
  },
  can_recheck_access: false,
};

describe("contextual feature policy", () => {
  it("has zero dispatches for unknown, unsupported, denied and unimplemented write intents", async () => {
    const dispatch = vi.fn(async () => undefined);
    const denied: ContextFacetCapability = {
      ...supported,
      saved_read: { state: "unavailable", reason: "permission_denied" },
      synchronize: { state: "unavailable", reason: "permission_denied" },
    };
    const unsupported: ContextFacetCapability = {
      ...supported,
      synchronize: { state: "unsupported", reason: "not_implemented" },
    };
    expect(
      await dispatchCapabilityIntent(undefined, "synchronize", dispatch),
    ).toBe(false);
    expect(
      await dispatchCapabilityIntent(denied, "synchronize", dispatch),
    ).toBe(false);
    expect(
      await dispatchCapabilityIntent(unsupported, "synchronize", dispatch),
    ).toBe(false);
    expect(
      await dispatchCapabilityIntent(supported, "remote_write", dispatch),
    ).toBe(false);
    expect(
      await dispatchCapabilityIntent(denied, "recheck_access", dispatch),
    ).toBe(false);
    expect(
      await dispatchCapabilityIntent(
        { ...unsupported, can_recheck_access: true },
        "recheck_access",
        dispatch,
      ),
    ).toBe(false);
    expect(dispatch).not.toHaveBeenCalled();
    expect(
      await dispatchCapabilityIntent(
        { ...denied, can_recheck_access: true },
        "recheck_access",
        dispatch,
      ),
    ).toBe(true);
    expect(dispatch).toHaveBeenCalledOnce();
  });

  it("allows local saved reads while remote synchronization is temporarily unavailable", () => {
    const offline: ContextFacetCapability = {
      ...supported,
      synchronize: { state: "unavailable", reason: "temporarily_unavailable" },
      observation: "complete",
    };
    expect(canReadSaved(offline)).toBe(true);
    render(
      <CapabilityBoundary policy={offline}>
        <p>Saved authorized text</p>
        <ReadOnlyCapability policy={offline} />
      </CapabilityBoundary>,
    );
    expect(screen.getByText("Saved authorized text")).toBeVisible();
    expect(screen.getByText("Read-only")).toBeVisible();
  });

  it.each([
    "not_loaded",
    "partial",
    "empty",
    "omitted",
    "oversized",
    "complete",
  ] as const)("maintains declared authorized interest for %s evidence", (observation) => {
    expect(canMaintainDemand({ ...supported, observation })).toBe(true);
  });

  it("retains authorized quota/offline interest while explicit sync remains unavailable", async () => {
    const paused: ContextFacetCapability = {
      ...supported,
      synchronize: { state: "unavailable", reason: "temporarily_unavailable" },
    };
    expect(canMaintainDemand(paused)).toBe(true);
    const dispatch = vi.fn();
    expect(
      await dispatchCapabilityIntent(paused, "synchronize", dispatch),
    ).toBe(false);
    expect(dispatch).not.toHaveBeenCalled();
  });

  it("never maintains automatic interest for denied, unknown, unsupported or disconnected policy", () => {
    expect(canMaintainDemand(undefined)).toBe(false);
    for (const reason of [
      "permission_denied",
      "authentication_required",
      "not_implemented",
    ] as const) {
      expect(
        canMaintainDemand({
          ...supported,
          synchronize: { state: "unavailable", reason },
        }),
      ).toBe(false);
      expect(
        canMaintainDemand({
          ...supported,
          saved_read: { state: "unavailable", reason },
        }),
      ).toBe(false);
    }
    expect(
      canMaintainDemand({
        ...supported,
        synchronize: { state: "unsupported", reason: "not_implemented" },
      }),
    ).toBe(false);
    expect(
      canMaintainDemand({
        ...supported,
        synchronize: { state: "unavailable", reason: "not_observed" },
      }),
    ).toBe(false);
  });

  it("does not mount provider content before policy grants a saved read", () => {
    const read = vi.fn();
    function ProviderContent() {
      read();
      return <p>Private provider content</p>;
    }
    const { rerender } = render(
      <CapabilityBoundary policy={undefined}>
        <ProviderContent />
      </CapabilityBoundary>,
    );
    expect(read).not.toHaveBeenCalled();
    rerender(
      <CapabilityBoundary
        policy={{
          ...supported,
          saved_read: { state: "unavailable", reason: "permission_denied" },
        }}
      >
        <ProviderContent />
      </CapabilityBoundary>,
    );
    expect(screen.getByText("Access denied")).toBeVisible();
    expect(read).not.toHaveBeenCalled();
    rerender(
      <CapabilityBoundary policy={supported}>
        <ProviderContent />
      </CapabilityBoundary>,
    );
    expect(screen.getByText("Private provider content")).toBeVisible();
    expect(read).toHaveBeenCalledOnce();
  });

  it("keeps to-do states and badge meaning distinct from native unread notifications", () => {
    expect(inboxPresentation("todos")).toMatchObject({
      title: "To-dos",
      initialState: "pending",
      badgeState: "pending",
      badgeMeaning: "pending to-dos",
    });
    expect(
      inboxPresentation("todos").filters.map((filter) => filter.value),
    ).toEqual(["pending", "done", "all"]);
    expect(inboxPresentation("native_notifications")).toMatchObject({
      initialState: "unread",
      badgeState: "unread",
      badgeMeaning: "unread notifications",
    });
    expect(inboxPresentation("none").badgeState).toBeNull();
  });
});
