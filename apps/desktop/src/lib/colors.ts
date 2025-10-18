export const Colors = [
  { key: "dark-classic", name: "Dark" },
  { key: "light", name: "Light" },
  { key: "dark-claude", name: "Claude Dark" },
  { key: "light-claude", name: "Claude Light" },
  { key: "dark-caffeine", name: "Caffeine Dark" },
  { key: "light-caffeine", name: "Caffeine Light" },
  { key: "dark-supabase", name: "Supabase" },
  { key: "dark-t3-chat", name: "T3 Chat" },
  { key: "dark-graphite", name: "Graphite" },
] as const;

export type ColorsType = (typeof Colors)[number]["key"];

export const colorKeyList = Colors.map((color) => color.key) as ColorsType[];
