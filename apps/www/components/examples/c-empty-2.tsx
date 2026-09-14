import { Button } from "@/components/ui/button"
import {
  Empty,
  EmptyContent,
  EmptyDescription,
  EmptyHeader,
  EmptyTitle,
} from "@/components/ui/empty"
import { ArrowUpRightIcon } from "@/components/icons/lucide"

export function Pattern() {
  return (
    <div className="flex items-center justify-center">
      <Empty className="bg-muted">
        <EmptyHeader>
          <EmptyTitle>No results found</EmptyTitle>
          <EmptyDescription>
            No results found for your search. Try adjusting your search terms.
          </EmptyDescription>
        </EmptyHeader>
        <EmptyContent>
          <Button>Try again</Button>
          <Button
            variant="link"
            render={<a href="#" />}
            className="text-muted-foreground"
            nativeButton={false}
          >
            Learn more{" "}
            <ArrowUpRightIcon
            />
          </Button>
        </EmptyContent>
      </Empty>
    </div>
  )
}