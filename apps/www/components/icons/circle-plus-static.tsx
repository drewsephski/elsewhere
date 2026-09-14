import { cn } from "@/lib/utils";
import { forwardRef, type SVGProps } from "react";

/** Static circle-plus — no hover animation. */
export const CirclePlusIcon = forwardRef<SVGSVGElement, SVGProps<SVGSVGElement>>(
  function CirclePlusIcon({ className, ...props }, ref) {
    return (
      <svg
        ref={ref}
        xmlns="http://www.w3.org/2000/svg"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        strokeWidth="2"
        strokeLinecap="round"
        strokeLinejoin="round"
        className={cn("size-4 shrink-0", className)}
        aria-hidden
        {...props}
      >
        <circle cx="12" cy="12" r="10" />
        <path d="M8 12h8" />
        <path d="M12 8v8" />
      </svg>
    );
  },
);

export const CirclePlus = CirclePlusIcon;
