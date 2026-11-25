# Handoff after queue-v86 batch A

## Delivered

Queue-v86 batch A: three families, 16 cards, one full batch gate on the final reviewed bytes.
Product commit `6160c9685`; `origin/card-campaign` verified at
`6160c9685cfcb2d44d899279798b2f81c19d8d91`.

| Family | Cards | Gate |
|---|---|---:|
| 01-ally-entry-counter | 6 | `verify-91491640-d516-4a21-b0fd-fd7570b2673a` |
| 02-crew-vehicles | 5 | `verify-91491640-d516-4a21-b0fd-fd7570b2673a` |
| 03-x-entry-counters | 5 | `verify-91491640-d516-4a21-b0fd-fd7570b2673a` |

Cards: Bojuka Brigand, Graypelt Hunter, Kazandu Blademaster, Makindi Shieldmate, Tuktuk Grunts,
Umara Raptor; Aradara Express, Dragonfly Suit, Ovalchase Dragster, Sky Skiff, Thundersteel
Colossus; Ivy Elemental, Nimbus Swimmer, Shifting Wall, Shivan Devastator, Worldsoul Colossus.

## Why a new queue was needed

Queue-v85 was exhausted. `build/campaign/queue-v86/exact-analogue-groups.json` and the exact-clause
scan both came back empty: no remaining ready card shares a complete clause-tuple with a shipped
card, and no two- or three-card exact-text cluster reaches a family band. Queue-v86 therefore groups
**reuse-only cohorts by typed shape**, each built only from primitives a shipped, reviewed card
already exercises.

## Review

- v1 patch `425239156e9d805f0cc01d06c653480038889d4f09c58f0524471e1b23352594`: CHANGES REQUIRED
  from all three reviewers. Material findings: every review map cited the *other* card's
  name-specific canary as covering it (ten false `covers` strings across 16 maps); the crew-4
  payers totalled five power, so a Crew 5 authoring error would have passed; the crew-1
  below-threshold payer was an illegal 0/0 that dies to CR 704.5f; the crew `AddTypes` guard did not
  check the addition carried no creature or land type; the family-02 record overstated which pinned
  rulings the shipped analogue asserts; and the all-colorless rejection supplied three mana for the
  four-cost cards, so it could pass on quantity alone.
- v2 patch `e0ccf155274b745432cdd3096320cdd2302dba66cdbfdd65b9bc5b8fdec67b31`: DELTA CHANGES
  REQUIRED. The 0/0 fix had not landed (the edit target missed on wrapped lines), the family-02
  ruling claim was only partly corrected, and the review brief still described the old inventory.
- v3 patch `d65f5b97023fff0ee6a9f0737de9788ab68e54b19ee7e996c5649f7b0b056992`: DELTA APPROVED
  for all product semantics. The only residuals were the review brief naming the wrong frozen
  bundle and one attribution inside the family-02 record; both are evidence-only changes with zero
  product byte difference, so no further semantic review was required.
- Additional v1 fixes now covered by tests: exact-threshold crew payers (3+1 in the canary, 2+2 in
  the shared loop), an Ornithopter 0/2 below-threshold payer, `creature_types`/`land_types`
  emptiness, per-card canary citations, Defender/Convoke absence assertions, `pool == 0` after the
  Convoke cast, a real `creatures_died` death record instead of a tautological counter read, and
  zero counters asserted on every convoked payer.

`CARDS.md`: 5843 full + 8 partial / 35533 cards across 343 sets (16 new flips). `status.tsv`: three
new verified rows / 16 cards. `blocked.tsv`: no new rows — no card in this batch needed an engine
capability.

## Gate

`pwsh -NoProfile -File ./scripts/verify.ps1 -Side Rust -CardData -CardsXml
build/campaign/source/cards-validation.xml` produced
`verify-91491640-d516-4a21-b0fd-fd7570b2673a`, Status Pass, exit 0: environment, diff check,
CardData, dependency boundary, all five package format checks, Clippy and full Rust tests all passed.

## Notes

- Intended red: `build/campaign/queue-v86/logs/red-v86.log` (19 failed, 0 passed on the final test
  bytes with all sixteen RONs absent).
- Final focused greens: `20261125-135249-666-Rust-scenario-q86-batch-A-focused-green.log` (19
  scenario tests) and `20261125-135250-465-Rust-registry-q86-batch-A-focused.log` (4 registry
  tests).
- Preflight: Clippy, both package format checks and card evidence all exit 0; `git diff --check`
  run separately, exit 0.
- Metadata refresh exit 0; only the 16 expected fingerprint rows and 16 checklist flips changed, no
  unrelated drift.
- Queue-v86 is exhausted after this batch.
- No context compaction occurred. This session delivered three families, reaching the family cap;
  no fourth family was started. No GUI acceptance surface.

## Next

Blockers #515–#532 remain excluded. Remaining leads for a future queue:

- `{M}, {M}: scry N` artifacts (Crystal Ball, Darksteel Pendant, Seer's Lantern) — three cards, each
  with a different second clause (none / Indestructible / `{T}: Add {C}`); all clauses are shipped.
- `{M}, {M}, sacrifice this artifact: draw a card` (Pendulum of Patterns, Spare Supplies, Supply
  Drop) — three cards with different entry clauses; Vexing Bauble's first clause is unsupported, so
  only a coerced remainder exists.
- `when this creature dies, you may draw a card` is already the delivered `08-flying-dies-draw`
  shape; do not re-admit.
- `when this artifact enters, draw a card` (Potion of Healing, Arcum's Astrolabe, Energy Refractor):
  Arcum's Astrolabe is `{S}` snow mana, blocked #516, so only two clean cards remain.
- Investigate remains a genuine capability gap: no `Investigate` primitive exists in the engine and
  no shipped card uses one.
- Infect, morph, banding, horsemanship, rampage, bushido, fear, exalted and fabricate still have no
  shipped keyword support and would each be an engine capability.
- Storage-counter lands do not reach a 3–5 cohort (see the queue-v85 handoff).
- Expand under `build/campaign/queue-v87/` and revalidate against the pinned bulk and rulings.
