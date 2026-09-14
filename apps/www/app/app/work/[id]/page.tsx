import { WorkDetail } from "@/components/app/work-detail";
import Link from "next/link";

export default async function WorkPage({ params }: { params: Promise<{ id: string }> }) {
  const { id } = await params;
  return (
    <div className="space-y-6">
      <Link href="/app/work" className="text-sm text-muted-foreground hover:text-foreground hover:underline">
        ← All work
      </Link>
      <WorkDetail runId={id} />
    </div>
  );
}
