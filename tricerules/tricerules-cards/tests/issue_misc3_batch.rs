//! Registry conformance for the reviewed token-creation direct-RON batch.
//!
//! Elder Auntie, Dragon Trainer, Glimmerburst, Release the Dogs and Hop to It were promoted after
//! complete-definition review against the pinned Scryfall snapshot (exact records and
//! `rulings_uri` fetched 2026-09-21), together with the Dog, Rabbit, Goblin, Dragon and Glimmer
//! predefined tokens they create. Governance: CR 111.1 (tokens), 121.1 (draw), 603.6a (entry
//! triggers), and 702.9 (flying).

use tricerules_cards::primitives::{Amount, SpellEffectKind, TriggerCondition};
use tricerules_cards::{CardRegistry, Color, Keyword, Layout};

#[test]
fn issue_misc3_batch_maps_definitions() {
    let registry = CardRegistry::global();

    // Token characteristics.
    for (id, name, types, colors, power, toughness, keywords) in [
        (
            "dog_w_1_1",
            "Dog",
            &["Creature", "Dog"][..],
            vec![Color::White],
            1u32,
            1u32,
            vec![],
        ),
        (
            "rabbit_w_1_1",
            "Rabbit",
            &["Creature", "Rabbit"][..],
            vec![Color::White],
            1,
            1,
            vec![],
        ),
        (
            "goblin_br_1_1",
            "Goblin",
            &["Creature", "Goblin"][..],
            vec![Color::Black, Color::Red],
            1,
            1,
            vec![],
        ),
        (
            "dragon_r_4_4_flying",
            "Dragon",
            &["Creature", "Dragon"][..],
            vec![Color::Red],
            4,
            4,
            vec![Keyword::Flying],
        ),
        (
            "glimmer_w_1_1",
            "Glimmer",
            &["Enchantment", "Creature", "Glimmer"][..],
            vec![Color::White],
            1,
            1,
            vec![],
        ),
    ] {
        let definition = registry
            .get(id)
            .unwrap_or_else(|| panic!("missing token {id}"));
        assert!(registry.is_token(id), "{id} is a token");
        assert_eq!(definition.layout, Layout::Normal, "{id}");
        assert_eq!(definition.name, name, "{id}");
        let f = definition.primary_face();
        assert_eq!(f.types, types, "{id} types");
        assert_eq!(f.colors_override, Some(colors), "{id} colors");
        assert_eq!(
            (f.power, f.toughness),
            (Some(power), Some(toughness)),
            "{id} stats"
        );
        assert_eq!(f.keywords, keywords, "{id} keywords");
    }

    // Elder Auntie: entry trigger creates one Goblin.
    for (id, name, mana, types, power, toughness, token, count) in [
        (
            "elder_auntie",
            "Elder Auntie",
            "{2}{R}",
            &["Creature", "Goblin", "Warlock"][..],
            2,
            2,
            "goblin_br_1_1",
            1,
        ),
        (
            "dragon_trainer",
            "Dragon Trainer",
            "{3}{R}{R}",
            &["Creature", "Human"][..],
            1,
            1,
            "dragon_r_4_4_flying",
            1,
        ),
    ] {
        let definition = registry.get(id).expect("registered");
        let f = definition.primary_face();
        assert_eq!(f.name, name);
        assert_eq!(f.mana_cost.to_string(), mana, "{id} mana cost");
        assert_eq!(f.types, types, "{id} types");
        assert_eq!((f.power, f.toughness), (Some(power), Some(toughness)));
        let [trigger] = f.triggered_abilities.as_slice() else {
            panic!("{id} has one trigger");
        };
        assert_eq!(trigger.trigger, TriggerCondition::WhenSelfEntersBattlefield);
        let [effect] = trigger.effect.as_slice() else {
            panic!("{id} has one effect");
        };
        assert!(
            matches!(effect, SpellEffectKind::CreateTokens { token: got, count: got_count, .. }
                if got == token && *got_count == Amount::Fixed(count)),
            "{id}: {effect:?}"
        );
    }

    // Glimmerburst: draw two, then one Glimmer token.
    let glimmer = registry
        .get("glimmerburst")
        .expect("registered")
        .primary_face();
    assert_eq!(glimmer.mana_cost.to_string(), "{3}{U}");
    assert_eq!(glimmer.types, ["Instant"]);
    let [draw, token] = glimmer.spell_effect.as_slice() else {
        panic!("draw then token");
    };
    assert!(
        matches!(draw, SpellEffectKind::Draw { count, .. } if *count == Amount::Fixed(2)),
        "{draw:?}"
    );
    assert!(
        matches!(token, SpellEffectKind::CreateTokens { token: got, count, .. }
            if got == "glimmer_w_1_1" && *count == Amount::Fixed(1)),
        "{token:?}"
    );

    // Release the Dogs / Hop to It: plain multi-token sorceries.
    for (id, name, mana, token, count) in [
        (
            "release_the_dogs",
            "Release the Dogs",
            "{3}{W}",
            "dog_w_1_1",
            4u32,
        ),
        ("hop_to_it", "Hop to It", "{2}{W}", "rabbit_w_1_1", 3),
    ] {
        let f = registry.get(id).expect("registered").primary_face();
        assert_eq!(f.name, name);
        assert_eq!(f.mana_cost.to_string(), mana, "{id} mana cost");
        assert_eq!(f.types, ["Sorcery"]);
        let [effect] = f.spell_effect.as_slice() else {
            panic!("{id} has one effect");
        };
        assert!(
            matches!(effect, SpellEffectKind::CreateTokens { token: got, count: got_count, .. }
                if got == token && *got_count == Amount::Fixed(count)),
            "{id}: {effect:?}"
        );
    }
}

#[test]
fn issue_misc3_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for (id, name, fingerprint) in [
        (
            "elder_auntie",
            "Elder Auntie",
            "2994a7e6fd76ff5b570039fd3ba5fecbc45a922bb84fbd56c2e58bb6c3befb75",
        ),
        (
            "dragon_trainer",
            "Dragon Trainer",
            "36e28591003b0385a1043c728717b297a57e619dd72c9d0ce4fc8fa6e9c7ffc3",
        ),
        (
            "glimmerburst",
            "Glimmerburst",
            "eaabf1f4a7be96b47cc22f94742f8277ac56b4f451c822615b302f35552861f5",
        ),
        (
            "release_the_dogs",
            "Release the Dogs",
            "1d0b98188979d8c19353852d6a02c1b19d4a027193462b6faad7e017cbf9a9cc",
        ),
        (
            "hop_to_it",
            "Hop to It",
            "30367544e4f55b22efd2ccb9152f61416cd6221c8f6717bfb8753d259af57505",
        ),
    ] {
        let row = fingerprints
            .lines()
            .find(|line| line.starts_with(&format!("{id}\t")))
            .unwrap_or_else(|| panic!("missing fingerprint row for {id}"));
        let fields: Vec<&str> = row.split('\t').collect();
        assert_eq!(fields.len(), 5, "fingerprint row shape: {row}");
        assert_eq!(fields[0], id);
        assert_eq!(fields[1], name);
        assert_eq!(fields[4], fingerprint, "fingerprint drift for {id}");
    }
}
