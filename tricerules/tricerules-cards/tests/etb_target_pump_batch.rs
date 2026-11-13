//! Registry conformance for the reviewed entry target-pump family.
//!
//! Every card is a normal-layout creature whose only rules text is an entry trigger that gives one chosen creature a printed power/toughness bonus until end of turn.
//! Each fingerprint is SHA-256 of the exact pinned normalized Oracle text.

use tricerules_cards::primitives::{EffectSubject, SpellEffectKind, TargetFilter};
use tricerules_cards::{AbilityPresentation, Layout, TriggerCondition};

type Card = (
    &'static str,
    &'static str,
    &'static str,
    &'static [&'static str],
    u32,
    u32,
    i32,
    i32,
    &'static str,
);

const CARDS: &[Card] = &[
    (
        "guardian_of_pilgrims",
        "Guardian of Pilgrims",
        "{1}{W}",
        &["Creature", "Spirit", "Cleric"],
        2,
        2,
        1,
        1,
        "7572e805dadc9b79467ea2654cc10ce266feafc138e47d1a0ae34257c0136189",
    ),
    (
        "jadecraft_artisan",
        "Jadecraft Artisan",
        "{3}{G}",
        &["Creature", "Merfolk", "Shaman"],
        3,
        3,
        2,
        2,
        "214c73d4f89caea148e7f22491b6d1a87e0cd0ab3207e47928d15b456e43f6a8",
    ),
    (
        "kinsbaile_skirmisher",
        "Kinsbaile Skirmisher",
        "{1}{W}",
        &["Creature", "Kithkin", "Soldier"],
        2,
        2,
        1,
        1,
        "7572e805dadc9b79467ea2654cc10ce266feafc138e47d1a0ae34257c0136189",
    ),
    (
        "rubblebelt_boar",
        "Rubblebelt Boar",
        "{3}{R}",
        &["Creature", "Boar"],
        3,
        3,
        2,
        0,
        "ddca67e5522246ea61bc6eebf6cf652309dbcce0b4e993b6173bab25224751f9",
    ),
    (
        "tenth_district_guard",
        "Tenth District Guard",
        "{1}{W}",
        &["Creature", "Human", "Soldier"],
        2,
        2,
        0,
        1,
        "137e1f1e5f71718aecdd25c18d44ee920ed21a46c95d0f56c2a88566c476a365",
    ),
    (
        "vulshok_heartstoker",
        "Vulshok Heartstoker",
        "{2}{R}",
        &["Creature", "Human", "Shaman"],
        2,
        2,
        2,
        0,
        "ddca67e5522246ea61bc6eebf6cf652309dbcce0b4e993b6173bab25224751f9",
    ),
    (
        "yevas_forcemage",
        "Yeva's Forcemage",
        "{2}{G}",
        &["Creature", "Elf", "Shaman"],
        2,
        2,
        2,
        2,
        "214c73d4f89caea148e7f22491b6d1a87e0cd0ab3207e47928d15b456e43f6a8",
    ),
];

#[test]
fn etb_target_pump_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for &(id, name, mana, types, power, toughness, dp, dt, _) in CARDS {
        let definition = registry.get(id).unwrap_or_else(|| panic!("missing {id}"));
        assert_eq!(definition.layout, Layout::Normal, "{id}");
        assert_eq!(registry.id_for_name(name), Some(id), "{id}");
        let f = face(registry, id);
        assert_eq!(f.face_id.as_str(), id.replace('-', "_"), "{id} face id");
        assert_eq!(f.name, name, "{id} name");
        assert_eq!(f.mana_cost.to_string(), mana, "{id} mana cost");
        assert_eq!(f.types, types, "{id} types");
        assert_eq!(
            (f.power, f.toughness),
            (Some(power), Some(toughness)),
            "{id} body"
        );
        assert!(f.keywords.is_empty(), "{id}");
        assert!(f.activated_abilities.is_empty(), "{id}");
        assert!(f.static_abilities.is_empty(), "{id}");
        assert!(f.spell_effect.is_empty(), "{id}");
        assert!(f.targeting.is_none(), "{id}");

        let [triggered] = f.triggered_abilities.as_slice() else {
            panic!("{id}: exactly one entry trigger");
        };
        assert_eq!(
            triggered.presentation,
            AbilityPresentation::OracleLines(vec![1]),
            "{id}"
        );
        assert_eq!(
            triggered.trigger,
            TriggerCondition::WhenSelfEntersBattlefield,
            "{id}"
        );
        assert!(!triggered.may, "{id}: the target is mandatory");
        assert!(
            matches!(
                triggered.effect.as_slice(),
                [SpellEffectKind::PumpTarget { power: p, toughness: t, subject: EffectSubject::Chosen(filter), .. }]
                    if *p == dp && *t == dt && filter.as_ref() == &TargetFilter::default_creature()
            ),
            "{id}: exact chosen-creature pump"
        );
    }
}

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
fn etb_target_pump_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for &(id, name, .., expected) in CARDS {
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
