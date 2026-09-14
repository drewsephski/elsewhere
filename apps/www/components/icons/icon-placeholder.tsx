"use client";

import {
  ArrowDownIcon,
  ArrowLeftIcon,
  ArrowLeftToLineIcon,
  ArrowRightIcon,
  ArrowRightToLineIcon,
  ArrowUpIcon,
  CheckIcon,
  ChevronDownIcon,
  ChevronLeftIcon,
  ChevronRightIcon,
  ChevronsUpDownIcon,
  ChevronUpIcon,
  CirclePlusIcon,
  GripHorizontalIcon,
  GripVerticalIcon,
  Loader2Icon,
  PinOffIcon,
  PlusIcon,
  Settings2Icon,
  XIcon,
} from "@/components/icons/lucide";

// eslint-disable-next-line @typescript-eslint/no-explicit-any
const lucideAnimatedIcons: Record<string, React.ComponentType<any>> = {
  ArrowDownIcon,
  ArrowLeftIcon,
  ArrowLeftToLineIcon,
  ArrowRightIcon,
  ArrowRightToLineIcon,
  ArrowUpIcon,
  CheckIcon,
  ChevronDownIcon,
  ChevronLeftIcon,
  ChevronRightIcon,
  ChevronUpIcon,
  ChevronsUpDownIcon,
  CirclePlusIcon,
  GripHorizontalIcon,
  GripVerticalIcon,
  Loader2Icon,
  PinOffIcon,
  PlusIcon,
  Settings2Icon,
  XIcon,
};

interface IconPlaceholderProps extends React.HTMLAttributes<HTMLElement> {
  lucide: string;
  tabler?: string;
  hugeicons?: string;
  phosphor?: string;
  remixicon?: string;
}

/** Renders lucide-animated icons for shadcn Base UI placeholder slots. */
export function IconPlaceholder({
  lucide,
  tabler: _t,
  hugeicons: _h,
  phosphor: _p,
  remixicon: _r,
  ...props
}: IconPlaceholderProps) {
  const Icon = lucideAnimatedIcons[lucide];
  if (!Icon) {
    if (process.env.NODE_ENV !== "production") {
      console.warn(`[IconPlaceholder] Unknown lucide icon: ${lucide}`);
    }
    return null;
  }
  return <Icon {...props} />;
}
