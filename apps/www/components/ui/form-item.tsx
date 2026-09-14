import { cn } from "cn"

function FormItem({ className, ...props }: React.ComponentProps<"div">) {
  return (
    <div
      data-slot="form-item"
      className={cn("grid gap-1.5", className)}
      {...props}
    />
  )
}

/** Vertical stack of labeled fields — use on `<form>` or inner form sections. */
function FormFields({ className, ...props }: React.ComponentProps<"div">) {
  return (
    <div
      data-slot="form-fields"
      className={cn("grid gap-3", className)}
      {...props}
    />
  )
}

function FormDescription({ className, ...props }: React.ComponentProps<"p">) {
  return (
    <p
      data-slot="form-description"
      className={cn("text-sm leading-normal text-muted-foreground", className)}
      {...props}
    />
  )
}

export { FormDescription, FormFields, FormItem }
