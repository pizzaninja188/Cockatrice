//! Registry conformance for the issue #430 reviewed direct-RON cards Sunset Saboteur and
//! Tragedy Feaster.
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
    AbilityPresentation, CastTriggerPlayer, Color, CounterKind, Keyword, Layout,
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
    let registry = tricerules_cards::registry::global();
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
    assert!(ward.effect.is_empty(), "Sunset Saboteur Ward is a Program");
    let program = ward.program.as_ref().expect("Sunset Saboteur Ward Program");
    assert!(program.body.yields.is_empty());
    let [instruction] = program.body.steps.as_slice() else {
        panic!("Sunset Saboteur Ward has one Program leaf");
    };
    assert!(instruction.bind.is_empty());
    assert!(instruction.input.is_none());
    let tricerules_cards::primitives::InstructionKind::Perform(effect) = &instruction.kind else {
        panic!("Sunset Saboteur Ward is a Perform leaf");
    };
    assert_eq!(
        **effect,
        SpellEffectKind::CounterTriggeringStackObjectUnlessPays {
            cost: ResolutionCost::DiscardCard { filter: None },
        }
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
    assert_eq!(
        crate::common::compiled_legacy_target_prompt(&attack.effect, Some(targeting), 0),
        "Choose target creature an opponent controls"
    );
    assert_eq!(group.effect_indices, [0]);
}

/// Tragedy Feaster (`0c4277ec-aa12-41f5-8896-c83cfc754d6e`): Trample, Ward—Discard a card, and
/// "Infusion — At the beginning of your end step, sacrifice a permanent unless you gained life
/// this turn." The record has no rulings. CR 608.2c reads the "unless" on resolution, so it is a
/// ProgramIf rather than an intervening-if; CR 701.21a makes the controller the chooser.
#[test]
fn issue_430_tragedy_feaster_maps_trample_ward_and_unless_sacrifice() {
    use tricerules_cards::primitives::{
        ConditionPlayerSet, GameCondition, InstructionKind, LifeChangeKind, PlayerQuantifier,
        PlayerRecipient, RelativePlayerSet, SacrificeRestriction,
    };

    let registry = tricerules_cards::registry::global();
    let definition = registry.get("tragedy_feaster").expect("registered");
    assert_eq!(
        registry.id_for_name("Tragedy Feaster"),
        Some("tragedy_feaster")
    );
    assert_eq!(definition.layout, Layout::Normal);
    assert_eq!(definition.face_count(), 1);
    let face = definition.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{2}{B}{B}");
    assert_eq!(face.types, ["Creature", "Demon"]);
    assert_eq!(face.colors(), vec![Color::Black]);
    assert_eq!((face.power, face.toughness), (Some(7), Some(6)));
    assert_eq!(face.keywords, [Keyword::Trample]);
    assert!(face.spell_effect.is_empty());
    assert!(face.activated_abilities.is_empty());
    assert!(face.static_abilities.is_empty());
    assert!(face.targeting.is_none());

    let [ward, infusion] = face.triggered_abilities.as_slice() else {
        panic!("Tragedy Feaster has exactly the Ward and end-step triggers");
    };
    assert_eq!(ward.ability_id.as_str(), "triggered_01");
    assert_eq!(ward.presentation, AbilityPresentation::OracleLines(vec![2]));
    assert_eq!(
        ward.trigger,
        TriggerCondition::WheneverSelfBecomesTarget {
            source: TargetingSourceFilter::SpellOrAbility,
            source_controller: CastTriggerPlayer::Opponent,
        }
    );
    let [ward_leaf] = ward
        .program
        .as_ref()
        .expect("Ward Program")
        .body
        .steps
        .as_slice()
    else {
        panic!("Ward has one Program leaf");
    };
    let InstructionKind::Perform(ward_effect) = &ward_leaf.kind else {
        panic!("Ward is a Perform leaf");
    };
    assert_eq!(
        **ward_effect,
        SpellEffectKind::CounterTriggeringStackObjectUnlessPays {
            cost: ResolutionCost::DiscardCard { filter: None },
        }
    );

    assert_eq!(infusion.ability_id.as_str(), "triggered_02");
    assert_eq!(
        infusion.presentation,
        AbilityPresentation::OracleLines(vec![3])
    );
    assert_eq!(
        infusion.trigger,
        TriggerCondition::AtBeginningOfEndStep {
            player: CastTriggerPlayer::Controller,
        },
        "\"your end step\" only"
    );
    assert!(
        infusion.intervening_if.is_none(),
        "CR 608.2c: \"unless\" is checked on resolution, not as an intervening-if"
    );
    assert!(infusion.effect.is_empty());
    assert!(infusion.targeting.is_none());
    let [branch] = infusion
        .program
        .as_ref()
        .expect("end-step Program")
        .body
        .steps
        .as_slice()
    else {
        panic!("one ProgramIf instruction");
    };
    let InstructionKind::If { condition, yes, no } = &branch.kind else {
        panic!("the unless clause is a ProgramIf");
    };
    assert_eq!(
        **condition,
        GameCondition::LifeChangedThisTurn {
            players: ConditionPlayerSet::Relative(RelativePlayerSet::Controller),
            change: LifeChangeKind::Gain,
            quantifier: PlayerQuantifier::Any,
        }
    );
    assert!(yes.steps.is_empty(), "gaining life does nothing");
    let [sacrifice] = no.steps.as_slice() else {
        panic!("not gaining life sacrifices");
    };
    let InstructionKind::Perform(sacrifice) = &sacrifice.kind else {
        panic!("the sacrifice is a Perform leaf");
    };
    assert_eq!(
        **sacrifice,
        SpellEffectKind::ChooseAndSacrifice {
            players: PlayerRecipient::Controller,
            count: 1,
            filter: TargetFilter {
                kind: TargetKind::AnyPermanent,
                ..TargetFilter::default()
            },
            restriction: SacrificeRestriction::None,
        }
    );
}

fn assert_fingerprint_row(id: &str, name: &str, fingerprint: &str) {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    let row = fingerprints
        .lines()
        .find(|line| line.starts_with(&format!("{id}\t")))
        .unwrap_or_else(|| panic!("missing fingerprint row for {id}"));
    let fields: Vec<&str> = row.split('\t').collect();
    assert_eq!(
        fields,
        [id, name, id, name, fingerprint],
        "fingerprint row for {id}"
    );
}

#[test]
fn issue_430_direct_ron_fingerprint_matches_the_pinned_oracle_text() {
    assert_fingerprint_row("sunset_saboteur", "Sunset Saboteur", FINGERPRINT);
    assert_fingerprint_row(
        "tragedy_feaster",
        "Tragedy Feaster",
        "f2f3d2a3a16248c5f9bf15822a2c4c6070bdf34b7eb75321d282d8b1b660e8d5",
    );
}
