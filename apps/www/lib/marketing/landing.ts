export interface DemoChecklistItem {
  label: string;
  detail: string;
}

export interface DemoMessage {
  id: string;
  role: "user" | "assistant";
  text: string;
  checklist?: DemoChecklistItem[];
}

export interface DemoRoutine {
  name: string;
  cadence: string;
}

export type DemoMailThread = {
  id: string;
  from: string;
  email: string;
  subject: string;
  snippet: string;
  body: string;
  draft?: string;
  time: string;
  state: "unread" | "draft" | "held" | "read";
  starred?: boolean;
  labels?: string[];
  active?: boolean;
};

export type DemoSheetRow = {
  name: string;
  score: string;
  status: "Short" | "Pass";
  note: string;
  active?: boolean;
};

export type DemoCalendarEvent = {
  id: string;
  weekday: string;
  date: string;
  time: string;
  startHour: number;
  durationHours: number;
  title: string;
  detail: string;
  color: string;
  active?: boolean;
};

export type DemoAccountRow = {
  name: string;
  domain: string;
  score: string;
  state: "Draft" | "Skip" | "Review";
  note: string;
  signals: string[];
  active?: boolean;
};

export type DemoIssueRow = {
  number: number;
  title: string;
  state: "reproduced" | "blocked";
  labels: string[];
  body: string;
  comments: number;
  active?: boolean;
};

export type DemoFrame = {
  label: string;
  size: string;
  note: string;
  rows: string[];
  active?: boolean;
};

export type DemoReceipt = {
  merchant: string;
  amount: string;
  date: string;
  category: string;
  state: "Matched" | "Flagged" | "Filed";
  note?: string;
  active?: boolean;
};

export type DemoCampaign = {
  name: string;
  state: "Active" | "Paused" | "Held";
  change: string;
  spend: string;
  conv: string;
  clicks: string;
  active?: boolean;
};

export type DemoBrowserPage =
  | {
      kind: "mail";
      folder: string;
      draftCount: number;
      threads: DemoMailThread[];
    }
  | {
      kind: "calendar";
      range: string;
      days: string[];
      today: string;
      selectedDay: string;
      events: DemoCalendarEvent[];
    }
  | {
      kind: "accounts";
      heading: string;
      rows: DemoAccountRow[];
    }
  | {
      kind: "github";
      repo: string;
      issues: DemoIssueRow[];
    }
  | {
      kind: "figma";
      file: string;
      frames: DemoFrame[];
    }
  | {
      kind: "expenses";
      report: string;
      receipts: DemoReceipt[];
    }
  | {
      kind: "sheet";
      title: string;
      columns: readonly [string, string, string];
      rows: DemoSheetRow[];
    }
  | {
      kind: "ads";
      heading: string;
      campaigns: DemoCampaign[];
    };

export interface DemoComputer {
  title: string;
  url: string;
  tab: string;
  page: DemoBrowserPage;
}

export interface DemoBot {
  id: string;
  name: string;
  image: string;
  time: string;
  preview: string;
  computer: DemoComputer;
  routines: DemoRoutine[];
  messages: DemoMessage[];
}

export const AGENT_SETUP_PROMPT =
  "Clone https://github.com/drewsephski/elsewhere and run `pnpm install && pnpm dev:www`. Sign in, connect ChatGPT, create a computer, then give a bot the work.";

