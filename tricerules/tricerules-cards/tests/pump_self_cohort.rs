//! Registry conformance for the reviewed self-pump creature cohort.
//!
//! Every included card is a normal-layout single-face creature with one
//! `{cost}: This creature gets +N/+M until end of turn.` activated ability. Card names, costs,
//! types, stats and activation costs are printed data; the shared scenario executes every card.
//! Each fingerprint is SHA-256 of the exact pinned normalized Oracle text.

use tricerules_cards::primitives::{AbilityCost, EffectSubject, SpellEffectKind};
use tricerules_cards::{AbilityPresentation, Layout, ManaCost};

enum Cost {
    Mana(&'static str),
    Tap,
}

type Card = (
    &'static str,
    &'static str,
    &'static str,
    &'static [&'static str],
    u32,
    u32,
    Cost,
    i32,
    i32,
);

const CARDS: &[Card] = &[
    (
        "adarkar_sentinel",
        "Adarkar Sentinel",
        "{5}",
        &["Artifact", "Creature", "Soldier"],
        3,
        3,
        Cost::Mana("{1}"),
        0,
        1,
    ),
    (
        "angelfire_crusader",
        "Angelfire Crusader",
        "{3}{W}",
        &["Creature", "Human", "Soldier", "Knight"],
        2,
        3,
        Cost::Mana("{R}"),
        1,
        0,
    ),
    (
        "boa_constrictor",
        "Boa Constrictor",
        "{4}{G}",
        &["Creature", "Snake"],
        3,
        3,
        Cost::Tap,
        3,
        3,
    ),
    (
        "capashen_templar",
        "Capashen Templar",
        "{2}{W}",
        &["Creature", "Human", "Knight"],
        2,
        2,
        Cost::Mana("{W}"),
        0,
        1,
    ),
    (
        "carrion_ants",
        "Carrion Ants",
        "{2}{B}{B}",
        &["Creature", "Insect"],
        0,
        1,
        Cost::Mana("{1}"),
        1,
        1,
    ),
    (
        "dragon_engine",
        "Dragon Engine",
        "{3}",
        &["Artifact", "Creature", "Dragon", "Construct"],
        1,
        3,
        Cost::Mana("{2}"),
        1,
        0,
    ),
    (
        "dread_shade",
        "Dread Shade",
        "{B}{B}{B}",
        &["Creature", "Shade"],
        3,
        3,
        Cost::Mana("{B}"),
        1,
        1,
    ),
    (
        "fetid_horror",
        "Fetid Horror",
        "{3}{B}",
        &["Creature", "Shade", "Horror"],
        1,
        2,
        Cost::Mana("{B}"),
        1,
        1,
    ),
    (
        "firescreamer",
        "Firescreamer",
        "{3}{B}",
        &["Creature", "Kavu"],
        2,
        2,
        Cost::Mana("{R}"),
        1,
        0,
    ),
    (
        "flame_spirit",
        "Flame Spirit",
        "{4}{R}",
        &["Creature", "Elemental", "Spirit"],
        2,
        3,
        Cost::Mana("{R}"),
        1,
        0,
    ),
    (
        "frozen_shade",
        "Frozen Shade",
        "{2}{B}",
        &["Creature", "Shade"],
        0,
        1,
        Cost::Mana("{B}"),
        1,
        1,
    ),
    (
        "hoar_shade",
        "Hoar Shade",
        "{3}{B}",
        &["Creature", "Shade"],
        1,
        2,
        Cost::Mana("{B}"),
        1,
        1,
    ),
    (
        "honor_guard",
        "Honor Guard",
        "{W}",
        &["Creature", "Human", "Soldier"],
        1,
        1,
        Cost::Mana("{W}"),
        0,
        1,
    ),
    (
        "jousting_dummy",
        "Jousting Dummy",
        "{2}",
        &["Artifact", "Creature", "Scarecrow", "Knight"],
        2,
        1,
        Cost::Mana("{3}"),
        1,
        0,
    ),
    (
        "kjeldoran_outrider",
        "Kjeldoran Outrider",
        "{1}{W}",
        &["Creature", "Human", "Soldier"],
        2,
        2,
        Cost::Mana("{W}"),
        0,
        1,
    ),
    (
        "llanowar_vanguard",
        "Llanowar Vanguard",
        "{2}{G}",
        &["Creature", "Dryad"],
        1,
        1,
        Cost::Tap,
        0,
        4,
    ),
    (
        "loch_korrigan",
        "Loch Korrigan",
        "{3}{B}",
        &["Creature", "Spirit"],
        1,
        1,
        Cost::Mana("{U/B}"),
        1,
        1,
    ),
    (
        "looming_shade",
        "Looming Shade",
        "{2}{B}",
        &["Creature", "Shade"],
        1,
        1,
        Cost::Mana("{B}"),
        1,
        1,
    ),
    (
        "molten_ravager",
        "Molten Ravager",
        "{2}{R}",
        &["Creature", "Elemental"],
        0,
        4,
        Cost::Mana("{R}"),
        1,
        0,
    ),
    (
        "nantuko_shade",
        "Nantuko Shade",
        "{B}{B}",
        &["Creature", "Insect", "Shade"],
        2,
        1,
        Cost::Mana("{B}"),
        1,
        1,
    ),
    (
        "narstad_scrapper",
        "Narstad Scrapper",
        "{5}",
        &["Artifact", "Creature", "Construct"],
        3,
        3,
        Cost::Mana("{2}"),
        1,
        0,
    ),
    (
        "parapet_watchers",
        "Parapet Watchers",
        "{2}{U}",
        &["Creature", "Kithkin", "Soldier"],
        2,
        2,
        Cost::Mana("{W/U}"),
        0,
        1,
    ),
    (
        "primeval_shambler",
        "Primeval Shambler",
        "{4}{B}",
        &["Creature", "Horror", "Mercenary"],
        3,
        3,
        Cost::Mana("{B}"),
        1,
        1,
    ),
    (
        "ridgeline_rager",
        "Ridgeline Rager",
        "{2}{R}",
        &["Creature", "Beast"],
        1,
        2,
        Cost::Mana("{R}"),
        1,
        0,
    ),
    (
        "sea_spirit",
        "Sea Spirit",
        "{4}{U}",
        &["Creature", "Elemental", "Spirit"],
        2,
        3,
        Cost::Mana("{U}"),
        1,
        0,
    ),
    (
        "storm_shaman",
        "Storm Shaman",
        "{2}{R}",
        &["Creature", "Human", "Cleric", "Shaman"],
        0,
        4,
        Cost::Mana("{R}"),
        1,
        0,
    ),
    (
        "flamekin_brawler",
        "Flamekin Brawler",
        "{R}",
        &["Creature", "Elemental", "Warrior"],
        0,
        2,
        Cost::Mana("{R}"),
        1,
        0,
    ),
    (
        "immolating_souleater",
        "Immolating Souleater",
        "{2}",
        &["Artifact", "Creature", "Phyrexian", "Dog"],
        1,
        1,
        Cost::Mana("{R/P}"),
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
fn pump_self_cohort_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for &(id, name, mana, types, power, toughness, ref activation, pump_power, pump_toughness) in
        CARDS
    {
        let definition = registry.get(id).unwrap_or_else(|| panic!("missing {id}"));
        assert_eq!(definition.layout, Layout::Normal, "{id}");
        let f = face(registry, id);
        assert_eq!(f.face_id.as_str(), id, "{id} face id");
        assert_eq!(f.name, name, "{id} name");
        assert_eq!(f.mana_cost.to_string(), mana, "{id} mana cost");
        assert_eq!(f.types, types, "{id} types");
        assert_eq!(f.power, Some(power), "{id} power");
        assert_eq!(f.toughness, Some(toughness), "{id} toughness");
        assert!(f.keywords.is_empty(), "{id} has no keyword");
        assert!(f.spell_effect.is_empty(), "{id} has no spell effect");
        assert!(
            f.triggered_abilities.is_empty(),
            "{id} has no triggered ability"
        );
        assert!(f.static_abilities.is_empty(), "{id} has no static ability");

        let [ability] = f.activated_abilities.as_slice() else {
            panic!("{id}: exactly one activated ability");
        };
        assert_eq!(ability.ability_id.as_str(), "activated_01", "{id}");
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![1]),
            "{id} presentation"
        );
        let expected_cost = match activation {
            Cost::Mana(cost) => {
                AbilityCost::Mana(ManaCost::parse(cost).expect("printed activation cost"))
            }
            Cost::Tap => AbilityCost::Tap,
        };
        assert_eq!(ability.costs, vec![expected_cost], "{id} activation cost");
        assert!(
            matches!(
                ability.effect.as_slice(),
                [SpellEffectKind::PumpTarget {
                    power,
                    toughness,
                    scale: None,
                    subject: EffectSubject::Source,
                }] if *power == pump_power && *toughness == pump_toughness
            ),
            "{id}: {:?}",
            ability.effect
        );
        assert!(ability.targeting.is_none(), "{id}: no target");
    }
}

