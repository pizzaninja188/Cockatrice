//! Registry conformance for the queue-v56 batch C cohorts.
use tricerules_cards::{registry, AbilityPresentation, CardFace, Layout};

fn identity(id: &str, name: &str, mana: &str, types: &[&str]) -> &'static CardFace {
    let definition = registry::global()
        .get(id)
        .unwrap_or_else(|| panic!("missing {id}"));
    assert_eq!(definition.layout, Layout::Normal, "{id}");
    let f = definition.primary_face();
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
fn q56_c_flashback_graveyard_map_definitions() {
    for (id, name, mana, flashback, filter) in [
        ("dryads_revival", "Dryad's Revival", "{2}{G}", "{4}{G}", ""),
        (
            "morgue_theft",
            "Morgue Theft",
            "{1}{B}",
            "{4}{B}",
            "Creature",
        ),
        (
            "mystic_retrieval",
            "Mystic Retrieval",
            "{3}{U}",
            "{2}{R}",
            "InstantOrSorcery",
        ),
    ] {
        let f = identity(id, name, mana, &["Sorcery"]);
        assert_eq!(
            f.flashback_cost.clone().map(|cost| cost.to_string()),
            Some(flashback.into()),
            "{id}"
        );
        let effects = format!("{:?}", f.spell_effect);
        assert!(effects.contains("MoveGraveyardCards"), "{id}: {effects}");
        assert!(effects.contains("destination: Hand"), "{id}: {effects}");
        assert!(effects.contains("linked_exile_id: None"), "{id}: {effects}");
        if !filter.is_empty() {
            assert!(effects.contains(filter), "{id}: {effects}");
        }
        assert!(
            f.targeting.is_none(),
            "{id}: graveyard targeting is implicit"
        );
    }
}

#[test]
fn q56_c_destroy_investigate_map_definitions() {
    for (id, name, mana, types, filter, clue) in [
        (
            "foul_play",
            "Foul Play",
            "{1}{B}",
            &["Sorcery"][..],
            "AtMost(2)",
            true,
        ),
        (
            "humble_the_brute",
            "Humble the Brute",
            "{4}{W}",
            &["Instant"][..],
            "AtLeast(4)",
            true,
        ),
        (
            "root_out",
            "Root Out",
            "{2}{G}",
            &["Sorcery"][..],
            "Artifact",
            true,
        ),
    ] {
        let f = identity(id, name, mana, types);
        let effects = format!("{:?}", f.spell_effect);
        assert!(effects.contains("Destroy"), "{id}: {effects}");
        assert!(effects.contains(filter), "{id}: {effects}");
        assert!(
            effects.contains("Investigate { count: Fixed(1)") == clue,
            "{id}: {effects}"
        );
        assert!(effects.contains("count: Fixed(1)"), "{id}: {effects}");
        let targeting = f.targeting.as_ref().expect("targeting");
        assert_eq!(targeting.groups.len(), 1, "{id}");
        assert_eq!(
            (targeting.groups[0].min, targeting.groups[0].max),
            (1, 1),
            "{id}"
        );
        assert_eq!(targeting.groups[0].effect_indices, vec![0], "{id}");
    }
}

#[test]
fn q56_c_sac_draw_map_definitions() {
    for (id, name, flashback, treasure) in [
        ("costly_plunder", "Costly Plunder", None, false),
        ("deadly_dispute", "Deadly Dispute", None, true),
        (
            "eviscerators_insight",
            "Eviscerator's Insight",
            Some("{4}{B}"),
            false,
        ),
    ] {
        let f = identity(id, name, "{1}{B}", &["Instant"]);
        assert_eq!(
            f.flashback_cost.clone().map(|cost| cost.to_string()),
            flashback.map(str::to_string),
            "{id}"
        );
        let [group] = f.cast_cost_groups.as_slice() else {
            panic!("{id}: exactly one additional-cost group");
        };
        assert_eq!(group.group_id.as_str(), "additional_cost", "{id}");
        assert_eq!(
            group.presentation,
            AbilityPresentation::OracleLines(vec![1]),
            "{id}"
        );
        assert_eq!((group.min, group.max), (1, 1), "{id}");
        assert_eq!(group.options.len(), 1, "{id}");
        let option = format!("{:?}", group.options[0]);
        assert!(option.contains("Artifact"), "{id}: {option}");
        assert!(option.contains("Creature"), "{id}: {option}");
        assert!(option.contains("AdditionalPayment"), "{id}: {option}");
        let effects = format!("{:?}", f.spell_effect);
        assert!(
            effects.contains("Draw { who: Controller, count: Fixed(2) }"),
            "{id}: {effects}"
        );
        assert_eq!(effects.contains("treasure"), treasure, "{id}: {effects}");
    }
}

#[test]
fn q56_c_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for (id, name, face, digest) in [
        (
            "dryads_revival",
            "Dryad's Revival",
            "dryads_revival",
            "4795fadfe6b99c18657f9b88fc360fe8f9f8fe51cf9c6d151c88518f0e634063",
        ),
        (
            "morgue_theft",
            "Morgue Theft",
            "morgue_theft",
            "5816f53a2ea88ac3f81e9a12b602d2c9cea41805388f4c0cb827161c58e4c3cd",
        ),
        (
            "mystic_retrieval",
            "Mystic Retrieval",
            "mystic_retrieval",
            "d356e528aa01d90a9946a3fa2032c818963414b3a0e7d58312ce1f44ae50f680",
        ),
        (
            "foul_play",
            "Foul Play",
            "foul_play",
            "825b79abb0113832d3bd0ebe7a14936f12f6232fdd9b3aa4d82ddf97ac16febe",
        ),
        (
            "humble_the_brute",
            "Humble the Brute",
            "humble_the_brute",
            "8d9dcd24e93d5e1816d4e2b7840bbe26c8da965c67a69ae3f11cee4578d885a9",
        ),
        (
            "root_out",
            "Root Out",
            "root_out",
            "dcce3d57b017e6072a7d738ead2dbd680b770c0154a28cce22acfcf3543c39a1",
        ),
        (
            "costly_plunder",
            "Costly Plunder",
            "costly_plunder",
            "35a5b3f4961c92c1f84b3b620726a56cce711c8b7b9e852903ee9caa257ef123",
        ),
        (
            "deadly_dispute",
            "Deadly Dispute",
            "deadly_dispute",
            "94b1df6f1c218ef1bafac29603d71a5ece66d275106de39d33f355a93b736aa1",
        ),
        (
            "eviscerators_insight",
            "Eviscerator's Insight",
            "eviscerators_insight",
            "d1f4f93dcd26f81b0dfd39c5c9af04c612454902e6c553afe2e83e9d0cbd6d6f",
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
        assert_eq!(fields[2], face, "face id drift for {id}");
        assert_eq!(fields[3], name);
        assert_eq!(fields[4], digest, "pinned fingerprint digest for {id}");
    }
}
