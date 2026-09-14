import { AuthShell } from "@/components/auth/auth-shell";
import { Suspense } from "react";
import { ResetPasswordView } from "./reset-password-view";

function ResetPasswordFallback() {
  return (
    <AuthShell title="Choose a new password" description="Loading your reset link…">
      <p className="mt-8 text-sm text-muted-foreground" aria-live="polite">Please wait…</p>
    </AuthShell>
  );
}

export default function ResetPasswordPage() {
  return (
    <Suspense fallback={<ResetPasswordFallback />}>
      <ResetPasswordView />
    </Suspense>
  );
}
