//! Registry conformance for the reviewed fixed-bonus Equipment family.
//!
//! Every card has one attached P/T modifier and one `Equip` activated ability. Card names, costs,
//! bonuses and face identities are printed data; the shared scenario executes every card. Each
//! fingerprint is SHA-256 of the exact pinned normalized Oracle text.

use tricerules_cards::primitives::{
    AbilityCost, SpellEffectKind, StaticAbilityDef, TargetController, TargetKind,
};
use tricerules_cards::{AbilityPresentation, AbilitySourceZone, ActivationTiming, Layout};

type Card = (
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    u32,
    u32,
);

type Fingerprint = (&'static str, &'static str, &'static str, &'static str);

const CARDS: &[Card] = &[
    ("+2_mace", "+2 Mace", "plus_2_mace", "{1}{W}", "{3}", 2, 2),
    ("bone_saw", "Bone Saw", "bone_saw", "{0}", "{1}", 1, 0),
    (
        "bronze_sword",
        "Bronze Sword",
        "bronze_sword",
        "{1}",
        "{3}",
        2,
        0,
    ),
    (
        "cultists_staff",
        "Cultist's Staff",
        "cultists_staff",
        "{2}",
        "{3}",
        2,
        2,
    ),
    ("greataxe", "Greataxe", "greataxe", "{1}", "{5}", 4, 0),
    ("greatsword", "Greatsword", "greatsword", "{3}", "{3}", 3, 0),
    (
        "honed_khopesh",
        "Honed Khopesh",
        "honed_khopesh",
        "{1}",
        "{1}",
        1,
        1,
    ),
    (
        "jackhammer",
        "Jackhammer",
        "jackhammer",
        "{1}{R}",
        "{2}",
        2,
        0,
    ),
    (
        "kite_shield",
        "Kite Shield",
        "kite_shield",
        "{0}",
        "{3}",
        0,
        3,
    ),
    (
        "leonin_scimitar",
        "Leonin Scimitar",
        "leonin_scimitar",
        "{1}",
        "{1}",
        1,
        1,
    ),
    (
        "marauders_axe",
        "Marauder's Axe",
        "marauders_axe",
        "{2}",
        "{2}",
        2,
        0,
    ),
    (
        "ogres_cleaver",
        "Ogre's Cleaver",
        "ogres_cleaver",
        "{2}",
        "{5}",
        5,
        0,
    ),
    ("riot_gear", "Riot Gear", "riot_gear", "{2}", "{2}", 1, 2),
    ("shuko", "Shuko", "shuko", "{1}", "{0}", 1, 0),
    (
        "slagwurm_armor",
        "Slagwurm Armor",
        "slagwurm_armor",
        "{1}",
        "{3}",
        0,
        6,
    ),
    (
        "torch_gauntlet",
        "Torch Gauntlet",
        "torch_gauntlet",
        "{2}",
        "{2}",
        2,
        0,
    ),
    (
        "trusty_machete",
        "Trusty Machete",
        "trusty_machete",
        "{1}",
        "{2}",
        2,
        1,
    ),
    (
        "vanquishers_axe",
        "Vanquisher's Axe",
        "vanquishers_axe",
        "{1}",
        "{2}",
        2,
        0,
    ),
    (
        "veterans_sidearm",
        "Veteran's Sidearm",
        "veterans_sidearm",
        "{2}",
        "{1}",
        1,
        1,
    ),
    (
        "vulshok_battlegear",
        "Vulshok Battlegear",
        "vulshok_battlegear",
        "{3}",
        "{3}",
        3,
        3,
    ),
    (
        "warlords_axe",
        "Warlord's Axe",
        "warlords_axe",
        "{3}",
        "{4}",
        3,
        1,
    ),
];

