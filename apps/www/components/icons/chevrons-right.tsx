import { cn } from "@/lib/utils";
import { forwardRef, type SVGProps } from "react";

/** Static double chevron — collapse/expand rail affordances. */
export const ChevronsRightIcon = forwardRef<SVGSVGElement, SVGProps<SVGSVGElement>>(
  function ChevronsRightIcon({ className, ...props }, ref) {
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
        <path d="m6 17 5-5-5-5" />
        <path d="m13 17 5-5-5-5" />
      </svg>
    );
  },
);

export const ChevronsLeftIcon = forwardRef<SVGSVGElement, SVGProps<SVGSVGElement>>(
  function ChevronsLeftIcon({ className, ...props }, ref) {
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
        <path d="m11 17-5-5 5-5" />
        <path d="m18 17-5-5 5-5" />
      </svg>
    );
  },
);

export const ChevronsRight = ChevronsRightIcon;
export const ChevronsLeft = ChevronsLeftIcon;
