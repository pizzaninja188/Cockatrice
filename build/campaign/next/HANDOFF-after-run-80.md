# Handoff after queue-v80 batch A

## Delivered

Queue-v80 batch A: three families, 12 cards, one full batch gate on the final reviewed bytes.
Product commit `8db0ff001b53ac6290545542119ccbce34bb31e9`; `origin/card-campaign` verified at
`8db0ff001` with `git ls-remote` before the evidence commit.

| Family | Cards | Gate |
|---|---|---:|
| 01-sliver-pump-grant | 3 | `verify-7230649c-8d9f-4a9c-8007-2f6539e126dd` |
| 02-protection-keywords | 5 | `verify-7230649c-8d9f-4a9c-8007-2f6539e126dd` |
| 03-kicker-spells | 4 | `verify-7230649c-8d9f-4a9c-8007-2f6539e126dd` |

Cards: Armor Sliver, Barbed Sliver, Spectral Sliver; Iridescent Angel, Oversoul of Dusk, Sabertooth
Nishoba, Sphinx of the Steel Wind, Akroma, Angel of Wrath; Blink of an Eye, Eject the Warp Core,
Final Flourish, Roil Eruption.

Review: shared frozen bundle `q80-batch-a`. Final patch `product-v4.patch` SHA-256
`cfd963ef78b55d61396d67ce702ce94ab178c6e3e8b1aa88d32bc078ad473d9a`; sliver APPROVED on v4 after
the presentation-exception fix, protection APPROVED on v2 after CR-citation/file-stem corrections,
kicker APPROVED on v1. Material findings were evidence-only.

`CARDS.md`: 5776 full + 8 partial / 35533 cards across 343 sets (12 new flips).
`status.tsv`: three new verified rows / 12 cards. `blocked.tsv` unchanged.

## Gate

`bash build/campaign/run-final-gate.sh` produced
`verify-7230649c-8d9f-4a9c-8007-2f6539e126dd`, exit 0
(`-Side Rust -CardData -CardsXml build/campaign/source/cards-validation.xml`): environment, diff
check, CardData, dependency boundary, package format checks, Clippy and full Rust tests all passed.

The first gate attempt failed only on three presentation-audit UNRESOLVED `Fallback` children for the
sliver grants. The audit reads `presentation-exceptions.tsv`, not review-map spans; three quoted-child
exception rows fixed it and a standalone CardData Check then passed before the successful gate.

## Notes

- Intended red: `20261124-181317-255-Q80-sliver-canary-intended-red-v2.log`,
  `20261124-183504-817-Q80-protection-canary-intended-red.log`,
  `20261124-184118-451-Q80-kicker-canary-intended-red.log`.
- Final focused greens: `20261124-195234-383-Q80-all-focused-scenarios-v3.log` (14 scenario tests)
  and `20261124-195314-510-Q80-all-registry-tests-v4.log` (6 registry tests); focused Clippy
  `20261124-195745-435-Q80-focused-clippy.log`; preparation
  `20261124-195330-705-Q80-card-evidence-preparation-v3.log`.
- Card identity trap: Akroma's `id` is `akroma,_angel_of_wrath` (comma retained), `face_id` is
  `akroma_angel_of_wrath`, and its RON/map stems use the portable id (`akroma,_angel_of_wrath.*`).
- Kicker composition detail: Final Flourish/Eject's printed -6/-6 "instead" is implemented as base
  `-2/-2` plus conditional `-4/-4`, matching Stomped by the Foot's additive composition.
- Queue-v80 is exhausted. This session delivered three families, reaching the campaign family cap;
  no fourth family was started. No context compaction occurred.

## Next

Expand under `build/campaign/queue-v81/`. Exact-text cohorts remain exhausted. Blockers #515–#530
remain excluded. Revalidated leads from the queue-v80 scan:

- "Activated abilities of artifacts can't be activated" (Collector Ouphe, Null Rod, Stony Silence)
  needs a global static prohibition; the admitted static form only prohibits the attached permanent.
- "Each player can't cast more than one spell each turn" (Arcane Laboratory, Eidolon of Rhetoric,
  Rule of Law) has no typed per-turn cast limit.
- "Change the target of target spell with a single target" (Deflection, Shunt, Swerve) has no typed
  stack-target redirection.
- "Take an extra turn after this one. At the beginning of that turn's end step, you lose the game"
  needs a turn-scoped delayed loss not in the admitted extra-turn support.
- Artifacts that pump a creature for as long as the artifact remains tapped (Endoskeleton, Spirit
  Shield, Tawnos's Weaponry, Zelyon Sword) need a state-linked duration.
- The ten Star Trek Planet gain-life dual lands are in the pinned bulk but absent from
  `cards-validation.xml` and `CARDS.md`; presentation-source work would be required first.
