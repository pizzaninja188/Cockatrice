//! Registry conformance for the reviewed combat-trick direct-RON batch.
//!
//! Mortify, Withering Torment, Moment of Triumph, Adamant Will, Blossoming Defense, Snakeskin Veil,
//! Take Up the Shield, Maximum Overdrive, Lightfoot Technique and Overprotect were promoted after
//! complete-definition review against the pinned Scryfall snapshot (exact records and `rulings_uri`
//! fetched 2026-09-21). Governance: CR 601.2b/f-h (cost), 115.1/608.2b (targeting and
//! revalidation), 701.7 (destroy), 119.3 (life gain/loss), 613 layer 6 and 611.2c (until-end-of-turn
//! keyword grants), 121.1 (draw is unused here), and 122.1 (counters).

use tricerules_cards::primitives::{
    Amount, EffectSubject, Keyword, LifeAmount, PlayerRecipient, SpellEffectKind, TargetController,
    TargetFilter, TargetKind,
};
use tricerules_cards::{CardRegistry, CounterKind, Layout};

fn chosen(kind: TargetKind, controller: TargetController) -> EffectSubject {
    EffectSubject::Chosen(Box::new(TargetFilter {
        kind,
        controller,
        ..TargetFilter::default()
    }))
}

fn creature_any() -> EffectSubject {
    chosen(TargetKind::Creature, TargetController::Any)
}

fn creature_you() -> EffectSubject {
    chosen(TargetKind::Creature, TargetController::You)
}

fn assert_targeting(face: &tricerules_cards::CardFace, prompt: &str, indices: &[u32]) {
    let [group] = face.targeting.as_ref().expect("targets").groups.as_slice() else {
        panic!("one target group");
    };
    assert_eq!((group.min, group.max), (1, 1));
    assert_eq!(group.prompt, prompt);
    assert_eq!(group.effect_indices, indices);
}

#[test]
fn issue_combat_tricks_batch_maps_definitions() {
    let registry = CardRegistry::global();

    // Destroy a creature or enchantment.
    for (id, name, mana) in [
        ("mortify", "Mortify", "{1}{W}{B}"),
        ("withering_torment", "Withering Torment", "{2}{B}"),
    ] {
        let definition = registry.get(id).expect("registered");
        assert_eq!(definition.name, name);
        assert_eq!(registry.id_for_name(name), Some(id));
        assert_eq!(definition.layout, Layout::Normal);
        let f = definition.primary_face();
        assert_eq!(f.mana_cost.to_string(), mana);
        assert_eq!(f.types, ["Instant"]);
        let destroy = SpellEffectKind::Destroy {
            subject: EffectSubject::Chosen(Box::new(TargetFilter {
                any_of: Some(vec![
                    TargetFilter {
                        kind: TargetKind::Creature,
                        ..TargetFilter::default()
                    },
                    TargetFilter {
                        kind: TargetKind::AnyPermanent,
                        permanent_types: vec![
                            tricerules_cards::primitives::PermanentTypeFilter::Enchantment,
                        ],
                        ..TargetFilter::default()
                    },
                ]),
                ..TargetFilter::default()
            })),
        };
        if id == "mortify" {
            assert_eq!(f.spell_effect, [destroy]);
            assert_targeting(f, "Choose target creature or enchantment", &[0]);
        } else {
            assert_eq!(
                f.spell_effect,
                [
                    destroy,
                    SpellEffectKind::LoseLife {
                        amount: LifeAmount::Fixed(2),
                        who: PlayerRecipient::Controller,
                    },
                ]
            );
            assert_targeting(f, "Choose target creature or enchantment", &[0]);
        }
    }

    // Pump and/or counter plus a keyword grant.
    let cases = [
        (
            "moment_of_triumph",
            "Moment of Triumph",
            "{W}",
            SpellEffectKind::PumpTarget {
                power: 2,
                toughness: 2,
                scale: None,
                subject: creature_any(),
            },
            SpellEffectKind::GainLife {
                amount: Amount::Fixed(2),
            },
            "Choose target creature",
            &[0u32][..],
        ),
        (
            "adamant_will",
            "Adamant Will",
            "{1}{W}",
            SpellEffectKind::PumpTarget {
                power: 2,
                toughness: 2,
                scale: None,
                subject: creature_any(),
            },
            SpellEffectKind::GrantKeywords {
                subject: creature_any(),
                keywords: vec![Keyword::Indestructible],
            },
            "Choose target creature",
            &[0u32, 1][..],
        ),
        (
            "blossoming_defense",
            "Blossoming Defense",
            "{G}",
            SpellEffectKind::PumpTarget {
                power: 2,
                toughness: 2,
                scale: None,
                subject: creature_you(),
            },
            SpellEffectKind::GrantKeywords {
                subject: creature_you(),
                keywords: vec![Keyword::Hexproof],
            },
            "Choose target creature you control",
            &[0u32, 1][..],
        ),
        (
            "snakeskin_veil",
            "Snakeskin Veil",
            "{G}",
            SpellEffectKind::PutCounters {
                counter: CounterKind::PlusOnePlusOne,
                count: Amount::Fixed(1),
                subject: creature_you(),
            },
            SpellEffectKind::GrantKeywords {
                subject: creature_you(),
                keywords: vec![Keyword::Hexproof],
            },
            "Choose target creature you control",
            &[0u32, 1][..],
        ),
        (
            "take_up_the_shield",
            "Take Up the Shield",
            "{1}{W}",
            SpellEffectKind::PutCounters {
                counter: CounterKind::PlusOnePlusOne,
                count: Amount::Fixed(1),
                subject: creature_any(),
            },
            SpellEffectKind::GrantKeywords {
                subject: creature_any(),
                keywords: vec![Keyword::Lifelink, Keyword::Indestructible],
            },
            "Choose target creature",
            &[0u32, 1][..],
        ),
        (
            "maximum_overdrive",
            "Maximum Overdrive",
            "{1}{B}",
            SpellEffectKind::PutCounters {
                counter: CounterKind::PlusOnePlusOne,
                count: Amount::Fixed(1),
                subject: creature_any(),
            },
            SpellEffectKind::GrantKeywords {
                subject: creature_any(),
                keywords: vec![Keyword::Deathtouch, Keyword::Indestructible],
            },
            "Choose target creature",
            &[0u32, 1][..],
        ),
        (
            "lightfoot_technique",
            "Lightfoot Technique",
            "{1}{W}",
            SpellEffectKind::PutCounters {
                counter: CounterKind::PlusOnePlusOne,
                count: Amount::Fixed(1),
                subject: creature_any(),
            },
            SpellEffectKind::GrantKeywords {
                subject: creature_any(),
                keywords: vec![Keyword::Flying, Keyword::Indestructible],
            },
            "Choose target creature",
            &[0u32, 1][..],
        ),
        (
            "overprotect",
            "Overprotect",
            "{1}{G}",
            SpellEffectKind::PumpTarget {
                power: 3,
                toughness: 3,
                scale: None,
                subject: creature_you(),
            },
            SpellEffectKind::GrantKeywords {
                subject: creature_you(),
                keywords: vec![Keyword::Trample, Keyword::Hexproof, Keyword::Indestructible],
            },
            "Choose target creature you control",
            &[0u32, 1][..],
        ),
    ];
    for (id, name, mana, first, second, prompt, indices) in cases {
        let definition = registry.get(id).expect("registered");
        assert_eq!(definition.name, name);
        assert_eq!(registry.id_for_name(name), Some(id));
        let f = definition.primary_face();
        assert_eq!(f.mana_cost.to_string(), mana);
        assert_eq!(f.types, ["Instant"]);
        assert_eq!(f.spell_effect, [first, second]);
        assert_targeting(f, prompt, indices);
    }
}

