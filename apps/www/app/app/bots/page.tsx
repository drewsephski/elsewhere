import { redirect } from "next/navigation";

export default function BotsIndexPage() {
  redirect("/app?create=1");
}