export const DEMO_BOTS: DemoBot[] = [
  {
    id: "writer",
    name: "Writer",
    image: "/marketing/job-bots/writer.png",
    time: "12:11 AM",
    preview: "sent. inbox is at zero, 5 drafts parked",
    computer: {
      title: "Writer’s computer",
      url: "mail.google.com/inbox",
      tab: "Inbox",
      page: {
        kind: "mail",
        folder: "Inbox",
        draftCount: 5,
        threads: [
          {
            id: "nora",
            from: "Nora Chen",
            email: "nora@helixloft.com",
            subject: "Does renewal cover the new seats?",
            snippet: "Quick one before I send this to legal — does the renewal…",
            body: "Quick one before I send this to legal — does the renewal cover the eight new seats we added in March, or do we need an amendment? I told them yes, but I want the contract line.",
            draft: "Yes — section 4.2 covers seat expansion within the term. The eight seats from March are already in the renewal. No amendment needed.",
            time: "12:04 AM",
            state: "held",
            starred: true,
            labels: ["Helix"],
            active: true,
          },
          {
            id: "maya",
            from: "Maya Ortiz",
            email: "maya@elsewhere.dev",
            subject: "Intro to Jules for Thursday",
            snippet: "Can you intro Jules to the offsite thread? She needs the…",
            body: "Can you intro Jules to the offsite thread? She needs the run of show and the loft address before Thursday.",
            time: "11:40 PM",
            state: "read",
            labels: ["Offsite"],
          },
          {
            id: "stripe",
            from: "Stripe",
            email: "receipts@stripe.com",
            subject: "Receipt · Invoice 1842",
            snippet: "Your receipt from Elsewhere Cloud is ready. Amount $1,240.00",
            body: "Your receipt from Elsewhere Cloud is ready.\n\nInvoice 1842 · $1,240.00 · paid with Visa ••4242.",
            time: "9:12 PM",
            state: "read",
          },
          {
            id: "substack",
            from: "Substack",
            email: "digest@substack.com",
            subject: "6 newsletters from the weekend",
            snippet: "Lenny’s Newsletter, The Batch, Stratechery, and 3 more.",
            body: "Here’s what you missed this weekend: Lenny’s Newsletter, The Batch, Stratechery, Not Boring, The Pragmatic Engineer, and Superhuman.",
            time: "Sun",
            state: "read",
          },
        ],
      },
    },
    routines: [
      { name: "Morning sweep", cadence: "Weekdays 6am" },
      { name: "Reply Zero", cadence: "Hourly" },
    ],
    messages: [
      {
        id: "w1",
        role: "user",
        text: "inbox got away from me over the weekend, sweep it?",
      },
      {
        id: "w2",
        role: "assistant",
        text: "on it. 41 unread since friday. archiving the noise, drafting the rest.",
      },
      {
        id: "w3",
        role: "assistant",
        text: "",
        checklist: [
          { label: "Archived", detail: "26 newsletters + receipts" },
          { label: "Replied", detail: "9 routine threads · scheduling and intros" },
          { label: "Drafted", detail: "6 that sound like you · held for your read" },
          { label: "Flagged", detail: "1 from nora · contract question" },
        ],
      },
      {
        id: "w4",
        role: "assistant",
        text: "nora is asking whether the renewal covers the new seats. her draft answers yes and quotes the contract line. it is at the top of the held pile.",
      },
      {
        id: "w5",
        role: "user",
        text: "send nora’s, i’ll take the rest tomorrow",
      },
      {
        id: "w6",
        role: "assistant",
        text: "sent. inbox is at zero, with 5 drafts parked for tomorrow.",
      },
    ],
  },
  {
    id: "cos",
    name: "Chief of Staff",
    image: "/marketing/job-bots/chief-of-staff.png",
    time: "Yesterday",
    preview: "venue booked, contract sent for signature",
    computer: {
      title: "Chief of Staff’s computer",
      url: "calendar.google.com/week",
      tab: "Calendar",
      page: {
        kind: "calendar",
        range: "Sep 14 – 20",
        days: ["14", "15", "16", "17", "18", "19", "20"],
        today: "16",
        selectedDay: "18",
        events: [
          {
            id: "briefing",
            weekday: "Mon",
            date: "15",
            time: "9:00 – 10:00 AM",
            startHour: 9,
            durationHours: 1,
            title: "Monday briefing",
            detail: "Run of show + vendor status",
            color: "#7986cb",
          },
          {
            id: "vendor",
            weekday: "Wed",
            date: "17",
            time: "11:00 – 11:30 AM",
            startHour: 11,
            durationHours: 0.5,
            title: "Helix walkthrough",
            detail: "Confirm AV and catering",
            color: "#33b679",
          },
          {
            id: "sam",
            weekday: "Thu",
            date: "18",
            time: "2:00 – 2:30 PM",
            startHour: 14,
            durationHours: 0.5,
            title: "1:1 Sam",
            detail: "Design review invite",
            color: "#039be5",
          },
          {
            id: "offsite",
            weekday: "Thu",
            date: "18",
            time: "4:00 – 8:00 PM",
            startHour: 16,
            durationHours: 4,
            title: "Helix Loft offsite",
            detail: "Confirmed · contract awaiting your signature",
            color: "#7c3aed",
            active: true,
          },
          {
            id: "debrief",
            weekday: "Fri",
            date: "19",
            time: "10:00 – 10:30 AM",
            startHour: 10,
            durationHours: 0.5,
            title: "Offsite debrief",
            detail: "Notes to Writer + Researcher",
            color: "#e67c73",
          },
        ],
      },
    },
    routines: [
      { name: "Monday briefing", cadence: "Weekly" },
      { name: "Open loops", cadence: "Daily 5pm" },
    ],
    messages: [
      {
        id: "c1",
        role: "user",
        text: "lock the offsite and send the vendor the contract. i’ll sign tonight.",
      },
      {
        id: "c2",
        role: "assistant",
        text: "booked helix loft for thursday 4–8. contract is in your approvals — two clauses on overtime and cancellation. once you sign, i’ll send it and ping the other bots with the run of show.",
      },
    ],
  },
  {
    id: "researcher",
    name: "Researcher",
    image: "/marketing/job-bots/researcher.png",
    time: "3:10 AM",
    preview: "done. 40 accounts researched, 18 drafts queued",
    computer: {
      title: "Researcher’s computer",
      url: "accounts.elsewhere/inbound",
      tab: "Inbound",
      page: {
        kind: "accounts",
        heading: "40 scored · 18 drafts queued",
        rows: [
          {
            name: "Northwind",
            domain: "northwind.ai",
            score: "0.92",
            state: "Draft",
            note: "Hiring page + new round",
            signals: ["Series B filed", "6 open eng roles", "Used us in 2024"],
            active: true,
          },
          {
            name: "Helix Labs",
            domain: "helixlabs.com",
            score: "0.88",
            state: "Draft",
            note: "Series B announced",
            signals: ["Launch week", "CTO posting", "Warm intro via Maya"],
          },
          {
            name: "Paloma",
            domain: "paloma.co",
            score: "0.81",
            state: "Review",
            note: "Intent, thin writing",
            signals: ["Pricing page hit 4×", "No hiring page"],
          },
          {
            name: "Grayline",
            domain: "grayline.io",
            score: "0.44",
            state: "Skip",
            note: "No intent",
            signals: ["Last visit 11 months ago"],
          },
        ],
      },
    },
    routines: [
      { name: "Overnight research", cadence: "Weeknights 11pm" },
      { name: "Intent refresh", cadence: "Daily 7am" },
    ],
    messages: [
      {
        id: "r1",
        role: "user",
        text: "work the inbound list overnight. leave me a review pile, not a send pile.",
      },
      {
        id: "r2",
        role: "assistant",
        text: "done. 40 accounts researched, 18 drafts queued in your voice. the top five have recent hiring pages and a new round. i held anything that needed a guess.",
      },
    ],
  },
  {
    id: "engineer",
    name: "Engineer",
    image: "/marketing/job-bots/engineer.png",
    time: "Monday",
    preview: "reproduced 4 of 6, steps in the issues",
    computer: {
      title: "Engineer’s computer",
      url: "github.com/drewsephski/elsewhere/issues",
      tab: "Issues",
      page: {
        kind: "github",
        repo: "drewsephski/elsewhere",
        issues: [
          {
            number: 418,
            title: "Login race on Safari",
            state: "reproduced",
            labels: ["bug", "safari"],
            body: "Sign-in on Safari 18 occasionally lands on /app before the session cookie is written. Reproduced 4/4 in a real browser. Steps and screenshots attached.",
            comments: 3,
            active: true,
          },
          {
            number: 421,
            title: "Settings overflow on iOS",
            state: "reproduced",
            labels: ["bug", "ios"],
            body: "Settings rows clip under the sticky save bar at 390px. Layout pass is in Designer’s file.",
            comments: 1,
          },
          {
            number: 406,
            title: "Composer focus trap",
            state: "reproduced",
            labels: ["a11y"],
            body: "Tabbing out of the composer loops. Fix is a one-line tabIndex on the send button.",
            comments: 2,
          },
          {
            number: 412,
            title: "Webhook retry flaky",
            state: "blocked",
            labels: ["infra"],
            body: "Could not replay. Only fails on staging with the old worker image. Needs a prod-like box.",
            comments: 1,
          },
          {
            number: 415,
            title: "Staging-only 404",
            state: "blocked",
            labels: ["blocked"],
            body: "Looks environment-specific. Did not guess at a patch.",
            comments: 0,
          },
        ],
      },
    },
    routines: [
      { name: "Triage inbox", cadence: "Weekdays 9am" },
    ],
    messages: [
      {
        id: "e1",
        role: "user",
        text: "the last six bug reports — reproduce what you can on the computer and write steps on the issues.",
      },
      {
        id: "e2",
        role: "assistant",
        text: "reproduced 4 of 6 in a real browser. steps and screenshots are on the issues. two look environment-specific; i did not guess. want me to open a patch for the login race?",
      },
    ],
  },
  {
    id: "designer",
    name: "Designer",
    image: "/marketing/job-bots/designer.png",
    time: "Yesterday",
    preview: "invite’s out to sam, helix note added",
    computer: {
      title: "Designer’s computer",
      url: "figma.com/file/settings-redesign",
      tab: "Settings",
      page: {
        kind: "figma",
        file: "Settings · layout pass",
        frames: [
          {
            label: "Settings",
            size: "390",
            note: "One column · sticky save",
            rows: ["Profile", "Notifications", "Connected bots", "Billing"],
            active: true,
          },
          {
            label: "Billing",
            size: "390",
            note: "Hierarchy tightened",
            rows: ["Cloud · Team", "Usage this month", "Invoices", "Payment"],
          },
          {
            label: "Danger",
            size: "390",
            note: "Moved to the bottom",
            rows: ["Export workspace", "Transfer owner", "Delete workspace"],
          },
        ],
      },
    },
    routines: [
      { name: "Visual QA", cadence: "On deploy" },
    ],
    messages: [
      {
        id: "d1",
        role: "user",
        text: "the settings page feels crowded. propose a layout and send sam the invite to review.",
      },
      {
        id: "d2",
        role: "assistant",
        text: "invite’s out to sam. i added a helix note with the hierarchy change: one column on mobile, sticky save, and the danger zone moved. screenshots are in results.",
      },
    ],
  },
  {
    id: "finance",
    name: "Finance",
    image: "/marketing/job-bots/finance.png",
    time: "Yesterday",
    preview: "report filed. 9 receipts, nothing outstanding",
    computer: {
      title: "Finance’s computer",
      url: "expensify.com/reports/sep-8",
      tab: "Report",
      page: {
        kind: "expenses",
        report: "Week of Sep 8 · filed",
        receipts: [
          {
            merchant: "Hilton",
            amount: "$48.12",
            date: "Sep 10",
            category: "Uncategorized",
            state: "Flagged",
            note: "Hotel tax. Ask before guessing.",
            active: true,
          },
          {
            merchant: "Lyft",
            amount: "$18.40",
            date: "Sep 9",
            category: "Travel",
            state: "Matched",
          },
          {
            merchant: "Figma",
            amount: "$15.00",
            date: "Sep 8",
            category: "Software",
            state: "Matched",
          },
          {
            merchant: "Sweetgreen",
            amount: "$24.10",
            date: "Sep 8",
            category: "Meals",
            state: "Matched",
          },
        ],
      },
    },
    routines: [
      { name: "Expense sweep", cadence: "Fridays 4pm" },
    ],
    messages: [
      {
        id: "f1",
        role: "user",
        text: "file last week’s expenses. ask before you guess a category.",
      },
      {
        id: "f2",
        role: "assistant",
        text: "report filed. 9 receipts matched. one hotel tax i could not place — it’s sitting in approvals, not in the filed report.",
      },
    ],
  },
  {
    id: "analyst",
    name: "Analyst",
    image: "/marketing/job-bots/analyst.png",
    time: "Yesterday",
    preview: "3 intros drafted in your voice, held",
    computer: {
      title: "Analyst’s computer",
      url: "docs.google.com/spreadsheets/applicants",
      tab: "Applicants",
      page: {
        kind: "sheet",
        title: "Shortlist · 3 of 22",
        columns: ["Applicant", "Score", "Status"],
        rows: [
          {
            name: "Jules Park",
            score: "9.1",
            status: "Short",
            note: "Intro drafted",
            active: true,
          },
          {
            name: "Mina Cole",
            score: "8.6",
            status: "Short",
            note: "Intro drafted",
          },
          {
            name: "Ari Voss",
            score: "8.2",
            status: "Short",
            note: "Intro drafted",
          },
          {
            name: "Ken Adey",
            score: "5.1",
            status: "Pass",
            note: "Systems depth",
          },
          {
            name: "Priya Shah",
            score: "4.8",
            status: "Pass",
            note: "Writing",
          },
        ],
      },
    },
    routines: [
      { name: "Applicant digest", cadence: "Daily 8am" },
    ],
    messages: [
      {
        id: "a1",
        role: "user",
        text: "read the new applicants against the bar. draft intros, don’t send.",
      },
      {
        id: "a2",
        role: "assistant",
        text: "3 intros drafted in your voice, held. 19 missed the bar on systems depth or writing. the shortlist notes are in results if you want to push any back.",
      },
    ],
  },
  {
    id: "marketing",
    name: "Marketing",
    image: "/marketing/job-bots/marketing.png",
    time: "2:40 AM",
    preview: "paused two ads, report in results",
    computer: {
      title: "Marketing’s computer",
      url: "ads.google.com/campaigns",
      tab: "Campaigns",
      page: {
        kind: "ads",
        heading: "Overnight spend watch",
        campaigns: [
          {
            name: "Search · demos",
            state: "Active",
            change: "+12%",
            spend: "$184",
            conv: "7",
            clicks: "142",
            active: true,
          },
          {
            name: "Display retarget",
            state: "Paused",
            change: "0 conv",
            spend: "$61",
            conv: "0",
            clicks: "40",
          },
          {
            name: "Lookalike · Q3",
            state: "Paused",
            change: "0 conv",
            spend: "$44",
            conv: "0",
            clicks: "28",
          },
          {
            name: "Brand keywords",
            state: "Held",
            change: "untouched",
            spend: "$22",
            conv: "3",
            clicks: "19",
          },
        ],
      },
    },
    routines: [
      { name: "Spend watch", cadence: "Hourly" },
    ],
    messages: [
      {
        id: "m1",
        role: "user",
        text: "watch spend overnight. pause anything that isn’t converting.",
      },
      {
        id: "m2",
        role: "assistant",
        text: "paused two ads with zero conversions after 40 hits. scaled the search campaign that is actually booking demos. writeup is in results — i did not touch brand keywords.",
      },
    ],
  },
];

