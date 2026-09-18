import { ProductThemeScope } from "@/components/app/product-theme-scope";
import { auth } from "@/lib/auth";
import { headers } from "next/headers";
import { redirect } from "next/navigation";
import { Suspense } from "react";
import { PairMacView } from "./pair-mac-view";

function pairingNextPath(pairingId?: string, userCode?: string) {
  const params = new URLSearchParams();
  if (pairingId) {
    params.set("pairingId", pairingId);
  }
  if (userCode) {
    params.set("userCode", userCode);
  }
  const query = params.toString();
  return query ? `/pair/mac?${query}` : "/pair/mac";
}

export default async function PairMacPage({
  searchParams,
}: {
  searchParams: Promise<{ pairingId?: string; userCode?: string }>;
}) {
  const session = await auth.api.getSession({
    headers: await headers(),
  });
  const params = await searchParams;
  if (!session) {
    redirect(`/sign-in?next=${encodeURIComponent(pairingNextPath(params.pairingId, params.userCode))}`);
  }

  return (
    <>
      <ProductThemeScope />
      <Suspense
        fallback={
          <main className="app-shell-bg flex min-h-screen items-center justify-center px-6">
            <p className="text-sm text-muted-foreground">Loading…</p>
          </main>
        }
      >
        <PairMacView />
      </Suspense>
    </>
  );
}
