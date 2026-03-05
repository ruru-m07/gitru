import { computeOdiffDifference } from "@gitru/commands";

export type PixelDiffInput = {
  beforeUrl: string;
  afterUrl: string;
  width: number;
  height: number;
  beforePath?: string;
  afterPath?: string;
};

export type PixelDiffResult = {
  maskDataUrl: string;
  mismatchRatio: number;
};

export interface PixelDiffProvider {
  compute(input: PixelDiffInput): Promise<PixelDiffResult | null>;
}

type OdiffCommandResult = {
  mask_data_url: string | null;
  mismatch_ratio: number | null;
};

export class WorkerPixelDiffProvider implements PixelDiffProvider {
  async compute(input: PixelDiffInput): Promise<PixelDiffResult | null> {
    try {
      const worker = new Worker(
        new URL("./pixelDiff.worker.ts", import.meta.url),
        {
          type: "module",
        },
      );

      const result = await new Promise<PixelDiffResult | null>((resolve) => {
        const timeout = window.setTimeout(() => {
          worker.terminate();
          resolve(null);
        }, 2500);

        worker.onmessage = (
          event: MessageEvent<PixelDiffResult | { error: string }>,
        ) => {
          clearTimeout(timeout);
          worker.terminate();
          if ("error" in event.data) {
            console.warn(
              "[ImageDiff] pixel diff worker failed",
              event.data.error,
            );
            resolve(null);
            return;
          }
          resolve(event.data);
        };

        worker.postMessage(input);
      });

      return result;
    } catch (error) {
      console.warn("[ImageDiff] pixel diff worker setup failed", error);
      return null;
    }
  }
}

export class OdiffNodePixelDiffProvider implements PixelDiffProvider {
  async compute(input: PixelDiffInput): Promise<PixelDiffResult | null> {
    try {
      if (!input.beforePath || !input.afterPath) {
        return null;
      }

      const result = (await computeOdiffDifference({
        beforePath: input.beforePath,
        afterPath: input.afterPath,
      })) as OdiffCommandResult;

      if (!result.mask_data_url) {
        return {
          maskDataUrl: "",
          mismatchRatio: result.mismatch_ratio ?? 0,
        };
      }

      return {
        maskDataUrl: result.mask_data_url,
        mismatchRatio: result.mismatch_ratio ?? 0,
      };
    } catch (error) {
      console.warn("[ImageDiff] odiff provider failed", error);
      return null;
    }
  }
}
