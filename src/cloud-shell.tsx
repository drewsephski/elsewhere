<<<<<<< HEAD
import AppHomePage from "@/app/app/page";
import BotWorkspacePage from "@/app/app/bots/[id]/page";
import GroupWorkspacePage from "@/app/app/groups/[id]/page";
import { WorkspaceAuthenticatedFrame } from "@/components/app/workspace/workspace-authenticated-frame";
=======
import { ProductThemeScope } from "@/components/app/product-theme-scope";
import { WorkspaceAppLayout } from "@/components/app/workspace/workspace-app-layout";
import { WorkspaceRouteOutlet } from "@/components/app/workspace/workspace-route-outlet";
import { Toaster } from "@/components/ui/sonner";
>>>>>>> bbbf20e (Document shared workspace route outlet and add shell wiring tests)
import { authClient } from "@/lib/auth-client";
import ApprovalsPage from "@/app/app/approvals/page";
import ComputersPage from "@/app/app/computers/page";
import ResultsPage from "@/app/app/results/page";
import RoutinesPage from "@/app/app/routines/page";
import WorkPage from "@/app/app/work/page";
import { SignInView } from "@/app/sign-in/sign-in-view";
import { Suspense, useEffect, useRef, useState } from "react";
import {
  Navigate,
  Outlet,
  Route,
  Routes,
  useLocation,
} from "react-router-dom";
import { WorkDetailPage } from "./routes/work-detail-page";

interface SessionUser {
  email: string;
}

function SignInRoute() {
  return (
    <Suspense
      fallback={
        <main className="app-shell-bg flex min-h-screen flex-col items-center justify-center px-6">
          <div className="surface-panel w-full max-w-md p-8">
            <p className="text-sm text-muted-foreground" aria-live="polite">
              Loading…
            </p>
          </div>
        </main>
      }
    >
      <SignInView />
    </Suspense>
  );
}

function AuthenticatedWorkspace() {
  const location = useLocation();
  const [user, setUser] = useState<SessionUser | null>(null);
  const [loading, setLoading] = useState(true);
  const hadSessionRef = useRef(false);

  useEffect(() => {
    let cancelled = false;
    async function loadSession() {
      const showBlockingLoader = !hadSessionRef.current;
      if (showBlockingLoader) {
        setLoading(true);
      }
      try {
        const { data } = await authClient.getSession();
        if (cancelled) {
          return;
        }
        if (data?.user?.email) {
          hadSessionRef.current = true;
          setUser({ email: data.user.email });
        } else {
          hadSessionRef.current = false;
          setUser(null);
        }
      } finally {
        if (!cancelled && showBlockingLoader) {
          setLoading(false);
        }
      }
    }
    void loadSession();
    return () => {
      cancelled = true;
    };
  }, [location.pathname, location.search]);

  if (loading) {
    return (
      <div className="app-shell-bg flex min-h-screen items-center justify-center text-sm text-muted-foreground">
        Loading workspace…
      </div>
    );
  }

  if (!user) {
    return <Navigate to="/sign-in" replace state={{ from: location.pathname }} />;
  }

  return (
    <WorkspaceAuthenticatedFrame userEmail={user.email}>
      <Outlet />
    </WorkspaceAuthenticatedFrame>
  );
}

function BotsIndexRedirect() {
  return <Navigate to="/app?create=1" replace />;
}

export default function CloudShell() {
  return (
    <Routes>
      <Route path="/" element={<Navigate to="/app" replace />} />
      <Route path="/sign-in" element={<SignInRoute />} />
      <Route path="/sign-up" element={<SignInRoute />} />
<<<<<<< HEAD
      <Route path="/app" element={<AuthenticatedWorkspace />}>
        <Route index element={<AppHomePage />} />
        <Route path="bots" element={<BotsIndexRedirect />} />
        <Route path="bots/:id" element={<BotWorkspacePage />} />
        <Route path="groups/:id" element={<GroupWorkspacePage />} />
=======
      <Route
        path="/app"
        element={<AuthenticatedWorkspace />}
      >
        <Route index element={<WorkspaceRouteOutlet />} />
        <Route path="bots" element={<BotsIndexRedirect />} />
        <Route path="bots/:id" element={<WorkspaceRouteOutlet />} />
        <Route path="groups/:id" element={<WorkspaceRouteOutlet />} />
>>>>>>> bbbf20e (Document shared workspace route outlet and add shell wiring tests)
        <Route path="computers" element={<ComputersPage />} />
        <Route path="routines" element={<RoutinesPage />} />
        <Route path="approvals" element={<ApprovalsPage />} />
        <Route path="work" element={<WorkPage />} />
        <Route path="work/:id" element={<WorkDetailPage />} />
        <Route path="results" element={<ResultsPage />} />
      </Route>
      <Route path="*" element={<Navigate to="/app" replace />} />
    </Routes>
  );
}