type Fingerprint = (&'static str, &'static str, &'static str);

const FINGERPRINTS: &[Fingerprint] = &[
    (
        "adarkar_sentinel",
        "Adarkar Sentinel",
        "271f072d39b221f0e24860fe08d0b24514c1dbef7012b1f22ca7fa38d9512c6e",
    ),
    (
        "angelfire_crusader",
        "Angelfire Crusader",
        "92eba44a0f19417ae40ef4355dfcbb9209aef8d0d7c27b23234fde0a82e36b7a",
    ),
    (
        "boa_constrictor",
        "Boa Constrictor",
        "e70207ddb03dc196c4298a5c5e52830503739c53aea4fb2b27832760db4d31f9",
    ),
    (
        "capashen_templar",
        "Capashen Templar",
        "cdde70dcf160bb34b148127740c6bf95ab7aedc092fba93398451c3d7a9d8cb4",
    ),
    (
        "carrion_ants",
        "Carrion Ants",
        "215d4e2c0391c0945ec33254154a31a0e691bdacb1aedce3cac945a8f5f60e46",
    ),
    (
        "dragon_engine",
        "Dragon Engine",
        "54ab453fcda6553b400cad6c675c5460422925af41572c4917139d6e682c1c83",
    ),
    (
        "dread_shade",
        "Dread Shade",
        "85f90aebc49e1d0dd16eccaa366d344f8e3f50704e2a4daba69b3e5d7eadc292",
    ),
    (
        "fetid_horror",
        "Fetid Horror",
        "85f90aebc49e1d0dd16eccaa366d344f8e3f50704e2a4daba69b3e5d7eadc292",
    ),
    (
        "firescreamer",
        "Firescreamer",
        "92eba44a0f19417ae40ef4355dfcbb9209aef8d0d7c27b23234fde0a82e36b7a",
    ),
    (
        "flame_spirit",
        "Flame Spirit",
        "92eba44a0f19417ae40ef4355dfcbb9209aef8d0d7c27b23234fde0a82e36b7a",
    ),
    (
        "flamekin_brawler",
        "Flamekin Brawler",
        "92eba44a0f19417ae40ef4355dfcbb9209aef8d0d7c27b23234fde0a82e36b7a",
    ),
    (
        "frozen_shade",
        "Frozen Shade",
        "85f90aebc49e1d0dd16eccaa366d344f8e3f50704e2a4daba69b3e5d7eadc292",
    ),
    (
        "hoar_shade",
        "Hoar Shade",
        "85f90aebc49e1d0dd16eccaa366d344f8e3f50704e2a4daba69b3e5d7eadc292",
    ),
    (
        "honor_guard",
        "Honor Guard",
        "cdde70dcf160bb34b148127740c6bf95ab7aedc092fba93398451c3d7a9d8cb4",
    ),
    (
        "immolating_souleater",
        "Immolating Souleater",
        "d2fb671aaebc589c3decba7e7da0a1a53ad1dc9e15344058175b987dbb533226",
    ),
    (
        "jousting_dummy",
        "Jousting Dummy",
        "8ba405c5d0d874ec3aec9ad654155159f41dbfc1d522d07aa61707a903ce4520",
    ),
    (
        "kjeldoran_outrider",
        "Kjeldoran Outrider",
        "cdde70dcf160bb34b148127740c6bf95ab7aedc092fba93398451c3d7a9d8cb4",
    ),
    (
        "llanowar_vanguard",
        "Llanowar Vanguard",
        "8f29384f12b97747d4638ae31b3d3e75687b9189d6eff8a015c42b3d39571ca1",
    ),
    (
        "loch_korrigan",
        "Loch Korrigan",
        "7eefb59bc56b050f185ade120e7fe55f2fb1f798fdbd49aade92d649f53b2368",
    ),
    (
        "looming_shade",
        "Looming Shade",
        "85f90aebc49e1d0dd16eccaa366d344f8e3f50704e2a4daba69b3e5d7eadc292",
    ),
    (
        "molten_ravager",
        "Molten Ravager",
        "92eba44a0f19417ae40ef4355dfcbb9209aef8d0d7c27b23234fde0a82e36b7a",
    ),
    (
        "nantuko_shade",
        "Nantuko Shade",
        "85f90aebc49e1d0dd16eccaa366d344f8e3f50704e2a4daba69b3e5d7eadc292",
    ),
    (
        "narstad_scrapper",
        "Narstad Scrapper",
        "54ab453fcda6553b400cad6c675c5460422925af41572c4917139d6e682c1c83",
    ),
    (
        "parapet_watchers",
        "Parapet Watchers",
        "7a6a0cd45922c193af367b082c64e2533c7703f74b32fe9afd731880b7627ffe",
    ),
    (
        "primeval_shambler",
        "Primeval Shambler",
        "85f90aebc49e1d0dd16eccaa366d344f8e3f50704e2a4daba69b3e5d7eadc292",
    ),
    (
        "ridgeline_rager",
        "Ridgeline Rager",
        "92eba44a0f19417ae40ef4355dfcbb9209aef8d0d7c27b23234fde0a82e36b7a",
    ),
    (
        "sea_spirit",
        "Sea Spirit",
        "9a0b70232584d8fb63bfc8657f21eb246e1672ad87e6372064c602eabb05b695",
    ),
    (
        "storm_shaman",
        "Storm Shaman",
        "92eba44a0f19417ae40ef4355dfcbb9209aef8d0d7c27b23234fde0a82e36b7a",
    ),
];

#[test]
fn pump_self_cohort_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for &(id, name, fingerprint) in FINGERPRINTS {
        let row = fingerprints
            .lines()
            .find(|line| line.starts_with(&format!("{id}\t")))
            .unwrap_or_else(|| panic!("missing fingerprint row for {id}"));
        let fields: Vec<&str> = row.split('\t').collect();
        assert_eq!(fields.len(), 5, "fingerprint row shape: {row}");
        assert_eq!(fields[0], id);
        assert_eq!(fields[1], name);
        assert_eq!(fields[2], id);
        assert_eq!(fields[4], fingerprint, "fingerprint drift for {id}");
    }
}
