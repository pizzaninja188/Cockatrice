//! Registry conformance for the issue #430 reviewed direct-RON card Sunset Saboteur.
//!
//! Sunset Saboteur (`c3731735-2b78-4a15-bff4-4d8559401f68`) was promoted after complete-definition
//! review against the pinned Scryfall snapshot. It prints `Menace`, `Ward—Discard a card.`, and
//! `Whenever this creature attacks, put a +1/+1 counter on target creature an opponent controls.`
//! The exact Scryfall record and `rulings_uri` were fetched 2026-09-20; the card has no rulings.
//! CR 702.111 (menace), 702.21a-b (Ward triggers for an opponent's spell or ability and its
//! controller pays), 508.1m/508.3a and 603.2c (attack triggers), and 122.1 (counters) govern.

use tricerules_cards::primitives::{
    Amount, EffectSubject, ResolutionCost, SpellEffectKind, TargetController, TargetFilter,
    TargetGroupDef, TargetKind, TargetingDef, TargetingSourceFilter, TriggerCondition,
};
use tricerules_cards::{
    AbilityPresentation, CardRegistry, CastTriggerPlayer, Color, CounterKind, Keyword, Layout,
};

const FINGERPRINT: &str = "563a00b9247ebe4736401ad582cc90e2e9be66d2621dd3fcf04364aafb92a308";

fn single_group(targeting: &TargetingDef) -> &TargetGroupDef {
    let [group] = targeting.groups.as_slice() else {
        panic!("expected exactly one authored target group");
    };
    group
}

#[test]
fn issue_430_sunset_saboteur_maps_menace_ward_and_attack_trigger() {
    let registry = CardRegistry::global();
    let definition = registry.get("sunset_saboteur").expect("registered");
    assert_eq!(definition.name, "Sunset Saboteur");
    assert_eq!(
        registry.id_for_name("Sunset Saboteur"),
        Some("sunset_saboteur")
    );
    assert_eq!(definition.layout, Layout::Normal);
    assert_eq!(definition.face_count(), 1);
    let face = definition.primary_face();
    assert_eq!(face.face_id.as_str(), "sunset_saboteur");
    assert_eq!(face.mana_cost.to_string(), "{1}{B}");
    assert_eq!(face.types, ["Creature", "Human", "Rogue"]);
    assert_eq!(face.colors(), vec![Color::Black]);
    assert_eq!((face.power, face.toughness), (Some(4), Some(1)));
    assert_eq!(face.keywords, [Keyword::Menace]);
    assert!(face.spell_effect.is_empty());
    assert!(face.activated_abilities.is_empty());
    assert!(face.static_abilities.is_empty());
    assert!(face.custom_effect.is_none());
    assert!(face.modal_spell.is_none());
    assert!(face.targeting.is_none());

    let [ward, attack] = face.triggered_abilities.as_slice() else {
        panic!("Sunset Saboteur has exactly the Ward and attack triggers");
    };

    // Oracle line 2: Ward—Discard a card. The triggering opponent's spell or ability is observed
    // without targeting it; that opponent is the payer (CR 702.21b).
    assert_eq!(ward.ability_id.as_str(), "triggered_01");
    assert_eq!(ward.presentation, AbilityPresentation::OracleLines(vec![2]));
    assert_eq!(
        ward.trigger,
        TriggerCondition::WheneverSelfBecomesTarget {
            source: TargetingSourceFilter::SpellOrAbility,
            source_controller: CastTriggerPlayer::Opponent,
        }
    );
    assert_eq!(
        ward.effect,
        [SpellEffectKind::CounterTriggeringStackObjectUnlessPays {
            cost: ResolutionCost::DiscardCard { filter: None },
        }]
    );
    assert!(ward.targeting.is_none());

    // Oracle line 3: the attack trigger targets only a creature an opponent controls.
    assert_eq!(attack.ability_id.as_str(), "triggered_02");
    assert_eq!(
        attack.presentation,
        AbilityPresentation::OracleLines(vec![3])
    );
    assert_eq!(
        attack.trigger,
        TriggerCondition::WheneverSelfAttacks {
            minimum_other_attackers: 0
        }
    );
    assert_eq!(
        attack.effect,
        [SpellEffectKind::PutCounters {
            counter: CounterKind::PlusOnePlusOne,
            count: Amount::Fixed(1),
            subject: EffectSubject::Chosen(Box::new(TargetFilter {
                kind: TargetKind::Creature,
                controller: TargetController::Opponent,
                ..TargetFilter::default()
            })),
        }]
    );
    let targeting = attack.targeting.as_ref().expect("attack trigger targets");
    let group = single_group(targeting);
    assert_eq!((group.min, group.max), (1, 1));
    assert_eq!(group.prompt, "Choose target creature an opponent controls");
    assert_eq!(group.effect_indices, [0]);
}

#[test]
fn issue_430_direct_ron_fingerprint_matches_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    let row = fingerprints
        .lines()
        .find(|line| line.starts_with("sunset_saboteur\t"))
        .expect("missing fingerprint row for sunset_saboteur");
    let fields: Vec<&str> = row.split('\t').collect();
    assert_eq!(fields.len(), 5, "fingerprint row shape: {row}");
    assert_eq!(fields[0], "sunset_saboteur");
    assert_eq!(fields[1], "Sunset Saboteur");
    assert_eq!(fields[2], "sunset_saboteur");
    assert_eq!(fields[3], "Sunset Saboteur");
    assert_eq!(
        fields[4], FINGERPRINT,
        "fingerprint drift for sunset_saboteur"
    );
}
