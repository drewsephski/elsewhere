"use client";

import { useLayoutEffect } from "react";

const PRODUCT_THEME_CLASS = "dark";

/**
 * Scopes the dark product theme to the signed-in workspace by toggling
 * `html.dark`. Portaled surfaces (dialogs, menus, popovers) render on `body`,
 * so the class has to live on the document root rather than on a wrapper.
 *
 * The inline script paints the correct theme on the server-rendered document
 * before hydration (the root `<html>` opts out of attribute hydration warnings
 * for this reason); the effect handles client-side navigation into and out of
 * `/app` (React does not execute inline scripts it inserts itself).
 */
export function ProductThemeScope() {
  useLayoutEffect(() => {
    const root = document.documentElement;
    root.classList.add(PRODUCT_THEME_CLASS);
    return () => {
      root.classList.remove(PRODUCT_THEME_CLASS);
    };
  }, []);

  return (
    <script
      dangerouslySetInnerHTML={{
        __html: `document.documentElement.classList.add(${JSON.stringify(PRODUCT_THEME_CLASS)});`,
      }}
    />
  );
}
