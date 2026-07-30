import z from "zod";

export const CoAuthers = z.array(z.tuple([z.string(), z.string()]));
export type CoAuthers = z.infer<typeof CoAuthers>;