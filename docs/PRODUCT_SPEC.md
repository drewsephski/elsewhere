You are acting as the principal software architect, macOS engineer,or this project.

I want to build **Elsewhere**, a production-quality macOS desktop application inspired by the functionality and interaction model of **Grok Bot**.

Reference documentation:
https://docs.x.ai/grok-bot/

I will also provide screenshots of my current Grok Bot setup. Treat those screenshots and the official Grok Bot documentation as behavioral/product references.

## Core objective

Build a **working end-to-end macOS implementation**, not a UI mockup.

The goal is essentially:

> “What would Grok Bot look like if the same general product concept were built around OpenAI/ChatGPT models and a persistent computer running locally on my Mac instead of a cloud computer?”

I want as much functional parity as reasonably possible:

* persistent AI Bots
* persistent conversations and context
* model selection
* an actual computer each Bot can operate
* browser control
* filesystem access
* terminal access
* application/computer use
* skills
* routines / scheduled automations
* background execution
* approvals
* human takeover
* bot-to-bot communication
* groups
* persistent browser sessions
* persistent files
* run history
* agent activity visibility
* settings
* security controls
* recovery
* everything else important that you discover in the Grok Bot documentation

This should be a real application I can install and use on macOS.

---

# 1. START WITH RESEARCH AND REPO INSPECTION

Before changing architecture or writing large amounts of code:

1. Inspect the entire existing repository.
2. Inspect the screenshots I provide.
3. Read the relevant Grok Bot documentation thoroughly.
4. Build a feature matrix of Grok Bot's major capabilities.
5. Determine which capabilities can be recreated locally.
6. Identify which Grok Bot behaviors depend inherently on cloud infrastructure.
7. Design the closest local equivalent.
8. Inspect any existing code, dependencies, architecture, build configuration, and UI before replacing anything.

Do not blindly recreate screenshots.

Understand the system first.

Create a concise internal implementation plan and then proceed with implementation. Do not stop after giving me a plan unless you encounter a genuine blocker that requires information only I can provide.

Make reasonable technical decisions yourself.

---

# 2. FUNCTIONAL PARITY, NOT IP COPYING

Grok Bot is the product reference, but Elsewhere must be its own implementation.

We want:

* comparable workflows
* comparable capabilities
* comparable usability
* comparable information hierarchy where useful
* comparable computer/agent interaction concepts

We do NOT want:

* xAI/Grok copyrighted assets
* Grok branding
* xAI logos
* copied proprietary source code
* copied illustrations
* copied textual content
* trademark-confusing presentation
* pixel-for-pixel reproduction merely for its own sake
* reverse engineering of private APIs or circumventing technical restrictions

Use a **clean-room implementation**.

Call the product **Elsewhere** for now.

Use original visual treatment and components while preserving the interaction patterns that make Grok Bot effective.

Think “functional equivalent,” not “reskin.”

---

# 3. OPENAI / CHATGPT MODEL LAYER

Elsewhere should be designed around OpenAI models.

I want a model selector capable of exposing the OpenAI/ChatGPT models actually available to the authenticated user.

Desired product/model families may include names such as:

* Astra
* Sol / Soul
* Terra
* Luna
* GPT models available through the account
* future models

However:

**Do not fabricate model IDs or assume all of these are programmatically accessible.**

Research the currently supported OpenAI authentication and model-access mechanisms first.

Important distinction:

A paid ChatGPT subscription does not necessarily imply unrestricted OpenAI API access.

Therefore build the AI layer around a clean provider abstraction.

For example:

`ModelProvider`

* authenticate()
* listAvailableModels()
* createResponse()
* streamResponse()
* executeToolLoop()
* reportUsage()
* cancelGeneration()

The UI should populate its model selector from the actual models exposed by the authenticated provider rather than hard-coding fictional availability.

### Authentication rules

Prefer official supported mechanisms.

Possible supported implementations may include:

1. an officially supported OpenAI account-auth mechanism, if one exists for this use case
2. OpenAI API credentials
3. another documented OpenAI SDK/auth path

Do **not**:

* steal browser cookies
* extract ChatGPT session tokens
* circumvent subscription/API restrictions
* depend on an undocumented private endpoint that is likely to break

If ChatGPT-subscription-backed programmatic use is not officially available, clearly isolate that limitation behind the provider layer and support an OpenAI API-backed implementation instead.

The architecture should make switching authentication mechanisms later straightforward.

---

# 4. CORE BOT SYSTEM

A Bot should be a durable AI teammate, not simply another chat thread.

Each Bot should have:

