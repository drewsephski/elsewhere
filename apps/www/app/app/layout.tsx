import { ProductThemeScope } from "@/components/app/product-theme-scope";
import { WorkspaceAppLayout } from "@/components/app/workspace/workspace-app-layout";
import { auth } from "@/lib/auth";
import { headers } from "next/headers";
import { redirect } from "next/navigation";
import { Suspense } from "react";

export default async function AppLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  const session = await auth.api.getSession({
    headers: await headers(),
  });

  if (!session) {
    redirect("/sign-in");
  }

  return (
    <>
      <ProductThemeScope />
      <Suspense
        fallback={
          <div className="app-shell-bg flex min-h-screen items-center justify-center text-sm text-muted-foreground">
            Loading workspace…
          </div>
        }
      >
        <WorkspaceAppLayout userEmail={session.user.email}>
          {children}
        </WorkspaceAppLayout>
      </Suspense>
    </>
  );
}
