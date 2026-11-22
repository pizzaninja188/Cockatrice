//! Registry conformance for queue-v21 basic-type tapped dual lands.
//!
//! Every included card is a normal-layout land with two basic land subtypes, an
//! intrinsic tap mana ability in the printed color order and an unconditional
//! enters-tapped replacement. Each fingerprint is SHA-256 of the exact pinned
//! normalized Oracle text.

use tricerules_cards::primitives::{
    AbilityCost, EntersTappedAffected, SpellEffectKind, StaticAbilityDef,
};
use tricerules_cards::{AbilityPresentation, Layout};

type Card = (
    &'static str,
    &'static str,
    bool,
    &'static [&'static str],
    [u32; 5],
    [u32; 5],
    &'static str,
);

const CARDS: &[Card] = &[
    (
        "contaminated_aquifer",
        "Contaminated Aquifer",
        false,
        &["Land", "Island", "Swamp"],
        [0, 1, 0, 0, 0],
        [0, 0, 1, 0, 0],
        "4e56d318fb96ba513619b016e5e51e122dc299b71581e8bf84e60e794741aeb9",
    ),
    (
        "geothermal_bog",
        "Geothermal Bog",
        false,
        &["Land", "Swamp", "Mountain"],
        [0, 0, 1, 0, 0],
        [0, 0, 0, 1, 0],
        "37c8b9a91a9154383f77e64a03917dc3deadf459551d0c1f8ca210a906f64191",
    ),
    (
        "haunted_mire",
        "Haunted Mire",
        false,
        &["Land", "Swamp", "Forest"],
        [0, 0, 1, 0, 0],
        [0, 0, 0, 0, 1],
        "3631706433e9260f2ba7acfc854c7dc4d687e43420042abf4436a80e175a8ef1",
    ),
    (
        "idyllic_beachfront",
        "Idyllic Beachfront",
        false,
        &["Land", "Plains", "Island"],
        [1, 0, 0, 0, 0],
        [0, 1, 0, 0, 0],
        "a09c380640bf6bc612bf1080ab4c201bd4b7bad2df520d1cf890dcd4a8de3da9",
    ),
    (
        "molten_tributary",
        "Molten Tributary",
        false,
        &["Land", "Island", "Mountain"],
        [0, 1, 0, 0, 0],
        [0, 0, 0, 1, 0],
        "2e2756308d52ec7638ce9fa585ada862094c92a9007abc5f6ddd5ae950dae9c5",
    ),
    (
        "radiant_grove",
        "Radiant Grove",
        false,
        &["Land", "Forest", "Plains"],
        [0, 0, 0, 0, 1],
        [1, 0, 0, 0, 0],
        "455d948cc0abdd0d57965b27bbd8e8b9238be65d4c7b6353046fb494ca9e3b96",
    ),
    (
        "sacred_peaks",
        "Sacred Peaks",
        false,
        &["Land", "Mountain", "Plains"],
        [0, 0, 0, 1, 0],
        [1, 0, 0, 0, 0],
        "325ca9f11f07d9f275713d496c77c26c5f8f56efd2aedd24ee22bb8f6c806668",
    ),
    (
        "sunlit_marsh",
        "Sunlit Marsh",
        false,
        &["Land", "Plains", "Swamp"],
        [1, 0, 0, 0, 0],
        [0, 0, 1, 0, 0],
        "93cceb355fba5b75ce176a5c20d436492f1a9d8f63e5c6719b54c788f57dd1d0",
    ),
    (
        "tangled_islet",
        "Tangled Islet",
        false,
        &["Land", "Forest", "Island"],
        [0, 0, 0, 0, 1],
        [0, 1, 0, 0, 0],
        "7ac4c0609a8fe28759ca5ccde68b4a626aa916f76405f5ca080ac39916b0adb4",
    ),
    (
        "wooded_ridgeline",
        "Wooded Ridgeline",
        false,
        &["Land", "Mountain", "Forest"],
        [0, 0, 0, 1, 0],
        [0, 0, 0, 0, 1],
        "b96adb1bc137945befff6d9dd4ada7077a6bb81df5f6821b9dae93065f341421",
    ),
];

