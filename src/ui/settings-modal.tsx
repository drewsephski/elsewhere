import { Button } from "@desktop/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@desktop/components/ui/dialog";
import { Input } from "@desktop/components/ui/input";
import { Label } from "@desktop/components/ui/label";
import { Alert, AlertDescription } from "@desktop/components/ui/alert";
import { KeyRound } from "@desktop/components/icons/lucide";

interface SettingsModalProps {
  open: boolean;
  apiKeyConfigured: boolean;
  apiKeyDraft: string;
  elsewhereConnected: boolean;
  elsewherePairing: boolean;
  error: string | null;
  saving: boolean;
  onClose: () => void;
  onApiKeyChange: (value: string) => void;
  onSave: () => void;
  onClear: () => void;
  onConnectElsewhere: () => void;
}

export function SettingsModal({
  open,
  apiKeyConfigured,
  apiKeyDraft,
  elsewhereConnected,
  elsewherePairing,
  error,
  saving,
  onClose,
  onApiKeyChange,
  onSave,
  onClear,
  onConnectElsewhere,
}: SettingsModalProps) {
  return (
    <Dialog open={open} onOpenChange={(next) => !next && onClose()}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <KeyRound className="size-4 text-primary" aria-hidden />
            Settings
          </DialogTitle>
          <DialogDescription>
            Your OpenAI API key is stored in the macOS Keychain. It is never logged
            or written to the local database.
          </DialogDescription>
        </DialogHeader>

        <div className="space-y-3">
          <div className="space-y-1.5">
            <Label htmlFor="openai-api-key">OpenAI API key</Label>
            <Input
              id="openai-api-key"
              type="password"
              value={apiKeyDraft}
              onChange={(e) => onApiKeyChange(e.target.value)}
              placeholder={apiKeyConfigured ? "•••••••• (configured)" : "sk-…"}
              autoComplete="off"
            />
          </div>
          {error && (
            <Alert variant="destructive">
              <AlertDescription>{error}</AlertDescription>
            </Alert>
          )}
          <div className="space-y-1.5 border-t border-border pt-3">
            <p className="text-sm font-medium">Elsewhere account</p>
            <p className="text-xs text-muted-foreground">
              {elsewhereConnected
                ? "This Mac is connected to your hosted Elsewhere account."
                : "Connect this Mac so it can appear as a Computer in your workspace."}
            </p>
            <Button
              type="button"
              variant={elsewhereConnected ? "outline" : "default"}
              onClick={onConnectElsewhere}
              disabled={saving || elsewherePairing}
            >
              {elsewherePairing
                ? "Waiting for approval…"
                : elsewhereConnected
                  ? "Reconnect to Elsewhere"
                  : "Connect to Elsewhere"}
            </Button>
          </div>
        </div>

        <DialogFooter className="gap-2 sm:gap-0">
          {apiKeyConfigured && (
            <Button
              type="button"
              variant="outline"
              onClick={onClear}
              disabled={saving}
            >
              Remove key
            </Button>
          )}
          <Button
            type="button"
            onClick={onSave}
            disabled={saving || !apiKeyDraft.trim()}
          >
            Save key
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