export const ROSTER_BOTS = [
  {
    id: "researcher",
    image: "/marketing/job-bots/researcher.png",
    name: "Researcher",
    description:
      "Gathers sources overnight, scores intent, and leaves a review list instead of sending.",
  },
  {
    id: "writer",
    image: "/marketing/job-bots/writer.png",
    name: "Writer",
    description:
      "Archives the noise, replies to routine threads, and parks drafts that need your read.",
  },
  {
    id: "analyst",
    image: "/marketing/job-bots/analyst.png",
    name: "Analyst",
    description:
      "Reads every applicant, shortlists against your bar, and drafts the intro emails.",
  },
  {
    id: "finance",
    image: "/marketing/job-bots/finance.png",
    name: "Finance",
    description:
      "Matches receipts to charges, files the report, and asks before guessing a category.",
  },
  {
    id: "engineer",
    image: "/marketing/job-bots/engineer.png",
    name: "Engineer",
    description:
      "Reproduces reports in a real browser and attaches steps to the issue.",
  },
  {
    id: "designer",
    image: "/marketing/job-bots/designer.png",
    name: "Designer",
    description:
      "Inspects live interfaces, proposes layout changes, and saves annotated screenshots.",
  },
  {
    id: "marketing",
    image: "/marketing/job-bots/marketing.png",
    name: "Marketing",
    description:
      "Watches spend daily, pauses what is not converting, and reports what changed.",
  },
  {
    id: "cos",
    image: "/marketing/job-bots/chief-of-staff.png",
    name: "Chief of Staff",
    description:
      "Runs the week: briefings, bookings, and handoffs between your other bots.",
  },
] as const;

export type DemoDockAppId = "browser" | "mail" | "calendar" | "github" | "docs" | "figma";

export function demoDockAppIdForPage(kind: DemoBrowserPage["kind"]): DemoDockAppId | null {
  switch (kind) {
    case "mail":
      return "mail";
    case "calendar":
      return "calendar";
    case "github":
      return "github";
    case "figma":
      return "figma";
    case "sheet":
      return "docs";
    default:
      return null;
  }
}

export function demoComputerForDockApp(
  appId: Exclude<DemoDockAppId, "browser">,
): DemoComputer {
  const kind = appId === "docs" ? "sheet" : appId;
  const bot = DEMO_BOTS.find((item) => item.computer.page.kind === kind);
  if (!bot) {
    throw new Error(`Missing demo computer for ${appId}`);
  }
  return bot.computer;
}