* id
* name
* avatar
* title
* description
* system instructions
* selected model
* working style
* permission profile
* memory
* conversation history
* enabled tools
* enabled skills
* routines
* files
* computer session
* current activity
* run history
* creation/update timestamps

Bots should persist across application restarts.

Users should be able to:

* create
* rename
* configure
* duplicate
* archive/hide
* delete
* pin
* search
* switch between Bots

Design the data model properly rather than serializing everything into random JSON blobs.

SQLite is acceptable for structured local application state if appropriate.

---

# 5. CHAT EXPERIENCE

Implement a high-quality streaming agent conversation interface.

Messages should support:

* Markdown
* code
* tool activity
* screenshots
* attachments
* created files
* approval requests
* computer actions
* errors
* status changes
* long-running jobs
* cancellation
* user interruption while the Bot is working

The transcript should make it obvious what the Bot is currently doing.

Examples:

* Reasoning / planning
* Opening browser
* Navigating to GitHub
* Reading file
* Running terminal command
* Waiting for approval
* Waiting for login
* Writing file
* Handing task to another Bot
* Routine started
* Routine completed
* Routine failed

Do not dump raw implementation logs into the conversation.

Create human-readable agent activity components.

---

# 6. THE “AGENT COMPUTER” — MOST IMPORTANT PART

This is the biggest technical difference from Grok Bot.

Grok Bot has a persistent cloud computer.

Elsewhere must provide a **persistent local agent computer**.

It should behave conceptually like:

`Elsewhere UI`
↓
`Agent Runtime`
↓
`Persistent Computer`
↓
Browser / terminal / filesystem / applications

I initially considered Firecracker.

Do **not** assume Firecracker is correct.

Research the virtualization requirements first.

Firecracker depends on Linux/KVM and therefore is probably not the correct native macOS virtualization layer.

For macOS, seriously evaluate Apple's:

**Virtualization.framework**

as the primary solution.

Also evaluate existing mature projects/libraries when doing so reduces complexity without compromising the product.

The result should be based on the best engineering decision, not my initial Firecracker suggestion.

---

# 7. LOCAL VM ARCHITECTURE

My preferred conceptual architecture is a persistent Linux-based agent environment running locally through macOS virtualization.

The computer should contain at minimum:

* Linux guest OS
* graphical desktop/session
* Chromium
* shell
* filesystem
* common CLI utilities
* networking
* persistent storage
* agent bridge/service

Potential architecture:

`Elsewhere.app`
↕
`Local Agent Daemon`
↕
`VM Manager`
↕
`Persistent Linux VM`
↕
`Agent Bridge`
↕
Browser / desktop / shell / filesystem

You may change this architecture if you can justify something better.

### Persistence

The environment must survive:

* closing Elsewhere
* reopening Elsewhere
* normal VM restarts
* application upgrades

Preserve:

* browser sessions
* cookies
* files
* installed software where appropriate
* `/workspace`
* Bot-created artifacts
* configuration

Provide:

* start
* stop
* suspend
* resume
* restart
* recover
* reset
* snapshot or checkpoint strategy where practical

Do not recreate the VM from scratch every session.

---

# 8. ONE SHARED COMPUTER, MULTIPLE BOTS

Follow Grok Bot's useful shared-computer concept.

Conceptually:

**One user → one persistent Elsewhere computer**

Bots should share:

* filesystem
* installed applications
* browser authentication where appropriate
* command-line tools
* common workspace

But each Bot should maintain its own:

* conversation
* context
* identity
* memory
* active task
* screen/work surface where feasible

Research the best technical implementation.

That might mean:

* multiple desktop sessions inside one VM
* separate browser profiles/contexts
* isolated Wayland/X sessions
* another architecture

Do not fake concurrency in the UI.

If two Bots can actually execute simultaneously, architect it correctly.

Where actual parallel desktop interaction is constrained, queue or arbitrate access safely and communicate that state in the UI.

---

# 9. AGENT COMPUTER VIEWER

Create an **Agent Computer** experience similar in purpose to Grok Bot.

Users should be able to open the computer and watch the agent operate it.

Show:

* desktop/browser
* cursor movement
* navigation
* typing
* current status

Provide controls such as:

* Take Control
* Return Control to Bot
* Pause
* Resume
* Stop
* Restart computer
* open fullscreen

The user should be able to temporarily take over for:

* passwords
* passkeys
* OAuth
* CAPTCHA
* 2FA
* payment confirmation
* sensitive operations
* websites explicitly requiring a human

The model should never be given passwords or authentication secrets unnecessarily.

---

# 10. COMPUTER-USE TOOLING

Design an actual tool interface between the AI and the computer.

The model should have controlled operations such as:

### Browser