#[test]
fn issue_combat_tricks_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for (id, name, fingerprint) in [
        (
            "mortify",
            "Mortify",
            "1866812c5542b2b0ca9e9077ec42cc57e427a16048e43aa5d1b8c5c3fa1b80a8",
        ),
        (
            "withering_torment",
            "Withering Torment",
            "bc56da5c59527e6d380f88e47d8c3852b5597c876af7225a5c1268c72ab6bdf6",
        ),
        (
            "moment_of_triumph",
            "Moment of Triumph",
            "ce5296b85107e6bc1a1c8ad94256d0ae6bc9634365c15b6c30ae6072ce63c05a",
        ),
        (
            "adamant_will",
            "Adamant Will",
            "d5acf5af3e90f960c8fcec2f47b5416cc49466a167b3861d8f9979c7b5b12b62",
        ),
        (
            "blossoming_defense",
            "Blossoming Defense",
            "f4895234262d05750afddba4722e3cf70d8bcdad0abf1d163b793135cb0a4035",
        ),
        (
            "snakeskin_veil",
            "Snakeskin Veil",
            "f21bb445c758b70e398fd87c5d3a8881eb575416464d5eafbcdd9f513e9215c4",
        ),
        (
            "take_up_the_shield",
            "Take Up the Shield",
            "7106f5289a8acfc4a9bfc808311f53d01000c4d4e223bda8c8624bd7513e12fd",
        ),
        (
            "maximum_overdrive",
            "Maximum Overdrive",
            "f6f5da57deee56c58cc8e8cc5dfb6be0d316e619668dc01ab1b279e1804e40f0",
        ),
        (
            "lightfoot_technique",
            "Lightfoot Technique",
            "4bbda6468011a334199a43b9f6dc98f319a0951008f33fc10e58f14ebb868881",
        ),
        (
            "overprotect",
            "Overprotect",
            "abc1835a21145bbf6f874954b28dc4e5acb80c00f5847666eb61e21d112a9dfe",
        ),
    ] {
        let row = fingerprints
            .lines()
            .find(|line| line.starts_with(&format!("{id}\t")))
            .unwrap_or_else(|| panic!("missing fingerprint row for {id}"));
        let fields: Vec<&str> = row.split('\t').collect();
        assert_eq!(fields.len(), 5, "fingerprint row shape: {row}");
        assert_eq!(fields[0], id);
        assert_eq!(fields[1], name);
        assert_eq!(fields[4], fingerprint, "fingerprint drift for {id}");
    }
}
