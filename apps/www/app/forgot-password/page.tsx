"use client";

import { AuthShell } from "@/components/auth/auth-shell";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { authClient } from "@/lib/auth-client";
import { publicAppOrigin } from "@/lib/auth.shared";
import Link from "next/link";
import { useState } from "react";

export default function ForgotPasswordPage() {
  const [email, setEmail] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [pending, setPending] = useState(false);
  const [sent, setSent] = useState(false);

  async function handleSubmit(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setError(null);
    setPending(true);
    try {
      const redirectTo = `${publicAppOrigin()}/reset-password`;
      const result = await authClient.requestPasswordReset({ email, redirectTo });
      if (result.error) {
        setError(result.error.message ?? "Could not send reset email");
        return;
      }
      setSent(true);
    } finally {
      setPending(false);
    }
  }

  return (
    <AuthShell
      title="Forgot password"
      description="Enter the email for your account and we will send a link to reset your password."
    >
      {sent ? (
        <div className="mt-8 space-y-4">
          <p className="text-sm text-muted-foreground" role="status">
            If an account exists for that email, you will receive a reset link shortly. Check your
            inbox and spam folder.
          </p>
          <Link
            href="/sign-in"
            className="block text-center text-sm text-primary underline-offset-4 hover:underline"
          >
            Back to sign in
          </Link>
        </div>
      ) : (
        <form className="mt-8 space-y-4" onSubmit={handleSubmit}>
          <div className="space-y-2">
            <Label htmlFor="email">Email</Label>
            <Input
              id="email"
              type="email"
              autoComplete="email"
              required
              value={email}
              onChange={(e) => setEmail(e.target.value)}
            />
          </div>
          {error ? (
            <p className="text-sm text-red-700" role="alert">
              {error}
            </p>
          ) : null}
          <Button type="submit" className="w-full" disabled={pending}>
            {pending ? "Sending…" : "Send reset link"}
          </Button>
          <Link
            href="/sign-in"
            className="block text-center text-sm text-primary underline-offset-4 hover:underline"
          >
            Back to sign in
          </Link>
        </form>
      )}
    </AuthShell>
  );
}
