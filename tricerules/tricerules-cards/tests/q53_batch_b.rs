//! Draft registry conformance for queue-v53 batch B.
#![allow(clippy::type_complexity)]
use tricerules_cards::primitives::{
    AbilityCost, Amount, EffectSubject, PermanentTypeFilter, SpellEffectKind, TargetController,
    TargetKind,
};
use tricerules_cards::{registry, AbilityPresentation, CardDefinition, CardFace, Layout};

fn definition(id: &str) -> &'static CardDefinition {
    registry::global()
        .get(id)
        .unwrap_or_else(|| panic!("missing {id}"))
}

fn face_id(name: &str) -> String {
    name.to_lowercase().replace(' ', "_").replace('\'', "")
}

fn identity(id: &str, name: &str, mana: &str, types: &[&str]) -> &'static CardFace {
    let definition = definition(id);
    assert_eq!(definition.layout, Layout::Normal, "{id}");
    assert_eq!(registry::global().id_for_name(name), Some(id), "{id}");
    let f = definition.primary_face();
    assert_eq!(f.face_id.as_str(), face_id(name), "{id} face id");
    assert_eq!(f.name, name, "{id} name");
    assert_eq!(f.mana_cost.to_string(), mana, "{id} mana");
    assert_eq!(
        f.types.iter().map(String::as_str).collect::<Vec<_>>(),
        types,
        "{id} types"
    );
    f
}

#[test]
fn q53_shrink_cards_map_definitions() {
    for (id, name, mana, types, power, toughness, costs, fp, ft, keywords, line) in [
        (
            "aquus_steed",
            "Aquus Steed",
            "{3}{U}",
            &["Creature", "Beast"][..],
            1,
            3,
            vec!["Mana", "Tap"],
            -2,
            0,
            &[][..],
            1,
        ),
        (
            "grandmother_sengir",
            "Grandmother Sengir",
            "{4}{B}",
            &["Creature", "Human", "Wizard"][..],
            3,
            3,
            vec!["Mana", "Tap"],
            -1,
            -1,
            &[][..],
            1,
        ),
        (
            "plague_witch",
            "Plague Witch",
            "{1}{B}",
            &["Creature", "Elf", "Spellshaper"][..],
            1,
            1,
            vec!["Mana", "Tap", "Discard"],
            -1,
            -1,
            &[][..],
            1,
        ),
        (
            "phyrexian_debaser",
            "Phyrexian Debaser",
            "{3}{B}",
            &["Creature", "Phyrexian", "Carrier"][..],
            2,
            2,
            vec!["Tap", "SacrificeSelf"],
            -2,
            -2,
            &["Flying"][..],
            2,
        ),
        (
            "nim_replica",
            "Nim Replica",
            "{3}",
            &["Artifact", "Creature", "Zombie"][..],
            3,
            1,
            vec!["Mana", "SacrificeSelf"],
            -1,
            -1,
            &[][..],
            1,
        ),
        (
            "bog_naughty",
            "Bog Naughty",
            "{3}{B}{B}",
            &["Creature", "Faerie"][..],
            3,
            3,
            vec!["Mana", "SacrificePermanent"],
            -3,
            -3,
            &["Flying"][..],
            2,
        ),
        (
            "plagued_rusalka",
            "Plagued Rusalka",
            "{B}",
            &["Creature", "Spirit"][..],
            1,
            1,
            vec!["Mana", "SacrificePermanent"],
            -1,
            -1,
            &[][..],
            1,
        ),
    ] {
        let f = identity(id, name, mana, types);
        assert_eq!(f.power, Some(power), "{id} power");
        assert_eq!(f.toughness, Some(toughness), "{id} toughness");
        assert_eq!(
            f.keywords
                .iter()
                .map(|k| format!("{k:?}"))
                .collect::<Vec<_>>(),
            keywords
                .iter()
                .map(|k| (*k).to_string())
                .collect::<Vec<_>>(),
            "{id} keywords"
        );
        if id == "grandmother_sengir" {
            assert_eq!(f.supertypes, ["Legendary".to_string()], "{id}: legendary");
        }
        let [ability] = f.activated_abilities.as_slice() else {
            panic!("{id}: one ability")
        };
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![line]),
            "{id}: printed Oracle line"
        );
        let groups = &ability
            .targeting
            .as_ref()
            .unwrap_or_else(|| panic!("{id}: targeting"))
            .groups;
        assert_eq!(groups.len(), 1, "{id}: exactly one target group");
        assert_eq!(
            (
                groups[0].min,
                groups[0].max,
                groups[0].effect_indices.as_slice()
            ),
            (1, 1, &[0][..]),
            "{id}: one target group"
        );
        let kinds = ability
            .costs
            .iter()
            .map(|c| match c {
                AbilityCost::Mana(_) => "Mana",
                AbilityCost::Tap => "Tap",
                AbilityCost::Discard => "Discard",
                AbilityCost::SacrificeSelf => "SacrificeSelf",
                AbilityCost::SacrificePermanent { .. } => "SacrificePermanent",
                _ => panic!("{id}: unexpected cost"),
            })
            .collect::<Vec<_>>();
        assert_eq!(kinds, costs, "{id}: costs");
        if id == "bog_naughty" {
            let AbilityCost::SacrificePermanent { filter, .. } = &ability.costs[1] else {
                panic!("{id}: sacrifice cost")
            };
            assert_eq!(filter.kind, TargetKind::AnyPermanent, "{id}: Food kind");
            assert_eq!(filter.controller, TargetController::You, "{id}: own Food");
            assert_eq!(filter.required_subtypes, ["Food"], "{id}: Food subtype");
        }
        if id == "plagued_rusalka" {
            let AbilityCost::SacrificePermanent { filter, .. } = &ability.costs[1] else {
                panic!("{id}: sacrifice cost")
            };
            assert_eq!(filter.kind, TargetKind::Creature, "{id}: creature cost");
            assert_eq!(
                filter.controller,
                TargetController::You,
                "{id}: own creature"
            );
        }
        let [SpellEffectKind::PumpTarget {
            power: p,
            toughness: t,
            subject,
            ..
        }] = ability.effect.as_slice()
        else {
            panic!("{id}: one PumpTarget")
        };
        assert_eq!((*p, *t), (fp, ft), "{id}: printed penalty");
        let EffectSubject::Chosen(filter) = subject else {
            panic!("{id}: chosen target")
        };
        assert_eq!(filter.kind, TargetKind::Creature, "{id}: creature target");
    }
}

