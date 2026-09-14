"use client";

import { authClient } from "@/lib/auth-client";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import Link from "next/link";
import { useRouter } from "next/navigation";
import { useState } from "react";

export default function SignInPage() {
  const router = useRouter();
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [name, setName] = useState("");
  const [mode, setMode] = useState<"sign-in" | "sign-up">("sign-in");
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
        });
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
      router.push("/app");
      router.refresh();
    } finally {
      setPending(false);
    }
  }

  async function handleGoogleSignIn() {
    setError(null);
    await authClient.signIn.social({
      provider: "google",
      callbackURL: "/app",
    });
  }

  const googleEnabled =
    typeof process.env.NEXT_PUBLIC_GOOGLE_AUTH_ENABLED !== "undefined"
      ? process.env.NEXT_PUBLIC_GOOGLE_AUTH_ENABLED === "1"
      : false;

  return (
    <main className="app-shell-bg flex min-h-screen flex-col items-center justify-center px-6">
      <div className="surface-panel w-full max-w-md p-8">
        <p className="text-xs font-medium uppercase tracking-[0.2em] text-primary">Elsewhere</p>
        <h1 className="mt-2 text-2xl font-semibold tracking-tight text-foreground">
          {mode === "sign-in" ? "Sign in" : "Create account"}
        </h1>
        <p className="mt-2 text-sm text-muted-foreground">
          Elsewhere accounts are separate from ChatGPT / Codex runner auth.
        </p>

        <form className="mt-8 space-y-4" onSubmit={handleEmailSubmit}>
          {mode === "sign-up" ? (
            <div className="space-y-2">
              <Label htmlFor="name">Name</Label>
              <Input
                id="name"
                autoComplete="name"
                value={name}
                onChange={(e) => setName(e.target.value)}
              />
            </div>
          ) : null}
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
          <div className="space-y-2">
            <Label htmlFor="password">Password</Label>
            <Input
              id="password"
              type="password"
              autoComplete={mode === "sign-up" ? "new-password" : "current-password"}
              required
              minLength={8}
              value={password}
              onChange={(e) => setPassword(e.target.value)}
            />
          </div>
          {error ? (
            <p className="text-sm text-red-700" role="alert">
              {error}
            </p>
          ) : null}
          <Button type="submit" className="w-full" disabled={pending}>
            {pending ? "Working…" : mode === "sign-in" ? "Sign in" : "Sign up"}
          </Button>
        </form>

        {googleEnabled ? (
          <Button
            type="button"
            variant="outline"
            className="mt-3 w-full"
            onClick={handleGoogleSignIn}
          >
            Continue with Google
          </Button>
        ) : null}

        <button
          type="button"
          className="mt-6 text-sm text-primary underline-offset-4 hover:underline"
          onClick={() => setMode(mode === "sign-in" ? "sign-up" : "sign-in")}
        >
          {mode === "sign-in" ? "Need an account? Sign up" : "Already have an account? Sign in"}
        </button>

        <Link
          href="/"
          className="mt-4 block text-center text-xs text-muted-foreground hover:text-foreground"
        >
          Back to home
        </Link>
      </div>
    </main>
  );
}