* navigate
* click
* type
* scroll
* inspect page
* read DOM/accessibility tree when appropriate
* capture screenshot
* open tab
* close tab
* download file

### Desktop

* screenshot
* click coordinates
* double click
* drag
* type
* press keys
* switch window

### Shell

* execute command
* inspect output
* terminate process
* set working directory

### Files

* list
* read
* write
* move
* copy
* delete
* search

Prefer structured/browser APIs when they are reliable.

Fall back to visual computer use when necessary.

The model should not burn vision tokens clicking blindly through interfaces if a more reliable structured mechanism exists.

---

# 11. LOCAL AGENT DAEMON

The GUI should not own all execution state.

Create a durable local service/daemon responsible for things such as:

* agent jobs
* tool execution
* VM lifecycle
* scheduler
* routines
* task queue
* computer sessions
* IPC
* run history
* notifications
* recovery

Consider a macOS **LaunchAgent** or other appropriate background-service architecture.

The UI should be able to close while a task continues locally.

Be truthful about physical limitations:

A local implementation cannot continue working while the Mac is completely powered off, and sleep may suspend work.

Do not fake Grok Bot's cloud availability.

We can potentially support:

* background execution while the application window is closed
* prevent-sleep option during active routines
* resume queued jobs after wake
* missed-routine recovery

Design this intentionally.

---

# 12. SKILLS

Implement reusable **Skills**.

A Skill is a durable workflow/instruction set that tells a Bot how to perform a repeatable task.

A skill should have:

* name
* description
* instructions
* expected inputs
* required tools
* expected output
* approval rules
* failure behavior
* version/history where reasonable

Users should be able to:

* create manually
* edit
* delete
* enable/disable
* attach to Bots
* invoke from chat
* generate a Skill from a successful task

Example:

User completes a workflow and says:

> Save what we just did as a skill.

The system should produce a structured reusable workflow.

---

# 13. ROUTINES

Implement real scheduled **Routines**.

A routine binds:

* Bot
* instruction/skill
* schedule
* enabled state
* approval policy

Support common schedules:

* once
* hourly
* daily
* weekdays
* weekly
* custom recurrence

Store run history:

* queued
* started
* completed
* failed
* skipped
* approval required
* duration
* error/result

Provide:

* enable
* pause
* edit
* delete
* Run Now
* Test Run
* recent run history

The background daemon should execute routines even when the main Elsewhere window is closed, assuming the Mac is awake.

Handle:

* missed schedules
* timezone
* retry strategy
* idempotency
* stale input
* network loss
* VM unavailable
* model unavailable

---

# 14. MULTI-BOT COLLABORATION

Implement direct Bot-to-Bot delegation.

A Bot should eventually be able to do something like:

> Ask Researcher to investigate this company, then send the findings back to me.

Support:

* direct handoff
* receiving Bot task queue
* response to originating Bot
* traceable delegation
* cancellation
* failures

Also support **group conversations**.

A group may contain multiple Bots.

Examples:

`@Researcher`
Collect sources.

`@Engineer`
Implement the solution.

`@Reviewer`
Inspect it for defects.

Do not merely make multiple model calls with no persistent identity.

Bots need their own durable context and responsibilities.

---

# 15. MEMORY

Implement practical persistent Bot memory.

Separate:

### Conversation history

Raw interaction history.

### Working context

Current task information.

### Durable memory

Stable facts/preferences discovered over time.

### Computer state

Files, browser sessions, installed software, etc.

Memory should be scoped to the relevant Bot unless explicitly shared.

Do not let memory become an uncontrolled ever-growing prompt.

Use:

* summarization
* retrieval
* structured facts
* relevance scoring
* timestamps
* source references where useful

---

# 16. APPROVAL SYSTEM

Build an explicit approval layer.

Potentially dangerous actions should be interceptable before execution.

Examples:

* deleting files
* sending email
* publishing
* posting
* purchasing
* financial actions
* production deployment
* destructive shell commands
* installing system software
* exposing sensitive information
* modifying important accounts

Create policies resembling:

* Always Allow
* Ask
* Always Deny

Support both:

* global defaults
* per-Bot overrides

Approval requests should clearly show:

* intended action
* target
* arguments
* risk
* requesting Bot

Allow:

* Approve Once
* Deny
* optionally Always Allow Similar

Safety policy must exist below the prompt layer.

Do not rely exclusively on “please don't do dangerous things” inside a system prompt.

---

# 17. SECRETS AND SECURITY

Treat security as a first-class architecture concern.

Use macOS Keychain or another appropriate secure storage solution for sensitive credentials.

Do not store secrets in:

