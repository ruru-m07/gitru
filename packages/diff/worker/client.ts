import type { HighlightResponse } from "../type";

type Pending = {
  resolve: (result: HighlightResponse) => void;
};

export class ShikiWorkerClient {
  private worker: Worker;
  private id = 0;
  private pending = new Map<number, Pending>();

  constructor() {
    this.worker = new Worker(new URL("./shiki.ts", import.meta.url), {
      type: "module",
    });

    this.worker.onmessage = (e) => {
      const { id, html } = e.data;
      const p = this.pending.get(id);
      if (!p) return;
      this.pending.delete(id);
      p.resolve({
        id,
        html,
      });
    };
  }

  highlight(code: string, lang: string | undefined, theme: string) {
    return new Promise<HighlightResponse>((resolve) => {
      const id = this.id++;

      this.pending.set(id, { resolve });

      this.worker.postMessage({
        id,
        code,
        lang,
        theme,
      });
    });
  }

  terminate() {
    this.worker.terminate();
  }
}
