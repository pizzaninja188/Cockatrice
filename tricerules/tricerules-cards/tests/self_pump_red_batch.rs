//! Registry conformance for the reviewed reviewed red `{1}{R}` self-pump cohort.
//!
//! Every included card is a normal-layout creature with one activated ability that gives itself the printed pump until end of turn.
//! Each included card is executed by the shared actual-card scenarios.
use tricerules_cards::primitives::{AbilityCost, EffectSubject, SpellEffectKind};
use tricerules_cards::{AbilityPresentation, Layout};

type Card = (
    &'static str,
    &'static str,
    &'static str,
    &'static [&'static str],
    &'static [&'static str],
    Option<(u32, u32)>,
    &'static str,
    &'static str,
    u32,
    u32,
);

const CARDS: &[Card] = &[
    (
        "bellows_lizard",
        "Bellows Lizard",
        "{R}",
        &["Creature", "Lizard"],
        &[],
        Some((1, 1)),
        "4adc80147c5e7fe8315e1da8c5b2df57acf7544a37fd36815fcd83de71db55c9",
        "{1}{R}",
        1,
        0,
    ),
    (
        "cavern_thoctar",
        "Cavern Thoctar",
        "{5}{G}",
        &["Creature", "Beast"],
        &[],
        Some((5, 5)),
        "4adc80147c5e7fe8315e1da8c5b2df57acf7544a37fd36815fcd83de71db55c9",
        "{1}{R}",
        1,
        0,
    ),
    (
        "fathom_fleet_firebrand",
        "Fathom Fleet Firebrand",
        "{1}{R}",
        &["Creature", "Human", "Pirate"],
        &[],
        Some((2, 2)),
        "4adc80147c5e7fe8315e1da8c5b2df57acf7544a37fd36815fcd83de71db55c9",
        "{1}{R}",
        1,
        0,
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
fn self_pump_red_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for &(id, name, mana, types, supertypes, body, _, activation, pump_power, pump_toughness) in
        CARDS
    {
        let definition = registry.get(id).unwrap_or_else(|| panic!("missing {id}"));
        assert_eq!(definition.layout, Layout::Normal, "{id}");
        assert_eq!(registry.id_for_name(name), Some(id), "{id}");
        let f = face(registry, id);
        assert_eq!(f.face_id.as_str(), id.replace('-', "_"), "{id} face id");
        assert_eq!(f.name, name, "{id} name");
        assert_eq!(f.mana_cost.to_string(), mana, "{id} mana cost");
        assert_eq!(
            f.types.iter().map(String::as_str).collect::<Vec<_>>(),
            types,
            "{id} types"
        );
        assert_eq!(
            f.supertypes.iter().map(String::as_str).collect::<Vec<_>>(),
            supertypes,
            "{id} supertypes"
        );
        assert_eq!(
            (f.power, f.toughness),
            body.map_or((None, None), |(p, t)| (Some(p), Some(t))),
            "{id} body"
        );
        assert!(f.spell_effect.is_empty(), "{id}");
        assert!(f.targeting.is_none(), "{id}");
        assert!(f.keywords.is_empty(), "{id}");
        assert!(f.protections.is_empty(), "{id}");
        assert!(f.triggered_abilities.is_empty(), "{id}");
        assert!(f.static_abilities.is_empty(), "{id}");
        let [ability] = f.activated_abilities.as_slice() else {
            panic!("{id}: exactly one activated ability");
        };
        assert_eq!(ability.ability_id.as_str(), "activated_01", "{id}");
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![1]),
            "{id} presentation"
        );
        assert!(
            matches!(ability.costs.as_slice(), [AbilityCost::Mana(cost)] if cost.to_string() == activation),
            "{id}: exact pump cost"
        );
        assert!(
            matches!(
                ability.effect.as_slice(),
                [SpellEffectKind::PumpTarget {
                    power,
                    toughness,
                    subject: EffectSubject::Source,
                    ..
                }] if *power == pump_power as i32 && *toughness == pump_toughness as i32
            ),
            "{id}: source-only printed pump"
        );
        assert!(ability.targeting.is_none(), "{id}");
    }
}

#[test]
fn self_pump_red_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for &(id, name, _, _, _, _, expected, _, _, _) in CARDS {
        let row = fingerprints
            .lines()
            .find(|line| line.starts_with(&format!("{id}\t")))
            .unwrap_or_else(|| panic!("missing fingerprint row for {id}"));
        let fields: Vec<&str> = row.split('\t').collect();
        assert_eq!(fields.len(), 5, "fingerprint row shape: {row}");
        assert_eq!(fields[0], id);
        assert_eq!(fields[1], name);
        assert_eq!(fields[2], id.replace('-', "_"));
        assert_eq!(fields[4], expected, "fingerprint drift for {id}");
    }
}
