import { EarlyAccessForm } from "@/components/early-access-form";

export function CtaSection() {
  return (
    <section id="early-access" className="scroll-mt-20 border-t border-border/60">
      <div className="mx-auto max-w-6xl px-4 py-20 sm:px-6">
        <div className="relative overflow-hidden rounded-3xl border border-border/70 bg-gradient-to-br from-card/80 via-card/50 to-muted/30 p-8 sm:p-12">
          <div className="pointer-events-none absolute -right-20 -top-20 size-64 rounded-full bg-primary/10 blur-3xl" aria-hidden />
          <div className="relative grid gap-10 lg:grid-cols-2 lg:items-center">
            <div>
              <p className="font-mono text-[0.7rem] uppercase tracking-[0.2em] text-primary">
                Cloud alpha
              </p>
              <h2 className="mt-3 text-3xl font-semibold tracking-tight sm:text-4xl">
                Join the waitlist for managed sandboxes
              </h2>
              <p className="mt-4 text-sm leading-relaxed text-muted-foreground">
                Tell us you want cloud VMs and cross-device sync. We will reach out when alpha opens
                — desktop users get first access.
              </p>
            </div>
            <div className="rounded-2xl border border-border/60 bg-background/60 p-6 backdrop-blur-sm">
              <EarlyAccessForm />
            </div>
          </div>
        </div>
      </div>
    </section>
  );
}