const FINGERPRINTS: &[Fingerprint] = &[
    (
        "+2_mace",
        "+2 Mace",
        "plus_2_mace",
        "3531a335e1ab8de63ba90293c3714526a02f5aaf9821901a4193edad8811a14e",
    ),
    (
        "bone_saw",
        "Bone Saw",
        "bone_saw",
        "75936d7a689b675e1cb68c1fdc2196ade8199c9419fbb0cf88670f98398d63d5",
    ),
    (
        "bronze_sword",
        "Bronze Sword",
        "bronze_sword",
        "1497d2abd667613711da9fc757427b3e0ac6a76b72afed8b7a33d97b0e92653c",
    ),
    (
        "cultists_staff",
        "Cultist's Staff",
        "cultists_staff",
        "3531a335e1ab8de63ba90293c3714526a02f5aaf9821901a4193edad8811a14e",
    ),
    (
        "greataxe",
        "Greataxe",
        "greataxe",
        "c9f4ac9d072cce276d606100f865612e1bc8d21fe31f87af0d1328e43804dd2f",
    ),
    (
        "greatsword",
        "Greatsword",
        "greatsword",
        "fc753e223198b333301c0d6044ae7bbc377fce03ac9ba00f39a3c7791f7a89ec",
    ),
    (
        "honed_khopesh",
        "Honed Khopesh",
        "honed_khopesh",
        "cd71f3a3931ac51cf73cf3725df13071822f34178d0ac90fa74fb073fe390be9",
    ),
    (
        "jackhammer",
        "Jackhammer",
        "jackhammer",
        "a45c06d9dd4afab6c88fb087c40f7dab8ad0ff1c5e5e75754335a29e854dce0d",
    ),
    (
        "kite_shield",
        "Kite Shield",
        "kite_shield",
        "e903557f0aa7e851c16e1208f6951b81b0eec0a5b422f379fc679b27ae07947f",
    ),
    (
        "leonin_scimitar",
        "Leonin Scimitar",
        "leonin_scimitar",
        "cd71f3a3931ac51cf73cf3725df13071822f34178d0ac90fa74fb073fe390be9",
    ),
    (
        "marauders_axe",
        "Marauder's Axe",
        "marauders_axe",
        "a45c06d9dd4afab6c88fb087c40f7dab8ad0ff1c5e5e75754335a29e854dce0d",
    ),
    (
        "ogres_cleaver",
        "Ogre's Cleaver",
        "ogres_cleaver",
        "62440bfb88ce055950e1599ee6abc35469bc276ac36ab46c75e71276f8729fd4",
    ),
    (
        "riot_gear",
        "Riot Gear",
        "riot_gear",
        "f14cb3a35c1708ea54d890251ac50bd8aa06433bbe7453ec001f4e939f0c60c9",
    ),
    (
        "shuko",
        "Shuko",
        "shuko",
        "674ade2c88c7eeaa308d094d6adba9bf32a3011e4b1b4839cbe5e353025a0b59",
    ),
    (
        "slagwurm_armor",
        "Slagwurm Armor",
        "slagwurm_armor",
        "aeb18f441f1cb7019f5dd26370a4ced3d08c0a08c022a761a4f81faaf4a971d3",
    ),
    (
        "torch_gauntlet",
        "Torch Gauntlet",
        "torch_gauntlet",
        "a45c06d9dd4afab6c88fb087c40f7dab8ad0ff1c5e5e75754335a29e854dce0d",
    ),
    (
        "trusty_machete",
        "Trusty Machete",
        "trusty_machete",
        "63bfce18e8c3243fcacbbbc669467d8ce6656b4ba61ce862385724b388bb8759",
    ),
    (
        "vanquishers_axe",
        "Vanquisher's Axe",
        "vanquishers_axe",
        "a45c06d9dd4afab6c88fb087c40f7dab8ad0ff1c5e5e75754335a29e854dce0d",
    ),
    (
        "veterans_sidearm",
        "Veteran's Sidearm",
        "veterans_sidearm",
        "cd71f3a3931ac51cf73cf3725df13071822f34178d0ac90fa74fb073fe390be9",
    ),
    (
        "vulshok_battlegear",
        "Vulshok Battlegear",
        "vulshok_battlegear",
        "1a4464876c77a2e181f17c2dae0ac8723d9fd2244ea63aa6a5727f6b1cc842b1",
    ),
    (
        "warlords_axe",
        "Warlord's Axe",
        "warlords_axe",
        "da70084e8366ab35b40df0adff89cba2e4c8ae8321a4985b55e43bf7dee84e75",
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
fn equipment_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for &(id, name, face_id, cast, equip, power, toughness) in CARDS {
        let definition = registry.get(id).unwrap_or_else(|| panic!("missing {id}"));
        assert_eq!(definition.layout, Layout::Normal, "{id}");
        let f = face(registry, id);
        assert_eq!(f.face_id.as_str(), face_id, "{id} face id");
        assert_eq!(f.name, name, "{id} name");
        assert_eq!(f.mana_cost.to_string(), cast, "{id} mana cost");
        assert_eq!(f.types, ["Artifact", "Equipment"], "{id} types");

        let [static_ability] = f.static_abilities.as_slice() else {
            panic!("{id}: one static ability");
        };
        assert_eq!(
            static_ability.presentation,
            AbilityPresentation::OracleLines(vec![1]),
            "{id} static presentation"
        );
        assert!(
            matches!(
                &static_ability.definition,
                StaticAbilityDef::AttachedModifier {
                    delta_power: p,
                    delta_toughness: t,
                    ..
                } if *p == power as i32 && *t == toughness as i32
            ),
            "{id}: {:?}",
            static_ability.definition
        );

        let [ability] = f.activated_abilities.as_slice() else {
            panic!("{id}: one activated ability");
        };
        assert_eq!(ability.ability_id.as_str(), "activated_01", "{id}");
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![2]),
            "{id} equip presentation"
        );
        assert_eq!(
            ability.source_zone,
            AbilitySourceZone::Battlefield,
            "{id} source zone"
        );
        assert_eq!(ability.timing, ActivationTiming::Normal, "{id} timing");
        assert!(
            matches!(
                ability.costs.as_slice(),
                [AbilityCost::Mana(cost)] if cost.to_string() == equip
            ),
            "{id}: {:?}",
            ability.costs
        );
        assert!(
            matches!(
                ability.effect.as_slice(),
                [SpellEffectKind::Equip { target }]
                    if target.kind == TargetKind::Creature
                        && target.controller == TargetController::You
            ),
            "{id}: {:?}",
            ability.effect
        );
        assert!(
            ability.targeting.is_none(),
            "{id}: equip uses its effect target"
        );
    }
}

#[test]
fn equipment_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for &(id, name, face_id, fingerprint) in FINGERPRINTS {
        let row = fingerprints
            .lines()
            .find(|line| line.starts_with(&format!("{id}\t")))
            .unwrap_or_else(|| panic!("missing fingerprint row for {id}"));
        let fields: Vec<&str> = row.split('\t').collect();
        assert_eq!(fields.len(), 5, "fingerprint row shape: {row}");
        assert_eq!(fields[0], id);
        assert_eq!(fields[1], name);
        assert_eq!(fields[2], face_id);
        assert_eq!(fields[4], fingerprint, "fingerprint drift for {id}");
    }
}
