"use client";

import { cn } from "@/lib/utils";
import {
  forwardRef,
  useEffect,
  useImperativeHandle,
  useRef,
  type ComponentProps,
  type ForwardRefExoticComponent,
  type RefAttributes,
} from "react";

import { useParentHoverAnimation } from "./use-parent-hover-animation";

export type AnimatedIconHandle = {
  startAnimation: () => void;
  stopAnimation: () => void;
};

type AnimatedIconComponent = ForwardRefExoticComponent<
  ComponentProps<"div"> & { size?: number } & RefAttributes<AnimatedIconHandle>
>;

function setIconRef(
  innerRef: React.RefObject<AnimatedIconHandle | null>,
  node: AnimatedIconHandle | null,
) {
  innerRef.current = node;
}

export function adaptAnimatedIcon(Base: AnimatedIconComponent) {
  const Wrapped = forwardRef<
    AnimatedIconHandle,
    ComponentProps<AnimatedIconComponent>
  >(function AnimatedIcon({ className, size = 16, ...props }, ref) {
    const anchorRef = useRef<HTMLSpanElement>(null);
    const iconRef = useRef<AnimatedIconHandle>(null);

    useImperativeHandle(ref, () => ({
      startAnimation: () => iconRef.current?.startAnimation(),
      stopAnimation: () => iconRef.current?.stopAnimation(),
    }));

    useParentHoverAnimation(iconRef, anchorRef);

    return (
      <span ref={anchorRef} className="inline-flex shrink-0">
        <Base
          ref={((node: AnimatedIconHandle | null) => setIconRef(iconRef, node)) as never}
          className={cn(
            "pointer-events-none inline-flex size-4 shrink-0 [&_svg]:size-full",
            className,
          )}
          size={size}
          {...props}
        />
      </span>
    );
  });
  Wrapped.displayName = Base.displayName ?? "AnimatedIcon";
  return Wrapped;
}

export function createSpinningIcon(Base: AnimatedIconComponent) {
  const Wrapped = forwardRef<
    AnimatedIconHandle,
    ComponentProps<AnimatedIconComponent>
  >(function SpinningIcon({ className, size = 16, ...props }, ref) {
    const innerRef = useRef<AnimatedIconHandle>(null);

    useImperativeHandle(ref, () => ({
      startAnimation: () => innerRef.current?.startAnimation(),
      stopAnimation: () => innerRef.current?.stopAnimation(),
    }));

    useEffect(() => {
      innerRef.current?.startAnimation();
    }, []);

    return (
      <Base
        ref={((node: AnimatedIconHandle | null) => setIconRef(innerRef, node)) as never}
        className={cn(
          "pointer-events-none inline-flex size-4 shrink-0 [&_svg]:size-full",
          className,
        )}
        size={size}
        {...props}
      />
    );
  });
  Wrapped.displayName = `${Base.displayName ?? "Icon"}Spinning`;
  return Wrapped;
}
