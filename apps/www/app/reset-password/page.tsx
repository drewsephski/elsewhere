"use client";

import { AuthShell } from "@/components/auth/auth-shell";
import { Button } from "@/components/ui/button";
import { FormFields, FormItem } from "@/components/ui/form-item";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { authClient } from "@/lib/auth-client";
import Link from "next/link";
import { useRouter, useSearchParams } from "next/navigation";
import { useMemo, useState } from "react";

export default function ResetPasswordPage() {
  const router = useRouter();
  const searchParams = useSearchParams();
  const token = searchParams.get("token");
  const linkError = searchParams.get("error");

  const [password, setPassword] = useState("");
  const [confirmPassword, setConfirmPassword] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [pending, setPending] = useState(false);

  const invalidLink = useMemo(() => {
    if (linkError === "INVALID_TOKEN") return true;
    return !token;
  }, [linkError, token]);

  async function handleSubmit(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setError(null);

    if (!token) {
      setError("This reset link is invalid or has expired.");
      return;
    }
    if (password !== confirmPassword) {
      setError("Passwords do not match.");
      return;
    }

    setPending(true);
    try {
      const result = await authClient.resetPassword({ newPassword: password, token });
      if (result.error) {
        setError(result.error.message ?? "Could not reset password");
        return;
      }
      router.push("/sign-in?reset=success");
      router.refresh();
    } finally {
      setPending(false);
    }
  }

  if (invalidLink) {
    return (
      <AuthShell
        title="Link expired"
        description="This password reset link is invalid or has expired. Request a new one to continue."
      >
        <div className="mt-8 space-y-4">
          <Link href="/forgot-password">
            <Button type="button" className="w-full">Request a new link</Button>
          </Link>
          <Link
            href="/sign-in"
            className="block text-center text-sm text-primary underline-offset-4 hover:underline"
          >
            Back to sign in
          </Link>
        </div>
      </AuthShell>
    );
  }

  return (
    <AuthShell
      title="Choose a new password"
      description="Enter a new password for your account. You will sign in again after saving it."
    >
      <form className="mt-8" onSubmit={handleSubmit}>
        <FormFields>
        <FormItem>
          <Label htmlFor="password">New password</Label>
          <Input
            id="password"
            type="password"
            autoComplete="new-password"
            required
            minLength={8}
            value={password}
            onChange={(e) => setPassword(e.target.value)}
          />
        </FormItem>
        <FormItem>
          <Label htmlFor="confirm-password">Confirm password</Label>
          <Input
            id="confirm-password"
            type="password"
            autoComplete="new-password"
            required
            minLength={8}
            value={confirmPassword}
            onChange={(e) => setConfirmPassword(e.target.value)}
          />
        </FormItem>
        {error ? (
          <p className="text-sm text-red-700" role="alert">
            {error}
          </p>
        ) : null}
        <Button type="submit" className="w-full" disabled={pending}>
          {pending ? "Saving…" : "Reset password"}
        </Button>
        <Link
          href="/sign-in"
          className="block text-center text-sm text-primary underline-offset-4 hover:underline"
        >
          Back to sign in
        </Link>
        </FormFields>
      </form>
    </AuthShell>
  );
}
