import {
  Link as RouterLink,
  type LinkProps as RouterLinkProps,
} from "react-router-dom";
import type { AnchorHTMLAttributes, ReactNode } from "react";

type NextLinkProps = Omit<AnchorHTMLAttributes<HTMLAnchorElement>, "href"> & {
  href: string;
  children?: ReactNode;
  prefetch?: boolean;
  replace?: boolean;
  scroll?: boolean;
};

export default function Link({
  href,
  replace,
  children,
  prefetch: _prefetch,
  scroll: _scroll,
  ...rest
}: NextLinkProps) {
  const to = href;
  const routerProps: RouterLinkProps = replace ? { replace: true, to } : { to };
  return (
    <RouterLink {...routerProps} {...rest}>
      {children}
    </RouterLink>
  );
}
