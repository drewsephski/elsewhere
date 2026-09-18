import { WorkDetail } from "@/components/app/work-detail";

export default async function WorkPage({ params }: { params: Promise<{ id: string }> }) {
  const { id } = await params;
  return <WorkDetail runId={id} />;
}
