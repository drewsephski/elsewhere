import { Suspense } from "react";
import { SignInView } from "../sign-in/sign-in-view";

function SignUpFallback() {
  return (
    <main className="app-shell-bg flex min-h-screen flex-col items-center justify-center px-6">
      <div className="surface-panel w-full max-w-md p-8">
        <p className="text-sm text-muted-foreground" aria-live="polite">Loading…</p>
      </div>
    </main>
  );
}

export default function SignUpPage() {
  return (
    <Suspense fallback={<SignUpFallback />}>
      <SignInView />
    </Suspense>
  );
}
