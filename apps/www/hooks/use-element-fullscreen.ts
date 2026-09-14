"use client";

import { useCallback, useEffect, useRef, useState } from "react";

export function useElementFullscreen<T extends HTMLElement>() {
  const ref = useRef<T | null>(null);
  const [isFullscreen, setIsFullscreen] = useState(false);

  useEffect(() => {
    function handleChange() {
      const node = ref.current;
      setIsFullscreen(Boolean(node && document.fullscreenElement === node));
    }
    document.addEventListener("fullscreenchange", handleChange);
    return () => document.removeEventListener("fullscreenchange", handleChange);
  }, []);

  const enter = useCallback(async () => {
    const node = ref.current;
    if (!node) {
      return;
    }
    if (document.fullscreenElement === node) {
      return;
    }
    try {
      await node.requestFullscreen();
    } catch {
      /* user gesture or policy blocked */
    }
  }, []);

  const exit = useCallback(async () => {
    if (!document.fullscreenElement) {
      return;
    }
    try {
      await document.exitFullscreen();
    } catch {
      /* ignore */
    }
  }, []);

  const toggle = useCallback(async () => {
    if (isFullscreen) {
      await exit();
    } else {
      await enter();
    }
  }, [enter, exit, isFullscreen]);

  return { ref, isFullscreen, enter, exit, toggle };
}
