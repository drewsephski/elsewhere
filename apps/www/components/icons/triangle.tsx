"use client";

import { cn } from "@/lib/utils";
import { motion, useAnimation } from "motion/react";
import {
  forwardRef,
  useEffect,
  useImperativeHandle,
  useRef,
  type HTMLAttributes,
} from "react";

import type { AnimatedIconHandle } from "./adapt-icon";
import { useParentHoverAnimation } from "./use-parent-hover-animation";

/** Brand mark — lucide-animated has no triangle icon. */
export const Triangle = forwardRef<AnimatedIconHandle, HTMLAttributes<HTMLDivElement>>(
  function Triangle({ className, ...props }, ref) {
    const anchorRef = useRef<HTMLDivElement>(null);
    const iconRef = useRef<AnimatedIconHandle>(null);
    const controls = useAnimation();

    useImperativeHandle(ref, () => ({
      startAnimation: () => {
        iconRef.current?.startAnimation();
      },
      stopAnimation: () => {
        iconRef.current?.stopAnimation();
      },
    }));

    useEffect(() => {
      iconRef.current = {
        startAnimation: () => {
          void controls.start({ scale: 1.08, rotate: 4 });
        },
        stopAnimation: () => {
          void controls.start({ scale: 1, rotate: 0 });
        },
      };
    }, [controls]);

    useParentHoverAnimation(iconRef, anchorRef);

    useEffect(() => {
      void controls.start({ scale: 1, rotate: 0 });
    }, [controls]);

    return (
      <div
        ref={anchorRef}
        className={cn("inline-flex size-5 shrink-0 pointer-events-none", className)}
        {...props}
      >
        <motion.svg
          viewBox="0 0 24 24"
          className="size-full"
          aria-hidden
          animate={controls}
          transition={{ type: "spring", stiffness: 380, damping: 16 }}
        >
          <path d="M12 3 22 21H2 12 3z" fill="currentColor" />
        </motion.svg>
      </div>
    );
  },
);
