import Image from "next/image";
import { cn } from "cn";

const sizeClasses = {
  sm: "size-7",
  md: "size-8",
  lg: "size-10",
} as const;

interface ProductLogoProps {
  size?: keyof typeof sizeClasses;
  className?: string;
  priority?: boolean;
}

export function ProductLogo({
  size = "md",
  className,
  priority = false,
}: ProductLogoProps) {
  return (
    <Image
      src="/logo.png"
      alt=""
      width={512}
      height={512}
      priority={priority}
      className={cn("shrink-0 object-contain", sizeClasses[size], className)}
      aria-hidden
    />
  );
}
