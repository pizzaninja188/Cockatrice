//! Registry conformance for the reviewed reviewed green-scaling self-pump cohort.
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
        "gravel-hide_goblin",
        "Gravel-Hide Goblin",
        "{1}{R}",
        &["Creature", "Goblin", "Shaman"],
        &[],
        Some((2, 1)),
        "bf9f2b7a8ea491353b03ecf3b9a46ddba46ae840248b089f3f256075de98cbd0",
        "{3}{G}",
        2,
        2,
    ),
    (
        "iridescent_blademaster",
        "Iridescent Blademaster",
        "{1}{G}",
        &["Creature", "Elf", "Warrior"],
        &[],
        Some((2, 2)),
        "bf9f2b7a8ea491353b03ecf3b9a46ddba46ae840248b089f3f256075de98cbd0",
        "{3}{G}",
        2,
        2,
    ),
    (
        "devkarin_dissident",
        "Devkarin Dissident",
        "{1}{G}",
        &["Creature", "Elf", "Warrior"],
        &[],
        Some((2, 2)),
        "cc6cbc19740aa5087771f13ffd74678258be888d403a6e03315d327f91611590",
        "{4}{G}",
        2,
        2,
    ),
    (
        "kraul_warrior",
        "Kraul Warrior",
        "{1}{G}",
        &["Creature", "Insect", "Warrior"],
        &[],
        Some((2, 2)),
        "c973b05734edc31a026e67b88344ac1a44529ccb977fdc211431de27e69dad65",
        "{5}{G}",
        3,
        3,
    ),
    (
        "quilled_wolf",
        "Quilled Wolf",
        "{1}{G}",
        &["Creature", "Wolf"],
        &[],
        Some((2, 2)),
        "f164478ea7048d0eb44b1186d1834509e39916756149f4af5629ff28137f8124",
        "{5}{G}",
        4,
        4,
    ),
    (
        "stonewood_invoker",
        "Stonewood Invoker",
        "{1}{G}",
        &["Creature", "Elf", "Mutant"],
        &[],
        Some((2, 2)),
        "c6994a446b88d274b967ce899750a029645301c69b6eb2445e96e94fea9cfee9",
        "{7}{G}",
        5,
        5,
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
fn self_pump_green_batch_maps_definitions() {
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
fn self_pump_green_batch_fingerprints_match_the_pinned_oracle_text() {
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
