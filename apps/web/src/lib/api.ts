import { treaty } from "@elysiajs/eden";
import type { App } from "../../../api/src/index";

export const api = treaty<App>(import.meta.env.VITE_API_URL as string);
