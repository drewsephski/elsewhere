import { cn } from "cn"

import { IconPlaceholder } from "@/components/icons/icon-placeholder"

type SpinnerProps = Omit<React.ComponentProps<typeof IconPlaceholder>, "lucide">;

function Spinner({ className, ...props }: SpinnerProps) {
  return (
    <IconPlaceholder
      lucide="Loader2Icon"
      data-slot="spinner"
      role="status"
      aria-label="Loading"
      className={cn("size-4", className)}
      {...props}
    />
  )
}

export { Spinner }
