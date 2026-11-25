//! Registry conformance for the queue-v53 batch A cohorts.
#![allow(clippy::type_complexity)]
use tricerules_cards::primitives::{
    AbilityCost, Amount, BasePowerToughnessValue, PermanentTypeFilter, SpellEffectKind, TargetKind,
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
fn q53_base_power_toughness_cards_map_definitions() {
    for (id, name, mana, power, toughness, draws) in [
        ("diminish", "Diminish", "{U}", 1, 1, false),
        ("square_up", "Square Up", "{1}{G/U}", 4, 4, false),
        (
            "zhalfirin_shapecraft",
            "Zhalfirin Shapecraft",
            "{1}{U}",
            4,
            3,
            true,
        ),
    ] {
        let f = identity(id, name, mana, &["Instant"]);
        let first = f
            .spell_effect
            .first()
            .unwrap_or_else(|| panic!("{id}: missing effect"));
        let SpellEffectKind::SetBasePowerToughness {
            target,
            power: set_power,
            toughness: set_toughness,
        } = first
        else {
            panic!("{id}: first effect must set base P/T");
        };
        assert_eq!(target.kind, TargetKind::Creature, "{id}: creature target");
        let groups = &f
            .targeting
            .as_ref()
            .unwrap_or_else(|| panic!("{id}: targeting"))
            .groups;
        assert_eq!(groups.len(), 1, "{id}: exactly one target group");
        let group = &groups[0];
        assert_eq!(
            (group.min, group.max, group.effect_indices.as_slice()),
            (1, 1, &[0][..]),
            "{id}: one target group"
        );
        assert_eq!(*set_power, BasePowerToughnessValue::Fixed(power), "{id}");
        assert_eq!(
            *set_toughness,
            BasePowerToughnessValue::Fixed(toughness),
            "{id}"
        );
        if draws {
            assert!(
                matches!(
                    f.spell_effect.get(1),
                    Some(SpellEffectKind::Draw {
                        count: Amount::Fixed(1),
                        ..
                    })
                ),
                "{id}: Shapecraft draws one"
            );
        } else {
            assert_eq!(f.spell_effect.len(), 1, "{id}: no rider");
        }
        assert_eq!(
            f.spell_effect.len(),
            if draws { 2 } else { 1 },
            "{id}: printed effect count"
        );
    }
}

#[test]
fn q53_player_damage_cards_map_definitions() {
    for (id, name, mana, types, amount) in [
        (
            "lava_spike",
            "Lava Spike",
            "{R}",
            &["Sorcery", "Arcane"][..],
            3,
        ),
        ("lava_axe", "Lava Axe", "{4}{R}", &["Sorcery"][..], 5),
        (
            "sarkhans_catharsis",
            "Sarkhan's Catharsis",
            "{4}{R}",
            &["Instant"][..],
            5,
        ),
    ] {
        let f = identity(id, name, mana, types);
        let [SpellEffectKind::DamageTarget {
            amount: dealt,
            target,
        }] = f.spell_effect.as_slice()
        else {
            panic!("{id}: one damage effect");
        };
        assert_eq!(*dealt, Amount::Fixed(amount), "{id}: printed damage");
        assert!(
            player_or_planeswalker(target),
            "{id}: player or planeswalker"
        );
        let groups = &f
            .targeting
            .as_ref()
            .unwrap_or_else(|| panic!("{id}: targeting"))
            .groups;
        assert_eq!(groups.len(), 1, "{id}: exactly one target group");
        let group = &groups[0];
        assert_eq!(
            (group.min, group.max, group.effect_indices.as_slice()),
            (1, 1, &[0][..]),
            "{id}: one target group"
        );
    }
}

#[test]
fn q53_activated_damage_cards_map_definitions() {
    for (id, name, mana, types, cost, amount, power, toughness) in [
        (
            "scalding_devil",
            "Scalding Devil",
            "{1}{R}",
            &["Creature", "Devil"][..],
            "{2}{R}",
            1,
            1,
            1,
        ),
        (
            "volcanic_rambler",
            "Volcanic Rambler",
            "{5}{R}",
            &["Creature", "Elemental"][..],
            "{2}{R}",
            1,
            6,
            4,
        ),
        (
            "flamewave_invoker",
            "Flamewave Invoker",
            "{2}{R}",
            &["Creature", "Goblin", "Mutant"][..],
            "{7}{R}",
            5,
            2,
            2,
        ),
    ] {
        let f = identity(id, name, mana, types);
        assert_eq!(f.power, Some(power), "{id}: printed power");
        assert_eq!(f.toughness, Some(toughness), "{id}: printed toughness");
        let [ability] = f.activated_abilities.as_slice() else {
            panic!("{id}: one activated ability");
        };
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![1]),
            "{id}: printed Oracle line"
        );
        let groups = &ability
            .targeting
            .as_ref()
            .unwrap_or_else(|| panic!("{id}: targeting"))
            .groups;
        assert_eq!(groups.len(), 1, "{id}: exactly one target group");
        let group = &groups[0];
        assert_eq!(
            (group.min, group.max, group.effect_indices.as_slice()),
            (1, 1, &[0][..]),
            "{id}: one target group"
        );
        assert_eq!(ability.costs.len(), 1, "{id}: one mana cost");
        assert!(
            matches!(&ability.costs[0], AbilityCost::Mana(actual) if actual.to_string() == *cost),
            "{id}: exact activation cost"
        );
        let [SpellEffectKind::DamageTarget {
            amount: dealt,
            target,
        }] = ability.effect.as_slice()
        else {
            panic!("{id}: one damage effect");
        };
        assert_eq!(*dealt, Amount::Fixed(amount), "{id}: printed damage");
        assert!(
            player_or_planeswalker(target),
            "{id}: player or planeswalker"
        );
    }
}

