"use client";

import { cn } from "cn";
import { LayoutGroup, motion, useReducedMotion } from "motion/react";
import {
  createContext,
  useCallback,
  useContext,
  useId,
  useMemo,
  useRef,
  type KeyboardEvent,
  type ReactNode,
} from "react";

const INDICATOR_SPRING = {
  type: "spring" as const,
  stiffness: 420,
  damping: 34,
  mass: 0.75,
};

export type AnimatedTabsVariant = "underline" | "pill" | "vertical";
export type AnimatedTabsSelection = "tab" | "radio";

interface AnimatedTabsContextValue {
  value: string;
  onValueChange: (value: string) => void;
  variant: AnimatedTabsVariant;
  selection: AnimatedTabsSelection;
  layoutId: string;
  reduceMotion: boolean | null;
  disabled: boolean;
  register: (itemValue: string, node: HTMLButtonElement | null) => void;
  values: () => string[];
}

const AnimatedTabsContext = createContext<AnimatedTabsContextValue | null>(null);

function useAnimatedTabsContext(): AnimatedTabsContextValue {
  const context = useContext(AnimatedTabsContext);
  if (!context) {
    throw new Error("AnimatedTabsTrigger must be used within AnimatedTabs");
  }
  return context;
}

interface AnimatedTabsProps {
  value: string;
  onValueChange: (value: string) => void;
  variant?: AnimatedTabsVariant;
  selection?: AnimatedTabsSelection;
  orientation?: "horizontal" | "vertical";
  disabled?: boolean;
  className?: string;
  children: ReactNode;
  "aria-label"?: string;
}

export function AnimatedTabs({
  value,
  onValueChange,
  variant = "underline",
  selection,
  orientation,
  disabled = false,
  className,
  children,
  "aria-label": ariaLabel,
}: AnimatedTabsProps) {
  const reactId = useId();
  const scopeId = `animated-tabs-${reactId}`;
  const reduceMotion = useReducedMotion();
  const orderRef = useRef<string[]>([]);
  const nodesRef = useRef(new Map<string, HTMLButtonElement>());

  const resolvedSelection: AnimatedTabsSelection =
    selection ?? (variant === "pill" ? "radio" : "tab");
  const resolvedOrientation =
    orientation ?? (variant === "vertical" ? "vertical" : "horizontal");

  const register = useCallback((itemValue: string, node: HTMLButtonElement | null) => {
    if (node) {
      nodesRef.current.set(itemValue, node);
      if (!orderRef.current.includes(itemValue)) {
        orderRef.current.push(itemValue);
      }
      return;
    }
    nodesRef.current.delete(itemValue);
    orderRef.current = orderRef.current.filter((entry) => entry !== itemValue);
  }, []);

  const context = useMemo<AnimatedTabsContextValue>(
    () => ({
      value,
      onValueChange,
      variant,
      selection: resolvedSelection,
      layoutId: "indicator",
      reduceMotion,
      disabled,
      register,
      values: () => orderRef.current,
    }),
    [disabled, onValueChange, reduceMotion, register, resolvedSelection, value, variant],
  );

  function focusValue(next: string) {
    onValueChange(next);
    nodesRef.current.get(next)?.focus();
  }

  function handleKeyDown(event: KeyboardEvent<HTMLDivElement>) {
    const values = orderRef.current;
    if (!values.length) {
      return;
    }
    const currentIndex = Math.max(0, values.indexOf(value));
    const vertical = resolvedOrientation === "vertical";
    const nextKey = vertical ? "ArrowDown" : "ArrowRight";
    const prevKey = vertical ? "ArrowUp" : "ArrowLeft";

    if (event.key === "Home") {
      event.preventDefault();
      focusValue(values[0]!);
      return;
    }
    if (event.key === "End") {
      event.preventDefault();
      focusValue(values[values.length - 1]!);
      return;
    }
    if (event.key === nextKey) {
      event.preventDefault();
      focusValue(values[(currentIndex + 1) % values.length]!);
      return;
    }
    if (event.key === prevKey) {
      event.preventDefault();
      focusValue(values[(currentIndex - 1 + values.length) % values.length]!);
    }
  }

  return (
    <LayoutGroup id={scopeId}>
      <div
        role={resolvedSelection === "radio" ? "radiogroup" : "tablist"}
        aria-label={ariaLabel}
        aria-orientation={resolvedOrientation}
        data-animated-tabs={scopeId}
        data-variant={variant}
        onKeyDown={handleKeyDown}
        className={cn(
          "relative",
          variant === "underline" && "flex items-end gap-4",
          variant === "pill" && "inline-flex rounded-md border border-border p-0.5",
          variant === "vertical" && "flex flex-col gap-0.5",
          className,
        )}
      >
        <AnimatedTabsContext.Provider value={context}>{children}</AnimatedTabsContext.Provider>
      </div>
    </LayoutGroup>
  );
}

interface AnimatedTabsTriggerProps {
  value: string;
  children: ReactNode;
  disabled?: boolean;
  className?: string;
}

export function AnimatedTabsTrigger({
  value,
  children,
  disabled,
  className,
}: AnimatedTabsTriggerProps) {
  const context = useAnimatedTabsContext();
  const selected = context.value === value;
  const isRadio = context.selection === "radio";
  const itemDisabled = context.disabled || disabled;

  return (
    <button
      type="button"
      role={isRadio ? "radio" : "tab"}
      aria-checked={isRadio ? selected : undefined}
      aria-selected={!isRadio ? selected : undefined}
      tabIndex={selected ? 0 : -1}
      disabled={itemDisabled}
      ref={(node) => context.register(value, node)}
      onClick={() => {
        if (!itemDisabled) {
          context.onValueChange(value);
        }
      }}
      className={cn(
        "relative shrink-0 outline-none transition-colors focus-visible:ring-2 focus-visible:ring-ring/50",
        context.variant === "underline" &&
          "pb-2 text-sm font-medium focus-visible:rounded-sm",
        context.variant === "pill" &&
          "h-7 rounded-[5px] px-2 text-[11px] font-medium",
        context.variant === "vertical" &&
          "w-full rounded-lg px-2.5 py-1.5 text-left text-[13px] font-medium",
        selected ? "text-foreground" : "text-muted-foreground hover:text-foreground",
        itemDisabled && "opacity-50",
        className,
      )}
    >
      {selected ? (
        <motion.span
          layoutId={context.layoutId}
          className={cn(
            "pointer-events-none absolute",
            context.variant === "underline" && "inset-x-0 -bottom-px h-0.5 bg-foreground",
            context.variant === "pill" && "inset-0 rounded-[5px] bg-surface-active",
            context.variant === "vertical" && "inset-0 rounded-lg bg-surface-active",
          )}
          transition={context.reduceMotion ? { duration: 0 } : INDICATOR_SPRING}
        />
      ) : null}
      <span className="relative z-10">{children}</span>
    </button>
  );
}