fn mask(amount: &tricerules_cards::ManaAmount) -> [u32; 5] {
    [amount.w, amount.u, amount.b, amount.r, amount.g]
}

fn check_card(id: &str, name: &str, snow: bool, types: &[&str], first: [u32; 5], second: [u32; 5]) {
    let registry = tricerules_cards::registry::global();
    let definition = registry.get(id).unwrap_or_else(|| panic!("missing {id}"));
    assert_eq!(definition.layout, Layout::Normal, "{id}");
    assert_eq!(definition.name, name, "{id}");
    assert_eq!(registry.id_for_name(name), Some(id), "{id}");
    let face = definition.primary_face();
    assert_eq!(face.face_id.as_str(), id, "{id} face id");
    assert_eq!(face.mana_cost.to_string(), "", "{id} mana cost");
    assert_eq!(face.types, types, "{id} types");
    assert_eq!(
        face.supertypes,
        if snow {
            vec!["Snow".to_string()]
        } else {
            Vec::<String>::new()
        },
        "{id} snow supertype"
    );
    assert_eq!((face.power, face.toughness), (None, None), "{id} body");
    assert!(face.keywords.is_empty(), "{id}");
    assert!(face.triggered_abilities.is_empty(), "{id}");
    assert!(face.spell_effect.is_empty(), "{id}");
    assert!(face.targeting.is_none(), "{id}");

    let [mana] = face.activated_abilities.as_slice() else {
        panic!("{id}: exactly one activated ability")
    };
    assert_eq!(mana.ability_id.as_str(), "activated_01", "{id}");
    assert_eq!(
        mana.presentation,
        AbilityPresentation::OracleLines(vec![1]),
        "{id} mana presentation"
    );
    assert_eq!(mana.costs, [AbilityCost::Tap], "{id}");
    assert!(mana.is_mana_ability(), "{id}: mana ability");
    assert!(mana.intrinsic_land_mana, "{id} intrinsic land mana");
    assert!(
        matches!(
            mana.effect.as_slice(),
            [SpellEffectKind::ProduceMana { options, .. }]
                if options.len() == 2
                    && mask(&options[0]) == first
                    && mask(&options[1]) == second
        ),
        "{id}: exact printed color options"
    );
    assert!(mana.targeting.is_none(), "{id}: no target");

    let [entry] = face.static_abilities.as_slice() else {
        panic!("{id}: exactly one static ability")
    };
    assert_eq!(entry.ability_id.as_str(), "static_01", "{id}");
    assert_eq!(
        entry.presentation,
        AbilityPresentation::OracleLines(vec![2]),
        "{id} entry presentation"
    );
    assert_eq!(
        entry.definition,
        StaticAbilityDef::EntersTapped {
            affected: EntersTappedAffected::Self_,
            condition: None,
            unless_cost: None,
        },
        "{id}"
    );
}

#[test]
fn tapped_dual_basic_batch_canary_cards_match_their_typed_shape() {
    check_card(
        "contaminated_aquifer",
        "Contaminated Aquifer",
        false,
        &["Land", "Island", "Swamp"],
        [0, 1, 0, 0, 0],
        [0, 0, 1, 0, 0],
    );
    check_card(
        "wooded_ridgeline",
        "Wooded Ridgeline",
        false,
        &["Land", "Mountain", "Forest"],
        [0, 0, 0, 1, 0],
        [0, 0, 0, 0, 1],
    );
}

#[test]
fn tapped_dual_basic_batch_maps_definitions() {
    for &(id, name, snow, types, first, second, _) in CARDS {
        check_card(id, name, snow, types, first, second);
    }
}

#[test]
fn tapped_dual_basic_batch_fingerprints_match_the_pinned_oracle_text() {
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
