/// <reference lib="webworker" />

type PixelDiffInput = {
  beforeUrl: string;
  afterUrl: string;
  width: number;
  height: number;
};

type PixelDiffResult = {
  maskDataUrl: string;
  mismatchRatio: number;
};

const createBitmap = async (url: string) => {
  const response = await fetch(url);
  const blob = await response.blob();
  return await createImageBitmap(blob);
};

self.onmessage = async (event: MessageEvent<PixelDiffInput>) => {
  try {
    const { beforeUrl, afterUrl, width, height } = event.data;
    if (width <= 0 || height <= 0) {
      self.postMessage({ error: "Invalid render size" });
      return;
    }

    const beforeBitmap = await createBitmap(beforeUrl);
    const afterBitmap = await createBitmap(afterUrl);

    const canvas = new OffscreenCanvas(width, height);
    const ctx = canvas.getContext("2d", { willReadFrequently: true });
    if (!ctx) {
      self.postMessage({ error: "2d context unavailable" });
      return;
    }

    ctx.clearRect(0, 0, width, height);
    ctx.drawImage(beforeBitmap, 0, 0, width, height);
    const beforeData = ctx.getImageData(0, 0, width, height);

    ctx.clearRect(0, 0, width, height);
    ctx.drawImage(afterBitmap, 0, 0, width, height);
    const afterData = ctx.getImageData(0, 0, width, height);

    const output = ctx.createImageData(width, height);

    let changedPixels = 0;
    const totalPixels = width * height;
    for (let i = 0; i < beforeData.data.length; i += 4) {
      const dr = Math.abs(beforeData.data[i] - afterData.data[i]);
      const dg = Math.abs(beforeData.data[i + 1] - afterData.data[i + 1]);
      const db = Math.abs(beforeData.data[i + 2] - afterData.data[i + 2]);
      const delta = dr + dg + db;

      if (delta > 18) {
        changedPixels += 1;
        output.data[i] = 255;
        output.data[i + 1] = 94;
        output.data[i + 2] = 58;
        output.data[i + 3] = Math.min(220, 120 + Math.floor(delta / 3));
      } else {
        output.data[i] = 0;
        output.data[i + 1] = 0;
        output.data[i + 2] = 0;
        output.data[i + 3] = 0;
      }
    }

    ctx.clearRect(0, 0, width, height);
    ctx.putImageData(output, 0, 0);

    const blob = await canvas.convertToBlob({ type: "image/png" });
    const base64 = await blob.arrayBuffer().then((buffer) => {
      let binary = "";
      const bytes = new Uint8Array(buffer);
      for (let i = 0; i < bytes.length; i++) {
        binary += String.fromCharCode(bytes[i]);
      }
      return btoa(binary);
    });

    const result: PixelDiffResult = {
      maskDataUrl: `data:image/png;base64,${base64}`,
      mismatchRatio: totalPixels === 0 ? 0 : changedPixels / totalPixels,
    };

    self.postMessage(result);
  } catch (error) {
    const message = error instanceof Error ? error.message : "unknown worker error";
    self.postMessage({ error: message });
  }
};

export {};
