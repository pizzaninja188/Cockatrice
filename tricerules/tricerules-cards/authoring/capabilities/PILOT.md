# Capability index pilot

The next experiment is the [paired research pilot](RESEARCH-PILOT.md), designed but not run.
It compares the same remaining cards in fresh baseline/index-assisted contexts. The historical
three-batch workflow and results below are retained as evidence, not instructions to repeat it.

**Pilot status: COMPLETE (three batches, 2026-09-27).** The baseline was the first batch without
index lookup; batches 2 and 3 used lookup-first. Initial seed effort and comparable per-batch
lookup/revalidation effort were not captured, so the pilot cannot quantify time saved. Do not infer
effort from commits or batch wall time.

The pilot asked whether consulting the [capability pattern index](README.md) reduces duplicated
lookup and preflight effort while preserving independent-review quality. Three completed batches
formed the minimum comparison set. The ten seeded entries were candidate lookup aids, not pilot
results or proof of support for a new card.

## Workflow

1. Establish the comparison baseline before the three batches. Prefer existing phase timings only
   when contemporaneous records contain actual timestamps and supporting evidence; mark them
   unavailable rather than estimating from commit spacing or reconstructing undocumented effort. If
   no usable baseline exists, the first future batch is conventional research without an index
   lookup, and the next two batches use the index. Do not duplicate card work or rerun commands just
   to create a baseline.
2. Before a batch starts, select a cohort around one genuinely shared behavior or composition.
   Use 3–5 cards only when their mechanics, source checks, and useful focused checks are compatible;
   use a smaller cohort when the evidence or complexity differs.
3. Prepare at most one batch ahead: identify candidate cards, inspect likely index entries and their
   limits, and plan source checks. Do not author the next batch early or let preparation bypass the
   per-card source review. For the conventional baseline batch, skip only the index lookup.
4. For each card, fetch the exact current Scryfall record and its rulings, then check relevant rules
   sources as required by the [card authoring guide](../CARD-AUTHORING.md). Record which entry was an
   exact hit, partial match, or miss, and note when revalidation changed the planned approach.
5. Implement and check the complete card. An entry routes research; it does not establish that the
   card is complete, that a generator recognizes it, or that a composition works. Keep focused
   checks tied to independent expected outcomes.
6. Require independent review of actual definitions, behavior, and evidence. Record review findings
   and rework, including whether an entry helped, misled, or failed to cover the case.
7. Capture timestamps and effort as the work happens. Run only the normal required checks once when
   valid; do not repeat commands solely to obtain timing data. Distinguish agent/worker effort from
   elapsed batch time, and record overlapping work separately rather than adding it to wall time.
8. Complete three batches before comparing results. Describe differences in cohort complexity and
   interruptions; do not claim the pilot proves causation.

Fresh source checks and independent review are required for every batch. Follow the
[dependency report](../DEPENDENCY-REPORT.md) and
[verification guide](../../../../docs/AGENT-VERIFICATION.md) for applicable commands; this pilot does
not change card-admission or verification gates.

## Measurement record

Keep a copy of this record in the working chat or an ignored `build/` artifact. Do not create a
persistent campaign tracker or add completed measurement records to the capability catalogue. Fill
fields prospectively; leave unknown values blank and explain why rather than reconstructing them.
Record initial index seeding effort once at the pilot level, outside the per-batch records, with
supporting time evidence or mark it unavailable. Do not copy or amortize that one-time value across
the batch forms below.

```text
Batch number (1 of 3 / 2 of 3 / 3 of 3):
Batch start UTC / end UTC:
Completed card IDs:
Cohort rationale and complexity (including material differences):

Baseline method (evidenced historical timings / first batch without index lookup):
Baseline records, capture timestamps/revision, and evidence; or why unavailable:
Candidate/source snapshot and timestamps (Scryfall record + rulings per card):
Capability index revision:
Entries consulted; result per card (exact hit / partial / miss / no entry; or index skipped for
  the conventional baseline batch):
Lookup, source research, and revalidation: effort by worker + elapsed span:
Implementation and focused checks: effort by worker + elapsed span:
Metadata work/check (if applicable): effort by worker + elapsed span:
Independent review: reviewer, scope, effort, elapsed span, actionable findings:
Rework: effort + elapsed span; reason; affected card IDs; relevant entry IDs or miss:
Final verification: commands/checks, result, evidence reference, effort by worker + elapsed span:
Delivery: completed artifact/revision and date; effort by worker + elapsed span (if authorized):

Worker/phase effort (record concurrent workers separately):
Overlapping intervals and workers (do not sum into wall time):
Interruptions (duration, reason, and whether work resumed):
Unplanned lookup or revalidation effort caused by an entry miss/stale claim:
Other observations affecting comparability:
```

