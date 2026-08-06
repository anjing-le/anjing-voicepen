import type { ThemeJson, ThemeMode } from "./types";

export const themeSchemaExample: ThemeJson = {
  name: "calm",
  colors: {
    background: "#f7f8f4",
    text: "#17201b",
    accent: "#1f8a70",
  },
  radius: 8,
};

export function applyTheme(theme: ThemeMode | string | undefined) {
  const next = theme === "light" || theme === "dark" || theme === "system" ? theme : "system";
  document.documentElement.dataset.theme = next;
}
