import {
  useLocation,
  useNavigate,
  useSearchParams as useRouterSearchParams,
} from "react-router-dom";
import { useCallback, useMemo } from "react";

export function usePathname(): string {
  return useLocation().pathname;
}

export function useRouter() {
  const navigate = useNavigate();
  return useMemo(
    () => ({
      push: (href: string) => {
        navigate(href);
      },
      replace: (href: string) => {
        navigate(href, { replace: true });
      },
      refresh: () => {
        navigate(0);
      },
      back: () => {
        navigate(-1);
      },
    }),
    [navigate],
  );
}

export function useSearchParams(): URLSearchParams {
  const [params] = useRouterSearchParams();
  return params;
}

export function redirect(url: string): never {
  throw new RedirectError(url);
}

export class RedirectError extends Error {
  url: string;

  constructor(url: string) {
    super(`Redirect to ${url}`);
    this.url = url;
  }
}

export function useRedirectOnThrow() {
  const navigate = useNavigate();
  return useCallback(
    (error: unknown) => {
      if (error instanceof RedirectError) {
        navigate(error.url, { replace: true });
        return true;
      }
      return false;
    },
    [navigate],
  );
}
