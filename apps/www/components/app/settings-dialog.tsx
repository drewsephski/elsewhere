"use client";

import {
  BotAdvancedSettings,
  BotDeleteSettings,
  BotGeneralSettings,
} from "@/components/app/bot-settings";
import { BotSkillsSettings } from "@/components/app/bot-skills-settings";
import { PermissionPolicyEditor } from "@/components/app/permission-policy-editor";
import { ProviderStatusCard } from "@/components/app/provider-status-card";
import { AnimatedTabs, AnimatedTabsTrigger } from "@/components/ui/animated-tabs";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import type { BotSummary } from "@/lib/api-types";
import {
  SETTINGS_SECTION_ITEMS,
  type SettingsSection,
} from "@/lib/settings-sections";
import { cn } from "cn";
import { useEffect, useState } from "react";

interface SettingsDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  section: SettingsSection;
  onSectionChange: (section: SettingsSection) => void;
  bot: BotSummary | null;
  onBotSaved: (bot: BotSummary) => void;
  onBotDeleted?: () => void;
}

function SettingsPanelHeader({
  title,
  description,
}: {
  title: string;
  description?: string;
}) {
  return (
    <div className="mb-5">
      <h3 className="text-[15px] font-medium tracking-tight">{title}</h3>
      {description ? (
        <p className="mt-1 max-w-xl text-[13px] leading-relaxed text-muted-foreground">
          {description}
        </p>
      ) : null}
    </div>
  );
}

function BotRequiredNotice() {
  return (
    <p className="text-[13px] text-muted-foreground">
      Select a Bot to edit these settings.
    </p>
  );
}

export function SettingsDialog({
  open,
  onOpenChange,
  section,
  onSectionChange,
  bot,
  onBotSaved,
  onBotDeleted,
}: SettingsDialogProps) {
  const [desktop, setDesktop] = useState(true);

  useEffect(() => {
    const media = window.matchMedia("(min-width: 768px)");
    function sync() {
      setDesktop(media.matches);
    }
    sync();
    media.addEventListener("change", sync);
    return () => media.removeEventListener("change", sync);
  }, []);

  const subtitle = bot?.name ?? "Workspace";

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent
        className="flex h-[min(80dvh,760px)] max-h-[80dvh] w-[calc(100%-1.25rem)] max-w-4xl flex-col gap-0 overflow-hidden p-0 sm:max-w-4xl"
      >
        <DialogHeader className="shrink-0 gap-1 border-b border-border px-5 py-3.5 pr-12">
          <DialogTitle className="text-[15px] font-medium">Settings</DialogTitle>
          <DialogDescription className="text-[12px] text-muted-foreground">
            {subtitle}
          </DialogDescription>
        </DialogHeader>

        <div className="flex min-h-0 flex-1 flex-col md:flex-row">
          <nav
            className={cn(
              "shrink-0 border-border",
              desktop ? "w-[11.5rem] border-r px-2 py-2" : "border-b px-3 pt-2",
            )}
          >
            <AnimatedTabs
              value={section}
              onValueChange={(next) => onSectionChange(next as SettingsSection)}
              variant={desktop ? "vertical" : "underline"}
              selection="tab"
              aria-label="Settings sections"
              className={desktop ? "w-full" : "overflow-x-auto"}
            >
              {SETTINGS_SECTION_ITEMS.map((item) => (
                <AnimatedTabsTrigger
                  key={item.value}
                  value={item.value}
                  disabled={item.botScoped && !bot}
                >
                  {item.label}
                </AnimatedTabsTrigger>
              ))}
            </AnimatedTabs>
          </nav>

          <div className="min-h-0 min-w-0 flex-1 overflow-y-auto px-5 py-4">
            {section === "general" ? (
              bot ? (
                <>
                  <SettingsPanelHeader
                    title="This Bot"
                    description="How this teammate shows up and what it is responsible for."
                  />
                  <BotGeneralSettings bot={bot} onSaved={onBotSaved} />
                </>
              ) : (
                <BotRequiredNotice />
              )
            ) : null}

            {section === "permissions" ? (
              bot ? (
                <>
                  <SettingsPanelHeader
                    title="Permissions"
                    description="Applies whenever this Bot works on its own, including Routines."
                  />
                  <PermissionPolicyEditor
                    endpoint={`/v1/bots/${bot.id}/permission-policies`}
                    mode="bot"
                    hideHeading
                  />
                </>
              ) : (
                <BotRequiredNotice />
              )
            ) : null}

            {section === "skills" ? (
              bot ? (
                <>
                  <SettingsPanelHeader
                    title="Skills"
                    description="Attached Skills run with this Bot. Pin a version to freeze behavior."
                  />
                  <BotSkillsSettings botId={bot.id} />
                </>
              ) : (
                <BotRequiredNotice />
              )
            ) : null}

            {section === "workspace" ? (
              <>
                <SettingsPanelHeader
                  title="Team rules"
                  description="Default permission rules for every Bot. Per-Bot settings can override these."
                />
                <PermissionPolicyEditor
                  endpoint="/v1/settings/permission-policies"
                  mode="owner"
                  hideHeading
                />
              </>
            ) : null}

            {section === "chatgpt" ? (
              <>
                <SettingsPanelHeader
                  title="ChatGPT"
                  description="Power Bots with your ChatGPT plan. Elsewhere never switches to paid API keys automatically."
                />
                <ProviderStatusCard variant="plain" />
              </>
            ) : null}

            {section === "advanced" ? (
              bot ? (
                <div className="space-y-8">
                  <div>
                    <SettingsPanelHeader
                      title="Model & workspace"
                      description="Power-user options. Most teams can leave these on the defaults."
                    />
                    <BotAdvancedSettings bot={bot} onSaved={onBotSaved} />
                  </div>
                  <div className="border-t border-border pt-6">
                    <SettingsPanelHeader title="Delete this Bot" />
                    <BotDeleteSettings bot={bot} onDeleted={onBotDeleted} />
                  </div>
                </div>
              ) : (
                <BotRequiredNotice />
              )
            ) : null}
          </div>
        </div>
      </DialogContent>
    </Dialog>
  );
}
