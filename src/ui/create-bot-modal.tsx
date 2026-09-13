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
import { Textarea } from "@/components/ui/textarea";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Alert, AlertDescription } from "@/components/ui/alert";

interface CreateBotModalProps {
  open: boolean;
  name: string;
  systemPrompt: string;
  model: string;
  models: { id: string; displayName: string }[];
  modelsLoading: boolean;
  error: string | null;
  onClose: () => void;
  onNameChange: (v: string) => void;
  onSystemPromptChange: (v: string) => void;
  onModelChange: (v: string) => void;
  onCreate: () => void;
}

export function CreateBotModal({
  open,
  name,
  systemPrompt,
  model,
  models,
  modelsLoading,
  error,
  onClose,
  onNameChange,
  onSystemPromptChange,
  onModelChange,
  onCreate,
}: CreateBotModalProps) {
  return (
    <Dialog open={open} onOpenChange={(next) => !next && onClose()}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>Create a bot</DialogTitle>
          <DialogDescription>
            Give your assistant a name, optional instructions, and a model.
          </DialogDescription>
        </DialogHeader>

        <div className="space-y-4">
          <div className="space-y-1.5">
            <Label htmlFor="create-bot-name">Name</Label>
            <Input
              id="create-bot-name"
              value={name}
              onChange={(e) => onNameChange(e.target.value)}
              placeholder="Research helper"
              autoFocus
            />
          </div>
          <div className="space-y-1.5">
            <Label htmlFor="create-bot-system">System instructions</Label>
            <Textarea
              id="create-bot-system"
              value={systemPrompt}
              onChange={(e) => onSystemPromptChange(e.target.value)}
              rows={4}
              placeholder="How should this bot behave?"
              className="resize-none"
            />
          </div>
          <div className="space-y-1.5">
            <Label htmlFor="create-bot-model">Model</Label>
            <Select
              value={model}
              onValueChange={(value) => {
                if (value) {
                  onModelChange(value);
                }
              }}
              disabled={modelsLoading || models.length === 0}
            >
              <SelectTrigger id="create-bot-model" className="w-full">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {models.map((m) => (
                  <SelectItem key={m.id} value={m.id}>
                    {m.displayName}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>
          {error && (
            <Alert variant="destructive">
              <AlertDescription>{error}</AlertDescription>
            </Alert>
          )}
        </div>

        <DialogFooter>
          <Button
            type="button"
            variant="outline"
            onClick={onClose}
          >
            Cancel
          </Button>
          <Button
            type="button"
            onClick={onCreate}
            disabled={!name.trim() || !model}
          >
            Create bot
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
