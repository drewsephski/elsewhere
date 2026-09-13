import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { KeyRound } from "lucide-react";

interface SettingsModalProps {
  open: boolean;
  apiKeyConfigured: boolean;
  apiKeyDraft: string;
  error: string | null;
  saving: boolean;
  onClose: () => void;
  onApiKeyChange: (value: string) => void;
  onSave: () => void;
  onClear: () => void;
}

export function SettingsModal({
  open,
  apiKeyConfigured,
  apiKeyDraft,
  error,
  saving,
  onClose,
  onApiKeyChange,
  onSave,
  onClear,
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
