# Capability index pilot

**At introduction: NOT RUN; the campaign is PAUSED.** No pilot batches have run and no baseline is
recorded for this pilot. Run this protocol only after the campaign is explicitly resumed. Do not
infer measurements from prior work, commits, or this seeded index.

The pilot asks whether consulting the [capability pattern index](README.md) reduces duplicated
lookup and preflight effort while preserving independent-review quality. Three future completed
batches are the minimum comparison set. The ten seeded entries are candidate lookup aids, not pilot
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