Use the same phase boundaries and correction definition across all three batches. Report lookup and
revalidation savings against the stated baseline. Record the index's initial seeding effort separately
from marginal per-batch upkeep; use evidenced time or mark it unavailable, and do not divide that
one-time cost across three batches. Show the observed comparison horizon. The comparison supports
retaining the index only when measured savings exceed marginal upkeep and the independent-review
correction rate does not increase. Show the correction count and denominator, and account for
complexity rather than pooling unlike cohorts. Three batches provide a practical decision signal, not
a causal guarantee.

## Completed pilot comparison

The pilot used the fallback baseline because no contemporaneous historical phase timings existed.
Batch 1 skipped only index lookup. Batches 2 and 3 consulted the index. These cohorts differ in
scope and complexity, so their total elapsed time is descriptive only.

| Batch | Completed cards | Index result | Batch elapsed | Review corrections |
|---|---:|---|---:|---:|
| 1 — Vivid Grove | 1 | Skipped (conventional baseline) | 1h 14m 28s | 0 blocking / 1 card |
| 2 — Astral Cornucopia | 1 | No exact entry; one partial analogy | 2h 31m 47s | 2 blocking / 1 card |
| 3 — Bridgeworks Battle // Tanglespan Bridgeworks; Sundering Eruption // Volcanic Fissure | 2 | No matching entry; miss/unassessed | 1h 39m 54s | 0 blocking / 2 cards |

Batch 3 ran from 08:55:52 UTC through remote delivery verification at 10:35:46 UTC. Phase evidence
is in the ignored `build/deck-coverage/capability-pilot-batch-{1,2,3}.txt` records and cited
verification logs. Approximate captured elapsed spans (not worker effort) are:

| Batch | Research/source | Implementation/focused checks | Metadata | Sol review | Final gate | Delivery |
|---|---|---|---|---|---|---|
| 1 | 16m 06s for two recorded screening/source windows; active effort unknown | Not fully timed; setup corrections and focused checks recorded in batch record | 7m 51s | Elapsed/effort unavailable | 8m 22s | About 1m 42s after final gate; active effort unknown |
| 2 | Active lookup/fetch time unavailable | About 70m 21s from first preserved source-packet milestone through last focused Clippy result; this is a broad span, not isolated coding effort | 7m 39s | Elapsed/effort unavailable | 18m 15s | About 2m 57s to issue reconciliation after final gate; active effort unknown |
| 3 | 24m 02s from candidate report to exact live-source/rulings snapshot; active effort unknown | About 29m 43s from test creation to final focused scenario pass, overlapping root and Luna | 5m 30s | Elapsed/effort unavailable | 7m 14s | 9m 14s from final gate completion to verified remote push; active effort unknown |

Do not add these phase spans: some overlap across workers, and the available timestamps have
different boundaries. Worker-active effort was not consistently recorded. Initial ten-entry index
seeding effort is unavailable. For batch 2, the lookup miss and partial analogy did not isolate
revalidation duration; batch 3 had no matching entry and its source/revalidation window is not
comparable to batch 1's screened candidate/source windows. Therefore measured lookup savings versus
baseline are **unavailable**, not zero. The repeated lookup observations are that the index did not
supply a directly reusable answer for either indexed cohort; it did motivate two bounded entries
after the relevant deliveries.

Using blocking independent-review findings per reviewed whole-card identity, counts were 0/1,
2/1, and 0/2. Optional wording suggestions are excluded from the numerator: batch 1's phrasing
suggestion and batch 3's evidence-wording suggestion were still corrected. The second batch's two
required corrections exceed the baseline rate; cohorts also differ materially (the second batch
introduced a shared engine/client primitive, while the other two reused established entry and spell
primitives). The evidence does not show that the index caused this difference. It also does not meet
the protocol's evidence threshold for measured savings exceeding marginal upkeep with no increase
in review corrections. Treat the three batches as a descriptive pilot, not proof that lookup-first
reduced elapsed time or improved quality.

Rework causes are recorded in each ignored batch record: batch 1 had two fixture/setup corrections,
one optional wording correction, and an initial format failure; batch 2 had two blocking Sol
findings, early test setup attempts, and an unavailable required clang-format v16 executable; batch
3 had two setup-red attempts before valid expected-red tests, two dynamic-blocker fixture failures,
a corrected nested JSON-pointer path in a review map, one initial format failure, and one optional
evidence-wording correction. No command was rerun solely to measure time. The final gates completed
for all three batches; batch 2's unavailable C++ clang-format v16 remains recorded as a limitation of
that batch's C++ formatting evidence.
