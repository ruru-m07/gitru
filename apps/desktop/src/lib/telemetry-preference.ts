import { useSyncExternalStore } from "react";

const TELEMETRY_CONSENT_KEY = "gitru:anonymous-telemetry";
const subscribers = new Set<() => void>();

export const telemetryConsentFromValue = (value: string | null) =>
  value === "granted";

const readTelemetryConsent = () => {
  try {
    return telemetryConsentFromValue(
      window.localStorage.getItem(TELEMETRY_CONSENT_KEY),
    );
  } catch {
    return false;
  }
};

const subscribe = (subscriber: () => void) => {
  subscribers.add(subscriber);

  const handleStorage = (event: StorageEvent) => {
    if (event.key === TELEMETRY_CONSENT_KEY) subscriber();
  };

  window.addEventListener("storage", handleStorage);
  return () => {
    subscribers.delete(subscriber);
    window.removeEventListener("storage", handleStorage);
  };
};

export const setTelemetryConsent = (granted: boolean) => {
  try {
    window.localStorage.setItem(
      TELEMETRY_CONSENT_KEY,
      granted ? "granted" : "denied",
    );
  } finally {
    for (const subscriber of subscribers) subscriber();
  }
};

export const useTelemetryConsent = () =>
  useSyncExternalStore(subscribe, readTelemetryConsent, () => false);
