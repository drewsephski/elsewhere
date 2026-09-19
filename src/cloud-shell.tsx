import { WorkspaceAuthenticatedFrame } from "@/components/app/workspace/workspace-authenticated-frame";
import { WorkspaceRouteOutlet } from "@/components/app/workspace/workspace-route-outlet";
import { authClient } from "@/lib/auth-client";
import ApprovalsPage from "@/app/app/approvals/page";
import ChannelsPage from "@/app/app/channels/page";
import ChannelsSlackCallbackPage from "@/app/app/channels/slack/callback/page";
import ComputersPage from "@/app/app/computers/page";
import ConnectorsPage from "@/app/app/connectors/page";
import ConnectorsGithubCallbackPage from "@/app/app/connectors/github/callback/page";
import ConnectorsMcpCallbackPage from "@/app/app/connectors/mcp/callback/page";
import ResultsPage from "@/app/app/results/page";
import RoutinesPage from "@/app/app/routines/page";
import RoutineDetailPage from "@/app/app/routines/[id]/page";
import SkillsPage from "@/app/app/skills/page";
import SkillDetailPage from "@/app/app/skills/[id]/page";
import WorkPage from "@/app/app/work/page";
import ForgotPasswordPage from "@/app/forgot-password/page";
import ResetPasswordPage from "@/app/reset-password/page";
import { DesktopShellBootstrap } from "@/components/app/desktop-shell-bootstrap";
import { PairMacView } from "@/app/pair/mac/pair-mac-view";
import { SignInView } from "@/app/sign-in/sign-in-view";
import { settingsDialogHref } from "@/lib/settings-sections";
import { Suspense, useEffect, useRef, useState } from "react";
import {
  Navigate,
  Outlet,
  Route,
  Routes,
  useLocation,
} from "react-router-dom";
import { MacCompanionConnect } from "./mac-companion-connect";
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
      <DesktopShellBootstrap hasSession={true} />
      <Outlet />
    </WorkspaceAuthenticatedFrame>
  );
}

function BotsIndexRedirect() {
  return <Navigate to="/app?create=1" replace />;
}

function SettingsRedirect() {
  return <Navigate to={settingsDialogHref("workspace")} replace />;
}

function ComputersRoute() {
  return <ComputersPage headerAction={<MacCompanionConnect />} />;
}

function PairMacRoute() {
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
      <main className="app-shell-bg flex min-h-screen items-center justify-center px-6">
        <p className="text-sm text-muted-foreground">Loading…</p>
      </main>
    );
  }

  if (!user) {
    const next = `/pair/mac${location.search}`;
    return (
      <Navigate
        to={`/sign-in?next=${encodeURIComponent(next)}`}
        replace
      />
    );
  }

  return (
    <Suspense
      fallback={
        <main className="app-shell-bg flex min-h-screen items-center justify-center px-6">
          <p className="text-sm text-muted-foreground">Loading…</p>
        </main>
      }
    >
      <PairMacView />
    </Suspense>
  );
}

export default function CloudShell() {
  return (
    <Routes>
      <Route path="/" element={<Navigate to="/app" replace />} />
      <Route path="/sign-in" element={<SignInRoute />} />
      <Route path="/sign-up" element={<SignInRoute />} />
      <Route path="/forgot-password" element={<ForgotPasswordPage />} />
      <Route path="/reset-password" element={<ResetPasswordPage />} />
      <Route path="/pair/mac" element={<PairMacRoute />} />
      <Route path="/app" element={<AuthenticatedWorkspace />}>
        <Route index element={<WorkspaceRouteOutlet />} />
        <Route path="bots" element={<BotsIndexRedirect />} />
        <Route path="bots/:id" element={<WorkspaceRouteOutlet />} />
        <Route path="groups/:id" element={<WorkspaceRouteOutlet />} />
        <Route path="computers" element={<ComputersRoute />} />
        <Route path="routines" element={<RoutinesPage />} />
        <Route path="routines/:id" element={<RoutineDetailPage />} />
        <Route path="connectors" element={<ConnectorsPage />} />
        <Route
          path="connectors/github/callback"
          element={<ConnectorsGithubCallbackPage />}
        />
        <Route path="connectors/mcp/callback" element={<ConnectorsMcpCallbackPage />} />
        <Route path="channels" element={<ChannelsPage />} />
        <Route path="channels/slack/callback" element={<ChannelsSlackCallbackPage />} />
        <Route path="approvals" element={<ApprovalsPage />} />
        <Route path="work" element={<WorkPage />} />
        <Route path="work/:id" element={<WorkDetailPage />} />
        <Route path="results" element={<ResultsPage />} />
        <Route path="skills" element={<SkillsPage />} />
        <Route path="skills/:id" element={<SkillDetailPage />} />
        <Route path="settings" element={<SettingsRedirect />} />
      </Route>
      <Route path="*" element={<Navigate to="/app" replace />} />
    </Routes>
  );
}
