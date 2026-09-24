mod common;

use common::FaceExpectation;
use tricerules_cards::primitives::{
    CastTriggerPlayer, EffectSubject, SpellEffectKind, TriggerCondition,
};
use tricerules_cards::{CardRegistry, CounterKind, Keyword};

#[test]
fn issue_296_accepts_every_successful_draw_trigger_without_an_ordinal() {
    let draft = r#"(
        id: "ravenhill_flock", name: "Ravenhill Flock", face_id: "ravenhill_flock",
        mana_cost: "{3}{U}", types: ["Creature", "Bird"], power: 1, toughness: 2,
        keywords: [Flying],
        triggered_abilities: [(
            ability_id: "triggered_01", presentation: OracleLines([2]),
            trigger: WheneverPlayerDrawsCard(drawer: Controller),
            effect: [PutCounters(counter: PlusOnePlusOne, count: 1, subject: Source)],
        )],
    )"#;
    CardRegistry::from_authoring_draft(draft).expect("every-draw trigger is a typed contract");
}

#[test]
fn issue_296_registers_both_complete_cards_with_distinct_characteristics() {
    for (id, name, types, pt) in [
        (
            "ravenhill_flock",
            "Ravenhill Flock",
            &["Creature", "Bird"][..].as_ref(),
            (1, 2),
        ),
        (
            "clinquant_skymage",
            "Clinquant Skymage",
            &["Creature", "Bird", "Wizard"][..].as_ref(),
            (1, 1),
        ),
    ] {
        assert_eq!(CardRegistry::global().id_for_name(name), Some(id));
        let face = FaceExpectation {
            id,
            name,
            face_id: id,
            mana_cost: "{3}{U}",
            types,
            keywords: &[Keyword::Flying],
            power_toughness: Some(pt),
        }
        .check();
        let [ability] = face.triggered_abilities.as_slice() else {
            panic!("one trigger for {name}")
        };
        assert!(matches!(
            ability.trigger,
            TriggerCondition::WheneverPlayerDrawsCard {
                drawer: CastTriggerPlayer::Controller
            }
        ));
        assert!(matches!(
            ability.effect.as_slice(),
            [SpellEffectKind::PutCounters {
                counter: CounterKind::PlusOnePlusOne,
                subject: EffectSubject::Source,
                ..
            }]
        ));
        assert!(face.spell_effect.is_empty());
    }
}
