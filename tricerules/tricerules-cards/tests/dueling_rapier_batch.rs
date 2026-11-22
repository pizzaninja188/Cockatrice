//! Registry conformance for the reviewed Dueling Rapier flash Equipment family.
//!
//! Every included card is a normal-layout Equipment with Flash, one entry trigger that
//! attaches it to a chosen creature you control, one attached P/T modifier and one Equip
//! ability. Card names, costs and bonuses are printed data; the shared scenario executes
//! every card. Each fingerprint is SHA-256 of the exact pinned normalized Oracle text.

use tricerules_cards::primitives::{
    AbilityCost, SpellEffectKind, StaticAbilityDef, TargetController, TargetKind, TriggerCondition,
};
use tricerules_cards::{AbilityPresentation, Layout, ManaCost};

type Card = (
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    u32,
    u32,
    &'static str,
);

const CARDS: &[Card] = &[
    (
        "dueling_rapier",
        "Dueling Rapier",
        "{R}",
        "{4}",
        2,
        0,
        "827fa37a6604c4bc4e5a48944c124ef6f69093697960b3bbd4936b5d48dbbd12",
    ),
    (
        "mirran_banesplitter",
        "Mirran Banesplitter",
        "{R}",
        "{3}",
        2,
        0,
        "5fd32465eace071270b488d39a40f6dec27edda26066388f9ba54b3c1adf50d2",
    ),
    (
        "paladins_shield",
        "Paladin's Shield",
        "{1}{W}",
        "{3}",
        0,
        2,
        "689a38e0c0095a730ca3087ccffc56a3154bb94cd286615eb97eac24178520f3",
    ),
];

fn face<'a>(
    registry: &'a tricerules_cards::CardRegistry,
    id: &str,
) -> &'a tricerules_cards::CardFace {
    registry
        .get(id)
        .unwrap_or_else(|| panic!("missing {id}"))
        .primary_face()
}

#[test]
fn dueling_rapier_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for &(id, name, mana, equip_cost, power, toughness, _) in CARDS {
        let definition = registry.get(id).unwrap_or_else(|| panic!("missing {id}"));
        assert_eq!(definition.layout, Layout::Normal, "{id}");
        assert_eq!(registry.id_for_name(name), Some(id), "{id}");
        let f = face(registry, id);
        assert_eq!(f.face_id.as_str(), id, "{id} face id");
        assert_eq!(f.name, name, "{id} name");
        assert_eq!(f.mana_cost.to_string(), mana, "{id} mana cost");
        assert_eq!(f.types, ["Artifact", "Equipment"], "{id} types");
        assert!(f.supertypes.is_empty(), "{id} supertypes");
        assert_eq!(f.power, None, "{id} is not a creature");
        assert_eq!(f.toughness, None, "{id} is not a creature");
        assert_eq!(f.keywords, [tricerules_cards::Keyword::Flash], "{id} flash");
        assert!(f.spell_effect.is_empty(), "{id}");

        let [trigger] = f.triggered_abilities.as_slice() else {
            panic!("{id}: exactly one entry trigger");
        };
        assert_eq!(trigger.ability_id.as_str(), "triggered_01", "{id}");
        assert_eq!(
            trigger.presentation,
            AbilityPresentation::OracleLines(vec![2]),
            "{id} trigger presentation"
        );
        assert_eq!(trigger.trigger, TriggerCondition::WhenSelfEntersBattlefield);
        assert!(
            matches!(
                trigger.effect.as_slice(),
                [SpellEffectKind::AttachSource { target }]
                    if target.kind == TargetKind::Creature
                        && target.controller == TargetController::You
            ),
            "{id}: attach source to a creature you control"
        );
        let groups = &trigger
            .targeting
            .as_ref()
            .unwrap_or_else(|| panic!("{id}: explicit trigger targeting"))
            .groups;
        assert_eq!(groups.len(), 1, "{id}");
        assert_eq!((groups[0].min, groups[0].max), (1, 1), "{id}");
        assert_eq!(groups[0].effect_indices, vec![0], "{id}");

        let [static_ability] = f.static_abilities.as_slice() else {
            panic!("{id}: exactly one static ability");
        };
        assert_eq!(static_ability.ability_id.as_str(), "static_01", "{id}");
        assert_eq!(
            static_ability.presentation,
            AbilityPresentation::OracleLines(vec![3]),
            "{id} static presentation"
        );
        assert!(
            matches!(
                static_ability.definition,
                StaticAbilityDef::AttachedModifier {
                    delta_power: printed_power,
                    delta_toughness: printed_toughness,
                    ..
                } if printed_power == power as i32 && printed_toughness == toughness as i32
            ),
            "{id}: exact attached modifier"
        );

        let [equip] = f.activated_abilities.as_slice() else {
            panic!("{id}: exactly one equip ability");
        };
        assert_eq!(equip.ability_id.as_str(), "activated_01", "{id}");
        assert_eq!(
            equip.presentation,
            AbilityPresentation::OracleLines(vec![4]),
            "{id} equip presentation"
        );
        assert_eq!(
            equip.costs,
            [AbilityCost::Mana(
                ManaCost::parse(equip_cost).expect("equip cost")
            )],
            "{id}"
        );
        assert!(
            matches!(equip.effect.as_slice(), [SpellEffectKind::Equip { .. }]),
            "{id}: equip effect"
        );
    }
}

#[test]
fn dueling_rapier_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for &(id, name, _, _, _, _, expected) in CARDS {
        let row = fingerprints
            .lines()
            .find(|line| line.starts_with(&format!("{id}\t")))
            .unwrap_or_else(|| panic!("missing fingerprint row for {id}"));
        let fields: Vec<&str> = row.split('\t').collect();
        assert_eq!(fields.len(), 5, "fingerprint row shape: {row}");
        assert_eq!(fields[0], id);
        assert_eq!(fields[1], name);
        assert_eq!(fields[2], id);
        assert_eq!(fields[4], expected, "fingerprint drift for {id}");
    }
}
