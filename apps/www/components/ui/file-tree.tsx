"use client";

import React, {
  createContext,
  forwardRef,
  useCallback,
  useContext,
  useEffect,
  useState,
} from "react";
import * as AccordionPrimitive from "@radix-ui/react-accordion";
import { FileText, Files } from "@/components/icons/lucide";

import { cn } from "cn";
import { Button } from "@/components/ui/button";

type TreeViewElement = {
  id: string;
  name: string;
  type?: "file" | "folder";
  isSelectable?: boolean;
  children?: TreeViewElement[];
};

type TreeSortMode =
  | "default"
  | "none"
  | ((a: TreeViewElement, b: TreeViewElement) => number);

type TreeContextProps = {
  selectedId: string | undefined;
  expandedItems: string[] | undefined;
  indicator: boolean;
  handleExpand: (id: string) => void;
  selectItem: (id: string) => void;
  setExpandedItems?: React.Dispatch<React.SetStateAction<string[] | undefined>>;
  openIcon?: React.ReactNode;
  closeIcon?: React.ReactNode;
  direction: "rtl" | "ltr";
};

const TreeContext = createContext<TreeContextProps | null>(null);

function useTree() {
  const context = useContext(TreeContext);
  if (!context) {
    throw new Error("useTree must be used within a TreeProvider");
  }
  return context;
}

type Direction = "rtl" | "ltr" | undefined;

function isFolderElement(element: TreeViewElement) {
  if (element.type) {
    return element.type === "folder";
  }

  return Array.isArray(element.children);
}

function mergeExpandedItems(
  currentItems: string[] | undefined,
  nextItems: string[],
) {
  return [...new Set([...(currentItems ?? []), ...nextItems])];
}

const treeCollator = new Intl.Collator("en", {
  numeric: true,
  sensitivity: "base",
});

function defaultTreeComparator(a: TreeViewElement, b: TreeViewElement) {
  const aIsFolder = isFolderElement(a);
  const bIsFolder = isFolderElement(b);

  if (aIsFolder !== bIsFolder) {
    return aIsFolder ? -1 : 1;
  }

  return treeCollator.compare(a.name, b.name);
}

function getTreeComparator(sort: TreeSortMode) {
  if (sort === "none") {
    return undefined;
  }

  if (sort === "default") {
    return defaultTreeComparator;
  }

  return sort;
}

function sortTreeElements(
  elements: TreeViewElement[],
  sort: TreeSortMode,
): TreeViewElement[] {
  const comparator = getTreeComparator(sort);

  const nextElements = elements.map((element) => {
    if (!Array.isArray(element.children)) {
      return element;
    }

    return {
      ...element,
      children: sortTreeElements(element.children, sort),
    };
  });

  if (!comparator) {
    return nextElements;
  }

  return [...nextElements].sort(comparator);
}

function renderTreeElements(
  elements: TreeViewElement[],
  sort: TreeSortMode,
): React.ReactNode {
  return sortTreeElements(elements, sort).map((element) => {
    if (isFolderElement(element)) {
      return (
        <Folder
          key={element.id}
          value={element.id}
          element={element.name}
          isSelectable={element.isSelectable}
        >
          {Array.isArray(element.children)
            ? renderTreeElements(element.children, sort)
            : null}
        </Folder>
      );
    }

    return (
      <File
        key={element.id}
        value={element.id}
        isSelectable={element.isSelectable}
      >
        <span>{element.name}</span>
      </File>
    );
  });
}

type TreeViewProps = {
  initialSelectedId?: string;
  indicator?: boolean;
  elements?: TreeViewElement[];
  initialExpandedItems?: string[];
  openIcon?: React.ReactNode;
  closeIcon?: React.ReactNode;
  sort?: TreeSortMode;
  header?: React.ReactNode;
  /** When false, the tree grows with content (parent should handle scrolling). */
  scrollable?: boolean;
} & Omit<
  React.ComponentPropsWithoutRef<typeof AccordionPrimitive.Root>,
  "defaultValue" | "onValueChange" | "type" | "value"
>;

