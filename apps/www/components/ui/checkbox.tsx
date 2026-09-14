"use client"

import { Checkbox as CheckboxPrimitive } from "@base-ui/react/checkbox"
import { useEffect, useRef } from "react"
import { cn } from "cn"

import { CheckIcon, type CheckIconHandle } from "@/components/icons/check"

function CheckboxIndicator() {
  const checkRef = useRef<CheckIconHandle>(null)

  useEffect(() => {
    checkRef.current?.startAnimation()
  }, [])

  return (
    <CheckboxPrimitive.Indicator
      data-slot="checkbox-indicator"
      className="absolute inset-0 flex items-center justify-center text-current"
    >
      <CheckIcon
        ref={checkRef}
        size={12}
        className="pointer-events-none text-primary-foreground"
      />
    </CheckboxPrimitive.Indicator>
  )
}

function Checkbox({ className, ...props }: CheckboxPrimitive.Root.Props) {
  return (
    <CheckboxPrimitive.Root
      data-slot="checkbox"
      className={cn(
        "peer relative flex size-5 shrink-0 items-center justify-center overflow-hidden rounded-[5px] border border-input transition-colors outline-none group-has-disabled/field:opacity-50 group-has-[:focus-visible]/field-label:ring-0 group-has-[:focus-visible]/field-label:not-data-checked:border-input after:absolute after:-inset-x-3 after:-inset-y-2 focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/50 disabled:cursor-not-allowed disabled:opacity-50 aria-invalid:border-destructive aria-invalid:ring-3 aria-invalid:ring-destructive/20 aria-invalid:aria-checked:border-primary dark:bg-input/30 dark:aria-invalid:border-destructive/50 dark:aria-invalid:ring-destructive/40 data-checked:border-primary data-checked:bg-primary data-checked:text-primary-foreground group-has-[:focus-visible]/field-label:data-checked:border-primary dark:data-checked:bg-primary",
        className
      )}
      {...props}
    >
      <CheckboxIndicator />
    </CheckboxPrimitive.Root>
  )
}

export { Checkbox }