* source files
* SQLite plaintext
* logs
* prompts
* conversation history
* Git

Separate secrets from normal app state.

Also think through:

* VM boundary
* host filesystem access
* network access
* localhost services
* command execution
* downloaded files
* malicious webpages
* prompt injection
* malicious attachments
* tool-output injection

Treat webpage content as untrusted data.

---

# 18. CONNECTORS / PLUGINS

Design an extensible connector system even if the first version only ships a small number.

Possible future connectors include:

* Gmail
* Google Calendar
* Google Drive
* GitHub
* Slack
* Linear
* Notion

A connector should expose structured operations to the agent rather than forcing browser automation for everything.

Prefer:

Connector/API → browser automation → raw desktop automation

in that order when appropriate.

Do not delay the entire MVP to implement dozens of connectors.

Build the interface correctly and implement enough to prove the architecture.

---

# 19. FILES AND WORKSPACE

Create a durable shared workspace.

Something conceptually like:

`/workspace`

Provide UI for artifacts produced by Bots.

Support:

* opening
* previewing
* downloading/exporting
* revealing location
* attaching to conversation
* handing a file to another Bot

Track files created during a task so they can appear inline in the conversation.

---

# 20. SEARCH

Implement useful search across:

* Bots
* conversations
* messages
* routines
* skills
* generated files

Start with local indexed search if appropriate.

Do not overengineer semantic search until normal search works properly.

---

# 21. NOTIFICATIONS

Use native macOS notifications for important background events:

* task completed
* task failed
* approval required
* routine completed
* routine failed
* login/takeover required

Clicking a notification should deep-link to the relevant Bot/run when practical.

---

# 22. SETTINGS

Settings should eventually include sections resembling:

### General

* startup
* background service
* notifications
* computer execution
* default model

### AI / Models

* provider
* authentication
* available models
* defaults
* usage

### Agent Computer

* CPU
* RAM
* disk size
* start/stop/rebuild
* storage usage
* VM status

### Permissions

* approvals
* host filesystem
* shell
* browser
* networking

### Connectors

* installed connectors
* authentication
* permissions

### Routines

* timezone
* routine execution preferences

### Advanced

* logs
* diagnostics
* daemon status
* computer reset
* developer options

---

# 23. UI / DESIGN

The screenshots I provide should inform:

* layout
* hierarchy
* interaction density
* navigation
* computer-view behavior
* agent activity presentation

But Elsewhere should establish its own design language.

Aim for:

* sophisticated
* minimal
* dense but understandable
* native-feeling
* extremely polished
* dark/light mode
* keyboard-friendly
* fast

Avoid:

* giant gradients
* excessive rounded cards
* generic AI SaaS styling
* random glowing borders
* unnecessary glassmorphism
* emoji-heavy interfaces
* excessive animations

Use subtle motion only where it improves comprehension.

It should feel closer to a serious developer/productivity application than a marketing-site component library.

---

# 24. MACOS IMPLEMENTATION

Inspect the existing repository first.

If this is greenfield, evaluate an architecture such as:

### Desktop shell

Tauri 2 + React + TypeScript

### Native/system components

Rust and/or small Swift helpers where macOS frameworks require them

### Virtualization

Apple Virtualization.framework

### Persistent daemon

Rust/native background service

### State

SQLite + durable filesystem storage

### Secrets

macOS Keychain

### IPC

Unix socket / local RPC / another robust typed transport

This is a recommendation, not an immutable requirement.

If a SwiftUI-native architecture produces a substantially better result, explain the tradeoff and use it.

Do not choose Electron automatically unless there is a compelling reason.

---

# 25. OBSERVABILITY

Create proper structured logging.

I want to be able to diagnose:

* model failures
* VM failures
* agent crashes
* tool errors
* routine failures
* daemon failures
* browser automation failures

Use meaningful IDs such as:

* botId
* conversationId
* runId
* routineId
* toolCallId

Logs should exclude secrets.

Provide a simple diagnostics view.

---

# 26. FAILURE AND RECOVERY

The application must recover gracefully.

Consider:

* provider timeout
* internet loss
* model rate limit
* malformed tool call
* VM crash
* Chromium crash
* daemon restart
* app restart
* Mac restart
* stale routine
* corrupted computer session
* insufficient disk space

Never leave a task permanently stuck in an unexplained “working” state.

Persist enough execution state to understand what happened.

---

# 27. TESTING

Do not call the project complete because the UI renders.

Create tests for core architecture.

At minimum test:

### Bot lifecycle

* create
* modify
* persist
* reopen
* delete

### Conversations

* streaming
* persistence
* interruption

### Model provider

