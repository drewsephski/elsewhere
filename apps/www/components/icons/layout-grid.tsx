import { cn } from "@/lib/utils";
import { forwardRef, type SVGProps } from "react";

/** Static 2×2 grid — "manage / browse" navigation affordance. */
export const LayoutGridIcon = forwardRef<SVGSVGElement, SVGProps<SVGSVGElement>>(
  function LayoutGridIcon({ className, ...props }, ref) {
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
        <rect width="7" height="7" x="3" y="3" rx="1.5" />
        <rect width="7" height="7" x="14" y="3" rx="1.5" />
        <rect width="7" height="7" x="14" y="14" rx="1.5" />
        <rect width="7" height="7" x="3" y="14" rx="1.5" />
      </svg>
    );
  },
);

export const LayoutGrid = LayoutGridIcon;
