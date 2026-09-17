import { siteConfig } from "@elsewhere/brand";
import { appRoutes } from "@/lib/app-routes";
import { Check } from "@/components/icons/lucide";
import Link from "next/link";

const desktopPoints = [
  "macOS app with an isolated guest computer",
  "ChatGPT / Codex device sign-in",
  "Routines, memory, and approvals",
  "Same workspace UI as the cloud",
  "Local experiments when you want the metal",
];

const cloudPoints = [
  "Managed sandboxes, always on",
  "Your ChatGPT subscription, not our API bill",
  "History and results persist in Postgres",
  "Same bots, same routines, no migration",
];

export function CompareSection() {
  return (
    <section id="open-source" className="landing-shell mt-20 md:mt-28">
      <p className="text-[12px] font-medium tracking-[0.14em] uppercase text-brand-dark/45">Open source</p>
      <h2 className="mt-2 max-w-2xl text-[32px] leading-9 font-medium tracking-[-0.02em] text-brand-dark md:text-[42px] md:leading-[48px] md:tracking-[-0.84px]">
        No pricing page. Just the work.
      </h2>
      <p className="mt-3 max-w-2xl text-[16px] leading-7 text-muted-foreground md:text-[17px] md:leading-8">
        Elsewhere is the cloud workspace for AI workers with real computers. Sign in and start today. A desktop shell is coming soon.
      </p>
      <div className="mt-10 grid gap-4 lg:grid-cols-2">
        <article className="landing-card flex flex-col p-8">
          <div className="flex items-start justify-between gap-3">
            <div>
              <h3 className="text-[20px] font-medium text-brand-dark">Desktop</h3>
              <p className="mt-1 text-[14px] text-muted-foreground">
                macOS app for local computers
              </p>
            </div>
            <span className="rounded-full border border-border/70 bg-transparent px-2.5 py-1 text-[12px] font-medium text-muted-foreground">
              Coming soon
            </span>
          </div>
          <ul className="mt-6 flex-1 space-y-3">
            {desktopPoints.map((point) => (
              <li key={point} className="flex items-start gap-2.5 text-[15px] leading-6 text-brand-dark/80">
                <Check className="mt-1 size-4 shrink-0 text-primary" aria-hidden />
                {point}
              </li>
            ))}
          </ul>
          <div className="mt-8">
            <a
              href={siteConfig.links.docs}
              target="_blank"
              rel="noreferrer"
              className="landing-btn-ghost inline-flex h-[42px] items-center rounded-[14px] px-[18px] text-[14px] font-medium"
            >
              View the repo
            </a>
          </div>
        </article>
        <article className="landing-card flex flex-col p-8">
          <div className="flex items-start justify-between gap-3">
            <div>
              <h3 className="text-[20px] font-medium text-brand-dark">Cloud</h3>
              <p className="mt-1 text-[14px] text-muted-foreground">
                We run the computers. You bring ChatGPT.
              </p>
            </div>
            <span className="rounded-full border border-border/70 bg-transparent px-2.5 py-1 text-[12px] font-medium text-brand-dark/70">
              Live
            </span>
          </div>
          <ul className="mt-6 flex-1 space-y-3">
            {cloudPoints.map((point) => (
              <li key={point} className="flex items-start gap-2.5 text-[15px] leading-6 text-brand-dark/80">
                <Check className="mt-1 size-4 shrink-0 text-primary" aria-hidden />
                {point}
              </li>
            ))}
          </ul>
          <div className="mt-8">
            <Link
              href={appRoutes.workspace}
              className="landing-btn-primary inline-flex h-[42px] items-center rounded-[14px] px-[18px] text-[14px] font-medium"
            >
              Open workspace
            </Link>
          </div>
        </article>
      </div>
    </section>
  );
}