* model enumeration
* streaming
* errors

### Tools

* shell
* browser
* files

### VM

* create
* boot
* persist
* restart
* execute bridge command

### Routines

* schedule
* execute
* history
* missed/failed runs

### Approvals

* intercept
* approve
* deny

### Daemon

* survives UI close
* reconnects UI

### Recovery

* app restart
* daemon restart

Use integration tests where mocks would give false confidence.

---

# 28. MVP ACCEPTANCE TEST

I should eventually be able to perform this exact workflow:

1. Install Elsewhere on my Mac.

2. Launch it.

3. Configure my OpenAI provider/account.

4. See the models actually available to that account.

5. Create a Bot called `Researcher`.

6. Tell it:

   “Open the web, research the latest developments in autonomous coding agents, save a concise report to the workspace, and show me the sources.”

7. Watch its Agent Computer.

8. See it use Chromium.

9. See it create the report.

10. Close the Elsewhere window while it is working.

11. Reopen Elsewhere.

12. See that the job and conversation persisted.

13. Open the resulting file.

14. Tell the Bot:

“Save this workflow as a skill.”

15. Create a routine that performs it every Monday morning.
16. Run the routine manually as a test.
17. See its execution in routine history.

Then:

18. Create another Bot called `Reviewer`.
19. Ask Researcher to hand its report to Reviewer.
20. Have Reviewer inspect the report.
21. See the handoff represented in the application.

If that works for real, the architecture is on the right track.

---

# 29. IMPLEMENTATION PRIORITIES

Prioritize actual vertical functionality over a huge number of incomplete screens.

Recommended sequence:

**Phase 1 — Foundation**

* application shell
* database
* Bot model
* conversations
* provider abstraction
* real OpenAI call
* streaming

**Phase 2 — Local Computer**

* VM manager
* persistent guest
* agent bridge
* browser
* terminal
* files
* computer viewer

**Phase 3 — Agent Runtime**

* tool loop
* activity events
* cancellation
* permissions
* takeover

**Phase 4 — Persistence**

* durable sessions
* memory
* workspace
* recovery

**Phase 5 — Skills + Routines**

* skills
* scheduler
* daemon
* run history
* notifications

**Phase 6 — Multi-Agent**

* delegation
* groups
* shared computer arbitration

**Phase 7 — Product Polish**

* search
* settings
* diagnostics
* onboarding
* packaging
* updates
* edge cases

It is acceptable to adjust the sequence if repository constraints justify it.

---

# 30. ENGINEERING RULES

Throughout the build:

* Prefer boring, reliable architecture.
* Avoid premature abstractions.
* Do not create fake implementations merely to make UI appear finished.
* No hard-coded fake agent responses.
* No fake routine history.
* No fake VM progress.
* No pretend “connected” states.
* No huge monolithic files.
* No silent error swallowing.
* No credentials in source.
* Keep interfaces typed.
* Keep business logic outside React components.
* Make background processes observable.
* Prefer real integration tests for system boundaries.
* Document important architectural decisions.

If a feature cannot yet function, clearly represent it as unavailable rather than pretending it works.

---

# 31. DOCUMENTATION TO MAINTAIN

As architecture stabilizes, maintain:

`README.md`

* local setup
* dependencies
* development
* build
* installation

`docs/ARCHITECTURE.md`

* process model
* app ↔ daemon communication
* VM architecture
* AI runtime
* persistence
* security boundaries

`docs/GROK_PARITY.md`

* reference functionality
* Elsewhere equivalent
* status
* deliberate differences

`docs/SECURITY.md`

* secrets
* permissions
* agent threats
* VM boundary
* approval architecture

`docs/ROADMAP.md`

* completed
* remaining
* known limitations

Do not spend hours writing documentation before functionality exists. Update it as the implementation becomes real.

---

# 32. DEFINITION OF DONE

I do not want a Grok Bot-looking frontend sitting on top of a normal chatbot.

Elsewhere is successful when it behaves like a persistent AI-computer product.

The core loop must be real:

**User → Bot → OpenAI model → tools → persistent local computer → real work → result → durable context → repeatable skill/routine**

Every major architectural decision should serve that loop.

Use Grok Bot as a very strong reference for product behavior, but solve the local-computer architecture correctly for macOS and make Elsewhere technically independent.

Start by inspecting the repository, screenshots, and Grok Bot documentation.

Determine the existing architecture and the cleanest path toward the system above.

Then begin implementing the highest-value vertical slice immediately.

Do not stop at research, planning, scaffolding, or a collection of TODOs. Continue until the first end-to-end slice genuinely works, test it, fix failures you find, and then proceed to the next logical slice.