#[test]
fn q53_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for (id, name, digest) in [
        (
            "diminish",
            "Diminish",
            "ccf61019eb2a9bc93abfe4b0ff4314c9806da73add66f609aaa0d439a2a9093b",
        ),
        (
            "square_up",
            "Square Up",
            "2a7ba316aa6a3ee4ba49f68ac83f8391cdee3d187587d916519f793aae3a33f4",
        ),
        (
            "zhalfirin_shapecraft",
            "Zhalfirin Shapecraft",
            "2c79e2015fe81f598fa131cffc6d9f0913fe72097e655f916bb57c3a9738841e",
        ),
        (
            "lava_spike",
            "Lava Spike",
            "bdcedd2233ac5327b573283b29142e746e7ccbf0ca49db8ef13f1158beefab6f",
        ),
        (
            "lava_axe",
            "Lava Axe",
            "0b69df50d0e6de306e46a9c11106359a0f88de46a29876a93d686b7196bc25ca",
        ),
        (
            "sarkhans_catharsis",
            "Sarkhan's Catharsis",
            "664db7b6d2f67d508e4c7abb082f95a7ecb51c8cc4b9d82355a8206e46371de1",
        ),
        (
            "scalding_devil",
            "Scalding Devil",
            "03de16ae229a81ab1945c2779c0839937aa3043b5918e1266efe06e89fdcde5f",
        ),
        (
            "volcanic_rambler",
            "Volcanic Rambler",
            "03de16ae229a81ab1945c2779c0839937aa3043b5918e1266efe06e89fdcde5f",
        ),
        (
            "flamewave_invoker",
            "Flamewave Invoker",
            "e3f1934bc38ed9a40c60fba1ea7f457993f7454b01e710dc01b026bdd500e05d",
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
        assert_eq!(fields[2], face_id(name), "face id drift for {id}");
        assert_eq!(fields[3], name);
        assert_eq!(fields[4], digest, "pinned fingerprint digest for {id}");
    }
}
