import { ComputersManager } from "@/components/app/computers-manager";

export default function ComputersPage() {
  return (
    <div className="space-y-4">
      <h1 className="text-2xl font-medium">Computers</h1>
      <ComputersManager />
    </div>
  );
}
