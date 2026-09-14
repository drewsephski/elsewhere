"use client";

import { useEffect, useRef, type RefObject } from "react";

import type { AnimatedIconHandle } from "./adapt-icon";

function findHoverRoot(anchor: HTMLElement | null): HTMLElement | null {
  if (!anchor) {
    return null;
  }

  let node: HTMLElement | null = anchor.parentElement;
  while (node) {
    const classNames = [...node.classList];
    const isGroup = classNames.some(
      (name) => name === "group" || name.startsWith("group/"),
    );
    if (
      isGroup ||
      node.matches(
        'button, a, [role="button"], [data-slot="button"], [role="menuitem"], [data-slot="dropdown-menu-item"], [data-slot="select-item"]',
      )
    ) {
      return node;
    }
    node = node.parentElement;
  }

  return null;
}

/**
 * Starts/stops lucide-animated icons when the nearest interactive parent is hovered.
 * Falls back to animating when the anchor element itself is hovered.
 */
export function useParentHoverAnimation(
  iconRef: RefObject<AnimatedIconHandle | null>,
  anchorRef: RefObject<HTMLElement | null>,
  enabled = true,
) {
  const hoverRootRef = useRef<HTMLElement | null>(null);

  useEffect(() => {
    if (!enabled) {
      return;
    }

    const anchor = anchorRef.current;
    if (!anchor) {
      return;
    }

    const hoverRoot = findHoverRoot(anchor);
    hoverRootRef.current = hoverRoot;

    function handleEnter() {
      iconRef.current?.startAnimation();
    }

    function handleLeave() {
      iconRef.current?.stopAnimation();
    }

    if (hoverRoot) {
      hoverRoot.addEventListener("mouseenter", handleEnter);
      hoverRoot.addEventListener("mouseleave", handleLeave);
      return () => {
        hoverRoot.removeEventListener("mouseenter", handleEnter);
        hoverRoot.removeEventListener("mouseleave", handleLeave);
      };
    }

    anchor.addEventListener("mouseenter", handleEnter);
    anchor.addEventListener("mouseleave", handleLeave);
    return () => {
      anchor.removeEventListener("mouseenter", handleEnter);
      anchor.removeEventListener("mouseleave", handleLeave);
    };
  }, [anchorRef, enabled, iconRef]);
}
