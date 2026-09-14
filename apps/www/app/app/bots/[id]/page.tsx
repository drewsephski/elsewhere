import { BotChat } from "@/components/app/bot-chat";
import Link from "next/link";

export default async function BotDetailPage({ params }: { params: Promise<{ id: string }> }) {
  const { id } = await params;

  return (
    <div>
      <Link href="/app/bots" className="text-xs text-brand-dark/50 underline-offset-4 hover:underline">
        ← Bots
      </Link>
      <BotChat botId={id} />
    </div>
  );
}
