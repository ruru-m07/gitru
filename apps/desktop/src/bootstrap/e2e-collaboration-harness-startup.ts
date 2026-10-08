const STARTUP_TIMEOUT_MS = 25_000;
const STARTUP_RETRY_MS = 100;

// Harness-only bootstrap: native fixture migrations can outlive the manifest
// command's short readiness wait. A transient NotReady must not permanently
// abort installation, but neither a held IPC nor late readiness may extend it.
export function waitForHarnessManifest<T>(
  readManifest: () => Promise<T>,
): Promise<T> {
  return new Promise((resolve, reject) => {
    let settled = false;
    let retry: ReturnType<typeof setTimeout> | undefined;
    const deadline = setTimeout(
      () =>
        fail(
          new Error(
            `Retained native harness startup exceeded ${STARTUP_TIMEOUT_MS}ms`,
          ),
        ),
      STARTUP_TIMEOUT_MS,
    );

    function finish() {
      settled = true;
      clearTimeout(deadline);
      clearTimeout(retry);
    }

    function fail(error: unknown) {
      if (settled) return;
      finish();
      reject(error);
    }

    function attempt() {
      if (settled) return;
      Promise.resolve()
        .then(readManifest)
        .then(
          (manifest) => {
            if (settled) return;
            finish();
            resolve(manifest);
          },
          (error: unknown) => {
            if (settled) return;
            if (
              typeof error === "object" &&
              error !== null &&
              "code" in error &&
              error.code === "not_ready"
            ) {
              retry = setTimeout(attempt, STARTUP_RETRY_MS);
            } else {
              fail(error);
            }
          },
        );
    }

    attempt();
  });
}
