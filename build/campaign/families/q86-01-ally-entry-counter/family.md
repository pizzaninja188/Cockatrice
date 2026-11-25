# 01-ally-entry-counter — six Allies with the Ally entry counter plus one printed clause

- Queue: `build/campaign/queue-v86`.
- Cards: 6.
- Band: simple single-primitive cohort (6).
- Status: **verified**; independent review verdict below.
- Pinned bulk: SHA-256 `a9a2fd3b550cc5cff4cec9bbc82aaf5fb8e3f781b4b95a896a0d94c15dce7853`.
- Pinned rulings: SHA-256 `3ee07dbf91347f7940b4067ab1c01e0d5097da8f6c27ef620c202ed4262aaf1f`.

## Typed shape and analogues

Every card prints "Whenever this creature or another Ally you control enters, you may put a
+1/+1 counter on this creature." That is exactly
`TriggerCondition::WheneverPermanentEntersBattlefield { controller: Controller, filter:
(permanent_type: Creature), creature_filter: (required_subtypes: ["Ally"]) }` with
`effect: [PutCounters(counter: PlusOnePlusOne, count: 1, subject: Source)]` and `may: true`,
already shipped on Hada Freeblade, Oran-Rief Survivalist and Nimana Sell-Sword
(`q29_ally_counter_batch`).

The family varies the second printed clause, so it is a composition cohort rather than a
normalized-text clone cycle:

| Card | Second clause | Shipped analogue for that clause |
|---|---|---|
| Bojuka Brigand | This creature can't block | Craven Giant (`SelfCombatRestriction`) |
| Graypelt Hunter | Trample | Flowstone Mauler |
| Kazandu Blademaster | First strike, vigilance | Silver Raven / Moorland Inquisitor |
| Makindi Shieldmate | Defender | Alabaster Wall |
| Tuktuk Grunts | Haste | Raging Minotaur |
| Umara Raptor | Flying | Silver Raven |

## Cards

| Card | Oracle ID | Cost | Type | P/T | Second clause | RON | Map |
|---|---|---|---|---|---|---|---|
| Bojuka Brigand | `a1d82070-3198-4ead-9431-d90661e29741` | `{1}{B}` | Creature — Human Warrior Ally | 1/1 | can't block | `data/bojuka_brigand.ron` | `review-maps/bojuka_brigand.json` |
| Graypelt Hunter | `b0553bf6-ecc1-4d08-9777-19827658355b` | `{3}{G}` | Creature — Human Warrior Ally | 2/2 | Trample | `data/graypelt_hunter.ron` | `review-maps/graypelt_hunter.json` |
| Kazandu Blademaster | `133f5d30-d883-493e-93a1-cf9583db460b` | `{W}{W}` | Creature — Human Soldier Ally | 1/1 | First strike, vigilance | `data/kazandu_blademaster.ron` | `review-maps/kazandu_blademaster.json` |
| Makindi Shieldmate | `b8a65e49-2379-42bf-823e-c208c2550aeb` | `{2}{W}` | Creature — Kor Soldier Ally | 0/3 | Defender | `data/makindi_shieldmate.ron` | `review-maps/makindi_shieldmate.json` |
| Tuktuk Grunts | `200ce414-8583-4bbd-88a6-ca30babb613d` | `{4}{R}` | Creature — Goblin Warrior Ally | 2/2 | Haste | `data/tuktuk_grunts.ron` | `review-maps/tuktuk_grunts.json` |
| Umara Raptor | `a58ee84f-1d9c-4924-b7b1-14a9b2ba3b98` | `{2}{U}` | Creature — Bird Ally | 1/1 | Flying | `data/umara_raptor.ron` | `review-maps/umara_raptor.json` |

## Pinned rulings

Bojuka Brigand and Graypelt Hunter have none. The four Zendikar Allies all carry the 2009-10-01
ruling — "This ability triggers on this creature entering, so it will enter with its unmodified
power and toughness, and then receive a +1/+1 counter a short time later. It does not enter with
the +1/+1 counter already on it." — which is exactly the `counter_count == 0` before the optional
choice and `1` after. Packets are under `build/campaign/queue-v86/families/01-ally-entry-counter/`.

