"use client";

import { ProductLogo } from "@/components/product-logo";
import { siteConfig } from "@elsewhere/brand";
import { authClient } from "@/lib/auth-client";
import { authModeFromPathname, authModeToggleHref, safeAuthNextPath } from "@/lib/auth-mode";
import { Button } from "@/components/ui/button";
import { FormFields, FormItem } from "@/components/ui/form-item";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import Link from "next/link";
import { usePathname, useRouter, useSearchParams } from "next/navigation";
import { DesktopTitlebar } from "@/components/app/desktop-titlebar";
import { useState } from "react";

export function SignInView() {
  const router = useRouter();
  const pathname = usePathname();
  const searchParams = useSearchParams();
  const resetSuccess = searchParams.get("reset") === "success";
  const nextPath = safeAuthNextPath(searchParams.get("next")) ?? "/app";
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [name, setName] = useState("");
  const [invitation, setInvitation] = useState("");
  const invitationRequired = process.env.NEXT_PUBLIC_ELSEWHERE_ALPHA_INVITE_REQUIRED === "1";
  const mode = authModeFromPathname(pathname);
  const [error, setError] = useState<string | null>(null);
  const [pending, setPending] = useState(false);

  async function handleEmailSubmit(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setError(null);
    setPending(true);
    try {
      if (mode === "sign-up") {
        const result = await authClient.signUp.email({
          email,
          password,
          name: name.trim() || (email.split("@")[0] ?? "User"),
        }, { headers: invitation ? { "x-elsewhere-invite": invitation } : undefined });
        if (result.error) {
          setError(result.error.message ?? "Sign up failed");
          return;
        }
      } else {
        const result = await authClient.signIn.email({ email, password });
        if (result.error) {
          setError(result.error.message ?? "Sign in failed");
          return;
        }
      }
      router.push(nextPath);
      router.refresh();
    } catch {
      setError(mode === "sign-up" ? "Sign up failed. Please try again." : "Sign in failed. Please try again.");
    } finally {
      setPending(false);
    }
  }

  async function handleGoogleSignIn() {
    setError(null);
    setPending(true);
    try {
      await authClient.signIn.social({
        provider: "google",
        callbackURL: nextPath,
      });
    } catch {
      setError("Google sign-in failed. Please try again.");
    } finally {
      setPending(false);
    }
  }

  const googleEnabled =
    typeof process.env.NEXT_PUBLIC_GOOGLE_AUTH_ENABLED !== "undefined"
      ? process.env.NEXT_PUBLIC_GOOGLE_AUTH_ENABLED === "1"
      : false;

  return (
    <main className="app-shell-bg flex min-h-screen flex-col">
      <DesktopTitlebar />
      <div className="flex flex-1 flex-col items-center justify-center px-6">
      <div className="surface-panel w-full max-w-md p-8">
        <div className="flex items-center gap-2.5">
          <ProductLogo size="md" />
          <span className="text-sm font-semibold tracking-tight text-foreground">
            {siteConfig.productName}
          </span>
        </div>
        <h1 className="mt-2 text-2xl font-semibold tracking-tight text-foreground">
          {mode === "sign-in" ? "Sign in" : "Create account"}
        </h1>
        <p className="mt-2 text-sm text-muted-foreground">
          Sign in to your workspace. You can connect your ChatGPT plan from the overview.
        </p>

        <form className="mt-8" onSubmit={handleEmailSubmit}>
          <FormFields>
            {mode === "sign-up" ? (
              <FormItem>
                <Label htmlFor="name">Name</Label>
                <Input
                  id="name"
                  autoComplete="name"
                  value={name}
                  onChange={(e) => setName(e.target.value)}
                />
              </FormItem>
            ) : null}
            <FormItem>
              <Label htmlFor="email">Email</Label>
              <Input
                id="email"
                type="email"
                autoComplete="email"
                required
                value={email}
                onChange={(e) => setEmail(e.target.value)}
              />
            </FormItem>
            <FormItem>
              <div className="flex items-center justify-between gap-2">
                <Label htmlFor="password">Password</Label>
                {mode === "sign-in" ? (
                  <Link
                    href="/forgot-password"
                    className="text-xs text-primary underline-offset-4 hover:underline"
                  >
                    Forgot password?
                  </Link>
                ) : null}
              </div>
              <Input
                id="password"
                type="password"
                autoComplete={mode === "sign-up" ? "new-password" : "current-password"}
                required
                minLength={8}
                value={password}
                onChange={(e) => setPassword(e.target.value)}
              />
            </FormItem>
            {resetSuccess && mode === "sign-in" ? (
              <p className="text-sm text-muted-foreground" role="status">
                Your password was reset. Sign in with your new password.
              </p>
            ) : null}
            {mode === "sign-up" && invitationRequired ? (
              <FormItem>
                <Label htmlFor="invitation">Alpha invitation code</Label>
                <Input
                  id="invitation"
                  type="password"
                  autoComplete="off"
                  required
                  value={invitation}
                  onChange={(e) => setInvitation(e.target.value)}
                />
              </FormItem>
            ) : null}
            {error ? (
              <p className="text-sm text-red-700" role="alert">
                {error}
              </p>
            ) : null}
            <Button type="submit" className="w-full" disabled={pending}>
              {pending ? "Working…" : mode === "sign-in" ? "Sign in" : "Sign up"}
            </Button>
          </FormFields>
        </form>

        {googleEnabled ? (
          <Button
            type="button"
            variant="outline"
            className="mt-3 w-full"
            disabled={pending}
            onClick={handleGoogleSignIn}
          >
            {pending ? "Working…" : "Continue with Google"}
          </Button>
        ) : null}

        <Link
          href={authModeToggleHref(mode)}
          className="mt-6 inline-block text-sm text-primary underline-offset-4 hover:underline"
        >
          {mode === "sign-in" ? "Need an account? Sign up" : "Already have an account? Sign in"}
        </Link>

        <Link
          href="/"
          className="mt-4 block text-center text-xs text-muted-foreground hover:text-foreground"
        >
          Back to home
        </Link>
      </div>
      </div>
    </main>
  );
}