const Tree = forwardRef<HTMLDivElement, TreeViewProps>(
  (
    {
      className,
      elements,
      initialSelectedId,
      initialExpandedItems,
      children,
      indicator = true,
      openIcon,
      closeIcon,
      sort = "default",
      dir,
      header,
      scrollable = true,
      ...props
    },
    ref,
  ) => {
    const [selectedId, setSelectedId] = useState<string | undefined>(
      initialSelectedId,
    );
    const [expandedItems, setExpandedItems] = useState<string[] | undefined>(
      initialExpandedItems,
    );

    const selectItem = useCallback((id: string) => {
      setSelectedId(id);
    }, []);

    const handleExpand = useCallback((id: string) => {
      setExpandedItems((prev) => {
        if (prev?.includes(id)) {
          return prev.filter((item) => item !== id);
        }
        return [...(prev ?? []), id];
      });
    }, []);

    const expandSpecificTargetedElements = useCallback(
      (treeElements?: TreeViewElement[], selectId?: string) => {
        if (!treeElements || !selectId) {
          return;
        }
        const findParent = (
          currentElement: TreeViewElement,
          currentPath: string[] = [],
        ) => {
          const isSelectable = currentElement.isSelectable ?? true;
          const newPath = [...currentPath, currentElement.id];
          if (currentElement.id === selectId) {
            if (isSelectable) {
              setExpandedItems((prev) => mergeExpandedItems(prev, newPath));
            } else if (newPath.includes(currentElement.id)) {
              newPath.pop();
              setExpandedItems((prev) => mergeExpandedItems(prev, newPath));
            }
            return;
          }
          if (
            Array.isArray(currentElement.children) &&
            currentElement.children.length > 0
          ) {
            currentElement.children.forEach((child) => {
              findParent(child, newPath);
            });
          }
        };
        treeElements.forEach((element) => {
          findParent(element);
        });
      },
      [],
    );

    useEffect(() => {
      if (initialSelectedId) {
        expandSpecificTargetedElements(elements, initialSelectedId);
      }
    }, [initialSelectedId, elements, expandSpecificTargetedElements]);

    const direction = dir === "rtl" ? "rtl" : "ltr";
    const treeChildren =
      children ?? (elements ? renderTreeElements(elements, sort) : null);

    return (
      <TreeContext.Provider
        value={{
          selectedId,
          expandedItems,
          handleExpand,
          selectItem,
          setExpandedItems,
          indicator,
          openIcon,
          closeIcon,
          direction,
        }}
      >
        <div className={cn(scrollable ? "size-full" : "w-full", className)}>
          {header}
          <div
            ref={ref}
            className={cn(
              "relative px-1",
              scrollable ? "h-full overflow-y-auto" : "overflow-visible",
            )}
            dir={dir as Direction}
          >
            <AccordionPrimitive.Root
              {...props}
              type="multiple"
              value={expandedItems}
              onValueChange={setExpandedItems}
              className="flex flex-col gap-0.5"
              dir={dir as Direction}
            >
              {treeChildren}
            </AccordionPrimitive.Root>
          </div>
        </div>
      </TreeContext.Provider>
    );
  },
);

Tree.displayName = "Tree";

const TreeIndicator = forwardRef<
  HTMLDivElement,
  React.HTMLAttributes<HTMLDivElement>
>(({ className, ...props }, ref) => {
  const { direction } = useTree();

  return (
    <div
      dir={direction}
      ref={ref}
      className={cn(
        "absolute left-1.5 h-full w-px rounded-md bg-border/80 py-3 duration-300 ease-in-out hover:bg-muted-foreground/30 rtl:right-1.5",
        className,
      )}
      {...props}
    />
  );
});

TreeIndicator.displayName = "TreeIndicator";

type FolderProps = {
  expandedItems?: string[];
  element: React.ReactNode;
  isSelectable?: boolean;
  isSelect?: boolean;
  onContextMenu?: React.MouseEventHandler<HTMLButtonElement>;
} & React.ComponentPropsWithoutRef<typeof AccordionPrimitive.Item>;

const Folder = forwardRef<
  HTMLDivElement,
  FolderProps & React.HTMLAttributes<HTMLDivElement>
