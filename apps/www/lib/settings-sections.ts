export const SETTINGS_SECTIONS = [
  "general",
  "permissions",
  "skills",
  "workspace",
  "chatgpt",
  "advanced",
] as const;

export type SettingsSection = (typeof SETTINGS_SECTIONS)[number];

export const SETTINGS_SECTION_ITEMS: readonly {
  value: SettingsSection;
  label: string;
  botScoped: boolean;
}[] = [
  { value: "general", label: "This Bot", botScoped: true },
  { value: "permissions", label: "Permissions", botScoped: true },
  { value: "skills", label: "Skills", botScoped: true },
  { value: "workspace", label: "Team rules", botScoped: false },
  { value: "chatgpt", label: "ChatGPT", botScoped: false },
  { value: "advanced", label: "Advanced", botScoped: true },
];

export function parseSettingsSection(raw: string | null | undefined): SettingsSection | null {
  if (!raw) {
    return null;
  }
  return SETTINGS_SECTIONS.find((section) => section === raw) ?? null;
}

export function settingsDialogHref(section: SettingsSection = "workspace"): string {
  return `/app?settings=${section}`;
}
