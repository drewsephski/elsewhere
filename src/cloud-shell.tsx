import { WorkspaceAppLayout } from "@/components/app/workspace/workspace-app-layout";
import { authClient } from "@/lib/auth-client";
import ApprovalsPage from "@/app/app/approvals/page";
import ComputersPage from "@/app/app/computers/page";
import ResultsPage from "@/app/app/results/page";
import RoutinesPage from "@/app/app/routines/page";
import WorkPage from "@/app/app/work/page";
import { SignInView } from "@/app/sign-in/sign-in-view";
import { Suspense, useEffect, useState } from "react";
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

  useEffect(() => {
    let cancelled = false;
    async function loadSession() {
      setLoading(true);
      try {
        const { data } = await authClient.getSession();
        if (cancelled) {
          return;
        }
        if (data?.user?.email) {
          setUser({ email: data.user.email });
        } else {
          setUser(null);
        }
      } finally {
        if (!cancelled) {
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
    <WorkspaceAppLayout userEmail={user.email}>
      <Outlet />
    </WorkspaceAppLayout>
  );
}

function AppHomePage() {
  return null;
}

function BotsIndexRedirect() {
  return <Navigate to="/app?create=1" replace />;
}

export default function CloudShell() {
  return (
    <Routes>
      <Route path="/" element={<Navigate to="/app" replace />} />
      <Route path="/sign-in" element={<SignInRoute />} />
      <Route
        path="/app"
        element={<AuthenticatedWorkspace />}
      >
        <Route index element={<AppHomePage />} />
        <Route path="bots" element={<BotsIndexRedirect />} />
        <Route path="bots/:id" element={<AppHomePage />} />
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
