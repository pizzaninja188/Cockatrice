# Paired capability-research pilot

**Status: designed, not run.** This replaces the next experiment, not the historical results in
[PILOT.md](PILOT.md). The coverage campaign stays paused until separately resumed.

## Question and scope

Does index-assisted research reach a correct, actionable support assessment faster than ordinary
source search for the same remaining deck card? Measure research first. Do not require every card
to be implementable: correctly identifying a missing composition is a useful result. This experiment
does not measure cards delivered per hour, source-fetch savings, or full implementation speed.

Use four paired cases from the pinned deck corpus. Each case gets two fresh researchers: baseline
and index-assisted. Both inspect the same code revision, Oracle record, rulings, relevant official
rules snapshot, and live issue snapshot. One arm may use the capability index. Everything else is
equal. Do not compare different cards between arms or give either arm the selection audit's answers.

## Nominated cases

These four identities are in the pinned corpus and have no shipped definition at design revision
`0ce073fdee9238fb516c5f7d36287d3b8659d076`. Recheck before freezing the run.

| Card | Oracle ID |
|---|---|
| Cultivate | `8b755881-a72d-4e21-a369-d2924eb4585a` |
| Liquimetal Torque | `b7d4b7dd-fbb1-4ca3-875f-ef13a95e66ad` |
| Codex Shredder | `ef7b11ab-24e7-4e7c-91a7-920bade6e60b` |
| Trading Post | `63788566-e25a-44bb-bb55-197e1b93b3e8` |

This is a deliberately selected sample of partial matches and missing compositions, not a random
sample or four promised admissions. It tests support triage on the current remainder. Report this
selection bias. Positive evidence retrieval and correct limits both matter; whole-card exact-hit
authoring speed remains outside this pilot. If an identity is already shipped when setup resumes,
revise and document the cohort before dispatch, never after seeing timed results.
Everflowing Chalice is excluded because an existing entry already names its specific blocker;
prefer targets without a named readiness answer in the index. Inspect for such leakage at freeze.

## Preparation and freeze

1. Recheck the nominated identities against the current registry and pinned Oracle map. Generator
   rejection alone does not establish missing runtime support. Preserve exact Oracle IDs and all
   faces. Record deck-section provenance as unresolved if the pinned corpus lacks it; this does not
   prevent research, but it must be resolved before claiming mainboard coverage.
2. Capture current exact card records and rulings once for both arms. Supply the same relevant
   official rules material and issue snapshot. Record URLs, fetch timestamps and hashes. Preparation
   time is outside researcher timing, but inside total experiment cost. If a source cannot be
   verified, defer that case before either arm starts; do not fill it with guessed text.
3. Finish any small index additions using shipped examples and inspected semantic assertions.
   Index authors must not research either arm. Do not add bespoke target recipes, a target
   readiness verdict, or expected pilot answers to the index. Record index preparation/review time
   separately; do not call this free setup.
   Run the source-only index checker with freshness checking. Inspect changed reviewed paths for
   entries relevant to these cases, repairing or narrowing any stale claim before freeze. A path
   warning is not proof of a semantic defect; do not blanket-bump revisions or expand this into
   a whole-catalogue rewrite. Preserve unresolved unrelated warnings in the setup record.
4. Freeze HEAD, the index file manifest and hashes, source packet hashes, four case identities,
   rubric and time limit in an ignored `build/deck-coverage/research-pilot/manifest.json`. Include
   hashes for any uncommitted documentation used. Keep reference answers and selection notes out
   of worker packets. No runtime or index edits during measurement.
5. Use Luna xhigh for both arms and Sol medium for independent review. Record actual model/effort.
   If that combination is unavailable, stop setup and report it rather than silently comparing
   different settings. Use fresh contexts, with no inherited campaign or audit history.

## Execution

- Run one card pair at a time, with its two researchers concurrently. Alternate which arm is
  dispatched first. Keep other heavy repository work paused and record dispatch/start skew.
  Concurrency affects elapsed time; do not compare summed worker spans with wall time.
- Each worker records a UTC start timestamp before research. Give each researcher a maximum of
  **15 minutes from that recorded start to first submitted packet**, excluding dispatch/start delay.
  Record startup delay separately. A worker that never starts within 15 minutes is an infrastructure
  failure, not a research timeout; retain that outcome and do not infer a speedup from that pair.
  Finish earlier when the requested assessment is complete. On timeout collect the partial packet
  and count the case as incomplete; never discard it or replace it after seeing results.
- Both arms may read the source tree, shipped definitions, review maps and semantic test source.
  Both may read the same frozen source packet. Neither may consult previous campaign conversations,
  previous candidate/research packets, selection notes, the other arm, or pilot results. Baseline
  additionally must not open/search capability-index contents. Neither arm reads this protocol's
  selection notes during timing; supply the identical assignment and operational restrictions
  directly. Scope searches accordingly. Audit tool traces for accidental index/answer exposure;
  mark an exposed pair contaminated and exclude it from timing conclusions, without replacing it
  after results are known. Baseline exclusion is a procedural control, not an access sandbox.
