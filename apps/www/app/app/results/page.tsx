import { ResultsPanel } from "@/components/app/results-panel";
export default function ResultsPage() {
  return <div className="space-y-6"><div><h1 className="text-2xl font-semibold">Results</h1><p className="mt-2 text-sm text-muted-foreground">Finished work you can take with you. Saved independently of your bots’ computers.</p></div><ResultsPanel /></div>;
}
