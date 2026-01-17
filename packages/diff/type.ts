import type { BundledLanguage } from "shiki";

export type RenderSuccessResponse = {
  type: "success";
  id: string;
  code: string;
  lang: string | undefined;
};

export type RenderErrorResponse = {
  type: "error";
  id: number;
  error: string;
  stack?: string;
};

export type WorkerResponse = RenderSuccessResponse | RenderErrorResponse;

export type SupportedLanguages = BundledLanguage | "text";

export type HighlightRequest = {
  id: number;
  code: string;
  lang: BundledLanguage;
  theme: string;
};

export type HighlightResponse = {
  id: number;
  html: string;
};
