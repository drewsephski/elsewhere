import { Badge } from "@/components/ui/badge";
import { appRoutes } from "@/lib/app-routes";
import Link from "next/link";

const terminalLines = [
  { kind: "prompt", text: "gptbot guest attach --bot research-01" },
  { kind: "out", text: "Connected to sandbox vm-7f2a · ubuntu 24.04 · 4 vCPU" },
  { kind: "prompt", text: "agent run nightly-sync --approve write:~/exports" },
  { kind: "out", text: "Routine scheduled · next run 02:00 UTC · audit log #8841" },
  { kind: "prompt", text: "memory sync status" },
  { kind: "out", text: "Desktop ↔ cloud · 12 threads · last push 4m ago" },
] as const;

export function SandboxPreview() {
  return (
    <div
      className="relative mx-auto w-full max-w-lg lg:max-w-none"
      aria-label="Agent sandbox preview"
      role="img"
    >
      <div
        className="pointer-events-none absolute -inset-px rounded-2xl bg-gradient-to-b from-primary/20 via-transparent to-transparent opacity-80"
        aria-hidden
      />
      <div className="relative overflow-hidden rounded-2xl border border-border/80 bg-card/90 shadow-[0_24px_80px_-24px_oklch(0_0_0/0.65)] ring-1 ring-foreground/[0.06]">
        <div className="flex items-center justify-between border-b border-border/70 bg-muted/40 px-4 py-2.5">
          <div className="flex items-center gap-2" aria-hidden>
            <span className="size-2.5 rounded-full bg-[oklch(0.55_0.14_25)]" />
            <span className="size-2.5 rounded-full bg-[oklch(0.72_0.12_95)]" />
            <span className="size-2.5 rounded-full bg-[oklch(0.62_0.12_145)]" />
          </div>
          <p className="font-mono text-[0.65rem] tracking-wide text-muted-foreground">
            sandbox · research-01
          </p>
          <Badge variant="secondary" className="h-5 px-2 font-mono text-[0.6rem] uppercase">
            Live
          </Badge>
        </div>
        <div className="grid gap-0 border-b border-border/60 sm:grid-cols-[1fr_9.5rem]">
          <div className="border-b border-border/60 p-4 font-mono text-[0.7rem] leading-relaxed sm:border-b-0 sm:border-r">
            {terminalLines.map((line, index) => (
              <p
                key={line.text}
                className="sandbox-line text-pretty"
                style={{ animationDelay: `${index * 0.35}s` }}
              >
                {line.kind === "prompt" ? (
                  <>
                    <span className="text-primary">λ</span>{" "}
                    <span className="text-foreground/90">{line.text}</span>
                  </>
                ) : (
                  <span className="text-muted-foreground">{line.text}</span>
                )}
              </p>
            ))}
            <p className="mt-2 flex items-center gap-1 text-primary" aria-hidden>
              <span className="sandbox-cursor inline-block h-3.5 w-2 bg-primary/90" />
            </p>
          </div>
          <div className="flex flex-col justify-between bg-background/40 p-3">
            <p className="font-mono text-[0.6rem] uppercase tracking-widest text-muted-foreground">
              Guest
            </p>
            <ul className="space-y-2 text-[0.65rem] text-muted-foreground">
              <li className="flex justify-between gap-2">
                <span>CPU</span>
                <span className="text-foreground/80">18%</span>
              </li>
              <li className="flex justify-between gap-2">
                <span>Mem</span>
                <span className="text-foreground/80">2.1 / 8 GB</span>
              </li>
              <li className="flex justify-between gap-2">
                <span>Uptime</span>
                <span className="text-foreground/80">14d 6h</span>
              </li>
            </ul>
          </div>
        </div>
        <div className="grid grid-cols-3 divide-x divide-border/60 bg-muted/20 text-center font-mono text-[0.6rem] text-muted-foreground">
          <Link href={appRoutes.computers} className="px-2 py-2 transition-colors hover:bg-muted/40 hover:text-foreground">
            terminal
          </Link>
          <Link href={appRoutes.results} className="px-2 py-2 transition-colors hover:bg-muted/40 hover:text-foreground">
            files
          </Link>
          <Link href={appRoutes.work} className="px-2 py-2 transition-colors hover:bg-muted/40 hover:text-foreground">
            browser
          </Link>
        </div>
      </div>
      <p className="mt-3 text-center font-mono text-[0.65rem] text-muted-foreground sm:text-left">
        One isolated environment per bot — same protocol on desktop and in the cloud.
      </p>
    </div>
  );
}
