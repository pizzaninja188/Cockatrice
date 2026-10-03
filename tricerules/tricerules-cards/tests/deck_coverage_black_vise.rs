use tricerules_cards::CardRegistry;

const PRODUCER: &str = r#"(ability_id: "producer", presentation: Fallback, definition: AsEntersChooseOpponent(link_id: "opponent"))"#;
const CONSUMER: &str = r#"(ability_id: "consumer", presentation: Fallback, trigger: AtBeginningOfChosenPlayerUpkeep(link_id: "opponent"), effect: [DamagePlayer(amount: 1, who: AffectedPlayer)])"#;

fn face(producers: &str, consumers: &str) -> String {
    format!(
        r#"(id: "chosen_opponent_fixture", name: "Chosen opponent fixture", face_id: "chosen_opponent_fixture", types: ["Artifact"], static_abilities: [{producers}], triggered_abilities: [{consumers}])"#
    )
}

fn reject(producers: &str, consumers: &str, reason: &str) {
    let raw = face(producers, consumers);
    let error = CardRegistry::from_chunks_and_tokens(&[&raw], &[])
        .unwrap_err()
        .to_string();
    assert!(error.contains(reason), "{error}");
}

#[test]
fn chosen_opponent_links_require_one_matching_printed_pair() {
    let valid = face(PRODUCER, CONSUMER);
    CardRegistry::from_chunks_and_tokens(&[&valid], &[]).unwrap();
    reject(PRODUCER, "", "one matching printed producer and consumer");
    reject("", CONSUMER, "one matching printed producer and consumer");
    reject(
        PRODUCER,
        &CONSUMER.replace("opponent\"", "other\""),
        "one matching printed producer and consumer",
    );
    reject(
        &format!(
            "{PRODUCER},{}",
            PRODUCER.replace("producer\"", "producer_two\"")
        ),
        CONSUMER,
        "one matching printed producer and consumer",
    );
    reject(
        PRODUCER,
        &format!(
            "{CONSUMER},{}",
            CONSUMER.replace("consumer\"", "consumer_two\"")
        ),
        "one matching printed producer and consumer",
    );
    reject(
        &PRODUCER.replace("\"opponent\"", "\"\""),
        &CONSUMER.replace("\"opponent\"", "\"\""),
        "link id",
    );
}

#[test]
fn chosen_opponent_upkeep_cannot_be_statically_or_dynamically_granted() {
    let grant = format!(
        r#"(ability_id: "grant", presentation: Fallback, definition: GrantTriggeredAbilityToPermanents(filter: (kind: AnyPermanent), triggered_abilities: [{CONSUMER}]))"#
    );
    reject(&grant, "", "printed face-local pair");
    let raw = format!(
        r#"(id: "grant_fixture", name: "Grant fixture", face_id: "grant_fixture", types: ["Instant"], spell_effect: [GrantTriggeredAbility(subject: Chosen((kind: AnyPermanent)), ability: {CONSUMER})])"#
    );
    let error = CardRegistry::from_chunks_and_tokens(&[&raw], &[])
        .unwrap_err()
        .to_string();
    assert!(error.contains("cannot be granted"), "{error}");
}
