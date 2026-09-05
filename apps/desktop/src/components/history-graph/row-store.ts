import type { GraphRow } from "@gitru/commands";
import { type ProcessedRow, processeRows } from "./helper";

export class HistoryGraphRowStore {
  readonly rows: ProcessedRow[] = [];
  maxLane = 0;

  sync(nextRows: readonly GraphRow[]) {
    const prefixChanged =
      nextRows.length < this.rows.length ||
      this.rows.some(
        (processed, index) =>
          index < nextRows.length && processed.row.oid !== nextRows[index]?.oid,
      );

    if (prefixChanged) {
      this.rows.length = 0;
      this.maxLane = 0;
    }

    let existingRowChanged = false;
    for (
      let index = 0;
      index < Math.min(this.rows.length, nextRows.length);
      index++
    ) {
      const nextRow = nextRows[index];
      if (!nextRow || this.rows[index]?.row === nextRow) continue;
      const processed = processeRows([nextRow])[0];
      if (!processed) continue;
      this.rows[index] = processed;
      existingRowChanged = true;
    }

    const appended = nextRows.slice(this.rows.length);
    for (const processed of processeRows([...appended])) {
      this.rows.push(processed);
      this.maxLane = Math.max(
        this.maxLane,
        processed.row.input_swimlanes.length - 1,
        processed.row.output_swimlanes.length - 1,
      );
    }

    if (existingRowChanged) {
      this.maxLane = this.rows.reduce(
        (maximum, processed) =>
          Math.max(
            maximum,
            processed.row.input_swimlanes.length - 1,
            processed.row.output_swimlanes.length - 1,
          ),
        0,
      );
    }
  }
}
