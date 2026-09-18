"use client";

import type { Variants } from "motion/react";
import { motion, useAnimation } from "motion/react";
import type { HTMLAttributes } from "react";
import { forwardRef, useCallback, useImperativeHandle, useRef } from "react";

import { cn } from "@/lib/utils";

export interface PictureInPicture2IconHandle {
  startAnimation: () => void;
  stopAnimation: () => void;
}

interface PictureInPicture2IconProps extends HTMLAttributes<HTMLDivElement> {
  size?: number;
}

const PIP_VARIANTS: Variants = {
  normal: {
    x: 0,
    y: 0,
    scale: 1,
  },
  animate: {
    x: [0, 1.5, 0],
    y: [0, -1.5, 0],
    scale: [1, 1.08, 1],
    transition: {
      duration: 0.45,
      ease: "easeInOut",
    },
  },
};

const PictureInPicture2Icon = forwardRef<
  PictureInPicture2IconHandle,
  PictureInPicture2IconProps
>(({ onMouseEnter, onMouseLeave, className, size = 28, ...props }, ref) => {
  const controls = useAnimation();
  const isControlledRef = useRef(false);

  useImperativeHandle(ref, () => {
    isControlledRef.current = true;
    return {
      startAnimation: () => controls.start("animate"),
      stopAnimation: () => controls.start("normal"),
    };
  });

  const handleMouseEnter = useCallback(
    (e: React.MouseEvent<HTMLDivElement>) => {
      if (!isControlledRef.current) controls.start("animate");
      onMouseEnter?.(e);
    },
    [controls, onMouseEnter],
  );

  const handleMouseLeave = useCallback(
    (e: React.MouseEvent<HTMLDivElement>) => {
      if (!isControlledRef.current) controls.start("normal");
      onMouseLeave?.(e);
    },
    [controls, onMouseLeave],
  );

  return (
    <div
      className={cn(className)}
      onMouseEnter={handleMouseEnter}
      onMouseLeave={handleMouseLeave}
      {...props}
    >
      <svg
        fill="none"
        height={size}
        stroke="currentColor"
        strokeLinecap="round"
        strokeLinejoin="round"
        strokeWidth="2"
        style={{ overflow: "visible" }}
        viewBox="0 0 24 24"
        width={size}
        xmlns="http://www.w3.org/2000/svg"
      >
        <path d="M21 9V6a2 2 0 0 0-2-2H4a2 2 0 0 0-2 2v10c0 1.1.9 2 2 2h4" />
        <motion.rect
          animate={controls}
          height="7"
          rx="2"
          variants={PIP_VARIANTS}
          width="10"
          x="12"
          y="13"
        />
      </svg>
    </div>
  );
});

PictureInPicture2Icon.displayName = "PictureInPicture2Icon";

export { PictureInPicture2Icon };
