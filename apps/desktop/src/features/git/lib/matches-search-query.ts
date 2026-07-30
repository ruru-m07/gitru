export const matchesSearchQuery = (value: string, query: string) => {
  const normalizedQuery = query.trim();
  if (!normalizedQuery) return true;
  if (normalizedQuery === "*") return true;

  if (value.toLowerCase().includes(normalizedQuery.toLowerCase())) {
    return true;
  }

  try {
    if (normalizedQuery.startsWith("/") && normalizedQuery.length > 1) {
      const lastSlashIndex = normalizedQuery.lastIndexOf("/");
      if (lastSlashIndex > 0) {
        const pattern = normalizedQuery.slice(1, lastSlashIndex);
        const flags = normalizedQuery.slice(lastSlashIndex + 1) || "i";
        return new RegExp(pattern, flags).test(value);
      }
    }

    return new RegExp(normalizedQuery, "i").test(value);
  } catch {
    try {
      const escapedGlob = normalizedQuery
        .replace(/[.+^${}()|[\]\\]/g, "\\$&")
        .replace(/\*/g, ".*")
        .replace(/\?/g, ".");
      return new RegExp(escapedGlob, "i").test(value);
    } catch {
      return false;
    }
  }
};