- Researchers inspect only: no code or documentation edits, tests, Cargo, builds, generators,
  formatting, metadata commands, Git writes, or external mutations. Return the packet in the
  agent response; the coordinator records it. A needed source missing from the shared packet is
  a recorded limitation; the coordinator supplies it equally before any predeclared rerun.
- Save both initial packets before review. Normalize their format and replace index citations
  with the underlying source references for the review copy; preserve originals and record this
  transformation. Give Sol packets labeled X/Y in randomized order without arm labels, timings,
  model names or worker identities. This is label-blinded review, not guaranteed perfect blinding.
- Review the actual assertions against source and tests. Do not reward a confident unsupported
  claim. If corrections are required, give each researcher only its own findings and allow one
  correction round capped at **5 minutes from recorded receipt of feedback**. Record correction
  time and unresolved findings. The coordinator interrupts workers at the applicable deadline.

## Identical worker assignment

Supply the card identity, frozen paths/hashes, arm permission and deadline before this text:

> Assess the complete card against the frozen implementation. For every face and Oracle clause,
> identify the closest existing typed expression, validator and runtime consumer, shipped example,
> and exact semantic test with the assertion it proves. Separate runtime support, generator
> recognition and whole-card readiness. Explain missing compositions and material limits rather
> than inferring support from symbols or a nearby card. Identify applicable costs, targets, choices,
> timing, player scope, physical identity, visibility and presentation requirements; mark irrelevant
> surfaces N/A with a reason. Give a concrete next action and focused test plan. Do not implement
> anything. Return at most 800 words plus a compact clause/evidence table. Label unassessed facts
> explicitly. Follow the inspection-only restrictions and stop at the supplied deadline.

The only arm-specific instruction is either “Do not consult capability-index contents; use ordinary
repository search” or “Consult the frozen capability index first, then verify its cited evidence;
do not infer readiness from a nearby pattern.” Both still perform the same source and evidence
checks and produce the same packet fields. Collect index exact/partial/miss labels only after
timing ends, in a separate note kept out of the blinded review packet.

## Review rubric and measurements

Sol reviews five dimensions, scored 0 (missing/wrong), 1 (partial), or 2 (adequate): complete
clause coverage; typed/runtime mapping; semantic evidence accuracy; material boundaries and
interaction risks; actionable next step. A passing packet needs at least 8/10, no missing whole
face or clause, and zero unresolved blocking semantic findings. A false “ready” claim is blocking.
Correctly identifying a blocker can pass. A timeboxed honest “unassessed” result is retained but
does not pass if it lacks the required assessment. The rubric is frozen before dispatch.

For every arm record dispatch, worker start, first response, review start/end, correction feedback
receipt/end and accepted
response timestamps in UTC; elapsed researcher spans; reviewer spans; score; blocking findings;
corrections; timeouts; and contamination or infrastructure failures. Record tool counts and token
usage only if observable. Unknown active time, tokens or monetary cost remain unknown.

Primary report: pass count out of four and per-card researcher elapsed time to the first passing
packet: worker-start to first response plus feedback-receipt to corrected response when required,
excluding startup, review and queue waits. These are elapsed spans, not active effort. A failed arm has
no time-to-pass; do not assign it a successful 15-minute result or silently omit it. For pairs where
both pass, show baseline/indexed ratio and median paired percentage change, alongside all failures.
Also show initial-packet quality, blocking findings per packet, review/correction overhead, total
experiment wall time, summed worker spans, source preparation and index upkeep. Do not describe
dispatch-to-response spans as active effort or infer costs from elapsed time.

Decision rule: a promising signal requires all four indexed packets to pass, no more initial
blocking findings than baseline, and at least 20% lower median paired researcher time with at least
three passing pairs. This is a predeclared practical threshold, not statistical proof. Otherwise
report mixed/inconclusive or worse, with the specific failure mode. Do not add cases retrospectively
to turn a failed threshold into a success.

Estimate index payback separately: index preparation plus review minutes divided by the mean
net paired minutes saved per task, including pairs where the indexed arm was slower. Label this
a rough worker-time estimate; it is unavailable unless all four pairs pass, reliable preparation
spans exist, and net savings are positive. Four cases do
not establish general performance across the deck backlog.

## Optional implementation follow-through

After reporting the paired experiment, only cards independently classified as complete candidates
may enter a separately resumed implementation batch. Resolve source and deck-scope gaps first.
Use the normal single writer, independent semantic review, focused checks, metadata and final
affected-side gates. Do not rerun gates solely for timing. Measure research, implementation,
review/rework, metadata, verification and delivery separately, with overlap recorded.

If none qualify, stop after the research report. Do not introduce a new primitive or expand deck
scope to rescue this experiment. Even successful follow-through is descriptive end-to-end timing,
not a controlled claim that the index reduced minutes per delivered card.

All measurement artifacts stay under ignored `build/`; no card is admitted by this protocol.
MTG applicability: the experiment assesses existing rules evidence and readiness judgments. It
changes no rules behavior and preserves per-card Oracle/rulings checks and normal admission gates.
