# Handoff after queue-v60 batch A

## Delivered

Queue-v60 batch A: three families, 12 cards, one consolidated full gate. Product commit
`97acb0bca3e5724b5dc870367446f381a2aaf99c`; delivery evidence commit follows.
`origin/card-campaign` was verified at `97acb0bca3e5724b5dc870367446f381a2aaf99c` with `git ls-remote`.

| Family | Cards | Gate | Product commit |
|---|---|---:|---|
| 01-artifact-enchantment-riders | 5 | `verify-321e29d5-5a9b-431b-b1ce-9974f0635da3` | `97acb0bca` |
| 02-unblockable-spells | 4 | `verify-321e29d5-5a9b-431b-b1ce-9974f0635da3` | `97acb0bca` |
| 03-exile-creature-planeswalker | 3 | `verify-321e29d5-5a9b-431b-b1ce-9974f0635da3` | `97acb0bca` |

Cards: Artisan's Sorrow, Crack Open, Expose to Daylight, Ray of Distortion, Slice in Twain;
Infiltrate, Trailblazer, Artful Dodge, Touch of Invisibility; Feed the Serpent, Eat to Extinction,
Vraska's Contempt.

Review: three independent read-only reviewers. Families 02 and 03 approved at revision 1. Family 01
required Crack Open stale-target/no-Treasure evidence, Ray flashback-fizzle exile, and an
indestructible legal-target rider case; the revision 2 delta added those tests and corrected map
fixture attribution, then was approved. Frozen manifest:
`build/campaign/review/q60-batch-a/manifest.json` (revision 2; revision 1 preserved as
`manifest-r1.json`).

`CARDS.md`: 5518 full + 8 partial / 35533 cards across 343 sets. `status.tsv`: three new verified
rows / 12 cards. `blocked.tsv` unchanged (67 rows).

## Gate

`bash build/campaign/run-final-gate.sh` produced
`verify-321e29d5-5a9b-431b-b1ce-9974f0635da3`, exit 0 (`-Side Rust -CardData -CardsXml
build/campaign/source/cards-validation.xml`): environment, early diff check, CardData Check,
dependency boundary, five package format checks, Clippy and full Rust tests all passed. Gate ran on
the frozen reviewed tree before the product commit.

## Notes

- The first prepare attempt (`prepare-card-batch-0ad6f1b4`) failed only the test-file rustfmt check;
  `cargo fmt -p tricerules-core -p tricerules-cards` and the subsequent prepare
  (`prepare-card-batch-403055ca`) passed. The post-delta prepare
  (`prepare-card-batch-55b904bc`) and map check (`card-evidence-57e7c6e9`) also passed.
- Metadata refresh used the pinned bulk and `build/campaign/source/cards-validation.xml`; the diff is
  exactly 12 fingerprint rows plus the CARDS.md count/flips. No unrelated drift.
- Queue-v60 is fully delivered. The exact-normalized-text pool still has no supported three-card
  cohort except the XML-stale Turbulent cycle; the next selection needs queue-v61 under the same
  bands, reusing `build/campaign/queue-v60/*.py` and the pinned inputs.
- #515–#526 remain excluded. No new engine blocker appeared this batch.
- Queue-v60 local evidence (selection, revalidation, sources, rulings, packets, family records,
  review bundle) is retained under `build/campaign/queue-v60/` and `build/campaign/review/q60-batch-a/`.
