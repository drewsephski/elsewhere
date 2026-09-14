import { WorkDetail } from "@/components/app/work-detail";
import Link from "next/link";
import { useParams } from "react-router-dom";

export function WorkDetailPage() {
  const { id } = useParams<{ id: string }>();
  if (!id) {
    return null;
  }
  return (
    <div className="mx-auto max-w-2xl space-y-6">
      <Link
        href="/app/work"
        className="text-sm text-muted-foreground hover:text-foreground hover:underline"
      >
        ← All work
      </Link>
      <WorkDetail runId={id} />
    </div>
  );
}