>(
  (
    {
      className,
      element,
      value,
      isSelectable = true,
      isSelect,
      children,
      onContextMenu,
      ...props
    },
    ref,
  ) => {
    const {
      direction,
      handleExpand,
      expandedItems,
      indicator,
      selectedId,
      selectItem,
      openIcon,
      closeIcon,
    } = useTree();
    const isSelected = isSelect ?? selectedId === value;
    const isOpen = expandedItems?.includes(value);

    return (
      <AccordionPrimitive.Item
        ref={ref}
        {...props}
        value={value}
        className="relative overflow-hidden"
      >
        <AccordionPrimitive.Trigger
          onContextMenu={onContextMenu}
          className={cn(
            "flex w-full min-w-0 items-center gap-1.5 rounded-md px-1 py-1 text-left text-xs font-medium transition-colors duration-200 ease-in-out",
            className,
            {
              "bg-muted/90": isSelected && isSelectable,
              "cursor-pointer hover:bg-surface-hover": isSelectable,
              "cursor-not-allowed opacity-50": !isSelectable,
            },
          )}
          disabled={!isSelectable}
          onClick={() => {
            selectItem(value);
            handleExpand(value);
          }}
        >
          {isOpen
            ? (openIcon ?? (
                <Files className="size-3.5 shrink-0 text-warning/90" />
              ))
            : (closeIcon ?? (
                <Files className="size-3.5 shrink-0 text-muted-foreground" />
              ))}
          <span className="min-w-0 flex-1 truncate">{element}</span>
        </AccordionPrimitive.Trigger>
        <AccordionPrimitive.Content
          className="relative overflow-hidden text-xs data-[state=closed]:animate-accordion-up data-[state=open]:animate-accordion-down"
        >
          {element && indicator ? (
            <TreeIndicator aria-hidden="true" />
          ) : null}
          <AccordionPrimitive.Root
            dir={direction}
            type="multiple"
            className="ml-4 flex flex-col gap-0.5 py-0.5 rtl:mr-4"
            value={expandedItems}
          >
            {children}
          </AccordionPrimitive.Root>
        </AccordionPrimitive.Content>
      </AccordionPrimitive.Item>
    );
  },
);

Folder.displayName = "Folder";

const File = forwardRef<
  HTMLButtonElement,
  {
    value: string;
    handleSelect?: (id: string) => void;
    isSelectable?: boolean;
    isSelect?: boolean;
    fileIcon?: React.ReactNode;
  } & React.ButtonHTMLAttributes<HTMLButtonElement>
>(
  (
    {
      value,
      className,
      handleSelect,
      onClick,
      isSelectable = true,
      isSelect,
      fileIcon,
      children,
      ...props
    },
    ref,
  ) => {
    const { direction, selectedId, selectItem } = useTree();
    const isSelected = isSelect ?? selectedId === value;
    return (
      <button
        ref={ref}
        type="button"
        disabled={!isSelectable}
        className={cn(
          "flex w-full min-w-0 items-center gap-1.5 rounded-md px-1 py-1 text-left text-xs transition-colors duration-200 ease-in-out rtl:pr-0 rtl:pl-1",
          {
            "bg-muted/90": isSelected && isSelectable,
          },
          isSelectable
            ? "cursor-pointer text-foreground/90 hover:bg-surface-hover"
            : "cursor-not-allowed opacity-50",
          direction === "rtl" ? "rtl" : "ltr",
          className,
        )}
        onClick={(event) => {
          selectItem(value);
          handleSelect?.(value);
          onClick?.(event);
        }}
        {...props}
      >
        {fileIcon ?? (
          <FileText className="size-3.5 shrink-0 text-muted-foreground" />
        )}
        <span className="truncate">{children}</span>
      </button>
    );
  },
);

File.displayName = "File";

const CollapseButton = forwardRef<
  HTMLButtonElement,
  {
    elements: TreeViewElement[];
    expandAll?: boolean;
  } & React.HTMLAttributes<HTMLButtonElement>
>(({ className, elements, expandAll = false, children, ...props }, ref) => {
  const { expandedItems, setExpandedItems } = useTree();

  const expendAllTree = useCallback((treeElements: TreeViewElement[]) => {
    const expandedElementIds: string[] = [];

    const expandTree = (element: TreeViewElement) => {
      const isSelectable = element.isSelectable ?? true;
      if (isSelectable && element.children && element.children.length > 0) {
        expandedElementIds.push(element.id);
        for (const child of element.children) {
          expandTree(child);
        }
      }
    };

    for (const element of treeElements) {
      expandTree(element);
    }

    return [...new Set(expandedElementIds)];
  }, []);

  const closeAll = useCallback(() => {
    setExpandedItems?.([]);
  }, [setExpandedItems]);

  useEffect(() => {
    if (expandAll) {
      setExpandedItems?.(expendAllTree(elements));
    }
  }, [expandAll, elements, expendAllTree, setExpandedItems]);

  return (
    <Button
      variant="ghost"
      size="xs"
      className={cn("h-7 w-fit px-2 text-[11px] text-muted-foreground", className)}
      onClick={
        expandedItems && expandedItems.length > 0
          ? closeAll
          : () => setExpandedItems?.(expendAllTree(elements))
      }
      ref={ref}
      type="button"
      {...props}
    >
      {children ??
        (expandedItems && expandedItems.length > 0 ? "Collapse all" : "Expand all")}
      <span className="sr-only">Toggle tree expansion</span>
    </Button>
  );
});

CollapseButton.displayName = "CollapseButton";

export {
  CollapseButton,
  File,
  Folder,
  Tree,
  useTree,
  type TreeViewElement,
};
export type { TreeSortMode };