## Scenario evidence

Module: `tricerules-core/tests/scenario/q86_batch_a.rs`. Registry coverage:
`tricerules-cards/tests/q86_batch_a.rs`.

- Canary (simplest): Bojuka Brigand — printed can't-block plus the exact counter trigger, with the
  printed body observed before and after the choice.
- Canary (hardest): Kazandu Blademaster — two printed keywords survive the counter.
- Decline path: every card keeps its printed body when the optional trigger is declined.
- Another Ally you control triggers the watcher; the counter lands only on the source.
- A non-Ally and an opponent's Ally never add a counter to the source.
- The printed can't-block restriction is published and still rejects the block after the counter,
  while an unrestricted creature you control may block (CR 509.1b, source-only).
- Every card's printed body and keywords are asserted after a real cast.

## Presentation

`OracleLines([2])` selects the Ally counter line on every card. Bojuka Brigand's
`OracleLines([1])` selects the printed blocking restriction; the five keyword cards model printed
line 1 as `keywords:` and have no separate presentation node for it. Token use: none.

## Focused evidence

- Intended red: `build/campaign/queue-v86/logs/red-v86.log` (19 failed, 0 passed with all sixteen
  RONs absent).
- Focused green (19 scenario tests):
  `build/verification-logs/20261125-122748-122-Rust-scenario-q86-batch-A-focused-green.log`.
- Registry green (4 tests):
  `build/verification-logs/20261125-122749-030-Rust-registry-q86-batch-A-focused.log`.
- Preflight: format, Clippy and card evidence all exit 0; `git diff --check` was run
  separately and also exited 0.
- Metadata refresh: `build/verification-logs/card-data-0e08cb09-bad8-4b78-b3a4-89af0501348e/`;
  diff contains only the 16 expected fingerprint rows and 16 checklist flips.

## Review and gate

- Independent review v1 `product-v1.patch` SHA-256
  `425239156e9d805f0cc01d06c653480038889d4f09c58f0524471e1b23352594`: CHANGES REQUIRED — the
  material finding was that each review map cited the other card's name-specific canary as
  covering it (ten false `covers` strings). Corrected so each map cites only its own canary plus
  the shared loop tests. Clerical fixes: this record's OracleLines wording, the diff-check claim,
  the stale "verified" status, and "Zen-dikar" → "Zendikar".
- Independent review v2 `product-v2.patch` SHA-256
  `e0ccf155274b745432cdd3096320cdd2302dba66cdbfdd65b9bc5b8fdec67b31`: DELTA CHANGES REQUIRED on
  four evidence-accuracy items (one remaining code defect: the crew-1 below-threshold payer was
  still a 0/0; plus three record/brief mismatches).
- Independent review v3 `product-v3.patch` SHA-256
  `d65f5b97023fff0ee6a9f0737de9788ab68e54b19ee7e996c5649f7b0b056992`: DELTA APPROVED for all
  product semantics; the only residuals were the review brief naming the wrong frozen bundle and
  one attribution in this record, both corrected in evidence-only changes with no product byte
  difference.
- Batch queue-v86 batch A full gate `verify-91491640-d516-4a21-b0fd-fd7570b2673a` covers
  01-ally-entry-counter..03-x-entry-counters (16 cards), Status Pass, exit 0: environment, diff
  check, CardData, dependency boundary, all five package format checks, Clippy and full Rust tests
  all passed.
- Product commit `6160c9685`; `origin/card-campaign` verified at
  `6160c9685cfcb2d44d899279798b2f81c19d8d91`.

MTG rules surface: CR 603.6a (the entry watcher includes this creature or another Ally), 603.5 (the
controller may decline), 122.1 (+1/+1 counters), 509.1b (source-only blocking restriction), 702.19
(trample), 702.7 (first strike), 702.20 (vigilance), 702.3 (defender), 702.10 (haste), 702.9
(flying).

Acceptance: rules-data only; no GUI surface (N/A).
