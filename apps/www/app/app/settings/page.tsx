import { redirect } from "next/navigation";
import { settingsDialogHref } from "@/lib/settings-sections";

export default function SettingsPage() {
  redirect(settingsDialogHref("workspace"));
}
