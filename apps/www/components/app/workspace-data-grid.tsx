"use client";

import {
  DataGrid,
  DataGridContainer,
  type DataGridTableInstance,
} from "@/components/reui/data-grid/data-grid";
import { DataGridPagination } from "@/components/reui/data-grid/data-grid-pagination";
import { DataGridScrollArea } from "@/components/reui/data-grid/data-grid-scroll-area";
import { DataGridTable } from "@/components/reui/data-grid/data-grid-table";
import { Card } from "@/components/ui/card";
import { cn } from "@/lib/utils";
import type { ReactNode } from "react";

interface WorkspaceDataGridProps<TData extends object> {
  table: DataGridTableInstance<TData>;
  recordCount: number;
  toolbar?: ReactNode;
  emptyMessage?: ReactNode;
  loading?: boolean;
  className?: string;
  pageSize?: number;
}

export function WorkspaceDataGrid<TData extends object>({
  table,
  recordCount,
  toolbar,
  emptyMessage,
  loading,
  className,
}: WorkspaceDataGridProps<TData>) {
  return (
    <DataGrid
      table={table}
      recordCount={recordCount}
      isLoading={loading}
      emptyMessage={
        emptyMessage ?? (
          <p className="py-10 text-center text-sm text-muted-foreground">Nothing here yet.</p>
        )
      }
      tableLayout={{
        rowBorder: true,
        headerBackground: true,
        headerBorder: true,
        cellBorder: false,
        width: "auto",
        columnsVisibility: false,
        columnsResizable: false,
        columnsMovable: false,
        columnsPinnable: false,
      }}
      tableClassNames={{
        base: "text-sm",
        header: "bg-surface-hover text-muted-foreground",
        headerRow: "border-b border-border/60",
        bodyRow: "transition-colors hover:bg-surface-hover",
      }}
    >
      <div className={cn("space-y-3", className)}>
        {toolbar}
        <Card
          className="min-w-0 overflow-hidden rounded-xl border-border bg-card p-0"
        >
          <DataGridContainer className="min-w-0">
            <DataGridScrollArea orientation="vertical">
              <DataGridTable />
            </DataGridScrollArea>
          </DataGridContainer>
        </Card>
        {recordCount > 0 ? <DataGridPagination /> : null}
      </div>
    </DataGrid>
  );
}

interface WorkspaceDataGridTabsProps {
  tabs: readonly { value: string; label: string }[];
  active: string;
  onChange: (value: string) => void;
  hint?: string;
}

export function WorkspaceDataGridTabs({
  tabs,
  active,
  onChange,
  hint,
}: WorkspaceDataGridTabsProps) {
  return (
    <div className="flex flex-wrap items-end justify-between gap-3 border-b border-border/60 pb-3">
      <div className="flex gap-4">
        {tabs.map((tab) => (
          <button
            key={tab.value}
            type="button"
            onClick={() => onChange(tab.value)}
            className={cn(
              "border-b-2 pb-2 text-sm font-medium transition-colors",
              active === tab.value
                ? "border-foreground text-foreground"
                : "border-transparent text-muted-foreground hover:text-foreground",
            )}
            aria-current={active === tab.value ? "true" : undefined}
          >
            {tab.label}
          </button>
        ))}
      </div>
      {hint ? <p className="text-xs text-muted-foreground">{hint}</p> : null}
    </div>
  );
}