fn player_or_planeswalker(filter: &tricerules_cards::primitives::ObjectFilter) -> bool {
    let branches = filter.any_of.as_ref().expect("disjunctive target filter");
    branches.len() == 2
        && branches
            .iter()
            .any(|branch| branch.kind == TargetKind::AnyPlayer)
        && branches.iter().any(|branch| {
            branch.kind == TargetKind::AnyPermanent
                && branch.permanent_types == [PermanentTypeFilter::Planeswalker]
        })
}

#[test]
fn q53_two_target_damage_cards_map_definitions() {
    for (id, name, mana, creature, player) in [
        ("lunge", "Lunge", "{2}{R}", 2, 2),
        ("hungry_flames", "Hungry Flames", "{2}{R}", 3, 2),
        ("shower_of_sparks", "Shower of Sparks", "{R}", 1, 1),
    ] {
        let f = identity(id, name, mana, &["Instant"]);
        let [SpellEffectKind::DamageTarget {
            amount: first,
            target: creature_target,
        }, SpellEffectKind::DamageTarget {
            amount: second,
            target: player_target,
        }] = f.spell_effect.as_slice()
        else {
            panic!("{id}: two damage effects");
        };
        assert_eq!(*first, Amount::Fixed(creature), "{id}: creature damage");
        assert_eq!(
            creature_target.kind,
            TargetKind::Creature,
            "{id}: creature target"
        );
        assert_eq!(*second, Amount::Fixed(player), "{id}: player damage");
        assert!(
            player_or_planeswalker(player_target),
            "{id}: player or planeswalker"
        );
    }
}

fn face_id_for_fingerprint(name: &str) -> String {
    name.to_lowercase().replace(' ', "_").replace('\'', "")
}

#[test]
fn q53_b_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for (id, name, digest) in [
        (
            "aquus_steed",
            "Aquus Steed",
            "d5c9fbe00412e6511967322d0a611bfb50aa578f7a93e1d21751e66c16db79c5",
        ),
        (
            "bog_naughty",
            "Bog Naughty",
            "ea60e1409b02673da8bcbf106aa724ced4a00c9a8ca94bc1836a94d88212049f",
        ),
        (
            "grandmother_sengir",
            "Grandmother Sengir",
            "22871dc027f52b2d49f0bb1abc6a8f9240d7709525fd512c549e9d0bfd65600d",
        ),
        (
            "hungry_flames",
            "Hungry Flames",
            "be1f97b7035fe30eba5e6f308e661619d8abf6345745f84c1938caeb7d935196",
        ),
        (
            "lunge",
            "Lunge",
            "f1c55a71cd8b38545d5f7c055679ed8174f6aafbf3a0ec0f282c588c6474be56",
        ),
        (
            "nim_replica",
            "Nim Replica",
            "16a5774332e5d0feba0ab992b44d1de7de796776035d5833a0f6b22d90c03ae8",
        ),
        (
            "phyrexian_debaser",
            "Phyrexian Debaser",
            "aabe9aacb4d70c744b48572383b2ee91528f6329da18c97384d148c6db78ce39",
        ),
        (
            "plague_witch",
            "Plague Witch",
            "3dd150c46699be2304f066194d3f74582400c69d829c27beada1dd2c03055f22",
        ),
        (
            "plagued_rusalka",
            "Plagued Rusalka",
            "46d2e5febd0058f6b435196155b3100f9c6dfd4f4380f591d80e6dc680cc284e",
        ),
        (
            "shower_of_sparks",
            "Shower of Sparks",
            "3063ed0fc2ed6b56455ae972322b97e001098b6f861be86ba59b1e0cdc3e43bb",
        ),
    ] {
        let row = fingerprints
            .lines()
            .find(|line| line.starts_with(&format!("{id}\t")))
            .unwrap_or_else(|| panic!("missing fingerprint row for {id}"));
        let fields: Vec<&str> = row.split('\t').collect();
        assert_eq!(fields.len(), 5, "{id}");
        assert_eq!(fields[0], id);
        assert_eq!(fields[1], name);
        assert_eq!(
            fields[2],
            face_id_for_fingerprint(name),
            "face id drift for {id}"
        );
        assert_eq!(fields[3], name);
        assert_eq!(fields[4], digest, "pinned fingerprint digest for {id}");
    }
}
