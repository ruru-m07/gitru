export const IS_QUALIFICATION_BUILD =
  import.meta.env.MODE === "qualification" || import.meta.env.MODE === "e2e";
