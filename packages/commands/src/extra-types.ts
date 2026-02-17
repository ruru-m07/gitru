import type z from "zod";
import type { BranchKindSchema, FileStatusKindSchema, UncommittedChangesStrategySchema } from "./types";

export type UncommittedChangesStrategy = z.infer<typeof UncommittedChangesStrategySchema>;

export type BranchKind = z.infer<typeof BranchKindSchema>;

export type FileStatusKind = z.infer<typeof FileStatusKindSchema>;
