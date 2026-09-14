import { cn } from "@/lib/utils";
import { forwardRef, type SVGProps } from "react";

/** Static plus — no hover animation (add/create affordances). */
export const PlusIcon = forwardRef<SVGSVGElement, SVGProps<SVGSVGElement>>(
  function PlusIcon({ className, ...props }, ref) {
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
        <path d="M5 12h14" />
        <path d="M12 5v14" />
      </svg>
    );
  },
);

export const Plus = PlusIcon;
