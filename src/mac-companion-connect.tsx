import { ThisMacControls } from "@/components/app/this-mac-controls";

/** Computers page header action — pairs / pauses This Mac in the desktop shell. */
export function MacCompanionConnect() {
  return <ThisMacControls layout="stack" className="sm:items-end" />;
}
