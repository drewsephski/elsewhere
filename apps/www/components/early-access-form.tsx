"use client";

import { siteConfig } from "@gptbot/brand";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";

export function EarlyAccessForm() {
  function handleSubmit(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const form = event.currentTarget;
    const email = new FormData(form).get("email");
    const address = typeof email === "string" ? email.trim() : "";
    const params = new URLSearchParams({
      subject: siteConfig.waitlistMailSubject,
      body: address ? `Please add me to the cloud waitlist: ${address}` : "",
    });
    window.location.href = `mailto:${siteConfig.contactEmail}?${params.toString()}`;
  }

  return (
    <form className="flex flex-col gap-4" onSubmit={handleSubmit}>
      <div className="space-y-2 text-left">
        <Label htmlFor="waitlist-email">Work email</Label>
        <Input
          id="waitlist-email"
          name="email"
          type="email"
          placeholder="you@company.com"
          autoComplete="email"
          required
        />
      </div>
      <Button type="submit" size="lg" className="w-full">
        Request early access
      </Button>
      <p className="text-xs text-muted-foreground">
        Opens your mail client for now. Add a server action when you pick a waitlist provider.
      </p>
    </form>
  );
}
