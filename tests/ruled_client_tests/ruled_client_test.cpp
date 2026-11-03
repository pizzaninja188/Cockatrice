// Headless client-core tests for the ruled view model.
//
// The client mirror of tests/ruled_batch_tests/: synthetic `ruled::v1::RuledEventBatch`
// messages go into `RuledEventDispatcher`, and the assertions are on `RuledClientState` —
// identity maps, legal-action parsing per action kind, stack tracking, combat staging, and
// pending player choices. Nothing here renders: `RuledClientState` and `RuledEventDispatcher`
// reach the Qt game objects only through `RuledClientHost`, and `FakeHost` below stands in for
// `GameEventHandler`, recording the commands the view model would have sent.
//
// This is the layer that used to be untestable — the same logic lived inline in the
// `RULED_PAYLOAD` case of the upstream `GameEventHandler`, behind the whole client.

#include "game/ruled/ruled_activation.h"
#include "game/ruled/ruled_auto_pass_policy.h"
#include "game/ruled/ruled_card_display_identity.h"
#include "game/ruled/ruled_card_popup.h"
#include "game/ruled/ruled_client_host.h"
#include "game/ruled/ruled_client_state.h"
#include "game/ruled/ruled_dev_command_parser.h"
#include "game/ruled/ruled_event_dispatcher.h"
#include "game/ruled/ruled_mana_pool_tracker.h"
#include "game/ruled/ruled_payment_resume_policy.h"
#include "game/ruled/ruled_pending_cast.h"
#include "game/ruled/ruled_presentation_resolver.h"
#include "game/ruled/ruled_private_card_look.h"
#include "game/ruled/ruled_public_zone_order_plan.h"
#include "game/ruled/ruled_restricted_mana_model.h"
#include "game/ruled/ruled_zone_snapshot_policy.h"
#include "game/ruled/ruled_zone_view_policy.h"

#include <QBuffer>
#include <QJsonArray>
#include <QRectF>
#include <QRegularExpression>
#include <QSet>
#include <QSignalSpy>
#include <QString>
#include <QTest>
#include <gtest/gtest.h>
#include <libcockatrice/card/database/parser/cockatrice_xml_4.h>
#include <libcockatrice/interfaces/noop_card_preference_provider.h>
#include <libcockatrice/interfaces/noop_card_set_priority_controller.h>
#include <libcockatrice/protocol/pb/ruled_v1.pb.h>

namespace
{

constexpr int kLocalPlayer = 0;
constexpr int kOpponent = 1;

/// Records everything the view model asks of the UI, and answers the few queries it makes.
class FakeHost : public RuledClientHost
{
public:
    int local = kLocalPlayer;
    int activePlayer = -1;
    int priorityPlayer = -1;
    int toolbarPhase = -1;
    int arrowSyncRequests = 0;
    CardNameMap presentationCards;
    CardInfoPtr presentationCard(const QString &name) const override
    {
        return presentationCards.value(name);
    }

    struct SyntheticCard
    {
        quint32 oid;
        QString name;
        int controllerPlayerId;
        QString setName;
        std::optional<TokenStackIdentity> sourceTokenIdentity;
    };
    QVector<SyntheticCard> createdSyntheticCards;
    QVector<quint32> removedSyntheticCards;
    QVector<PreparationCopy> preparationCopies;
    int preparationSyncs = 0;
    void reconcilePreparationCopies(const QVector<PreparationCopy> &copies) override
    {
        preparationCopies = copies;
        ++preparationSyncs;
    }
    QVector<ruled::v1::RuledCommand> sentCommands;
    int dialogRequests = 0;
    QString lastDialogPrompt;
    quint64 lastDialogPromptId = 0;
    QVector<quint32> lastDialogCandidateOids;
    QStringList lastDialogCandidateNames;
    bool lastDialogOrdered = false;
    bool autoSubmitDialogChoice = false;

    /// Optional P/T fallback, keyed by engine oid, for the ZoneView-stripped path.
    QHash<quint32, QPair<int, int>> fallbackPt;
    QHash<quint32, QString> cardNames;
    QHash<quint32, QString> providerIds;

    [[nodiscard]] int localPlayerId() const override
    {
        return local;
    }
    [[nodiscard]] int currentActivePlayerId() const override
    {
        return activePlayer;
    }
    void setActivePlayerId(int playerId) override
    {
        activePlayer = playerId;
    }
    void setPriorityPlayerId(int playerId) override
    {
        priorityPlayer = playerId;
    }
    void setToolbarPhase(int phase) override
    {
        toolbarPhase = phase;
    }
    void createSyntheticStackCard(quint32 virtualOid,
                                  const QString &displayName,
                                  int controllerPlayerId,
                                  const QString &setName,
                                  const std::optional<TokenStackIdentity> &sourceTokenIdentity) override
    {
        createdSyntheticCards.append({virtualOid, displayName, controllerPlayerId, setName, sourceTokenIdentity});
    }
    void removeSyntheticStackCard(quint32 virtualOid) override
    {
        removedSyntheticCards.append(virtualOid);
    }
    [[nodiscard]] QString stackCardProviderId(quint32 oid) const override
    {
        return providerIds.value(oid);
    }
    [[nodiscard]] bool fallbackCreaturePt(quint32 engineOid, int *power, int *toughness) const override
    {
        const auto it = fallbackPt.constFind(engineOid);
        if (it == fallbackPt.constEnd()) {
            return false;
        }
        *power = it->first;
        *toughness = it->second;
        return true;
    }
    [[nodiscard]] QString battlefieldCardName(quint32 engineOid) const override
    {
        return cardNames.value(engineOid);
    }
    void sendRuledCommand(const ruled::v1::RuledCommand &command) override
    {
        sentCommands.append(command);
    }
    void sendRuledCommandExpectingAck(const ruled::v1::RuledCommand &command,
                                      std::function<void(bool accepted)> onFinished) override
    {
        sentCommands.append(command);
        pendingAck = std::move(onFinished);
    }
    void requestResolutionChoiceDialog(const QString &prompt,
                                       quint64 resolutionPromptId,
                                       const QVector<quint32> &candidateOids,
                                       const QStringList &candidateNames,
                                       int,
                                       int,
                                       bool ordered,
                                       bool) override
    {
        ++dialogRequests;
        lastDialogPrompt = prompt;
        lastDialogPromptId = resolutionPromptId;
        lastDialogCandidateOids = candidateOids;
        lastDialogCandidateNames = candidateNames;
        lastDialogOrdered = ordered;
        if (autoSubmitDialogChoice && !candidateOids.isEmpty()) {
            ruled::v1::RuledCommand command;
            auto *submission = command.mutable_submit_resolution_choice();
            submission->set_resolution_prompt_id(resolutionPromptId);
            submission->add_chosen_object_ids(candidateOids.first());
            sendRuledCommand(command);
        }
    }
    void scheduleSpellTargetArrowSync() override
    {
        ++arrowSyncRequests;
    }

    /// Answer the most recent command that asked for an ack (confirmBlockers).
    void answerPendingAck(bool accepted)
    {
        ASSERT_TRUE(static_cast<bool>(pendingAck));
        auto cb = pendingAck;
        pendingAck = nullptr;
        cb(accepted);
    }

private:
    std::function<void(bool)> pendingAck;
};

class RuledClientTest : public ::testing::Test
{
protected:
    FakeHost host;
    RuledClientState *state = nullptr;
    RuledEventDispatcher *dispatcher = nullptr;

    void SetUp() override
    {
        state = new RuledClientState(&host);
        dispatcher = new RuledEventDispatcher(state, &host);
    }

    void TearDown() override
    {
        delete dispatcher;
        delete state;
    }

    template <typename Batch>
    void apply(Batch &&batch)
    {
        // Existing positive prompt fixtures predate prompt IDs. Give those fixtures an explicit
        // synthetic offer only when they contain an uncorrelated RCR and no authoritative batch
        // state. Tests for missing, zero, or stale IDs must use applyRaw() so this adapter never
        // repairs the malformed payload being tested.
        if (!batch.has_resolution_prompt_state()) {
            int lastChoiceIndex = -1;
            bool allChoiceIdsAreUnset = true;
            for (int i = 0; i < batch.events_size(); ++i) {
                const auto &event = batch.events(i);
                if (!event.has_resolution_choice_required())
                    continue;
                lastChoiceIndex = i;
                allChoiceIdsAreUnset &= event.resolution_choice_required().resolution_prompt_id() == 0;
            }
            if (lastChoiceIndex >= 0 && allChoiceIdsAreUnset) {
                auto *activeChoice = batch.mutable_events(lastChoiceIndex)->mutable_resolution_choice_required();
                const quint64 promptId = nextSyntheticResolutionPromptId++;
                if (activeChoice->deciding_player_id() == host.localPlayerId()) {
                    activeChoice->set_resolution_prompt_id(promptId);
                }
                auto *snapshot = batch.mutable_resolution_prompt_state();
                snapshot->set_active(true);
                snapshot->set_deciding_player_id(activeChoice->deciding_player_id());
                if (activeChoice->deciding_player_id() == host.localPlayerId())
                    snapshot->set_resolution_prompt_id(promptId);
                // Earlier choices in one batch are transient publications. Their public reveal
                // data is retained, while only the last RCR becomes answerable.
                for (int i = 0; i < lastChoiceIndex; ++i) {
                    auto *event = batch.mutable_events(i);
                    if (event->has_resolution_choice_required())
                        event->mutable_resolution_choice_required()->set_resolution_prompt_id(0);
                }
            }
        }
        applyRaw(batch);
    }

    void applyRaw(const ruled::v1::RuledEventBatch &input)
    {
        ruled::v1::RuledEventBatch batch = input;
        // Populate the shared snapshot for tests constructing a public choice. Tests can supply
        // an explicit reveal_id to exercise malformed payloads without this fixture completion.
        for (auto &event : *batch.mutable_events()) {
            if (!event.has_resolution_choice_required())
                continue;
            auto *choice = event.mutable_resolution_choice_required();
            if (!choice->has_public_reveal() || !choice->public_reveal().reveal_id().empty())
                continue;
            auto *reveal = choice->mutable_public_reveal();
            reveal->set_source_object_id(choice->source_object_id());
            reveal->set_source_zone(choice->choice_kind() == ruled::v1::CHOICE_KIND_LIBRARY_LOOK
                                        ? ruled::v1::CHOICE_CANDIDATE_SOURCE_ZONE_LIBRARY
                                        : ruled::v1::CHOICE_CANDIDATE_SOURCE_ZONE_HAND);
            std::string key = "test:" + std::to_string(choice->source_object_id()) + ":" +
                              std::to_string(reveal->zone_owner_player_id());
            for (int i = 0; i < choice->candidate_names_size(); ++i) {
                auto *card = reveal->add_cards();
                if (i < choice->candidate_object_ids_size())
                    card->set_object_id(choice->candidate_object_ids(i));
                if (i < choice->candidate_card_ids_size())
                    card->set_card_id(choice->candidate_card_ids(i));
                card->set_card_name(choice->candidate_names(i));
                key += ":" + choice->candidate_names(i);
            }
            reveal->set_reveal_id(key);
        }
        // Go through the serialized entry point so the per-batch reset runs exactly as it does
        // when an Event_RuledPayload arrives.
        std::string payload;
        ASSERT_TRUE(batch.SerializeToString(&payload));
        ASSERT_TRUE(dispatcher->processPayload(payload));
    }

    void stageChoiceForTest(RuledClientState::RuledPendingChoice choice)
    {
        const quint64 promptId = nextSyntheticResolutionPromptId++;
        state->resolutionPromptSnapshotValid = true;
        state->resolutionPromptActive = true;
        state->resolutionPromptDeciderPlayerId = host.localPlayerId();
        state->resolutionPromptId = promptId;
        state->resolutionPromptSemantics.clear();
        state->setPendingChoice(std::move(choice));
    }

    /// Batch carrying just a phase change, to drive the combat state machine.
    static ruled::v1::RuledEventBatch phaseBatch(ruled::v1::PhaseId phase, int activePlayerId)
    {
        ruled::v1::RuledEventBatch batch;
        auto *pc = batch.add_events()->mutable_phase_changed();
        pc->set_phase_id(phase);
        pc->set_active_player_id(activePlayerId);
        return batch;
    }

    quint64 nextSyntheticResolutionPromptId = 1000;

    static ruled::v1::BattlefieldObjectMap::Entry *
    addPermanent(ruled::v1::RuledEvent *event, int playerId, quint32 oid, int serverCardId)
    {
        auto *entry = event->mutable_battlefield_object_map()->add_entries();
        entry->set_player_id(playerId);
        entry->set_engine_object_id(oid);
        entry->set_server_card_id(serverCardId);
        entry->set_is_creature(true);
        return entry;
    }

    static ruled::v1::LegalHandAction *addHandAction(ruled::v1::LegalActions &actions,
                                                     ruled::v1::HandActionKind kind,
                                                     int handIndex,
                                                     const std::string &cardName,
                                                     int faceIndex = 0,
                                                     bool needsTarget = false,
                                                     const std::string &cost = {})
    {
        auto *action = actions.add_hand_actions();
        action->set_kind(kind);
        action->set_cast_method(ruled::v1::CAST_METHOD_NORMAL);
        action->set_hand_index(static_cast<quint32>(handIndex));
        action->set_card_name(cardName);
        action->set_face_index(static_cast<quint32>(faceIndex));
        action->set_needs_target(needsTarget);
        action->set_cost(cost);
        return action;
    }

    static void addLegalBlockPair(ruled::v1::LegalActions &actions, quint32 blockerOid, quint32 attackerOid)
    {
        auto *pair = actions.add_legal_block_pairs();
        pair->set_blocker_id(blockerOid);
        pair->set_attacker_id(attackerOid);
    }

    static ruled::v1::AttackAssignment playerAttackAssignment(quint32 attackerOid, int playerId = kOpponent)
    {
        ruled::v1::AttackAssignment assignment;
        assignment.set_attacker_object_id(attackerOid);
        assignment.set_defending_player_id(playerId);
        assignment.mutable_defender()->set_kind(ruled::v1::TARGET_REF_KIND_PLAYER);
        assignment.mutable_defender()->set_object_id(static_cast<quint32>(playerId));
        return assignment;
    }

    static void addLegalPlayerAttack(ruled::v1::LegalActions &actions, quint32 attackerOid)
    {
        *actions.add_legal_attack_assignments() = playerAttackAssignment(attackerOid);
    }

    static ruled::v1::AttackAssignment
    permanentAttackAssignment(quint32 attackerOid, quint32 defenderOid, quint64 defenderGeneration)
    {
        ruled::v1::AttackAssignment assignment;
        assignment.set_attacker_object_id(attackerOid);
        assignment.set_attacker_zone_change_generation(7);
        assignment.set_defending_player_id(kOpponent);
        assignment.mutable_defender()->set_kind(ruled::v1::TARGET_REF_KIND_PERMANENT);
        assignment.mutable_defender()->set_object_id(defenderOid);
        assignment.set_defender_zone_change_generation(defenderGeneration);
        return assignment;
    }
};

constexpr quint32 kUnsupportedCostSourceOid = 700;
constexpr quint64 kUnsupportedCostSourceGeneration = 4;
constexpr int kUnsupportedCostAbilityIndex = 1;
constexpr quint32 kNightCounterOptionId = 0xf00dbaad;
constexpr quint32 kChargeCounterOptionId = 0x51a7e001;

void setWireCostChoiceKind(ruled::v1::LegalCostChoice *choice, int wireKind)
{
    if (wireKind == static_cast<int>(ruled::v1::COST_CHOICE_KIND_TAP)) {
        choice->set_kind(ruled::v1::COST_CHOICE_KIND_TAP);
        return;
    }
    const auto *kindField = choice->GetDescriptor()->FindFieldByName("kind");
    choice->GetReflection()->SetEnumValue(choice, kindField, wireKind);
}

ruled::v1::RuledEventBatch unsupportedCostBatch(int laterWireKind = static_cast<int>(ruled::v1::COST_CHOICE_KIND_TAP),
                                                bool twoChoices = false,
                                                bool publishPermanentAction = false)
{
    ruled::v1::RuledEventBatch batch;
    auto *view = batch.add_events()->mutable_zone_view()->add_per_player();
    view->set_player_id(kLocalPlayer);
    auto *source = view->add_battlefield_objects();
    source->set_object_id(kUnsupportedCostSourceOid);
    source->set_zone_change_generation(kUnsupportedCostSourceGeneration);
    auto *ability = source->add_activated_abilities();
    ability->set_ability_index(kUnsupportedCostAbilityIndex);
    ability->set_text("Pay an ordinary activated cost.");
    ability->set_mana_cost("{B/P}{W/P}");
    ability->set_activatable(true);

    auto &actions = (*batch.mutable_legal_by_player())[kLocalPlayer];
    const quint64 wireKey = (static_cast<quint64>(kUnsupportedCostSourceOid) << 32) |
                            static_cast<quint32>(kUnsupportedCostAbilityIndex);
    auto &costs = (*actions.mutable_cost_choices_by_ability())[wireKey];
    costs.set_non_mana_costs_payable(true);
    const auto addChoice = [&](int costIndex, int wireKind, quint32 candidateId, quint64 generation) {
        auto *choice = costs.add_choices();
        choice->set_cost_index(costIndex);
        choice->set_zone(ruled::v1::COST_CHOICE_ZONE_BATTLEFIELD);
        choice->set_min(1);
        choice->set_max(1);
        setWireCostChoiceKind(choice, wireKind);
        choice->add_candidate_ids(candidateId);
        auto *candidate = choice->add_candidate_objects();
        candidate->mutable_object()->set_object_id(candidateId);
        candidate->mutable_object()->set_zone_change_generation(generation);
        candidate->set_contribution(1);
        if (wireKind == static_cast<int>(ruled::v1::COST_CHOICE_KIND_REMOVE_COUNTERS_FROM_PERMANENTS)) {
            auto *group = choice->mutable_counter_removal_group();
            group->set_required_count(3);
            auto *groupCandidate = group->add_candidates();
            groupCandidate->mutable_object()->set_object_id(candidateId);
            groupCandidate->mutable_object()->set_zone_change_generation(generation);
            auto *option = groupCandidate->add_options();
            option->set_option_id(1);
            option->set_label("+1/+1");
            option->set_available_count(3);
        }
        return choice;
    };
    if (twoChoices) {
        addChoice(0, static_cast<int>(ruled::v1::COST_CHOICE_KIND_TAP), 902, 14);
        addChoice(1, laterWireKind, 901, 13);
    } else {
        addChoice(1, laterWireKind, 901, 13);
    }
    if (publishPermanentAction) {
        auto *action = actions.add_permanent_actions();
        action->set_kind(ruled::v1::PERMANENT_ACTION_KIND_TURN_FACE_UP);
        action->set_object_id(kUnsupportedCostSourceOid);
        action->set_zone_change_generation(kUnsupportedCostSourceGeneration);
        action->set_mana_cost("{1}");
    }
    return batch;
}

ruled::v1::RuledEventBatch aggregateCounterOfferBatch()
{
    auto batch = unsupportedCostBatch(static_cast<int>(ruled::v1::COST_CHOICE_KIND_REMOVE_COUNTERS_FROM_PERMANENTS));
    auto &actions = (*batch.mutable_legal_by_player())[kLocalPlayer];
    const quint64 wireKey = (static_cast<quint64>(kUnsupportedCostSourceOid) << 32) |
                            static_cast<quint32>(kUnsupportedCostAbilityIndex);
    auto *choice = (*actions.mutable_cost_choices_by_ability())[wireKey].mutable_choices(0);
    auto *group = choice->mutable_counter_removal_group();
    group->set_required_count(3);
    group->clear_candidates();

    auto addCandidate = [&](quint32 objectId, quint64 generation, quint32 chargeAvailable, quint32 nightAvailable) {
        auto *candidate = group->add_candidates();
        candidate->mutable_object()->set_object_id(objectId);
        candidate->mutable_object()->set_zone_change_generation(generation);
        if (nightAvailable > 0) {
            auto *night = candidate->add_options();
            night->set_option_id(kNightCounterOptionId);
            night->set_label("Night");
            night->set_available_count(nightAvailable);
        }
        if (chargeAvailable > 0) {
            auto *charge = candidate->add_options();
            charge->set_option_id(kChargeCounterOptionId);
            charge->set_label("Charge");
            charge->set_available_count(chargeAvailable);
        }
    };
    addCandidate(901, 13, 3, 2);
    addCandidate(902, 17, 2, 0);
    addCandidate(903, 21, 1, 0);
    return batch;
}

ruled::v1::RuledEventBatch aggregateCounterTwoGroupOfferBatch()
{
    auto batch = aggregateCounterOfferBatch();
    auto &actions = (*batch.mutable_legal_by_player())[kLocalPlayer];
    const quint64 wireKey = (static_cast<quint64>(kUnsupportedCostSourceOid) << 32) |
                            static_cast<quint32>(kUnsupportedCostAbilityIndex);
    auto &costs = (*actions.mutable_cost_choices_by_ability())[wireKey];
    auto *choice = costs.add_choices();
    choice->set_cost_index(4);
    choice->set_zone(ruled::v1::COST_CHOICE_ZONE_BATTLEFIELD);
    choice->set_min(1);
    choice->set_max(1);
    setWireCostChoiceKind(choice,
                          static_cast<int>(ruled::v1::COST_CHOICE_KIND_REMOVE_COUNTERS_FROM_PERMANENTS));
    choice->add_candidate_ids(901);
    auto *legacyCandidate = choice->add_candidate_objects();
    legacyCandidate->mutable_object()->set_object_id(901);
    legacyCandidate->mutable_object()->set_zone_change_generation(13);
    legacyCandidate->set_contribution(1);

    auto *group = choice->mutable_counter_removal_group();
    group->set_required_count(1);
    auto *candidate = group->add_candidates();
    candidate->mutable_object()->set_object_id(901);
    candidate->mutable_object()->set_zone_change_generation(13);
    auto *charge = candidate->add_options();
    charge->set_option_id(kChargeCounterOptionId);
    charge->set_label("Charge");
    charge->set_available_count(2);
    return batch;
}

ruled::v1::RuledEventBatch aggregateCounterSharedReservationOfferBatch(bool exhaustBothOptions)
{
    auto batch = aggregateCounterTwoGroupOfferBatch();
    auto &actions = (*batch.mutable_legal_by_player())[kLocalPlayer];
    const quint64 wireKey = (static_cast<quint64>(kUnsupportedCostSourceOid) << 32) |
                            static_cast<quint32>(kUnsupportedCostAbilityIndex);
    auto &costs = (*actions.mutable_cost_choices_by_ability())[wireKey];
    auto *group = costs.mutable_choices(1)->mutable_counter_removal_group();
    group->set_required_count(exhaustBothOptions ? 2 : 1);
    auto *candidate = group->mutable_candidates(0);
    candidate->mutable_options(0)->set_available_count(1);
    if (exhaustBothOptions) {
        auto *night = candidate->add_options();
        night->set_option_id(kNightCounterOptionId);
        night->set_label("Night");
        night->set_available_count(1);
    }
    return batch;
}

PendingActivatedAbility makeAggregateCounterDraft(const RuledClientState &state)
{
    PendingActivatedAbility pending;
    pending.valid = true;
    pending.permanentOid = kUnsupportedCostSourceOid;
    pending.sourceZone = state.abilitySourceZone(kUnsupportedCostSourceOid);
    pending.expectedZoneChangeGeneration = state.abilitySourceGeneration(kUnsupportedCostSourceOid);
    pending.abilityIndex = kUnsupportedCostAbilityIndex;
    pending.costChoices = state.abilityCostChoices(kUnsupportedCostSourceOid, kUnsupportedCostAbilityIndex);
    const auto ability = state.activatedAbilityForOid(kUnsupportedCostSourceOid, kUnsupportedCostAbilityIndex);
    if (ability) {
        pending.flexPips = RuledPendingCast::parseFlexPips(ability->manaCost);
    }
    return pending;
}

PendingActivatedAbility &prepareOwnedAggregateCounterDraft(RuledPendingCast &owner, const RuledClientState &state)
{
    auto &draft = owner.beginAbility();
    const auto assignedDraftId = draft.draftId;
    draft = makeAggregateCounterDraft(state);
    draft.draftId = assignedDraftId;
    draft.waitingForCost = true;
    return draft;
}

QVector<ruled::v1::FlexPipPayment> aggregatePhiPayments(const PendingActivatedAbility &pending,
                                                       bool payLifeForFirstPhi = false)
{
    QVector<ruled::v1::FlexPipPayment> payments;
    int phiCount = 0;
    for (const auto &pip : pending.flexPips) {
        if (!pip.phyrexian) {
            continue;
        }
        ruled::v1::FlexPipPayment payment;
        payment.set_pip_index(pip.pipIndex);
        payment.set_pay_life((phiCount == 0) == payLifeForFirstPhi);
        payments.append(payment);
        ++phiCount;
    }
    return payments;
}

QVector<RuledPendingCounterContribution> aggregateAcceptedCounterRows()
{
    return {{1, 901, 13, kNightCounterOptionId, 1}, {1, 901, 13, kChargeCounterOptionId, 2}};
}

std::optional<RuledOwnedCounterGroupBegin> prepareOwnedAggregateCounterBegin(RuledPendingCast &owner,
                                                                              const RuledClientState &state)
{
    auto &draft = prepareOwnedAggregateCounterDraft(owner, state);
    if (!ruledStageCounterContributions(draft, 1, 901, 13, aggregateAcceptedCounterRows()) ||
        !ruledConfirmCounterContributions(draft)) {
        return std::nullopt;
    }
    draft.waitingForCost = false;
    return owner.prepareCounterGroupBegin(state, kLocalPlayer, aggregatePhiPayments(draft));
}

ruled::v1::PendingAbilityActivation makeOwnedAggregateCounterPaymentSnapshot(
    const RuledOwnedCounterGroupBegin &captured,
    quint64 transactionId)
{
    ruled::v1::PendingAbilityActivation current;
    current.set_transaction_id(transactionId);
    current.set_revision(1);
    current.set_stage(ruled::v1::ABILITY_ACTIVATION_STAGE_PAYMENT);
    current.set_actor_player_id(captured.actorPlayerId);
    current.set_deciding_player_id(captured.actorPlayerId);
    current.set_source_object_id(captured.announcement.source_object_id());
    current.set_source_zone_change_generation(captured.announcement.expected_zone_change_generation());
    current.set_source_zone(captured.announcement.source_zone());
    current.set_ability_index(captured.announcement.ability_index());
    current.set_reserved_object_id(kUnsupportedCostSourceOid + 5000);
    current.set_source_description("Aggregate counter activation");
    current.set_locked_total_cost("opaque engine cost label");
    for (const auto &selection : captured.announcement.cost_selections())
        current.add_cost_selections()->CopyFrom(selection);
    for (const auto &payment : captured.announcement.flex_payments())
        current.add_flex_payments()->CopyFrom(payment);
    return current;
}

std::optional<ruled::v1::PendingAbilityActivation> prepareAcceptedAggregateCounterPayment(
    RuledPendingCast &owner,
    const RuledClientState &state,
    quint64 transactionId,
    quint64 revision)
{
    const auto captured = prepareOwnedAggregateCounterBegin(owner, state);
    if (!captured) {
        return std::nullopt;
    }
    auto accepted = makeOwnedAggregateCounterPaymentSnapshot(*captured, transactionId);
    accepted.set_revision(revision);
    if (owner.acknowledgeCounterGroupBegin(*captured, true, &accepted) !=
        RuledOwnedCounterGroupBeginUpdate::Accepted) {
        return std::nullopt;
    }
    return accepted;
}

ruled::v1::PendingAbilityActivation localPaymentSnapshot(quint64 transactionId, quint64 revision)
{
    ruled::v1::PendingAbilityActivation snapshot;
    snapshot.set_transaction_id(transactionId);
    snapshot.set_revision(revision);
    snapshot.set_stage(ruled::v1::ABILITY_ACTIVATION_STAGE_PAYMENT);
    return snapshot;
}

ruled::v1::PendingAbilityActivation localWaitingActivationSnapshot(quint64 transactionId, quint64 revision)
{
    ruled::v1::PendingAbilityActivation snapshot;
    snapshot.set_transaction_id(transactionId);
    snapshot.set_revision(revision);
    snapshot.set_stage(ruled::v1::ABILITY_ACTIVATION_STAGE_CHOOSE_OPPONENT);
    return snapshot;
}

bool sameAggregateCounterRows(const QVector<RuledPendingCounterContribution> &left,
                              const QVector<RuledPendingCounterContribution> &right)
{
    if (left.size() != right.size())
        return false;
    for (int i = 0; i < left.size(); ++i) {
        const auto &a = left.at(i);
        const auto &b = right.at(i);
        if (a.costIndex != b.costIndex || a.objectId != b.objectId ||
            a.zoneChangeGeneration != b.zoneChangeGeneration || a.optionId != b.optionId || a.count != b.count ||
            a.counterLabel != b.counterLabel) {
            return false;
        }
    }
    return true;
}

bool sameAggregateFlexPayments(const QVector<ruled::v1::FlexPipPayment> &left,
                               const QVector<ruled::v1::FlexPipPayment> &right)
{
    if (left.size() != right.size())
        return false;
    for (int i = 0; i < left.size(); ++i) {
        if (left.at(i).SerializeAsString() != right.at(i).SerializeAsString())
            return false;
    }
    return true;
}

PendingActivatedAbility &prepareUnsupportedCostDraft(RuledPendingCast &owner, const RuledClientState &state)
{
    auto &draft = owner.beginAbility();
    draft.permanentOid = kUnsupportedCostSourceOid;
    draft.sourceZone = state.abilitySourceZone(kUnsupportedCostSourceOid);
    draft.expectedZoneChangeGeneration = kUnsupportedCostSourceGeneration;
    draft.abilityIndex = kUnsupportedCostAbilityIndex;
    draft.costChoices = state.abilityCostChoices(kUnsupportedCostSourceOid, kUnsupportedCostAbilityIndex);
    draft.waitingForCost = true;
    return draft;
}

enum class AbilityCommandProbe
{
    Activate,
    Begin,
    Preview,
    Commit,
};

void sendAbilityCommandProbe(FakeHost &host, const PendingActivatedAbility &draft, AbilityCommandProbe probe)
{
    switch (probe) {
        case AbilityCommandProbe::Activate: {
            ruled::v1::RuledCommand command;
            draft.writeActivationHeader(*command.mutable_activate_ability());
            host.sendRuledCommand(command);
            return;
        }
        case AbilityCommandProbe::Begin: {
            ruled::v1::RuledCommand command;
            auto *begin = command.mutable_begin_ability_activation();
            begin->set_source_object_id(draft.permanentOid);
            begin->set_source_zone(draft.sourceZone);
            begin->set_expected_zone_change_generation(draft.expectedZoneChangeGeneration);
            begin->set_ability_index(static_cast<quint32>(draft.abilityIndex));
            host.sendRuledCommandExpectingAck(command, [](bool) {});
            return;
        }
        case AbilityCommandProbe::Preview: {
            RuledPayment payment;
            payment.begin();
            ruled::v1::RuledCommand activation;
            draft.writeActivationHeader(*activation.mutable_activate_ability());
            const auto preview = payment.requestAction(activation);
            ruled::v1::RuledCommand command;
            *command.mutable_preview_payment() = preview;
            host.sendRuledCommand(command);
            return;
        }
        case AbilityCommandProbe::Commit: {
            RuledPayment payment;
            payment.begin();
            ruled::v1::RuledCommand command;
            auto *commit = command.mutable_commit_ability_activation();
            commit->set_transaction_id(91);
            commit->set_expected_revision(3);
            const auto preview = payment.requestAction(command);
            if (!preview.has_commit_ability_activation()) {
                return;
            }
            payment.writePayment(command);
            host.sendRuledCommand(command);
            return;
        }
    }
}

bool isAbilityCommandProbe(const ruled::v1::RuledCommand &command, AbilityCommandProbe probe)
{
    switch (probe) {
        case AbilityCommandProbe::Activate:
            return command.has_activate_ability();
        case AbilityCommandProbe::Begin:
            return command.has_begin_ability_activation();
        case AbilityCommandProbe::Preview:
            return command.has_preview_payment();
        case AbilityCommandProbe::Commit:
            return command.has_commit_ability_activation();
    }
    return false;
}

// ---------------------------------------------------------------------------------------
// Identity maps
// ---------------------------------------------------------------------------------------

TEST_F(RuledClientTest, ConvokePreviewPreservesLegalActionsAndRejectsObsoleteReplies)
{
    state->handActions[ruled::v1::HAND_ACTION_CAST_SPELL].handIndices.insert(4);
    auto &payment = state->payment;
    payment.begin();
    const auto old = payment.request({});
    ASSERT_TRUE(payment.payMana('U'));
    const auto current = payment.request({});
    ruled::v1::RuledEventBatch batch;
    auto *reply = batch.mutable_payment_preview();
    reply->set_transaction_id(old.transaction_id());
    reply->set_revision(old.revision());
    reply->set_valid(true);
    reply->set_complete(true);
    QSignalSpy spy(state, &RuledClientState::paymentPreviewReceived);
    EXPECT_TRUE(dispatcher->processPayload(batch.SerializeAsString()));
    EXPECT_TRUE(payment.pending);
    EXPECT_EQ(spy.count(), 0);
    reply->set_revision(current.revision());
    *reply->mutable_selection() = payment.selection;
    EXPECT_TRUE(dispatcher->processPayload(batch.SerializeAsString()));
    EXPECT_FALSE(payment.pending);
    EXPECT_EQ(spy.count(), 1);
    EXPECT_TRUE(state->isHandActionLegal(ruled::v1::HAND_ACTION_CAST_SPELL, 4));
    EXPECT_TRUE(dispatcher->processPayload(batch.SerializeAsString()));
    EXPECT_EQ(spy.count(), 1);
    payment.clear();
    EXPECT_TRUE(dispatcher->processPayload(batch.SerializeAsString()));
    EXPECT_FALSE(payment.active);
    EXPECT_EQ(spy.count(), 1);
}

TEST_F(RuledClientTest, HandActivationUsesAuthoritativePreviewAndRetiresSanitizedMana)
{
    ruled::v1::RuledEventBatch legal;
    auto *offer = (*legal.mutable_legal_by_player())[kLocalPlayer].add_zone_ability_actions();
    offer->set_source_zone(ruled::v1::ABILITY_SOURCE_ZONE_HAND);
    offer->set_object_id(7101);
    offer->set_zone_change_generation(4);
    offer->set_hand_index(3);
    offer->set_ability_index(1);
    offer->set_card_name("Boseiju, Who Endures");
    offer->mutable_ability()->set_ability_index(1);
    offer->mutable_ability()->set_mana_cost("{G}"); // Base offer with two legends; no chosen tax yet.
    offer->mutable_ability()->set_activatable(true);
    apply(legal);
    ASSERT_TRUE(state->activatedAbilityForOid(7101, 1));
    EXPECT_EQ(state->activatedAbilityForOid(7101, 1)->manaCost, QStringLiteral("{G}"));

    PendingActivatedAbility pending;
    pending.permanentOid = 7101;
    pending.sourceZone = state->abilitySourceZone(7101);
    pending.expectedZoneChangeGeneration = state->abilitySourceGeneration(7101);
    pending.abilityIndex = 1;
    ruled::v1::RuledCommand action;
    pending.writeActivationHeader(*action.mutable_activate_ability());
    auto *target = action.mutable_activate_ability()->add_targets();
    target->set_kind(ruled::v1::TARGET_REF_KIND_PERMANENT);
    target->set_group_index(0);
    target->set_object_id(203);

    auto &payment = state->payment;
    payment.begin(true); // Production activation path guards sanitized payment.
    auto request = payment.requestAction(action);
    EXPECT_EQ(request.activate_ability().source_zone(), ruled::v1::ABILITY_SOURCE_ZONE_HAND);
    EXPECT_EQ(request.activate_ability().expected_zone_change_generation(), 4u);
    ruled::v1::RuledEventBatch replyBatch; // No legal_by_player replacement in a preview.
    auto *reply = replyBatch.mutable_payment_preview();
    reply->set_transaction_id(request.transaction_id());
    reply->set_revision(request.revision());
    reply->set_valid(true);
    reply->set_total_cost("{1}{G}"); // (1 + Kopala's 2) - two legends.
    reply->set_remaining_cost("{1}{G}");
    QSignalSpy received(state, &RuledClientState::paymentPreviewReceived);
    apply(replyBatch);
    EXPECT_EQ(received.count(), 1);
    EXPECT_EQ(payment.view.total_cost(), "{1}{G}");
    EXPECT_EQ(payment.view.remaining_cost(), "{1}{G}");
    EXPECT_FALSE(payment.beginSubmission());
    EXPECT_EQ(state->zoneAbilityOidForHandSlot(3), 7101u);

    ASSERT_TRUE(payment.payMana('G', 0, 17));
    ASSERT_TRUE(payment.payMana('C', 0, 18));
    const auto obsolete = payment.requestAction(action); // Full payment is awaiting its quote.
    payment.invalidate(); // Same production operation on an authoritative gameplay refresh.
    const auto current = payment.requestAction(action);
    reply->set_revision(obsolete.revision());
    reply->set_complete(true);
    *reply->mutable_selection() = obsolete.activate_ability().payment();
    apply(replyBatch);
    EXPECT_TRUE(payment.pending);
    EXPECT_EQ(received.count(), 1);

    reply->set_revision(current.revision());
    reply->set_total_cost("{G}"); // A third legend now consumes the whole generic tax.
    reply->set_remaining_cost("");
    reply->set_selection_changed(true);
    reply->mutable_selection()->mutable_mana()->set_c(0);
    apply(replyBatch);
    EXPECT_EQ(received.count(), 2);
    EXPECT_EQ(payment.view.total_cost(), "{G}");
    EXPECT_EQ(payment.selection.mana().g(), 1u);
    EXPECT_EQ(payment.selection.mana().c(), 0u);
    EXPECT_EQ(payment.takeRetiredOptimisticManaCounterIds(), QVector<int>({18}));
    EXPECT_EQ(payment.optimisticManaCounterSpendCount(17), 1);
    EXPECT_FALSE(payment.beginSubmission()); // Sanitation requires renewed player intent.

    payment.clear(); // Existing cancellation model; UI restores these retired counter IDs.
    EXPECT_EQ(payment.takeRetiredOptimisticManaCounterIds(), QVector<int>({17}));
    apply(replyBatch);
    EXPECT_EQ(received.count(), 2);
    EXPECT_FALSE(payment.active);
    EXPECT_TRUE(host.sentCommands.isEmpty());
}

TEST(RuledZoneSnapshotPolicyTest, OpenPermissionMirrorDoesNotDuplicatePublicExileOnFullSnapshots)
{
    EXPECT_TRUE(ruledSnapshotPreservesEventAuthoritativeZone(QString::fromLatin1(ZoneNames::STACK)));
    EXPECT_TRUE(ruledSnapshotPreservesEventAuthoritativeZone(QString::fromLatin1(ZoneNames::GRAVE)));
    EXPECT_TRUE(ruledSnapshotPreservesEventAuthoritativeZone(QString::fromLatin1(ZoneNames::EXILE)));
    EXPECT_TRUE(ruledSnapshotPreservesEventAuthoritativeZone(QString::fromLatin1(ZoneNames::COMMAND)));
    EXPECT_FALSE(ruledSnapshotPreservesEventAuthoritativeZone(QString::fromLatin1(ZoneNames::HAND)));
}

TEST(RuledZoneViewPolicyTest, OpponentCommandZoneIsPublicOnlyInRuledGames)
{
    EXPECT_TRUE(ruledPlayerNeedsCustomZoneMenu(false, true));
    EXPECT_FALSE(ruledPlayerNeedsCustomZoneMenu(false, false));
    EXPECT_TRUE(ruledPlayerNeedsCustomZoneMenu(true, true));
    EXPECT_TRUE(ruledPlayerNeedsCustomZoneMenu(true, false));
    const auto command = QString::fromLatin1(ZoneNames::COMMAND);
    EXPECT_TRUE(ruledCustomZoneViewAllowed(false, true, command));
    EXPECT_FALSE(ruledCustomZoneViewAllowed(false, false, command));
    EXPECT_TRUE(ruledCustomZoneViewAllowed(true, false, command));
    for (const auto &zone : {QString::fromLatin1(ZoneNames::HAND), QString::fromLatin1(ZoneNames::DECK),
                             QStringLiteral("private_custom_zone")}) {
        EXPECT_FALSE(ruledCustomZoneViewAllowed(false, true, zone));
        EXPECT_FALSE(ruledCustomZoneViewAllowed(false, false, zone));
        EXPECT_TRUE(ruledCustomZoneViewAllowed(true, false, zone));
    }
}

TEST(RuledZoneViewPolicyTest, EmptyReadOnlyRevealUsesItsAuthoritativeEmptyCardList)
{
    EXPECT_TRUE(ruledZoneViewHasAuthoritativeCardList(true, false));
    EXPECT_FALSE(ruledZoneViewHasAuthoritativeCardList(true, true));
    EXPECT_FALSE(ruledZoneViewHasAuthoritativeCardList(false, false));
}

TEST(RuledZoneViewPolicyTest, LibrarySearchPickerKeepsPreferredPositionWhenClear)
{
    const QRectF stackSceneRect(520.0, 120.0, 300.0, 420.0);
    const QSizeF pickerSize(340.0, 280.0);
    const QPointF preferredPosition(40.0, 40.0);
    const QRectF viewportSceneRect(0.0, 0.0, 1280.0, 900.0);

    const QPointF position = ruledLibrarySearchPickerPosition(
        stackSceneRect, pickerSize, preferredPosition, viewportSceneRect);

    EXPECT_EQ(position, preferredPosition);
    EXPECT_FALSE(QRectF(position, pickerSize).intersects(stackSceneRect));
    EXPECT_TRUE(viewportSceneRect.contains(QRectF(position, pickerSize)));
}

TEST(RuledZoneViewPolicyTest, LibrarySearchPickerMovesAwayFromMovedAndResizedStack)
{
    // This is the stack's current sceneBoundingRect, not its default position: the stack can be
    // moved and resized while a resolution-time library picker is being prepared.
    const QRectF stackSceneRect(280.0, 160.0, 440.0, 500.0);
    const QSizeF pickerSize(360.0, 320.0);
    const QPointF preferredPosition(340.0, 80.0);
    const QRectF viewportSceneRect(0.0, 0.0, 1280.0, 900.0);
    ASSERT_TRUE(QRectF(preferredPosition, pickerSize).intersects(stackSceneRect));

    const QPointF position = ruledLibrarySearchPickerPosition(
        stackSceneRect, pickerSize, preferredPosition, viewportSceneRect);
    const QRectF pickerSceneRect(position, pickerSize);

    EXPECT_NE(position, preferredPosition);
    EXPECT_FALSE(pickerSceneRect.intersects(stackSceneRect));
    EXPECT_TRUE(viewportSceneRect.contains(pickerSceneRect));
}

TEST(RuledZoneViewPolicyTest, LibrarySearchPickerRespondsToChangedStackGeometry)
{
    const QSizeF pickerSize(360.0, 320.0);
    const QPointF preferredPosition(340.0, 80.0);
    const QRectF viewportSceneRect(0.0, 0.0, 1280.0, 900.0);
    const QRectF firstStackSceneRect(280.0, 160.0, 440.0, 500.0);
    const QRectF movedAndResizedStackSceneRect(600.0, 300.0, 300.0, 200.0);

    const QPointF firstPosition = ruledLibrarySearchPickerPosition(
        firstStackSceneRect, pickerSize, preferredPosition, viewportSceneRect);
    const QPointF secondPosition = ruledLibrarySearchPickerPosition(
        movedAndResizedStackSceneRect, pickerSize, preferredPosition, viewportSceneRect);

    EXPECT_NE(firstPosition, secondPosition);
    EXPECT_FALSE(QRectF(firstPosition, pickerSize).intersects(firstStackSceneRect));
    EXPECT_FALSE(QRectF(secondPosition, pickerSize).intersects(movedAndResizedStackSceneRect));
    EXPECT_TRUE(viewportSceneRect.contains(QRectF(firstPosition, pickerSize)));
    EXPECT_TRUE(viewportSceneRect.contains(QRectF(secondPosition, pickerSize)));
}

TEST(RuledZoneViewPolicyTest, LibrarySearchPickerMovesFromItsDefaultPositionWhenStackIsThere)
{
    const QRectF stackSceneRect(340.0, 80.0, 480.0, 360.0);
    const QSizeF pickerSize(360.0, 320.0);
    const QPointF preferredPosition(340.0, 80.0);
    const QRectF viewportSceneRect(0.0, 0.0, 1280.0, 900.0);
    ASSERT_TRUE(QRectF(preferredPosition, pickerSize).intersects(stackSceneRect));

    const QPointF position = ruledLibrarySearchPickerPosition(
        stackSceneRect, pickerSize, preferredPosition, viewportSceneRect);
    const QRectF pickerSceneRect(position, pickerSize);

    EXPECT_NE(position, preferredPosition);
    EXPECT_FALSE(pickerSceneRect.intersects(stackSceneRect));
    EXPECT_TRUE(viewportSceneRect.contains(pickerSceneRect));
}

TEST(RuledZoneViewPolicyTest, AbsentStackClampsOffscreenPreferenceAndEmptyViewportKeepsDefault)
{
    const QSizeF pickerSize(340.0, 280.0);
    const QPointF offscreenPosition(1100.0, 800.0);
    const QRectF viewportSceneRect(0.0, 0.0, 1280.0, 900.0);
    const QPointF position = ruledLibrarySearchPickerPosition(
        QRectF(), pickerSize, offscreenPosition, viewportSceneRect);

    EXPECT_NE(position, offscreenPosition);
    EXPECT_TRUE(viewportSceneRect.contains(QRectF(position, pickerSize)));
    EXPECT_EQ(position, QPointF(940.0, 620.0));
    EXPECT_EQ(ruledLibrarySearchPickerPosition(QRectF(), pickerSize, offscreenPosition, QRectF()),
              offscreenPosition);
}

TEST(RuledZoneViewPolicyTest, ConstrainedPickerPrefersReachableNonoverlapWithMostVisibleArea)
{
    const QRectF stackSceneRect(100.0, 100.0, 350.0, 250.0);
    const QSizeF pickerSize(300.0, 300.0);
    const QPointF preferredPosition(200.0, 60.0);
    const QRectF viewportSceneRect(0.0, 0.0, 500.0, 400.0);

    const QPointF position = ruledLibrarySearchPickerPosition(
        stackSceneRect, pickerSize, preferredPosition, viewportSceneRect);
    const QRectF pickerSceneRect(position, pickerSize);

    EXPECT_FALSE(viewportSceneRect.contains(pickerSceneRect));
    EXPECT_TRUE(viewportSceneRect.contains(position));
    EXPECT_FALSE(pickerSceneRect.intersects(stackSceneRect));
    EXPECT_DOUBLE_EQ(pickerSceneRect.intersected(viewportSceneRect).width() *
                         pickerSceneRect.intersected(viewportSceneRect).height(),
                     15000.0);
}

TEST(RuledZoneViewPolicyTest, NearEdgeSliverUsesClampedPlacementWithUsableTitleRegion)
{
    const QRectF stackSceneRect(0.0, 0.0, 499.9, 400.0);
    const QSizeF pickerSize(300.0, 300.0);
    const QPointF preferredPosition(340.0, 80.0);
    const QRectF viewportSceneRect(0.0, 0.0, 500.0, 400.0);

    const QPointF position = ruledLibrarySearchPickerPosition(
        stackSceneRect, pickerSize, preferredPosition, viewportSceneRect);
    const QRectF pickerSceneRect(position, pickerSize);
    // Match ZoneViewWidget's 100-unit minimum visible width and 24-unit title-bar height: a
    // reachable top-left point alone must not treat a 0.1-unit title sliver as operable.
    const QRectF visibleTitleBarRect =
        QRectF(position, QSizeF(pickerSize.width(), 24.0)).intersected(viewportSceneRect);

    EXPECT_EQ(position, QPointF(200.0, 80.0));
    EXPECT_TRUE(viewportSceneRect.contains(pickerSceneRect));
    EXPECT_GE(visibleTitleBarRect.width(), qMin(100.0, pickerSize.width()));
    EXPECT_GE(visibleTitleBarRect.height(), qMin(24.0, pickerSize.height()));
}

TEST(RuledZoneViewPolicyTest, StackCoveringViewportKeepsConstrainedPickerOperable)
{
    const QRectF stackSceneRect(-100.0, -100.0, 700.0, 600.0);
    const QSizeF pickerSize(360.0, 300.0);
    const QPointF preferredPosition(340.0, 80.0);
    const QRectF viewportSceneRect(0.0, 0.0, 500.0, 400.0);

    const QPointF position = ruledLibrarySearchPickerPosition(
        stackSceneRect, pickerSize, preferredPosition, viewportSceneRect);
    const QRectF pickerSceneRect(position, pickerSize);

    EXPECT_TRUE(viewportSceneRect.contains(pickerSceneRect));
    EXPECT_TRUE(viewportSceneRect.contains(position));
    EXPECT_TRUE(pickerSceneRect.intersects(stackSceneRect));
}

TEST(RuledZoneViewPolicyTest, OversizePickerFallbackMaximizesVisibleAreaAndKeepsTitleReachable)
{
    const QRectF stackSceneRect(-100.0, -100.0, 700.0, 600.0);
    const QSizeF pickerSize(700.0, 600.0);
    const QPointF preferredPosition(340.0, 80.0);
    const QRectF viewportSceneRect(0.0, 0.0, 500.0, 400.0);

    const QPointF position = ruledLibrarySearchPickerPosition(
        stackSceneRect, pickerSize, preferredPosition, viewportSceneRect);
    const QRectF pickerSceneRect(position, pickerSize);

    EXPECT_EQ(position, viewportSceneRect.topLeft());
    EXPECT_TRUE(viewportSceneRect.contains(position));
    EXPECT_EQ(pickerSceneRect.intersected(viewportSceneRect), viewportSceneRect);
    EXPECT_TRUE(pickerSceneRect.intersects(stackSceneRect));
}

TEST_F(RuledClientTest, ConvokeSelectionUsesPublishedColorAndGeneration)
{
    auto &payment = state->payment;
    payment.begin();
    auto request = payment.request({});
    ruled::v1::PaymentPreview response;
    response.set_transaction_id(request.transaction_id());
    response.set_revision(request.revision());
    response.set_valid(true);
    auto *candidate = response.add_candidates();
    candidate->mutable_object()->set_object_id(44);
    candidate->mutable_object()->set_zone_change_generation(7);
    candidate->add_options(ruled::v1::OBJECT_PAYMENT_KIND_BLUE);
    candidate->add_options(ruled::v1::OBJECT_PAYMENT_KIND_GENERIC);
    ASSERT_TRUE(payment.apply(response));
    EXPECT_FALSE(payment.select(44, ruled::v1::OBJECT_PAYMENT_KIND_RED));
    EXPECT_TRUE(payment.select(44, ruled::v1::OBJECT_PAYMENT_KIND_BLUE));
    EXPECT_FALSE(payment.select(44, ruled::v1::OBJECT_PAYMENT_KIND_GENERIC));
    EXPECT_EQ(payment.selection.convoke(0).object().zone_change_generation(), 7u);
    request = payment.request({});
    response.set_revision(request.revision());
    *response.mutable_selection() = payment.selection;
    ASSERT_TRUE(payment.apply(response));
    EXPECT_TRUE(payment.remove(44));
    EXPECT_FALSE(payment.selected(44));
}

TEST_F(RuledClientTest, WaterbendUsesSharedPaymentSelectionWithoutConvokeColorChoices)
{
    auto &payment = state->payment;
    payment.begin();
    const auto request = payment.request({});
    ruled::v1::PaymentPreview response;
    response.set_transaction_id(request.transaction_id());
    response.set_revision(request.revision());
    response.set_valid(true);
    auto *candidate = response.add_candidates();
    candidate->mutable_object()->set_object_id(44);
    candidate->mutable_object()->set_zone_change_generation(7);
    candidate->add_options(ruled::v1::OBJECT_PAYMENT_KIND_WATERBEND);
    ASSERT_TRUE(payment.apply(response));
    EXPECT_FALSE(payment.select(44, ruled::v1::OBJECT_PAYMENT_KIND_BLUE));
    ASSERT_TRUE(payment.select(44, ruled::v1::OBJECT_PAYMENT_KIND_WATERBEND));
    ASSERT_EQ(payment.selection.waterbend_size(), 1);
    EXPECT_EQ(payment.selection.waterbend(0).zone_change_generation(), 7u);
    EXPECT_EQ(payment.selection.convoke_size(), 0);
}

TEST_F(RuledClientTest, SharedPaymentTracksOptimisticPoolDebitsAndRetiresSanitizedMana)
{
    auto &payment = state->payment;
    payment.begin();
    auto request = payment.request({});
    ruled::v1::PaymentPreview response;
    response.set_transaction_id(request.transaction_id());
    response.set_revision(request.revision());
    response.set_valid(true);
    ASSERT_TRUE(payment.apply(response));

    ASSERT_TRUE(payment.payMana('G', 0, 17));
    EXPECT_EQ(payment.optimisticManaCounterSpendCount(17), 1);
    RuledManaPoolTracker tracker;
    EXPECT_EQ(tracker.observe(17, 0, 3, 0).displayedBeforeNewStaging, 3);
    EXPECT_EQ(tracker.observe(17, 2, 3, payment.optimisticManaCounterSpendCount(17)).displayedBeforeNewStaging, 2);
    request = payment.request({});
    response.set_revision(request.revision());
    *response.mutable_selection() = request.cast_spell().payment();
    ASSERT_TRUE(payment.apply(response));
    EXPECT_TRUE(payment.takeRetiredOptimisticManaCounterIds().isEmpty());

    // The engine can sanitize a stale or excessive staged pip. The UI must restore precisely
    // that optimistic display debit instead of leaving the pool counter visually reduced.
    request = payment.request({});
    response.set_revision(request.revision());
    response.mutable_selection()->Clear();
    response.set_selection_changed(true);
    ASSERT_TRUE(payment.apply(response));
    EXPECT_EQ(payment.optimisticManaCounterSpendCount(17), 0);
    EXPECT_EQ(payment.takeRetiredOptimisticManaCounterIds(), QVector<int>({17}));

    ASSERT_TRUE(payment.payMana('G', 0, 17));
    payment.clear();
    EXPECT_EQ(payment.takeRetiredOptimisticManaCounterIds(), QVector<int>({17}));
}

TEST_F(RuledClientTest, SanitizedWardUndoDoesNotRestoreManaRemovedByTheEngine)
{
    auto &payment = state->payment;
    payment.begin(true);
    RuledManaPoolTracker tracker;
    EXPECT_EQ(tracker.observe(17, 0, 1, 0).displayedBeforeNewStaging, 1);
    ASSERT_TRUE(payment.payMana('W', 0, 17));
    auto request = payment.requestAction({});
    ruled::v1::PaymentPreview preview;
    preview.set_transaction_id(request.transaction_id());
    preview.set_revision(request.revision());
    preview.set_valid(true);
    *preview.mutable_selection() = payment.selection;
    ASSERT_TRUE(payment.apply(preview));

    // Undo's absolute pool event arrives before its fresh preview removes the stale pip.
    int displayed = tracker.observe(17, 0, 0, payment.optimisticManaCounterSpendCount(17)).displayedBeforeNewStaging;
    request = payment.requestAction({});
    preview.set_revision(request.revision());
    preview.mutable_selection()->Clear();
    preview.set_selection_changed(true);
    ASSERT_TRUE(payment.apply(preview));
    for (const int id : payment.takeRetiredOptimisticManaCounterIds())
        displayed = tracker.restoreOptimisticDebit(id, displayed, payment.optimisticManaCounterSpendCount(id));
    EXPECT_EQ(displayed, 0) << "retiring a staged debit must not recreate undone engine mana";
    EXPECT_FALSE(payment.beginSubmission());
    EXPECT_TRUE(payment.active);
    EXPECT_EQ(tracker.observe(17, displayed, 0, 0).newlyProduced, 0);
}

TEST_F(RuledClientTest, WardUndoRetainsOldStagedManaAndCancellationRefundsOnlyThatMana)
{
    auto &payment = state->payment;
    payment.begin(true);
    RuledManaPoolTracker tracker;
    tracker.observe(17, 0, 1, 0);
    ASSERT_TRUE(payment.payMana('W', 0, 17));
    EXPECT_EQ(tracker.observe(17, 0, 2, 1).newlyProduced, 1);
    ASSERT_TRUE(payment.payMana('W', 0, 17));
    int displayed = tracker.observe(17, 0, 1, 2).displayedBeforeNewStaging;
    const auto request = payment.requestAction({});
    ruled::v1::PaymentPreview preview;
    preview.set_transaction_id(request.transaction_id());
    preview.set_revision(request.revision());
    preview.set_valid(true);
    preview.mutable_selection()->mutable_mana()->set_w(1);
    preview.set_selection_changed(true);
    ASSERT_TRUE(payment.apply(preview));
    EXPECT_EQ(payment.optimisticManaCounterSpendCount(17), 1);
    for (const int id : payment.takeRetiredOptimisticManaCounterIds())
        displayed = tracker.restoreOptimisticDebit(id, displayed, payment.optimisticManaCounterSpendCount(id));
    EXPECT_EQ(displayed, 0);
    payment.clear();
    for (const int id : payment.takeRetiredOptimisticManaCounterIds())
        displayed = tracker.restoreOptimisticDebit(id, displayed, payment.optimisticManaCounterSpendCount(id));
    EXPECT_EQ(displayed, 1) << "cancellation restores the old pip, never the undone pip";
}

TEST_F(RuledClientTest, SubmissionRefundsQueuedExcessWithoutRefundingSelectedMana)
{
    auto &payment = state->payment;
    payment.begin(true);
    RuledManaPoolTracker tracker;
    tracker.observe(17, 0, 3, 0);
    ASSERT_TRUE(payment.payMana('W', 0, 17));
    ASSERT_TRUE(payment.payMana('W', 0, 17));
    const auto request = payment.requestAction({});
    ASSERT_TRUE(payment.stageMana('W', 0, 17)); // Produced after the pending preview's selection.
    ruled::v1::PaymentPreview preview;
    preview.set_transaction_id(request.transaction_id());
    preview.set_revision(request.revision());
    preview.set_valid(true);
    preview.set_complete(true);
    *preview.mutable_selection() = payment.selection;
    ASSERT_TRUE(payment.apply(preview));
    ASSERT_TRUE(payment.beginSubmission());
    int displayed = 0;
    for (const int id : payment.takeRetiredOptimisticManaCounterIds())
        displayed = tracker.restoreOptimisticDebit(id, displayed, payment.optimisticManaCounterSpendCount(id));
    EXPECT_EQ(displayed, 1) << "two selected pips remain hidden until the engine consumes them";
    EXPECT_EQ(tracker.restoreOptimisticDebit(18, 0, 0), 1) << "unknown snapshots retain local credit behavior";
}

TEST_F(RuledClientTest, SuspendingPaymentDoesNotRefundTheOuterSpellIntoTheManaPool)
{
    auto &payment = state->payment;
    payment.begin();
    ASSERT_TRUE(payment.payMana('U', 0, 17));
    auto spell = payment.suspend();
    payment.begin(true);
    EXPECT_TRUE(payment.takeRetiredOptimisticManaCounterIds().isEmpty())
        << "opening Gloomlake Verge's mana ability must not refund the suspended spell's pip";
    EXPECT_EQ(spell.optimisticManaCounterSpendCount(17), 1);
    payment.clear();
    payment = std::move(spell);
    EXPECT_EQ(payment.takeAllOptimisticManaCounterIds(), QVector<int>({17}));
}

TEST_F(RuledClientTest, XCounterManaPromptSubmitsTheChosenSplitAndCancellationRestoresOuterPayment)
{
    QVector<QPair<RuledXCounterManaPromptStep, quint32>> prompts;
    const auto selected = ruledPromptXCounterManaSplit(
        4, [&](RuledXCounterManaPromptStep step, quint32 maximum) -> std::optional<quint32> {
            prompts.append({step, maximum});
            return prompts.size() == 1 ? 3u : 2u;
        });
    ASSERT_TRUE(selected);
    EXPECT_EQ(selected->xValue, 3u);
    EXPECT_EQ(selected->firstColorCount, 2u);
    const QVector<QPair<RuledXCounterManaPromptStep, quint32>> expectedPrompts = {
        {RuledXCounterManaPromptStep::ChooseX, 4},
        {RuledXCounterManaPromptStep::ChooseFirstColorCount, 3},
    };
    EXPECT_EQ(prompts, expectedPrompts);

    PendingActivatedAbility pending;
    pending.permanentOid = 501;
    pending.abilityIndex = 2;
    pending.xValue = selected->xValue;
    pending.manaSplitFirstColorCount = selected->firstColorCount;
    ruled::v1::ActivateAbility command;
    pending.writeActivationHeader(command);
    EXPECT_EQ(command.x_value(), 3u);
    EXPECT_EQ(command.mana_split_first_color_count(), 2u);

    QVector<RuledXCounterManaPromptStep> canceledPrompts;
    const auto canceled =
        ruledPromptXCounterManaSplit(4, [&](RuledXCounterManaPromptStep step, quint32) -> std::optional<quint32> {
            canceledPrompts.append(step);
            if (step == RuledXCounterManaPromptStep::ChooseX) {
                return 3u;
            }
            return std::nullopt;
        });
    EXPECT_FALSE(canceled);
    const QVector<RuledXCounterManaPromptStep> expectedCanceledPrompts = {
        RuledXCounterManaPromptStep::ChooseX,
        RuledXCounterManaPromptStep::ChooseFirstColorCount,
    };
    EXPECT_EQ(canceledPrompts, expectedCanceledPrompts);

    auto &payment = state->payment;
    payment.begin();
    ASSERT_TRUE(payment.payMana('U', 0, 17));
    auto outer = payment.suspend();
    payment.begin(true);
    EXPECT_TRUE(payment.active);
    payment.clear();            // The canceled activation never spends its nested payment.
    payment = std::move(outer); // Same restoration path used by cancelPendingActivatedAbility.
    EXPECT_TRUE(payment.active);
    EXPECT_EQ(payment.selection.mana().u(), 1u);
}

TEST_F(RuledClientTest, LoyaltyXPromptUsesInclusiveRangeAndExistingActivationWire)
{
    const auto checkSelection = [](RuledLoyaltyXChoice range, quint32 expectedX) {
        PendingActivatedAbility pending;
        pending.permanentOid = 330;
        pending.sourceZone = ruled::v1::ABILITY_SOURCE_ZONE_BATTLEFIELD;
        pending.expectedZoneChangeGeneration = 17;
        pending.abilityIndex = 4;
        pending.xValue = 99;
        quint32 offeredMin = std::numeric_limits<quint32>::max();
        quint32 offeredMax = std::numeric_limits<quint32>::max();

        ASSERT_TRUE(ruledApplyLoyaltyXChoice(pending, range, [&](quint32 minX, quint32 maxX) {
            offeredMin = minX;
            offeredMax = maxX;
            return std::optional<quint32>(expectedX);
        }));
        EXPECT_EQ(offeredMin, range.minX);
        EXPECT_EQ(offeredMax, range.maxX);
        EXPECT_EQ(pending.xValue, expectedX);
        ASSERT_TRUE(pending.loyaltyXChoice);
        EXPECT_EQ(*pending.loyaltyXChoice, range);

        ruled::v1::ActivateAbility command;
        pending.writeActivationHeader(command);
        EXPECT_EQ(command.source_object_id(), 330u);
        EXPECT_EQ(command.source_zone(), ruled::v1::ABILITY_SOURCE_ZONE_BATTLEFIELD);
        EXPECT_EQ(command.expected_zone_change_generation(), 17u);
        EXPECT_EQ(command.ability_index(), 4u);
        EXPECT_EQ(command.x_value(), expectedX);
        EXPECT_EQ(command.mana_split_first_color_count(), 0u);
    };

    checkSelection({0, 0}, 0);
    checkSelection({2, 7}, 2);
    checkSelection({2, 7}, 7);
}

TEST_F(RuledClientTest, LoyaltyXPromptCancellationAndMalformedChoicesFailClosed)
{
    PendingActivatedAbility pending;
    pending.xValue = 5;
    int promptCount = 0;
    EXPECT_FALSE(ruledApplyLoyaltyXChoice(pending, {2, 7}, [&](quint32 minX, quint32 maxX) {
        ++promptCount;
        EXPECT_EQ(minX, 2u);
        EXPECT_EQ(maxX, 7u);
        return std::optional<quint32>{};
    }));
    EXPECT_EQ(promptCount, 1);
    EXPECT_EQ(pending.xValue, 5u);
    EXPECT_FALSE(pending.loyaltyXChoice);

    EXPECT_FALSE(ruledApplyLoyaltyXChoice(pending, {2, 7}, [](quint32, quint32) {
        return std::optional<quint32>(8u);
    }));
    EXPECT_EQ(pending.xValue, 5u);
    EXPECT_FALSE(pending.loyaltyXChoice);

    bool invalidRangePrompted = false;
    EXPECT_FALSE(ruledApplyLoyaltyXChoice(pending, {7, 2}, [&](quint32, quint32) {
        invalidRangePrompted = true;
        return std::optional<quint32>(7u);
    }));
    EXPECT_FALSE(invalidRangePrompted);
    EXPECT_FALSE(ruledApplyLoyaltyXChoice(pending, {0, 0x80000000u}, [&](quint32, quint32) {
        invalidRangePrompted = true;
        return std::optional<quint32>(0u);
    }));
    EXPECT_FALSE(invalidRangePrompted);
    EXPECT_EQ(pending.xValue, 5u);
}

TEST_F(RuledClientTest, PendingLoyaltyXOfferMustMatchCurrentSourceSlotAndRange)
{
    constexpr quint32 sourceOid = 0x731u;
    constexpr quint64 sourceGeneration = 19;
    const RuledLoyaltyXChoice range{0, 7};
    RuledAbilityEntry currentOffer;
    currentOffer.text = QStringLiteral("Sorin -X");
    currentOffer.activatable = true;
    currentOffer.loyaltyXChoice = range;
    RuledAbilityEntries abilities;
    abilities.append(currentOffer);
    state->activatedAbilitiesByOid.insert(sourceOid, abilities);
    state->battlefieldGenerationByOid.insert(sourceOid, sourceGeneration);

    PendingActivatedAbility pending;
    pending.valid = true;
    pending.permanentOid = sourceOid;
    pending.sourceZone = ruled::v1::ABILITY_SOURCE_ZONE_BATTLEFIELD;
    pending.expectedZoneChangeGeneration = sourceGeneration;
    pending.abilityIndex = 0;
    pending.loyaltyXChoice = range;
    EXPECT_TRUE(ruledPendingLoyaltyXOfferStillCurrent(*state, pending));

    state->battlefieldGenerationByOid.insert(sourceOid, sourceGeneration + 1);
    EXPECT_FALSE(ruledPendingLoyaltyXOfferStillCurrent(*state, pending));
    state->battlefieldGenerationByOid.insert(sourceOid, sourceGeneration);
    state->zoneAbilitySourceByOid.insert(sourceOid, ruled::v1::ABILITY_SOURCE_ZONE_HAND);
    EXPECT_FALSE(ruledPendingLoyaltyXOfferStillCurrent(*state, pending));
    state->zoneAbilitySourceByOid.remove(sourceOid);
    pending.abilityIndex = 1;
    EXPECT_FALSE(ruledPendingLoyaltyXOfferStillCurrent(*state, pending));
    pending.abilityIndex = 0;
    state->activatedAbilitiesByOid[sourceOid][0]->loyaltyXChoice = RuledLoyaltyXChoice{0, 6};
    EXPECT_FALSE(ruledPendingLoyaltyXOfferStillCurrent(*state, pending));
    state->activatedAbilitiesByOid[sourceOid][0]->loyaltyXChoice = range;
    state->activatedAbilitiesByOid[sourceOid][0]->activatable = false;
    EXPECT_FALSE(ruledPendingLoyaltyXOfferStillCurrent(*state, pending));
}

TEST(RuledPendingCastTest, XCounterManaPromptAllowsZeroWithoutAskingForASplit)
{
    int promptCount = 0;
    const auto selected = ruledPromptXCounterManaSplit(
        0, [&](RuledXCounterManaPromptStep step, quint32 maximum) -> std::optional<quint32> {
            ++promptCount;
            EXPECT_EQ(step, RuledXCounterManaPromptStep::ChooseX);
            EXPECT_EQ(maximum, 0u);
            return 0u;
        });
    ASSERT_TRUE(selected);
    EXPECT_EQ(selected->xValue, 0u);
    EXPECT_EQ(selected->firstColorCount, 0u);
    EXPECT_EQ(promptCount, 1);
}

TEST_F(RuledClientTest, NestedPaymentsDoNotReusePreviewTransactionIds)
{
    auto &payment = state->payment;
    payment.begin();
    auto spell = payment.suspend();
    payment.begin(true);
    const auto firstManaAbility = payment.request({});
    payment = std::move(spell);
    spell = payment.suspend();
    payment.begin(true);
    const auto secondManaAbility = payment.request({});
    EXPECT_NE(firstManaAbility.transaction_id(), secondManaAbility.transaction_id());
    ruled::v1::PaymentPreview stale;
    stale.set_transaction_id(firstManaAbility.transaction_id());
    stale.set_revision(firstManaAbility.revision());
    stale.set_valid(true);
    stale.set_complete(true);
    EXPECT_FALSE(payment.apply(stale));
}

TEST_F(RuledClientTest, ManaProducedByNestedActivationPaysTheSuspendedSpellOrAbility)
{
    for (const bool abilityPayment : {false, true}) {
        RuledPayment payment;
        payment.begin(abilityPayment);
        payment.invalidate();
        ASSERT_TRUE(payment.stageMana('U', 0, 17));
        auto outer = payment.suspend();
        payment.begin(true);
        // While paying the mana ability's own cost, further mana belongs to that inner cost.
        EXPECT_EQ(payment.producedManaRecipient(&outer), &payment);
        payment.submitting = true;
        // Gloomlake Verge / Hidden Lair's pool event arrives before the activation acknowledgement.
        auto *recipient = payment.producedManaRecipient(&outer);
        ASSERT_EQ(recipient, &outer);
        ASSERT_TRUE(recipient->stageMana('B', 0, 18));
        EXPECT_EQ(outer.optimisticManaCounterSpendCount(17), 1);
        EXPECT_EQ(outer.optimisticManaCounterSpendCount(18), 1);
        EXPECT_TRUE(payment.takeAllOptimisticManaCounterIds().isEmpty());
        payment = std::move(outer);
        EXPECT_EQ(payment.queuedMana.size(), 2);
        // Cancel restores the original and newly produced pips once each.
        EXPECT_EQ(payment.takeAllOptimisticManaCounterIds(), QVector<int>({17, 18}));
        EXPECT_TRUE(payment.takeAllOptimisticManaCounterIds().isEmpty());
    }
}

TEST_F(RuledClientTest, CompletingPaymentReleasesQueuedSurplusMana)
{
    auto &payment = state->payment;
    payment.begin();
    ASSERT_TRUE(payment.stageMana('U', 0, 17));
    auto request = payment.request({});
    // A multi-mana activation produces another pip while the final pip's preview is in flight.
    ASSERT_TRUE(payment.stageMana('U', 0, 17));
    ruled::v1::PaymentPreview response;
    response.set_transaction_id(request.transaction_id());
    response.set_revision(request.revision());
    response.set_valid(true);
    response.set_complete(true);
    *response.mutable_selection() = request.cast_spell().payment();
    ASSERT_TRUE(payment.apply(response));
    ASSERT_TRUE(payment.beginSubmission());
    EXPECT_EQ(payment.selection.mana().u(), 1u);
    EXPECT_TRUE(payment.queuedMana.isEmpty());
    EXPECT_EQ(payment.takeRetiredOptimisticManaCounterIds(), QVector<int>({17}))
        << "the extra pip was never submitted, so its display debit must be restored";
    EXPECT_TRUE(payment.takeRetiredOptimisticManaCounterIds().isEmpty());
}

TEST_F(RuledClientTest, VariableContributionDrainsClicksQueuedAcrossDelayedPreviews)
{
    RuledPayment payment;
    payment.begin();
    ASSERT_TRUE(payment.stageMana('U', 0, 11));
    const auto firstRequest = payment.request({});
    ASSERT_TRUE(payment.pending);
    ASSERT_TRUE(payment.stageMana('G', 0, 12));
    ASSERT_TRUE(payment.stageMana('R', 0, 13));
    ASSERT_EQ(payment.queuedMana.size(), 2);

    const auto previewFor = [](const ruled::v1::PreviewPayment &request) {
        ruled::v1::PaymentPreview preview;
        preview.set_transaction_id(request.transaction_id());
        preview.set_revision(request.revision());
        preview.set_valid(true);
        preview.set_complete(true);
        *preview.mutable_selection() = request.cast_spell().payment();
        return preview;
    };
    ASSERT_TRUE(payment.apply(previewFor(firstRequest)));

    EXPECT_EQ(payment.nextPreviewAction(false), RuledPayment::PreviewAction::DrainQueuedMana);
    auto queued = payment.queuedMana.takeFirst();
    ASSERT_TRUE(payment.payMana(queued.symbol, queued.groupId, queued.counterId));
    const auto secondRequest = payment.request({});
    ASSERT_EQ(payment.queuedMana.size(), 1);
    ASSERT_TRUE(payment.apply(previewFor(secondRequest)));

    EXPECT_EQ(payment.nextPreviewAction(false), RuledPayment::PreviewAction::DrainQueuedMana);
    queued = payment.queuedMana.takeFirst();
    ASSERT_TRUE(payment.payMana(queued.symbol, queued.groupId, queued.counterId));
    const auto thirdRequest = payment.request({});
    ASSERT_TRUE(payment.queuedMana.isEmpty());
    ASSERT_TRUE(payment.apply(previewFor(thirdRequest)));

    EXPECT_EQ(payment.nextPreviewAction(false), RuledPayment::PreviewAction::None);
    EXPECT_FALSE(payment.submitting) << "variable contributions require explicit confirmation";
    EXPECT_EQ(payment.selection.mana().u(), 1u);
    EXPECT_EQ(payment.selection.mana().g(), 1u);
    EXPECT_EQ(payment.selection.mana().r(), 1u);
}

TEST_F(RuledClientTest, ProducedManaWithoutAPaymentRecipientRemainsFloating)
{
    RuledPayment payment;
    EXPECT_EQ(payment.producedManaRecipient(nullptr), nullptr);
    payment.begin();
    payment.submitting = true;
    EXPECT_EQ(payment.producedManaRecipient(nullptr), nullptr);
}

TEST_F(RuledClientTest, RestrictedManaDisplayTracksQueuedAndSuspendedSharedPayments)
{
    auto &payment = state->payment;
    payment.begin();
    ASSERT_TRUE(payment.stageMana('R', 9));
    ASSERT_TRUE(payment.stageMana('R', 9));
    EXPECT_EQ(payment.restrictedManaSpendCount(9, 'R'), 2);
    EXPECT_EQ(payment.restrictedManaSpendCount(8, 'R'), 0);
    auto outer = payment.suspend();
    payment.begin(true);
    EXPECT_EQ(payment.restrictedManaSpendCount(9, 'R'), 0);
    EXPECT_EQ(outer.restrictedManaSpendCount(9, 'R'), 2);
    payment = std::move(outer);
    auto request = payment.request({});
    ruled::v1::PaymentPreview response;
    response.set_transaction_id(request.transaction_id());
    response.set_revision(request.revision());
    response.set_valid(true);
    response.set_complete(true);
    *response.mutable_restricted_mana() = request.cast_spell().restricted_mana();
    ASSERT_TRUE(payment.apply(response));
    ASSERT_TRUE(payment.beginSubmission());
    EXPECT_EQ(payment.restrictedManaSpendCount(9, 'R'), 0)
        << "the engine's submitted pool snapshot already includes the selected restricted mana";
    EXPECT_TRUE(payment.takeRetiredOptimisticManaCounterIds().isEmpty())
        << "restricted mana must never refund an ordinary counter";
}

TEST_F(RuledClientTest, SubmittedSharedPaymentStopsDebitingAuthoritativeSnapshots)
{
    auto &payment = state->payment;
    payment.begin();
    auto request = payment.request({});
    ruled::v1::PaymentPreview response;
    response.set_transaction_id(request.transaction_id());
    response.set_revision(request.revision());
    response.set_valid(true);
    ASSERT_TRUE(payment.apply(response));
    ASSERT_TRUE(payment.payMana('R', 0, 17));

    request = payment.request({});
    response.set_revision(request.revision());
    response.set_complete(true);
    *response.mutable_selection() = request.cast_spell().payment();
    ASSERT_TRUE(payment.apply(response));
    ASSERT_TRUE(payment.beginSubmission());

    EXPECT_EQ(payment.optimisticManaCounterSpendCount(17), 0)
        << "the accepted server snapshot already contains the submitted debit";
    EXPECT_EQ(payment.takeAllOptimisticManaCounterIds(), QVector<int>({17}))
        << "the identity remains available until acceptance or rejection is known";
}

TEST_F(RuledClientTest, SharedPaymentCarriesExactPermanentActionMana)
{
    auto &payment = state->payment;
    payment.begin();
    ruled::v1::RuledCommand action;
    auto *permanent = action.mutable_execute_permanent_action();
    permanent->set_kind(ruled::v1::PERMANENT_ACTION_KIND_UNLOCK_ROOM_DOOR);
    permanent->set_object_id(81);
    permanent->set_expected_zone_change_generation(9);

    auto request = payment.requestAction(action);
    ASSERT_TRUE(request.has_execute_permanent_action());
    EXPECT_EQ(request.execute_permanent_action().object_id(), 81u);

    ruled::v1::PaymentPreview response;
    response.set_transaction_id(request.transaction_id());
    response.set_revision(request.revision());
    response.set_valid(true);
    response.mutable_selection()->set_expected_state_revision(22);
    response.mutable_selection()->mutable_source()->set_object_id(81);
    response.mutable_selection()->mutable_source()->set_zone_change_generation(9);
    ASSERT_TRUE(payment.apply(response));
    ASSERT_TRUE(payment.payMana('R'));

    request = payment.requestAction(action);
    ASSERT_TRUE(request.has_execute_permanent_action());
    EXPECT_EQ(request.execute_permanent_action().payment().mana().r(), 1u);
    payment.writePayment(action);
    EXPECT_EQ(action.execute_permanent_action().payment().source().object_id(), 81u);
    EXPECT_EQ(action.execute_permanent_action().payment().mana().r(), 1u);
}

TEST_F(RuledClientTest, ConvokeCompletionSubmitsExactlyOnceAndStateRefreshInvalidatesReplies)
{
    auto &payment = state->payment;
    payment.begin();
    auto request = payment.request({});
    ruled::v1::PaymentPreview response;
    response.set_transaction_id(request.transaction_id());
    response.set_revision(request.revision());
    response.set_valid(true);
    response.set_complete(true);
    payment.invalidate(); // authoritative gameplay batch, before the queued refresh query
    EXPECT_FALSE(payment.apply(response));
    EXPECT_FALSE(payment.beginSubmission());
    request = payment.request({});
    response.set_revision(request.revision());
    response.set_selection_changed(true);
    response.set_error("Payment changed; stale or unavailable selections were removed.");
    ASSERT_TRUE(payment.apply(response));
    EXPECT_FALSE(payment.beginSubmission());
    request = payment.request({});
    response.set_revision(request.revision());
    response.set_selection_changed(false);
    response.clear_error();
    ASSERT_TRUE(payment.apply(response));
    EXPECT_EQ(payment.view.error(), "Payment changed; stale or unavailable selections were removed.");
    EXPECT_TRUE(payment.beginSubmission());
    EXPECT_FALSE(payment.beginSubmission());
    EXPECT_FALSE(payment.apply(response));
    payment.clear();
    EXPECT_FALSE(payment.beginSubmission());
}

TEST_F(RuledClientTest, WaterbendDisarmsSanitizedPaymentsAndSerializesEachContext)
{
    auto &payment = state->payment;
    payment.begin(true);
    ruled::v1::RuledCommand ability;
    ability.mutable_activate_ability()->set_source_object_id(81);
    auto request = payment.requestAction(ability);
    EXPECT_TRUE(request.has_activate_ability());
    EXPECT_FALSE(request.has_cast_spell());
    ruled::v1::PaymentPreview response;
    response.set_transaction_id(request.transaction_id());
    response.set_revision(request.revision());
    response.set_valid(true);
    response.set_complete(false);
    ASSERT_TRUE(payment.apply(response));
    EXPECT_FALSE(payment.beginSubmission());
    ASSERT_TRUE(payment.payMana('U'));
    request = payment.requestAction(ability);
    response.set_revision(request.revision());
    response.set_selection_changed(true);
    response.set_complete(true);
    ASSERT_TRUE(payment.apply(response));
    EXPECT_FALSE(payment.beginSubmission());
    request = payment.requestAction(ability);
    response.set_revision(request.revision());
    response.set_selection_changed(false);
    ASSERT_TRUE(payment.apply(response));
    EXPECT_FALSE(payment.beginSubmission()); // Sanitizing a stale selection disarms submission.
    ASSERT_TRUE(payment.payMana('U'));
    request = payment.requestAction(ability);
    response.set_revision(request.revision());
    *response.mutable_selection() = request.activate_ability().payment();
    ASSERT_TRUE(payment.apply(response));
    ASSERT_TRUE(payment.beginSubmission());
    EXPECT_FALSE(payment.beginSubmission());
    payment.writePayment(ability);
    EXPECT_EQ(ability.activate_ability().payment().mana().u(), 1u);
    ruled::v1::RuledCommand resolution;
    resolution.mutable_submit_resolution_choice()->set_decision(ruled::v1::RESOLUTION_CHOICE_DECISION_PAY_MANA);
    payment.writePayment(resolution);
    EXPECT_EQ(resolution.submit_resolution_choice().payment().SerializeAsString(),
              ability.activate_ability().payment().SerializeAsString());
}

TEST_F(RuledClientTest, ZeroCostWaterbendCompletesWithoutAnImpossibleContribution)
{
    auto &payment = state->payment;
    payment.begin(true); // Opening the activation/payment branch is itself intentional.
    const auto request = payment.request({});
    ruled::v1::PaymentPreview response;
    response.set_transaction_id(request.transaction_id());
    response.set_revision(request.revision());
    response.set_valid(true);
    response.set_complete(true);
    ASSERT_TRUE(payment.apply(response));
    EXPECT_TRUE(payment.beginSubmission());
    EXPECT_FALSE(payment.beginSubmission());
}

TEST_F(RuledClientTest, ResolutionChoiceWithoutAuthoritativePromptSnapshotFailsClosed)
{
    ruled::v1::RuledEventBatch batch;
    auto *choice = batch.add_events()->mutable_resolution_choice_required();
    choice->set_deciding_player_id(kLocalPlayer);
    choice->set_source_object_id(8801);
    choice->set_choice_kind(ruled::v1::CHOICE_KIND_REVEALED);
    choice->set_min(1);
    choice->set_max(1);
    choice->set_resolution_prompt_id(41);
    choice->add_candidate_object_ids(8802);
    choice->add_candidate_card_ids("PROMPT-CANDIDATE");
    choice->add_candidate_server_card_ids(1);
    choice->add_candidate_names("Prompt candidate");

    // An event alone is not authoritative: a missing batch snapshot must not leave a stale or
    // malformed UI action available, even when its private event ID is nonzero.
    applyRaw(batch);

    EXPECT_FALSE(state->pendingChoice.has_value());
    EXPECT_EQ(state->choiceWaitingPlayerId, -1);
    EXPECT_TRUE(host.sentCommands.isEmpty());
}

TEST_F(RuledClientTest, PurePaymentPreviewPreservesResolutionPromptAndStaging)
{
    ruled::v1::RuledEventBatch active;
    auto *choice = active.add_events()->mutable_resolution_choice_required();
    choice->set_deciding_player_id(kLocalPlayer);
    choice->set_source_object_id(8001);
    choice->set_choice_kind(ruled::v1::CHOICE_KIND_PERMANENT_OBJECTS);
    choice->set_min(0);
    choice->set_max(1);
    choice->add_candidate_object_ids(8002);
    apply(active);
    ASSERT_TRUE(state->pendingChoice.has_value());
    ASSERT_NE(state->resolutionPromptId, 0u);
    state->pendingChoice->selectedObjectOids.append(8002);
    const quint64 promptId = state->resolutionPromptId;

    ruled::v1::RuledEventBatch preview;
    preview.mutable_payment_preview()->set_transaction_id(81);
    std::string payload;
    ASSERT_TRUE(preview.SerializeToString(&payload));
    EXPECT_TRUE(dispatcher->processPayload(payload));

    ASSERT_TRUE(state->pendingChoice.has_value());
    EXPECT_EQ(state->resolutionPromptId, promptId);
    EXPECT_EQ(state->pendingChoice->resolutionPromptId, promptId);
    EXPECT_EQ(state->pendingChoice->selectedObjectOids, QVector<quint32>({8002}));
}

TEST_F(RuledClientTest, SuspendedResolutionPaymentResumeMatchesPromptAndIgnoresReadiness)
{
    constexpr quint64 promptId = 8301;
    const std::string semantics = "normalized-answer-semantics";
    const RuledResolutionPaymentIdentity parent{promptId, semantics};
    const auto setOffer = [&](bool paymentCurrentlyLegal) {
        state->resolutionPromptSnapshotValid = true;
        state->resolutionPromptActive = true;
        state->resolutionPromptDeciderPlayerId = kLocalPlayer;
        state->resolutionPromptId = promptId;
        state->resolutionPromptSemantics = semantics;
        RuledClientState::RuledPendingChoice choice;
        choice.kind = RuledClientState::ChoiceKind::ResolutionPayment;
        choice.resolutionPromptId = promptId;
        choice.resolutionPromptSemantics = semantics;
        choice.paymentCurrentlyLegal = paymentCurrentlyLegal;
        state->pendingChoice = std::move(choice);
    };

    state->payment.begin();
    ASSERT_TRUE(state->payment.stageMana('R', 0, 17));
    const quint64 transaction = state->payment.transaction();
    const std::string stagedSelection = state->payment.selection.SerializeAsString();
    ASSERT_EQ(state->payment.optimisticManaCounterSpendCount(17), 1);

    setOffer(false);
    EXPECT_EQ(ruledResolutionPaymentResumeDecision(*state, parent, false, false),
              RuledPaymentResumeDecision::Resume);
    setOffer(true);
    EXPECT_EQ(ruledResolutionPaymentResumeDecision(*state, parent, false, false),
              RuledPaymentResumeDecision::Resume);
    EXPECT_EQ(state->payment.transaction(), transaction);
    EXPECT_EQ(state->payment.selection.SerializeAsString(), stagedSelection);
    EXPECT_EQ(state->payment.optimisticManaCounterSpendCount(17), 1);
}

TEST_F(RuledClientTest, SuspendedResolutionPaymentResumeWaitsForNestedWorkAndDiscardsStaleReceipts)
{
    constexpr quint64 parentPromptId = 8302;
    const std::string parentSemantics = "parent-answer-semantics";
    const RuledResolutionPaymentIdentity parent{parentPromptId, parentSemantics};
    const auto setOffer = [&](quint64 promptId, const std::string &semantics,
                              RuledClientState::ChoiceKind kind = RuledClientState::ChoiceKind::ResolutionPayment) {
        state->resolutionPromptSnapshotValid = true;
        state->resolutionPromptActive = true;
        state->resolutionPromptDeciderPlayerId = kLocalPlayer;
        state->resolutionPromptId = promptId;
        state->resolutionPromptSemantics = semantics;
        state->choiceWaitingPlayerId = -1;
        RuledClientState::RuledPendingChoice choice;
        choice.kind = kind;
        choice.resolutionPromptId = promptId;
        choice.resolutionPromptSemantics = semantics;
        state->pendingChoice = std::move(choice);
    };

    setOffer(parentPromptId + 1, parentSemantics);
    EXPECT_EQ(ruledResolutionPaymentResumeDecision(*state, parent, false, false), RuledPaymentResumeDecision::Discard);

    setOffer(parentPromptId, "changed-answer-semantics");
    EXPECT_EQ(ruledResolutionPaymentResumeDecision(*state, parent, false, false), RuledPaymentResumeDecision::Discard);

    setOffer(parentPromptId + 1, "nested-payment-semantics");
    EXPECT_EQ(ruledResolutionPaymentResumeDecision(*state, parent, false, false), RuledPaymentResumeDecision::Wait);

    setOffer(parentPromptId + 1, "foreign-choice-semantics", RuledClientState::ChoiceKind::ResolutionBranch);
    EXPECT_EQ(ruledResolutionPaymentResumeDecision(*state, parent, false, false), RuledPaymentResumeDecision::Wait);

    state->pendingChoice.reset();
    state->resolutionPromptSnapshotValid = false;
    state->resolutionPromptActive = false;
    state->resolutionPromptId = 0;
    state->resolutionPromptSemantics.clear();
    state->choiceWaitingPlayerId = -1;
    EXPECT_EQ(ruledResolutionPaymentResumeDecision(*state, parent, false, false), RuledPaymentResumeDecision::Discard);

    state->choiceWaitingPlayerId = 1;
    EXPECT_EQ(ruledResolutionPaymentResumeDecision(*state, parent, false, false), RuledPaymentResumeDecision::Wait);
    state->choiceWaitingPlayerId = -1;

    ASSERT_TRUE(state->beginEngineCommand());
    EXPECT_EQ(ruledResolutionPaymentResumeDecision(*state, parent, false, false), RuledPaymentResumeDecision::Wait);
    state->finishEngineCommand();
    EXPECT_EQ(ruledResolutionPaymentResumeDecision(*state, parent, true, false), RuledPaymentResumeDecision::Wait);
    EXPECT_EQ(ruledResolutionPaymentResumeDecision(*state, parent, false, true), RuledPaymentResumeDecision::Wait);
    state->payment.submitting = true;
    EXPECT_EQ(ruledResolutionPaymentResumeDecision(*state, parent, false, false), RuledPaymentResumeDecision::Wait);
}

TEST_F(RuledClientTest, MixedPaymentPreviewDoesNotBypassMissingPromptSnapshotValidation)
{
    ruled::v1::RuledEventBatch mixed;
    mixed.mutable_payment_preview()->set_transaction_id(82);
    auto *choice = mixed.add_events()->mutable_resolution_choice_required();
    choice->set_deciding_player_id(kLocalPlayer);
    choice->set_source_object_id(8003);
    choice->set_choice_kind(ruled::v1::CHOICE_KIND_PERMANENT_OBJECTS);
    choice->set_min(0);
    choice->set_max(1);
    choice->set_resolution_prompt_id(8201);
    choice->add_candidate_object_ids(8004);
    std::string payload;
    ASSERT_TRUE(mixed.SerializeToString(&payload));

    EXPECT_TRUE(dispatcher->processPayload(payload));
    EXPECT_FALSE(state->pendingChoice.has_value());
    EXPECT_EQ(state->resolutionPromptId, 0u);
    EXPECT_FALSE(state->resolutionPromptActive);
}

TEST_F(RuledClientTest, NewActivePromptWithoutMatchingRcrFailsClosed)
{
    ruled::v1::RuledEventBatch batch;
    auto *prompt = batch.mutable_resolution_prompt_state();
    prompt->set_active(true);
    prompt->set_deciding_player_id(kLocalPlayer);
    prompt->set_resolution_prompt_id(4101);

    applyRaw(batch);

    EXPECT_FALSE(state->pendingChoice.has_value());
    EXPECT_EQ(state->resolutionPromptId, 0u);
    EXPECT_TRUE(state->resolutionPromptSnapshotValid);
    EXPECT_TRUE(state->resolutionPromptActive);
    EXPECT_EQ(state->choiceWaitingPlayerId, -1);
}

TEST_F(RuledClientTest, InactivePromptWithoutRcrRetiresStagedChoice)
{
    ruled::v1::RuledEventBatch active;
    auto *choice = active.add_events()->mutable_resolution_choice_required();
    choice->set_deciding_player_id(kLocalPlayer);
    choice->set_source_object_id(8101);
    choice->set_choice_kind(ruled::v1::CHOICE_KIND_PERMANENT_OBJECTS);
    choice->set_min(0);
    choice->set_max(1);
    choice->add_candidate_object_ids(8102);
    apply(active);

    ASSERT_TRUE(state->pendingChoice.has_value());
    ASSERT_NE(state->resolutionPromptId, 0u);
    state->pendingChoice->selectedObjectOids.append(8102);
    ruled::v1::RuledEventBatch inactive;
    inactive.mutable_resolution_prompt_state()->set_active(false);
    inactive.mutable_resolution_prompt_state()->set_deciding_player_id(0);
    applyRaw(inactive);

    EXPECT_FALSE(state->pendingChoice.has_value());
    EXPECT_EQ(state->resolutionPromptId, 0u);
    EXPECT_TRUE(state->resolutionPromptSnapshotValid);
    EXPECT_FALSE(state->resolutionPromptActive);
}

TEST_F(RuledClientTest, SameActivePromptWithoutRcrRetainsStagedChoice)
{
    ruled::v1::RuledEventBatch active;
    auto *choice = active.add_events()->mutable_resolution_choice_required();
    choice->set_deciding_player_id(kLocalPlayer);
    choice->set_source_object_id(8201);
    choice->set_choice_kind(ruled::v1::CHOICE_KIND_PERMANENT_OBJECTS);
    choice->set_min(0);
    choice->set_max(1);
    choice->add_candidate_object_ids(8202);
    apply(active);

    ASSERT_TRUE(state->pendingChoice.has_value());
    const quint64 promptId = state->resolutionPromptId;
    state->pendingChoice->selectedObjectOids.append(8202);
    ruled::v1::RuledEventBatch refresh;
    refresh.mutable_resolution_prompt_state()->CopyFrom(active.resolution_prompt_state());
    applyRaw(refresh);

    ASSERT_TRUE(state->pendingChoice.has_value());
    EXPECT_EQ(state->pendingChoice->resolutionPromptId, promptId);
    EXPECT_EQ(state->pendingChoice->selectedObjectOids, QVector<quint32>({8202}));
    EXPECT_EQ(state->resolutionPromptId, promptId);
}

TEST_F(RuledClientTest, SameActivePromptKeepsStagedChoiceAcrossTransientNonmatchingRcr)
{
    ruled::v1::RuledEventBatch active;
    auto *choice = active.add_events()->mutable_resolution_choice_required();
    choice->set_deciding_player_id(kLocalPlayer);
    choice->set_source_object_id(8251);
    choice->set_choice_kind(ruled::v1::CHOICE_KIND_PERMANENT_OBJECTS);
    choice->set_min(0);
    choice->set_max(1);
    choice->add_candidate_object_ids(8252);
    apply(active);

    ASSERT_TRUE(state->pendingChoice.has_value());
    const quint64 promptId = state->resolutionPromptId;
    state->pendingChoice->selectedObjectOids.append(8252);

    ruled::v1::RuledEventBatch refresh;
    refresh.mutable_resolution_prompt_state()->CopyFrom(active.resolution_prompt_state());
    auto *transient = refresh.add_events()->mutable_resolution_choice_required();
    transient->set_deciding_player_id(kLocalPlayer);
    transient->set_source_object_id(8253);
    transient->set_choice_kind(ruled::v1::CHOICE_KIND_OPPONENT_HAND);
    transient->set_resolution_prompt_id(0);
    transient->set_min(1);
    transient->set_max(1);
    transient->add_candidate_object_ids(8254);
    transient->add_candidate_card_ids("forest");
    transient->add_candidate_names("Forest");
    transient->add_candidate_server_card_ids(25);
    transient->mutable_public_reveal()->set_zone_owner_player_id(kLocalPlayer + 1);

    applyRaw(refresh);

    ASSERT_TRUE(state->pendingChoice.has_value());
    EXPECT_EQ(state->pendingChoice->resolutionPromptId, promptId);
    EXPECT_EQ(state->pendingChoice->selectedObjectOids, QVector<quint32>({8252}));
    EXPECT_EQ(state->resolutionPromptId, promptId);
    ASSERT_EQ(state->reveals.entries().size(), 1);
    EXPECT_EQ(state->reveals.entries().first().cards.first().name, QStringLiteral("Forest"));
}

TEST_F(RuledClientTest, SameActivePublicRevealChoiceSurvivesRefreshWithoutMatchingRcr)
{
    ruled::v1::RuledEventBatch active;
    auto *prompt = active.mutable_resolution_prompt_state();
    prompt->set_active(true);
    prompt->set_deciding_player_id(kLocalPlayer);
    prompt->set_resolution_prompt_id(8351);

    ruled::v1::CardsRevealed receipt;
    receipt.set_reveal_id("same-prompt-public-choice-8351");
    receipt.set_source_object_id(8350);
    receipt.set_source_zone(ruled::v1::CHOICE_CANDIDATE_SOURCE_ZONE_LIBRARY);
    receipt.set_zone_owner_player_id(kLocalPlayer);
    auto *revealedCard = receipt.add_cards();
    revealedCard->set_object_id(8352);
    revealedCard->set_zone_change_generation(4);
    revealedCard->set_card_id("forest");
    revealedCard->set_card_name("Forest");

    auto *choice = active.add_events()->mutable_resolution_choice_required();
    choice->set_deciding_player_id(kLocalPlayer);
    choice->set_source_object_id(8350);
    choice->set_choice_kind(ruled::v1::CHOICE_KIND_LIBRARY_LOOK);
    choice->set_resolution_prompt_id(8351);
    choice->set_min(1);
    choice->set_max(1);
    choice->add_candidate_object_ids(8352);
    choice->add_candidate_card_ids("forest");
    choice->add_candidate_names("Forest");
    choice->add_candidate_server_card_ids(35);
    choice->mutable_public_reveal()->CopyFrom(receipt);
    active.add_events()->mutable_active_public_reveal_snapshot()->add_reveals()->CopyFrom(receipt);
    (*active.mutable_legal_by_player())[host.localPlayerId()].set_undoable_mana_abilities(0);

    applyRaw(active);
    EXPECT_EQ(state->resolutionPromptId, 8351u);
    EXPECT_TRUE(state->pendingChoice.has_value());
    EXPECT_TRUE(state->reveals.hasChoice());
    ASSERT_TRUE(state->isResolutionHandPickActive());
    ASSERT_NE(state->reveals.choice(), nullptr);
    EXPECT_EQ(state->reveals.choice()->phase, RuledRevealState::Phase::Choice);
    state->toggleResolutionHandPickCard(35);
    ASSERT_EQ(state->pendingChoice->selectedServerCardIds, QList<int>({35}));

    ruled::v1::RuledEventBatch refresh;
    refresh.mutable_resolution_prompt_state()->CopyFrom(active.resolution_prompt_state());
    refresh.add_events()->mutable_active_public_reveal_snapshot()->add_reveals()->CopyFrom(receipt);
    (*refresh.mutable_legal_by_player())[host.localPlayerId()].set_undoable_mana_abilities(0);
    applyRaw(refresh);

    EXPECT_TRUE(state->isResolutionHandPickActive());
    ASSERT_NE(state->reveals.choice(), nullptr);
    EXPECT_EQ(state->reveals.choice()->phase, RuledRevealState::Phase::Choice);
    EXPECT_EQ(state->reveals.choice()->choiceCardIds, QVector<int>({35}));
    ASSERT_TRUE(state->pendingChoice.has_value());
    EXPECT_EQ(state->pendingChoice->selectedServerCardIds, QList<int>({35}));
}

TEST_F(RuledClientTest, SameActivePublicRevealChoiceSurvivesTransientNonmatchingRcr)
{
    ruled::v1::RuledEventBatch active;
    auto *prompt = active.mutable_resolution_prompt_state();
    prompt->set_active(true);
    prompt->set_deciding_player_id(kLocalPlayer);
    prompt->set_resolution_prompt_id(8361);

    ruled::v1::CardsRevealed receipt;
    receipt.set_reveal_id("same-prompt-public-choice-8361");
    receipt.set_source_object_id(8360);
    receipt.set_source_zone(ruled::v1::CHOICE_CANDIDATE_SOURCE_ZONE_LIBRARY);
    receipt.set_zone_owner_player_id(kLocalPlayer);
    auto *revealedCard = receipt.add_cards();
    revealedCard->set_object_id(8362);
    revealedCard->set_zone_change_generation(5);
    revealedCard->set_card_id("island");
    revealedCard->set_card_name("Island");

    auto *choice = active.add_events()->mutable_resolution_choice_required();
    choice->set_deciding_player_id(kLocalPlayer);
    choice->set_source_object_id(8360);
    choice->set_choice_kind(ruled::v1::CHOICE_KIND_LIBRARY_LOOK);
    choice->set_resolution_prompt_id(8361);
    choice->set_min(1);
    choice->set_max(1);
    choice->add_candidate_object_ids(8362);
    choice->add_candidate_card_ids("island");
    choice->add_candidate_names("Island");
    choice->add_candidate_server_card_ids(36);
    choice->mutable_public_reveal()->CopyFrom(receipt);
    active.add_events()->mutable_active_public_reveal_snapshot()->add_reveals()->CopyFrom(receipt);
    (*active.mutable_legal_by_player())[host.localPlayerId()].set_undoable_mana_abilities(0);

    applyRaw(active);
    EXPECT_EQ(state->resolutionPromptId, 8361u);
    EXPECT_TRUE(state->pendingChoice.has_value());
    EXPECT_TRUE(state->reveals.hasChoice());
    ASSERT_TRUE(state->isResolutionHandPickActive());
    state->toggleResolutionHandPickCard(36);
    ASSERT_EQ(state->pendingChoice->selectedServerCardIds, QList<int>({36}));

    ruled::v1::RuledEventBatch refresh;
    refresh.mutable_resolution_prompt_state()->CopyFrom(active.resolution_prompt_state());
    refresh.add_events()->mutable_active_public_reveal_snapshot()->add_reveals()->CopyFrom(receipt);
    (*refresh.mutable_legal_by_player())[host.localPlayerId()].set_undoable_mana_abilities(0);
    auto *transient = refresh.add_events()->mutable_resolution_choice_required();
    transient->set_deciding_player_id(kLocalPlayer);
    transient->set_source_object_id(8363);
    transient->set_choice_kind(ruled::v1::CHOICE_KIND_OPPONENT_HAND);
    transient->set_resolution_prompt_id(0);
    transient->set_min(1);
    transient->set_max(1);
    transient->add_candidate_object_ids(8364);
    transient->add_candidate_card_ids("forest");
    transient->add_candidate_names("Forest");
    transient->add_candidate_server_card_ids(37);
    auto *transientReceipt = transient->mutable_public_reveal();
    transientReceipt->set_reveal_id("transient-public-reveal-8363");
    transientReceipt->set_source_object_id(8363);
    transientReceipt->set_source_zone(ruled::v1::CHOICE_CANDIDATE_SOURCE_ZONE_HAND);
    transientReceipt->set_zone_owner_player_id(kLocalPlayer + 1);
    auto *transientCard = transientReceipt->add_cards();
    transientCard->set_object_id(8364);
    transientCard->set_zone_change_generation(2);
    transientCard->set_card_id("forest");
    transientCard->set_card_name("Forest");

    applyRaw(refresh);

    EXPECT_TRUE(state->isResolutionHandPickActive());
    ASSERT_TRUE(state->pendingChoice.has_value());
    EXPECT_EQ(state->pendingChoice->selectedServerCardIds, QList<int>({36}));
    ASSERT_NE(state->reveals.choice(), nullptr);
    EXPECT_EQ(state->reveals.choice()->id, QStringLiteral("same-prompt-public-choice-8361"));
    EXPECT_EQ(state->reveals.choice()->phase, RuledRevealState::Phase::Choice);
    ASSERT_TRUE(state->reveals.entries().contains(QStringLiteral("transient-public-reveal-8363")));
    EXPECT_FALSE(state->reveals.entries().value(QStringLiteral("transient-public-reveal-8363")).hasInteractiveChoice());
}

TEST_F(RuledClientTest, LaterMalformedMatchingPublicChoiceCannotReuseEarlierRevealSeenFlag)
{
    ruled::v1::RuledEventBatch batch;
    auto *prompt = batch.mutable_resolution_prompt_state();
    prompt->set_active(true);
    prompt->set_deciding_player_id(host.localPlayerId());
    prompt->set_resolution_prompt_id(8371);

    ruled::v1::CardsRevealed receipt;
    receipt.set_reveal_id("same-batch-last-matching-choice-8371");
    receipt.set_source_object_id(8370);
    receipt.set_source_zone(ruled::v1::CHOICE_CANDIDATE_SOURCE_ZONE_LIBRARY);
    receipt.set_zone_owner_player_id(host.localPlayerId());
    auto *card = receipt.add_cards();
    card->set_object_id(8372);
    card->set_zone_change_generation(6);
    card->set_card_id("plains");
    card->set_card_name("Plains");

    auto *valid = batch.add_events()->mutable_resolution_choice_required();
    valid->set_deciding_player_id(host.localPlayerId());
    valid->set_source_object_id(8370);
    valid->set_choice_kind(ruled::v1::CHOICE_KIND_LIBRARY_LOOK);
    valid->set_resolution_prompt_id(8371);
    valid->set_min(1);
    valid->set_max(1);
    valid->add_candidate_object_ids(8372);
    valid->add_candidate_card_ids("plains");
    valid->add_candidate_names("Plains");
    valid->add_candidate_server_card_ids(38);
    valid->mutable_public_reveal()->CopyFrom(receipt);

    auto *malformed = batch.add_events()->mutable_resolution_choice_required();
    malformed->CopyFrom(*valid);
    malformed->clear_candidate_server_card_ids();
    batch.add_events()->mutable_active_public_reveal_snapshot()->add_reveals()->CopyFrom(receipt);
    (*batch.mutable_legal_by_player())[host.localPlayerId()].set_undoable_mana_abilities(0);

    applyRaw(batch);

    EXPECT_FALSE(state->pendingChoice.has_value());
    EXPECT_EQ(state->resolutionPromptId, 0u);
    EXPECT_FALSE(state->reveals.hasChoice());
    ASSERT_TRUE(state->reveals.entries().contains(QString::fromStdString(receipt.reveal_id())));
    EXPECT_EQ(state->reveals.entries().value(QString::fromStdString(receipt.reveal_id())).phase,
              RuledRevealState::Phase::Active);
}

TEST_F(RuledClientTest, PublicRevealRejectsMinimumLargerThanSelectableCohort)
{
    ruled::v1::RuledEventBatch batch;
    auto *prompt = batch.mutable_resolution_prompt_state();
    prompt->set_active(true);
    prompt->set_deciding_player_id(host.localPlayerId());
    prompt->set_resolution_prompt_id(8381);

    ruled::v1::CardsRevealed receipt;
    receipt.set_reveal_id("impossible-minimum-public-choice-8381");
    receipt.set_source_object_id(8380);
    receipt.set_source_zone(ruled::v1::CHOICE_CANDIDATE_SOURCE_ZONE_LIBRARY);
    receipt.set_zone_owner_player_id(host.localPlayerId());
    auto *card = receipt.add_cards();
    card->set_object_id(8382);
    card->set_zone_change_generation(7);
    card->set_card_id("mountain");
    card->set_card_name("Mountain");

    auto *choice = batch.add_events()->mutable_resolution_choice_required();
    choice->set_deciding_player_id(host.localPlayerId());
    choice->set_source_object_id(8380);
    choice->set_choice_kind(ruled::v1::CHOICE_KIND_LIBRARY_LOOK);
    choice->set_resolution_prompt_id(8381);
    choice->set_min(2);
    choice->set_max(2);
    choice->add_candidate_object_ids(8382);
    choice->add_candidate_card_ids("mountain");
    choice->add_candidate_names("Mountain");
    choice->add_candidate_server_card_ids(39);
    choice->mutable_public_reveal()->CopyFrom(receipt);
    batch.add_events()->mutable_active_public_reveal_snapshot()->add_reveals()->CopyFrom(receipt);
    (*batch.mutable_legal_by_player())[host.localPlayerId()].set_undoable_mana_abilities(0);

    applyRaw(batch);

    EXPECT_FALSE(state->pendingChoice.has_value());
    EXPECT_EQ(state->resolutionPromptId, 0u);
    EXPECT_FALSE(state->reveals.hasChoice());
    ASSERT_TRUE(state->reveals.entries().contains(QString::fromStdString(receipt.reveal_id())));
    EXPECT_EQ(state->reveals.entries().value(QString::fromStdString(receipt.reveal_id())).phase,
              RuledRevealState::Phase::Active);
}

TEST_F(RuledClientTest, MalformedMatchingRcrDoesNotFallBackToSameIdStaging)
{
    ruled::v1::RuledEventBatch active;
    auto *choice = active.add_events()->mutable_resolution_choice_required();
    choice->set_deciding_player_id(kLocalPlayer);
    choice->set_source_object_id(8261);
    choice->set_choice_kind(ruled::v1::CHOICE_KIND_REPLACEMENT_EFFECT);
    choice->set_min(1);
    choice->set_max(1);
    choice->add_candidate_object_ids(8262);
    auto *option = choice->add_replacement_options();
    option->set_application_id(8262);
    option->set_effect_summary("Valid replacement");
    apply(active);

    ASSERT_TRUE(state->pendingChoice.has_value());
    const quint64 promptId = state->resolutionPromptId;
    ASSERT_NE(promptId, 0u);

    ruled::v1::RuledEventBatch malformed;
    malformed.mutable_resolution_prompt_state()->CopyFrom(active.resolution_prompt_state());
    auto *bad = malformed.add_events()->mutable_resolution_choice_required();
    bad->CopyFrom(active.events(0).resolution_choice_required());
    bad->mutable_replacement_options(0)->clear_effect_summary();

    applyRaw(malformed);

    EXPECT_FALSE(state->pendingChoice.has_value());
    EXPECT_EQ(state->resolutionPromptId, 0u);
    EXPECT_TRUE(state->resolutionPromptActive);
}

TEST_F(RuledClientTest, InactiveTransientRcrKeepsItsPublicRevealButNeverStagesChoice)
{
    ruled::v1::RuledEventBatch batch;
    batch.mutable_resolution_prompt_state()->set_active(false);
    batch.mutable_resolution_prompt_state()->set_deciding_player_id(0);
    auto *choice = batch.add_events()->mutable_resolution_choice_required();
    choice->set_deciding_player_id(kLocalPlayer);
    choice->set_source_object_id(8301);
    choice->set_choice_kind(ruled::v1::CHOICE_KIND_OPPONENT_HAND);
    choice->set_resolution_prompt_id(0);
    choice->add_candidate_object_ids(8302);
    choice->add_candidate_card_ids("forest");
    choice->add_candidate_names("Forest");
    choice->add_candidate_server_card_ids(3);
    choice->mutable_public_reveal()->set_zone_owner_player_id(kLocalPlayer + 1);

    applyRaw(batch);

    EXPECT_FALSE(state->pendingChoice.has_value());
    EXPECT_FALSE(state->resolutionPromptActive);
    EXPECT_EQ(state->resolutionPromptId, 0u);
    ASSERT_EQ(state->reveals.entries().size(), 1);
    EXPECT_EQ(state->reveals.entries().first().cards.first().name, QStringLiteral("Forest"));
}

TEST_F(RuledClientTest, SamePromptRefreshRebasesStagedServerHandlesByObjectId)
{
    auto makeBatch = [](int serverCardId) {
        ruled::v1::RuledEventBatch batch;
        auto *prompt = batch.mutable_resolution_prompt_state();
        prompt->set_active(true);
        prompt->set_deciding_player_id(kLocalPlayer);
        prompt->set_resolution_prompt_id(8401);
        auto *choice = batch.add_events()->mutable_resolution_choice_required();
        choice->set_deciding_player_id(kLocalPlayer);
        choice->set_source_object_id(8400);
        choice->set_choice_kind(ruled::v1::CHOICE_KIND_LIBRARY_LOOK);
        choice->set_resolution_prompt_id(8401);
        choice->set_min(1);
        choice->set_max(2);
        choice->add_candidate_object_ids(8402);
        choice->add_candidate_card_ids("forest");
        choice->add_candidate_names("Forest");
        choice->add_candidate_server_card_ids(serverCardId);
        choice->mutable_public_reveal()->set_zone_owner_player_id(kLocalPlayer);
        return batch;
    };

    auto first = makeBatch(10);
    applyRaw(first);
    ASSERT_TRUE(state->isResolutionHandPickActive());
    state->toggleResolutionHandPickCard(10);
    ASSERT_EQ(state->pendingChoice->selectedServerCardIds, QList<int>({10}));

    auto refreshed = makeBatch(110);
    applyRaw(refreshed);

    ASSERT_TRUE(state->isResolutionHandPickActive());
    EXPECT_EQ(state->pendingChoice->resolutionPromptId, 8401u);
    EXPECT_EQ(state->pendingChoice->selectedServerCardIds, QList<int>({110}));
    EXPECT_EQ(state->pendingChoice->serverCardIdToOid.value(110), 8402u);
}

TEST_F(RuledClientTest, WaterbendRefreshAndImmediateRejectionPreserveStagingButNewPromptSupersedesAck)
{
    ruled::v1::RuledEventBatch batch;
    auto *choice = batch.add_events()->mutable_resolution_choice_required();
    choice->set_deciding_player_id(kLocalPlayer);
    choice->set_source_object_id(81);
    choice->set_choice_kind(ruled::v1::CHOICE_KIND_MANA_PAYMENT);
    choice->set_waterbend(true);
    choice->set_generic_mana_cost(2);
    apply(batch);
    ASSERT_TRUE(state->isWaterbendResolutionPayment());
    auto &payment = state->payment;
    payment.begin(true);
    payment.selection.add_waterbend()->set_object_id(44);
    payment.selection.mutable_waterbend(0)->set_zone_change_generation(7);
    const auto transaction = payment.transaction();
    apply(batch); // Mana abilities republish the same parked choice.
    ASSERT_TRUE(payment.active);
    EXPECT_EQ(payment.transaction(), transaction);
    ASSERT_EQ(payment.selection.waterbend_size(), 1);
    payment.view.set_valid(true);
    payment.view.set_complete(true);
    state->payResolutionMana();
    ASSERT_EQ(host.sentCommands.size(), 1);
    EXPECT_EQ(host.sentCommands.last().submit_resolution_choice().payment().waterbend(0).zone_change_generation(), 7u);
    host.answerPendingAck(false);
    ASSERT_TRUE(state->isWaterbendResolutionPayment());
    ASSERT_EQ(payment.selection.waterbend_size(), 1);
    EXPECT_FALSE(payment.submitting);
    EXPECT_FALSE(state->resolutionPaymentCurrentlyLegal());
    payment.pending = false;
    state->payResolutionMana();
    apply(batch); // A same-source refresh can arrive before the rejection acknowledgement.
    const auto freshRevision = state->pendingChoiceRevision;
    const auto freshTransaction = payment.transaction();
    QSignalSpy finished(state, &RuledClientState::resolutionPaymentSubmissionFinished);
    host.answerPendingAck(false);
    EXPECT_FALSE(payment.submitting);
    // The source object is not a choice-occurrence identity. A freshly published prompt must
    // not be overwritten by the previous submission's ACK, even when the source is unchanged.
    EXPECT_EQ(payment.selection.waterbend_size(), 0);
    EXPECT_EQ(state->pendingChoiceRevision, freshRevision);
    EXPECT_EQ(payment.transaction(), freshTransaction);
    EXPECT_EQ(finished.count(), 0);
    state->declineResolutionMana();
    EXPECT_FALSE(host.sentCommands.last().submit_resolution_choice().has_payment());
    host.answerPendingAck(true);
    choice->set_source_object_id(82);
    apply(batch);
    EXPECT_FALSE(payment.active); // A different resolution owns a new transaction.
}

TEST_F(RuledClientTest, BattlefieldObjectMapBuildsIdentityMapsBothWays)
{
    ruled::v1::RuledEventBatch batch;
    auto *ev = batch.add_events();
    auto *e1 = addPermanent(ev, kLocalPlayer, 100, 7);
    e1->set_summoning_sick(true);
    e1->add_keywords("Haste");
    auto *e2 = addPermanent(ev, kOpponent, 200, 9);
    e2->add_keywords("Trample");
    e2->set_is_creature(false);
    apply(batch);

    EXPECT_EQ(state->engineOidForCardId(kLocalPlayer, 7), 100u);
    EXPECT_EQ(state->cardIdForEngineOid(100), 7);
    EXPECT_EQ(state->playerIdForEngineOid(100), kLocalPlayer);
    EXPECT_TRUE(state->isEngineOidSummoningSick(100));
    EXPECT_TRUE(state->isEngineOidHaste(100));
    EXPECT_TRUE(state->isEngineOidCreature(100));

    EXPECT_EQ(state->engineOidForCardId(kOpponent, 9), 200u);
    EXPECT_TRUE(state->isEngineOidTrample(200));
    EXPECT_FALSE(state->isEngineOidCreature(200));

    // Unknown ids answer with the documented sentinels rather than asserting.
    EXPECT_EQ(state->engineOidForCardId(kLocalPlayer, 999), 0u);
    EXPECT_EQ(state->cardIdForEngineOid(999), -1);
    EXPECT_EQ(state->playerIdForEngineOid(999), -1);
}

TEST_F(RuledClientTest, PhasedOutStatusRebuildsOnFullMapAndResetsWithSession)
{
    ruled::v1::RuledEventBatch initial;
    auto *initialMap = initial.add_events();
    addPermanent(initialMap, kOpponent, 801, 17)->set_phased_out(true);
    addPermanent(initialMap, kLocalPlayer, 802, 18)->set_phased_out(false);
    apply(initial);

    ASSERT_TRUE(state->isEngineOidPhasedOut(801))
        << "the complete public battlefield map must retain an engine-reported phasing state";
    EXPECT_FALSE(state->isEngineOidPhasedOut(802));

    ruled::v1::RuledEventBatch replacement;
    addPermanent(replacement.add_events(), kLocalPlayer, 802, 18)->set_phased_out(false);
    apply(replacement);
    EXPECT_FALSE(state->isEngineOidPhasedOut(801))
        << "a full-map replacement must retire phased status for absent ObjectIds";
    EXPECT_FALSE(state->isEngineOidPhasedOut(802));

    ruled::v1::RuledEventBatch nextSession;
    addPermanent(nextSession.add_events(), kOpponent, 803, 19)->set_phased_out(true);
    apply(nextSession);
    ASSERT_TRUE(state->isEngineOidPhasedOut(803));
    state->clearSessionState(RuledSessionResetScope::All);
    EXPECT_FALSE(state->isEngineOidPhasedOut(803))
        << "session reset must not carry phasing state into the next game";

    ruled::v1::RuledEventBatch restoredSession;
    addPermanent(restoredSession.add_events(), kLocalPlayer, 804, 20)->set_phased_out(true);
    apply(restoredSession);
    EXPECT_TRUE(state->isEngineOidPhasedOut(804));
}

TEST_F(RuledClientTest, EarthbendAnimationAndReturnReplaceDerivedCharacteristics)
{
    auto publish = [&](bool creature, quint64 generation) {
        ruled::v1::RuledEventBatch batch;
        auto *entry = addPermanent(batch.add_events(), kLocalPlayer, 100, 7);
        entry->set_is_creature(creature);
        if (creature) {
            entry->add_keywords("Haste");
        }
        auto *player = batch.add_events()->mutable_zone_view()->add_per_player();
        player->set_player_id(kLocalPlayer);
        auto *object = player->add_battlefield_objects();
        object->set_object_id(100);
        object->set_zone_change_generation(generation);
        object->set_power(creature ? 2 : 0);
        object->set_toughness(creature ? 2 : 0);
        apply(batch);
    };
    publish(false, 0);
    EXPECT_FALSE(state->isEngineOidCreature(100));
    publish(true, 0);
    EXPECT_TRUE(state->isEngineOidCreature(100));
    EXPECT_TRUE(state->isEngineOidHaste(100));
    EXPECT_EQ(state->combatPowerForCreatureOid(100), 2);
    EXPECT_EQ(state->combatToughnessForCreatureOid(100), 2);
    publish(false, 2);
    EXPECT_FALSE(state->isEngineOidCreature(100));
    EXPECT_FALSE(state->isEngineOidHaste(100));
    EXPECT_EQ(state->battlefieldGenerationByOid.value(100), 2u);
    EXPECT_EQ(state->cardIdForEngineOid(100), 7);
    EXPECT_EQ(state->combatPowerForCreatureOid(100), 0);
}

TEST(RuledAutoPassPolicyTest, MapsToolbarStopsAndSharesCombatDamageStop)
{
    std::array<bool, 11> own{};
    std::array<bool, 11> opponent{};
    own[2] = true;      // Draw
    own[7] = true;      // Combat damage (both CR 510.4 steps)
    opponent[4] = true; // Beginning of combat

    const ruled::v1::SetAutoPassPolicy policy = RuledAutoPassPolicy::fromToolbarStops(own, opponent);
    EXPECT_EQ(policy.stop_on_own_turn_size(), 3);
    EXPECT_EQ(policy.stop_on_own_turn(0), ruled::v1::PHASE_ID_DRAW);
    EXPECT_EQ(policy.stop_on_own_turn(1), ruled::v1::PHASE_ID_FIRST_STRIKE_DAMAGE);
    EXPECT_EQ(policy.stop_on_own_turn(2), ruled::v1::PHASE_ID_COMBAT_DAMAGE);
    ASSERT_EQ(policy.stop_on_opponent_turn_size(), 1);
    EXPECT_EQ(policy.stop_on_opponent_turn(0), ruled::v1::PHASE_ID_BEGIN_COMBAT);
}

TEST(RuledAutoPassPolicyTest, RestoresEveryToolbarStopWithoutAddingDefaults)
{
    for (int index = 1; index < 11; ++index) {
        std::array<bool, 11> own{};
        std::array<bool, 11> opponent{};
        own[index] = true;
        opponent[11 - index] = true;
        const auto policy = RuledAutoPassPolicy::fromToolbarStops(own, opponent);
        EXPECT_EQ(RuledAutoPassPolicy::toToolbarStops(policy.stop_on_own_turn()), own);
        EXPECT_EQ(RuledAutoPassPolicy::toToolbarStops(policy.stop_on_opponent_turn()), opponent);
    }
}

TEST_F(RuledClientTest, EngineCommandPendingLocksImmediatelyAndRejectsDuplicateBegin)
{
    QSignalSpy spy(state, &RuledClientState::engineCommandPendingUiChanged);
    EXPECT_TRUE(state->beginEngineCommand());
    EXPECT_TRUE(state->isEngineCommandPending());
    EXPECT_FALSE(state->isEngineCommandIndicatorVisible());
    EXPECT_FALSE(state->beginEngineCommand());
    EXPECT_EQ(spy.count(), 1);

    state->showEngineCommandIndicator();
    EXPECT_TRUE(state->isEngineCommandIndicatorVisible());
    EXPECT_EQ(spy.count(), 2);

    state->finishEngineCommand();
    EXPECT_FALSE(state->isEngineCommandPending());
    EXPECT_FALSE(state->isEngineCommandIndicatorVisible());
    EXPECT_EQ(spy.count(), 3);
}

TEST_F(RuledClientTest, EngineCommandIndicatorAppearsOnlyAfterDelay)
{
    ASSERT_TRUE(state->beginEngineCommand());
    QTest::qWait(100);
    EXPECT_FALSE(state->isEngineCommandIndicatorVisible());
    QTRY_VERIFY_WITH_TIMEOUT(state->isEngineCommandIndicatorVisible(), 200);

    state->finishEngineCommand();
}

TEST_F(RuledClientTest, FinishedCommandCancelsItsDelayedIndicator)
{
    ASSERT_TRUE(state->beginEngineCommand());
    state->finishEngineCommand();
    QTest::qWait(175);
    EXPECT_FALSE(state->isEngineCommandPending());
    EXPECT_FALSE(state->isEngineCommandIndicatorVisible());
}

TEST_F(RuledClientTest, StaleIndicatorCallbackCannotAffectLaterCommand)
{
    ASSERT_TRUE(state->beginEngineCommand());
    QTest::qWait(50);
    state->finishEngineCommand();
    ASSERT_TRUE(state->beginEngineCommand());

    // The first command's 150 ms callback has now fired, but the second command has not reached
    // its own threshold. Its generation token must keep the new prompt unchanged.
    QTest::qWait(110);
    EXPECT_TRUE(state->isEngineCommandPending());
    EXPECT_FALSE(state->isEngineCommandIndicatorVisible());
    QTRY_VERIFY_WITH_TIMEOUT(state->isEngineCommandIndicatorVisible(), 100);
    state->finishEngineCommand();
}

TEST(RuledPendingPaymentTest, LastManaPipConsumedDuringEngineCommandIsReadyAfterUnlock)
{
    PendingRuledSpellCast spell;
    spell.valid = true;
    spell.waitingForTarget = false;
    spell.remainingCost.insert(QChar('G'), 0);
    EXPECT_EQ(readyRuledPendingPaymentAction(spell, {}), RuledPendingPaymentAction::CastSpell);

    spell.remainingCost[QChar('G')] = 1;
    EXPECT_EQ(readyRuledPendingPaymentAction(spell, {}), RuledPendingPaymentAction::None);

    PendingActivatedAbility ability;
    ability.valid = true;
    ability.waitingForTarget = false;
    ability.waitingForMana = false; // the last-pip path clears this before command submission
    EXPECT_EQ(readyRuledPendingPaymentAction({}, ability), RuledPendingPaymentAction::ActivateAbility);
    ability.waitingForCost = true;
    EXPECT_EQ(readyRuledPendingPaymentAction({}, ability), RuledPendingPaymentAction::None);
}

TEST(RuledPendingPaymentTest, LegacyOverlapResumesTheSpellBeforeTheAbility)
{
    PendingRuledSpellCast spell;
    spell.valid = true;

    PendingActivatedAbility ability;
    ability.valid = true;

    // This overlap should disappear when the pending controller becomes exclusive. Characterize
    // the current recovery order first so an in-flight client cannot submit two commands while the
    // state is being extracted: the older code always resumes the spell and leaves the ability
    // pending for the next settled batch.
    EXPECT_EQ(readyRuledPendingPaymentAction(spell, ability), RuledPendingPaymentAction::CastSpell);
}

TEST(RuledPendingPaymentTest, ControllerKeepsSpellAndAbilityInteractionsExclusive)
{
    RuledPendingCast pending;
    pending.beginSpell().cardName = QStringLiteral("Fireball");
    EXPECT_EQ(pending.activeInteraction(), RuledPendingCast::InteractionKind::Spell);
    EXPECT_TRUE(pending.spell.valid);
    EXPECT_FALSE(pending.ability.valid);

    pending.beginAbility().cardName = QStringLiteral("Bottle Gnomes");
    EXPECT_EQ(pending.activeInteraction(), RuledPendingCast::InteractionKind::Ability);
    EXPECT_FALSE(pending.spell.valid);
    EXPECT_TRUE(pending.ability.valid);

    pending.clearAbility();
    EXPECT_EQ(pending.activeInteraction(), RuledPendingCast::InteractionKind::None);
}

TEST(RuledPendingTargetTest, ClickEligibilityUsesLatestAuthoritativeModalTargets)
{
    FakeHost host;
    RuledClientState state(&host);
    PendingRuledSpellCast spell;
    spell.valid = true;
    spell.waitingForTarget = true;
    spell.handIndex = 3;
    spell.faceIndex = 0;
    spell.activeModePosition = 0;
    spell.activeTargetGroupPosition = 0;

    RuledSpellTargetData stale;
    stale.validPermanentIds.insert(100);
    spell.selectedModes.append({7, QStringLiteral("mode"), true, stale, {}, {}});

    RuledSpellTargetData current;
    current.validPermanentIds.insert(200);
    current.groups.append(static_cast<const RuledTargetGroupData &>(current));
    RuledModalSpellOption mode{7, QStringLiteral("mode"), true, true, current};
    state.handActions[ruled::v1::HAND_ACTION_CAST_SPELL]
        .modalOptionsByCastKey[RuledClientState::handCastActionKey(3, 0, ruled::v1::CAST_METHOD_NORMAL)] = {mode};

    EXPECT_EQ(ruledTargetClickEligibility(spell, {}, state, RuledTargetCandidateKind::Battlefield, 100, 0),
              RuledTargetClickEligibility::Illegal);
    EXPECT_EQ(ruledTargetClickEligibility(spell, {}, state, RuledTargetCandidateKind::Battlefield, 200, 0),
              RuledTargetClickEligibility::Legal);
    EXPECT_EQ(ruledTargetClickEligibility(spell, {}, state, RuledTargetCandidateKind::Player, 0, 0),
              RuledTargetClickEligibility::Illegal);
}

TEST(RuledPendingTargetTest, SpellPromptUsesFaceAndActiveModeContext)
{
    FakeHost host;
    RuledClientState state(&host);
    PendingRuledSpellCast spell;
    spell.valid = true;
    spell.waitingForTarget = true;
    spell.handIndex = 3;
    spell.faceIndex = 0;
    spell.cardName = QStringLiteral("Mystic Confluence");
    spell.activeModePosition = 0;
    spell.activeTargetGroupPosition = 0;

    RuledSpellTargetData targets;
    RuledTargetGroupData group;
    group.promptText = QStringLiteral("Choose target spell.");
    group.minTargets = 1;
    group.maxTargets = 1;
    targets.groups.append(group);
    spell.selectedModes.append(
        {7, QStringLiteral("Counter target spell unless its controller pays {3}"), true, targets, {}, {}});
    state.handActions[ruled::v1::HAND_ACTION_CAST_SPELL]
        .modalOptionsByCastKey[RuledClientState::handCastActionKey(3, 0, ruled::v1::CAST_METHOD_NORMAL)] = {
        {7, QStringLiteral("Counter target spell unless its controller pays {3}"), true, true, targets}};

    EXPECT_EQ(ruledPendingSpellTargetPrompt(spell, state),
              QString::fromUtf8("Choose a target for “Counter target spell unless its controller pays {3}”.\n"
                                "Choose target spell."));
}

CardNameMap readPresentationCards(QByteArray xml)
{
    QBuffer input(&xml);
    input.open(QIODevice::ReadOnly);
    NoopCardPreferenceProvider preferences;
    NoopCardSetPriorityController priorities;
    CockatriceXml4Parser parser(&preferences, &priorities);
    CardNameMap cards;
    QObject::connect(&parser, &ICardDatabaseParser::addCard,
                     [&cards](CardInfoPtr card) { cards.insert(card->getName(), card); });
    parser.parseFile(input);
    return cards;
}

TEST(RuledPresentationResolverTest, ResolvesExactFaceLinesAndFallsBackAsOneUnit)
{
    auto cards = readPresentationCards(R"(<cockatrice_carddatabase version="4"><cards><card>
      <name>Fire // Ice</name><text>Combined text must never be used</text>
      <ruled-oracle><face card-name="Fire // Ice" face-name="Fire">
      <text>Choose one —
• First mode
• Second mode</text></face></ruled-oracle></card></cards></cockatrice_carddatabase>)");
    RuledPresentationResolver resolver([&cards](const QString &name) { return cards.value(name); });
    const QString oracleText = QStringLiteral("Choose one —\n• First mode\n• Second mode");
    ruled::v1::PresentationRef presentation;
    presentation.set_external_card_name("Fire // Ice");
    presentation.set_external_face_name("Fire");
    presentation.set_oracle_text_sha256(RuledOracleText::textSha256(oracleText).toStdString());
    presentation.add_oracle_line_indices(1);
    presentation.add_oracle_line_indices(3);
    presentation.set_fallback_text("fallback");
    EXPECT_EQ(resolver.resolve(presentation), QString::fromUtf8("Choose one —\n• Second mode"));
    presentation.set_oracle_text_sha256(std::string(64, '0'));
    EXPECT_EQ(resolver.resolve(presentation), QStringLiteral("fallback"));
    presentation.set_oracle_text_sha256(RuledOracleText::textSha256(oracleText).toStdString());
    presentation.add_oracle_line_indices(99);
    EXPECT_EQ(resolver.resolve(presentation), QStringLiteral("fallback"));
}

TEST(RuledPresentationResolverTest, EmptyMappingAlwaysUsesEngineFallback)
{
    ruled::v1::PresentationRef presentation;
    presentation.set_fallback_text("engine-authored fallback");
    RuledPresentationResolver resolver;
    EXPECT_EQ(resolver.resolve(presentation), QStringLiteral("engine-authored fallback"));
    EXPECT_TRUE(resolver.inspect(presentation).diagnostic.isEmpty());
}

TEST(RuledPresentationResolverTest, RejectsDuplicateAndDescendingMappings)
{
    CardNameMap cards;
    auto card = CardInfo::newInstance("Card");
    card->ruled().addFace("Card", "Card", "One\nTwo");
    cards.insert("Card", card);
    RuledPresentationResolver resolver([&cards](const QString &name) { return cards.value(name); });
    ruled::v1::PresentationRef presentation;
    presentation.set_external_card_name("Card");
    presentation.set_external_face_name("Card");
    presentation.set_oracle_text_sha256(RuledOracleText::textSha256("One\nTwo").toStdString());
    presentation.set_fallback_text("fallback");
    presentation.add_oracle_line_indices(2);
    presentation.add_oracle_line_indices(2);
    EXPECT_EQ(resolver.resolve(presentation), QStringLiteral("fallback"));
    presentation.set_oracle_line_indices(1, 1);
    EXPECT_EQ(resolver.resolve(presentation), QStringLiteral("fallback"));
}

TEST(RuledPresentationResolverTest, UsesDatabaseFaceTextWithoutCompanionCache)
{
    auto cards = readPresentationCards(R"(<cockatrice_carddatabase version="4"><cards><card><name>Forest</name>
      <text>Ordinary display</text><ruled-oracle><face card-name="Forest" face-name="Forest">
      <text>{T}: Add {G}.</text></face></ruled-oracle></card></cards></cockatrice_carddatabase>)");
    RuledPresentationResolver resolver([&cards](const QString &name) { return cards.value(name); });
    ruled::v1::PresentationRef presentation;
    presentation.set_external_card_name("Forest");
    presentation.set_external_face_name("Forest");
    presentation.set_oracle_text_sha256(RuledOracleText::textSha256("{T}: Add {G}.").toStdString());
    presentation.set_fallback_text("fallback");
    presentation.add_oracle_line_indices(1);
    EXPECT_EQ(resolver.resolve(presentation), QStringLiteral("{T}: Add {G}."));
    EXPECT_TRUE(resolver.inspect(presentation).diagnostic.isEmpty());
    // Lookup follows the loaded database, with no retained file or stale face snapshot.
    cards.clear();
    EXPECT_EQ(resolver.resolve(presentation), QStringLiteral("fallback"));
    EXPECT_TRUE(resolver.inspect(presentation).diagnostic.contains("Card absent"));
    cards = readPresentationCards(R"(<cockatrice_carddatabase version="4"><cards><card><name>Forest</name>
      <text>Ordinary display</text><ruled-oracle><face card-name="Forest" face-name="Forest">
      <text>Changed external wording</text></face></ruled-oracle></card></cards></cockatrice_carddatabase>)");
    EXPECT_EQ(resolver.resolve(presentation), QStringLiteral("fallback"));
    EXPECT_TRUE(resolver.inspect(presentation).diagnostic.contains("Oracle fingerprint mismatch"));
    presentation.set_oracle_text_sha256(RuledOracleText::textSha256("Changed external wording").toStdString());
    EXPECT_EQ(resolver.resolve(presentation), QStringLiteral("Changed external wording"));
}

TEST(RuledPresentationResolverTest, NormalizesLineEndingsWhitespaceHeadingsAndBullets)
{
    EXPECT_EQ(RuledOracleText::normalizedText(
                  QString::fromUtf8("  Choose two —\r\n\r\n • First mode  \r  • Second mode\n\t\nTail  ")),
              QString::fromUtf8("Choose two —\n• First mode\n• Second mode\nTail"));
}

TEST(RuledPresentationResolverTest, NormalizesMtgjsonBracketedLoyaltyLabelsForScryfallCompatibility)
{
    auto cards = readPresentationCards(R"(<cockatrice_carddatabase version="4"><cards><card>
      <name>Kaito, Bane of Nightmares</name><ruled-oracle>
      <face card-name="Kaito, Bane of Nightmares" face-name="Kaito, Bane of Nightmares"><text>Ninjutsu {1}{U}{B}
[+1]: You get an emblem.
[0]: Surveil 2.
[−2]: Tap target creature.</text></face></ruled-oracle></card></cards></cockatrice_carddatabase>)");
    RuledPresentationResolver resolver([&cards](const QString &name) { return cards.value(name); });
    const QString scryfallText =
        QString::fromUtf8("Ninjutsu {1}{U}{B}\n+1: You get an emblem.\n0: Surveil 2.\n−2: Tap target creature.");
    ruled::v1::PresentationRef presentation;
    presentation.set_external_card_name("Kaito, Bane of Nightmares");
    presentation.set_external_face_name("Kaito, Bane of Nightmares");
    presentation.set_oracle_text_sha256(RuledOracleText::textSha256(scryfallText).toStdString());
    presentation.set_fallback_text("activated_02");
    presentation.add_oracle_line_indices(2);
    EXPECT_EQ(resolver.resolve(presentation), QStringLiteral("+1: You get an emblem."));
}

TEST(RuledPresentationResolverTest, SelectsExactSplitRoomAdventureAndTransformFaces)
{
    const QList<RuledOracleTextFace> faces{
        {QStringLiteral("Fire // Ice"), QStringLiteral("Fire"), QStringLiteral("Fire deals 2 damage.")},
        {QStringLiteral("Fire // Ice"), QStringLiteral("Ice"), QStringLiteral("Tap target permanent.\nDraw a card.")},
        {QStringLiteral("Derelict Attic // Widow's Walk"), QStringLiteral("Derelict Attic"),
         QStringLiteral("Door text")},
        {QStringLiteral("Brutal Cathar // Moonrage Brute"), QStringLiteral("Moonrage Brute"),
         QStringLiteral("First strike\nWard—Pay 3 life.")},
        {QStringLiteral("Beanstalk Giant // Fertile Footsteps"), QStringLiteral("Fertile Footsteps"),
         QStringLiteral("Adventure text")},
        {QStringLiteral("Æther Adept"), QStringLiteral("Æther Adept"), QStringLiteral("Adept text")},
    };
    CardNameMap cards;
    for (const auto &face : faces) {
        QString name = face.cardName;
        if (face.faceName == QStringLiteral("Moonrage Brute")) {
            name = face.faceName;
        }
        name.replace(QStringLiteral("Æ"), QStringLiteral("AE"));
        if (!cards.contains(name)) {
            cards.insert(name, CardInfo::newInstance(name));
        }
        cards.value(name)->ruled().addFace(face.cardName, face.faceName, face.oracleText);
    }
    RuledPresentationResolver resolver([&cards](const QString &name) { return cards.value(name); });
    for (const auto &face : faces) {
        ruled::v1::PresentationRef presentation;
        presentation.set_external_card_name(face.cardName.toStdString());
        presentation.set_external_face_name(face.faceName.toStdString());
        presentation.set_oracle_text_sha256(RuledOracleText::textSha256(face.oracleText).toStdString());
        presentation.set_fallback_text("fallback");
        presentation.add_oracle_line_indices(1);
        EXPECT_EQ(resolver.resolve(presentation), face.oracleText.split('\n').first());
        presentation.set_external_face_name("Wrong face");
        EXPECT_EQ(resolver.resolve(presentation), QStringLiteral("fallback"));
    }
}

TEST(RuledPresentationResolverTest, RejectsMissingMalformedIncompatibleAndIncompleteFacesAsOneUnit)
{
    auto cards = readPresentationCards(R"(<cockatrice_carddatabase version="4"><cards><card><name>Card</name>
      <ruled-oracle><face card-name="Card" face-name="Face"><text>One
Two</text></face></ruled-oracle></card></cards></cockatrice_carddatabase>)");
    RuledPresentationResolver resolver([&cards](const QString &name) { return cards.value(name); });
    ruled::v1::PresentationRef presentation;
    presentation.set_external_card_name("Missing");
    presentation.set_external_face_name("Face");
    presentation.set_oracle_text_sha256(RuledOracleText::textSha256("One\nTwo").toStdString());
    presentation.set_fallback_text("whole fallback");
    presentation.add_oracle_line_indices(1);
    EXPECT_EQ(resolver.resolve(presentation), QStringLiteral("whole fallback"));
    presentation.set_external_card_name("Card");
    presentation.set_external_face_name("Missing");
    EXPECT_EQ(resolver.resolve(presentation), QStringLiteral("whole fallback"));
    EXPECT_TRUE(resolver.inspect(presentation).diagnostic.contains("Missing exact Oracle face data"));
    presentation.set_external_face_name("Face");
    presentation.set_oracle_text_sha256(std::string(64, 'f'));
    EXPECT_EQ(resolver.resolve(presentation), QStringLiteral("whole fallback"));
    presentation.set_oracle_text_sha256(RuledOracleText::textSha256("One\nTwo").toStdString());
    presentation.set_oracle_line_indices(0, 0);
    EXPECT_EQ(resolver.resolve(presentation), QStringLiteral("whole fallback"));
    EXPECT_TRUE(resolver.inspect(presentation).diagnostic.contains("Invalid Oracle mapping at line 0"));
    presentation.set_oracle_line_indices(0, 1);
    presentation.add_oracle_line_indices(3);
    EXPECT_EQ(resolver.resolve(presentation), QStringLiteral("whole fallback"));
    presentation.clear_oracle_line_indices();
    presentation.add_oracle_line_indices(1);
    for (const QByteArray &block :
         {QByteArray(""), QByteArray(R"(<ruled-oracle><face card-name="Card"><text>One</text></face></ruled-oracle>)"),
          QByteArray(R"(<ruled-oracle><face card-name="Card" face-name="Face"><text>One</text></face>
               <face card-name="Card" face-name="Face"><text>One</text></face></ruled-oracle>)"),
          QByteArray(R"(<ruled-oracle><face card-name="Card" face-name="Face"/></ruled-oracle>)")}) {
        cards = readPresentationCards("<cockatrice_carddatabase version=\"4\"><cards><card><name>Card</name>"
                                      "<text>One</text>" +
                                      block + "</card></cards></cockatrice_carddatabase>");
        presentation.set_oracle_text_sha256(RuledOracleText::textSha256("One").toStdString());
        EXPECT_EQ(resolver.resolve(presentation), QStringLiteral("whole fallback"));
    }
}

TEST(RuledPendingTargetTest, SpellPromptUsesCardNameWithEngineGuidance)
{
    FakeHost host;
    RuledClientState state(&host);
    PendingRuledSpellCast spell;
    spell.valid = true;
    spell.waitingForTarget = true;
    spell.handIndex = 4;
    spell.faceIndex = 0;
    spell.cardName = QStringLiteral("Deterministic spell fallback");
    spell.activeTargetGroupPosition = 0;
    RuledSpellTargetData targets;
    RuledTargetGroupData group;
    group.promptText = QStringLiteral("Choose a target controlled by an opponent.");
    targets.groups.append(group);
    state.validTargetsByHandSlot.insert(RuledClientState::spellTargetKey(4, 0), targets);

    EXPECT_EQ(ruledPendingSpellTargetPrompt(spell, state),
              QString::fromUtf8("Choose a target for “Deterministic spell fallback”.\n"
                                "Choose a target controlled by an opponent."));
}

TEST(RuledPendingTargetTest, ActivatedAbilityPromptUsesAbilityTextAndTargetGuidance)
{
    FakeHost host;
    RuledClientState state(&host);
    PendingActivatedAbility ability;
    ability.valid = true;
    ability.permanentOid = 44;
    ability.abilityIndex = 2;
    ability.abilityText = QStringLiteral("{W}, {T}: Tap target creature.");

    RuledSpellTargetData targets;
    RuledTargetGroupData group;
    group.promptText = QStringLiteral("Choose target creature an opponent controls.");
    group.minTargets = 1;
    group.maxTargets = 1;
    targets.groups.append(group);
    state.validTargetsByAbility.insert(RuledClientState::abilityTargetKey(44, 2), targets);

    EXPECT_EQ(ruledPendingAbilityTargetPrompt(ability, state),
              QString::fromUtf8("Choose a target for “{W}, {T}: Tap target creature.”\n"
                                "Choose target creature an opponent controls."));
}

TEST(RuledPendingTargetTest, ClickEligibilityCoversAbilitiesTriggersAndCopyRetargeting)
{
    FakeHost host;
    RuledClientState state(&host);
    PendingActivatedAbility ability;
    ability.valid = true;
    ability.waitingForTarget = true;
    ability.permanentOid = 44;
    ability.abilityIndex = 2;
    RuledSpellTargetData targets;
    targets.validGraveyardIds.insert(300);
    targets.canTargetOpponent = true;
    state.validTargetsByAbility.insert(RuledClientState::abilityTargetKey(44, 2), targets);

    EXPECT_EQ(ruledTargetClickEligibility({}, ability, state, RuledTargetCandidateKind::Graveyard, 300, 0),
              RuledTargetClickEligibility::Legal);
    EXPECT_EQ(ruledTargetClickEligibility({}, ability, state, RuledTargetCandidateKind::Player, 1, 0),
              RuledTargetClickEligibility::Legal);
    EXPECT_EQ(ruledTargetClickEligibility({}, ability, state, RuledTargetCandidateKind::Battlefield, 301, 0),
              RuledTargetClickEligibility::Illegal);

    ability = {};
    RuledClientState::RuledPendingChoice trigger;
    trigger.kind = RuledClientState::ChoiceKind::TriggerTarget;
    state.setPendingChoice(trigger);
    state.lastTriggerSourceOid = 55;
    state.lastTriggerAbilityIndex = 4;
    targets = {};
    targets.validStackIds.insert(400);
    state.validTargetsByAbility.insert(RuledClientState::abilityTargetKey(55, 4), targets);
    EXPECT_EQ(ruledTargetClickEligibility({}, {}, state, RuledTargetCandidateKind::Stack, 400, 0),
              RuledTargetClickEligibility::Legal);

    RuledClientState::RuledPendingChoice copy;
    copy.kind = RuledClientState::ChoiceKind::CopyTarget;
    copy.candidateOids = {500};
    copy.candidatePlayerIds = {0u, 1u};
    state.setPendingChoice(copy);
    EXPECT_EQ(ruledTargetClickEligibility({}, {}, state, RuledTargetCandidateKind::Battlefield, 500, 0),
              RuledTargetClickEligibility::Legal);
    EXPECT_EQ(ruledTargetClickEligibility({}, {}, state, RuledTargetCandidateKind::Stack, 500, 0),
              RuledTargetClickEligibility::Legal);
    EXPECT_EQ(ruledTargetClickEligibility({}, {}, state, RuledTargetCandidateKind::Graveyard, 500, 0),
              RuledTargetClickEligibility::Legal);
    EXPECT_EQ(ruledTargetClickEligibility({}, {}, state, RuledTargetCandidateKind::Graveyard, 501, 0),
              RuledTargetClickEligibility::Illegal);
    EXPECT_EQ(ruledTargetClickEligibility({}, {}, state, RuledTargetCandidateKind::Player, 0, 0),
              RuledTargetClickEligibility::Legal);
    EXPECT_EQ(ruledTargetClickEligibility({}, {}, state, RuledTargetCandidateKind::Player, 1, 0),
              RuledTargetClickEligibility::Legal);
    // Client-side identity domains stay separate even under a forged numeric collision. Engine
    // candidate object and player IDs do not collide this way in a valid offer.
    EXPECT_EQ(ruledTargetClickEligibility({}, {}, state, RuledTargetCandidateKind::Player, 500, 0),
              RuledTargetClickEligibility::Illegal);
    EXPECT_EQ(ruledTargetClickEligibility({}, {}, state, RuledTargetCandidateKind::Graveyard, 0, 0),
              RuledTargetClickEligibility::Illegal);
}

TEST_F(RuledClientTest, PendingCopyTargetOffersOpenOnlyOfferedOwnersGraveyardUntilStackCopyResolves)
{
    QSignalSpy spy(state, &RuledClientState::graveyardTargetsNeeded);

    const auto addGraveyardMap = [](ruled::v1::RuledEventBatch &batch) {
        auto *map = batch.add_events()->mutable_graveyard_object_map();
        auto *local = map->add_entries();
        local->set_player_id(kLocalPlayer);
        local->set_engine_object_id(500);
        local->set_server_card_id(11);
        auto *opponent = map->add_entries();
        opponent->set_player_id(kOpponent);
        opponent->set_engine_object_id(501);
        opponent->set_server_card_id(11); // Owner-local card ids can repeat across graveyards.
    };
    const auto addPrompt = [](ruled::v1::RuledEventBatch &batch, int decider, quint64 promptId,
                              const QVector<quint32> &objects, const QVector<int> &players) {
        auto *snapshot = batch.mutable_resolution_prompt_state();
        snapshot->set_active(true);
        snapshot->set_deciding_player_id(decider);
        if (decider == kLocalPlayer)
            snapshot->set_resolution_prompt_id(promptId);

        auto *choice = batch.add_events()->mutable_resolution_choice_required();
        choice->set_deciding_player_id(decider);
        choice->set_choice_kind(ruled::v1::CHOICE_KIND_TARGET_OBJECTS);
        if (decider == kLocalPlayer)
            choice->set_resolution_prompt_id(promptId);
        choice->set_min(0);
        choice->set_max(1);
        choice->set_prompt_text("Choose a new target for the copy or keep the original.");
        for (quint32 oid : objects) {
            choice->add_candidate_object_ids(oid);
            choice->add_candidate_card_ids("test_card");
            choice->add_candidate_names("Test card");
        }
        for (int playerId : players)
            choice->add_candidate_player_ids(playerId);
    };
    const auto addInactivePrompt = [](ruled::v1::RuledEventBatch &batch) {
        auto *snapshot = batch.mutable_resolution_prompt_state();
        snapshot->set_active(false);
        snapshot->set_deciding_player_id(kLocalPlayer);
    };

    // The valid RCR has aligned object metadata and an active, exact local prompt ID. Only the
    // opponent's graveyard OID is offered although both owners have the same server card ID.
    ruled::v1::RuledEventBatch promptBatch;
    addGraveyardMap(promptBatch);
    addPrompt(promptBatch, kLocalPlayer, 9401, {501u}, {kLocalPlayer});
    applyRaw(promptBatch);
    ASSERT_GE(spy.count(), 1);
    EXPECT_EQ(spy.last().at(0).value<QList<int>>(), QList<int>{kOpponent});

    // Once that occurrence is chosen, its copy stack object keeps only the latched graveyard open.
    ruled::v1::RuledEventBatch copyPushBatch;
    addInactivePrompt(copyPushBatch);
    addGraveyardMap(copyPushBatch);
    auto *copy = copyPushBatch.add_events()->mutable_stack_pushed();
    copy->set_object_id(900);
    copy->set_is_copy(true);
    copy->add_targets()->set_object_id(501);
    copy->mutable_targets(0)->set_kind(ruled::v1::TARGET_REF_KIND_GRAVEYARD);
    applyRaw(copyPushBatch);
    EXPECT_FALSE(state->hasPendingChoiceOfKind(RuledClientState::ChoiceKind::CopyTarget));
    EXPECT_EQ(spy.last().at(0).value<QList<int>>(), QList<int>{kOpponent});

    ruled::v1::RuledEventBatch resolvedBatch;
    addInactivePrompt(resolvedBatch);
    addGraveyardMap(resolvedBatch);
    resolvedBatch.add_events()->mutable_stack_resolved()->set_object_id(900);
    applyRaw(resolvedBatch);
    EXPECT_TRUE(spy.last().at(0).value<QList<int>>().isEmpty());

    // Typed player candidates (including player 0) and an observer's offer never name a graveyard.
    ruled::v1::RuledEventBatch playerOnlyBatch;
    addGraveyardMap(playerOnlyBatch);
    addPrompt(playerOnlyBatch, kLocalPlayer, 9402, {}, {kLocalPlayer});
    applyRaw(playerOnlyBatch);
    EXPECT_TRUE(spy.last().at(0).value<QList<int>>().isEmpty());

    ruled::v1::RuledEventBatch observerBatch;
    addGraveyardMap(observerBatch);
    addPrompt(observerBatch, kOpponent, 0, {501u}, {});
    applyRaw(observerBatch);
    EXPECT_TRUE(spy.last().at(0).value<QList<int>>().isEmpty());
}

TEST(RuledXTargetChoiceTest, PublishesOnlyEngineBoundValuesAndGenerations)
{
    RuledTargetGroupData group;
    group.hasXTargetChoices = true;
    RuledXTargetChoiceData zero;
    zero.xValue = 0;
    zero.candidateGenerations.insert(41, 3);
    RuledXTargetChoiceData four;
    four.xValue = 4;
    four.candidateGenerations.insert(42, 9);
    group.xTargetChoices = {zero, four};

    EXPECT_EQ(ruledAvailableXTargetValues(group), QVector<quint32>({0, 4}));
    EXPECT_EQ(ruledXTargetCandidateGeneration(group, 0, 41), std::optional<quint64>(3));
    EXPECT_EQ(ruledXTargetCandidateGeneration(group, 4, 42), std::optional<quint64>(9));
    EXPECT_FALSE(ruledXTargetCandidateGeneration(group, 4, 41).has_value());
}

TEST(RuledXTargetChoiceTest, ActivationTargetRequiresTheChosenValueAndCurrentIncarnation)
{
    FakeHost host;
    RuledClientState state(&host);
    RuledTargetGroupData group;
    group.groupIndex = 0;
    group.validPermanentIds.insert(42);
    group.hasXTargetChoices = true;
    RuledXTargetChoiceData choice;
    choice.xValue = 4;
    choice.candidateGenerations.insert(42, 9);
    group.xTargetChoices.append(choice);
    RuledSpellTargetData targets;
    targets.groups.append(group);
    state.validTargetsByAbility.insert(RuledClientState::abilityTargetKey(44, 2), targets);
    state.battlefieldGenerationByOid.insert(42, 9);

    PendingActivatedAbility ability;
    ability.valid = true;
    ability.waitingForTarget = true;
    ability.permanentOid = 44;
    ability.abilityIndex = 2;
    ability.xValue = 4;
    EXPECT_EQ(ruledTargetClickEligibility({}, ability, state, RuledTargetCandidateKind::Battlefield, 42, 0),
              RuledTargetClickEligibility::Legal);
    ability.xValue = 0;
    EXPECT_EQ(ruledTargetClickEligibility({}, ability, state, RuledTargetCandidateKind::Battlefield, 42, 0),
              RuledTargetClickEligibility::Illegal);

    ability.xValue = 4;
    state.battlefieldGenerationByOid.insert(42, 10);
    EXPECT_EQ(ruledTargetClickEligibility({}, ability, state, RuledTargetCandidateKind::Battlefield, 42, 0),
              RuledTargetClickEligibility::Illegal);

    PendingActivatedAbility::Target selected;
    selected.ref.set_object_id(42);
    selected.ref.set_group_index(0);
    selected.ref.set_kind(ruled::v1::TARGET_REF_KIND_PERMANENT);
    selected.ref.set_expected_zone_change_generation(9);
    selected.zoneChangeGeneration = 9;
    ability.selectedTargets.append(selected);
    PendingRuledSpellCast spell;
    EXPECT_TRUE(reconcileRuledPendingTargets(spell, ability, state, 0));
    EXPECT_TRUE(ability.selectedTargets.isEmpty());
    EXPECT_TRUE(ability.waitingForTarget);
}

TEST(RuledXTargetChoiceTest, StaleTargetAfterTargetCostApplicationRequiresActivationRestart)
{
    FakeHost host;
    RuledClientState state(&host);
    RuledTargetGroupData group;
    group.groupIndex = 0;
    group.validPermanentIds.insert(42);
    group.hasXTargetChoices = true;
    RuledXTargetChoiceData currentChoice;
    currentChoice.xValue = 4;
    currentChoice.candidateGenerations.insert(42, 10);
    group.xTargetChoices.append(currentChoice);
    RuledSpellTargetData targets;
    targets.groups.append(group);
    state.validTargetsByAbility.insert(RuledClientState::abilityTargetKey(44, 2), targets);
    state.battlefieldGenerationByOid.insert(42, 10);

    PendingActivatedAbility ability;
    ability.valid = true;
    ability.waitingForTarget = true;
    ability.waitingForMana = true;
    ability.targetingCostApplied = true;
    ability.remainingCost.insert(QChar('X'), 3);
    ability.permanentOid = 44;
    ability.abilityIndex = 2;
    ability.xValue = 4;
    PendingActivatedAbility::Target selected;
    selected.ref.set_object_id(42);
    selected.ref.set_group_index(0);
    selected.ref.set_kind(ruled::v1::TARGET_REF_KIND_PERMANENT);
    selected.ref.set_expected_zone_change_generation(9);
    selected.zoneChangeGeneration = 9;
    ability.selectedTargets.append(selected);
    PendingRuledSpellCast spell;

    EXPECT_TRUE(reconcileRuledPendingTargets(spell, ability, state, 0));
    EXPECT_TRUE(ability.selectedTargets.isEmpty());
    EXPECT_TRUE(ability.waitingForTarget);
    EXPECT_TRUE(ability.restartAfterTargetInvalidation);
    EXPECT_TRUE(ability.targetingCostApplied);
    EXPECT_EQ(ability.remainingCost.value(QChar('X')), 3);
}

TEST(RuledActivatedAbilityModeTest, TargetClicksUseTheSelectedModeOffer)
{
    FakeHost host;
    RuledClientState state(&host);
    constexpr quint32 sourceOid = 0x502u;
    constexpr int abilityIndex = 1;
    constexpr quint32 targetOid = 0x503u;
    constexpr quint64 targetGeneration = 17;

    RuledTargetGroupData group;
    group.groupIndex = 0;
    group.minTargets = 1;
    group.maxTargets = 1;
    group.validPermanentIds.insert(targetOid);
    group.validPermanentGenerations.insert(targetOid, targetGeneration);
    RuledModalSpellOption mode;
    mode.modeIndex = 2;
    mode.label = QStringLiteral("Return a tapped creature");
    mode.selectable = true;
    mode.needsTarget = true;
    mode.targets.groups.append(group);

    RuledActivatedAbilityModeOffer offer;
    offer.minModes = 1;
    offer.maxModes = 1;
    offer.modes.append(mode);
    state.activatedAbilityModesByAbility.insert(RuledClientState::abilityTargetKey(sourceOid, abilityIndex), offer);
    state.battlefieldGenerationByOid.insert(targetOid, targetGeneration);

    PendingActivatedAbility ability;
    ability.valid = true;
    ability.waitingForTarget = true;
    ability.permanentOid = sourceOid;
    ability.abilityIndex = abilityIndex;
    ability.selectedMode = mode;

    EXPECT_EQ(ruledTargetClickEligibility({}, ability, state, RuledTargetCandidateKind::Battlefield, targetOid, 0),
              RuledTargetClickEligibility::Legal);
}

TEST(RuledActivatedAbilityModeTest, PendingModeMustRemainInTheLatestOffer)
{
    FakeHost host;
    RuledClientState state(&host);
    constexpr quint32 sourceOid = 0x512u;
    constexpr int abilityIndex = 2;
    state.abilitySourceGenerationByOid.insert(sourceOid, 9);
    state.zoneAbilityIndicesByOid[sourceOid].insert(abilityIndex);

    RuledModalSpellOption mode;
    mode.modeIndex = 1;
    mode.label = QStringLiteral("Targetless mode");
    mode.selectable = true;
    RuledActivatedAbilityModeOffer offer;
    offer.minModes = 1;
    offer.maxModes = 1;
    offer.modes.append(mode);
    const quint64 abilityKey = RuledClientState::abilityTargetKey(sourceOid, abilityIndex);
    state.activatedAbilityModesByAbility.insert(abilityKey, offer);

    PendingActivatedAbility ability;
    ability.valid = true;
    ability.permanentOid = sourceOid;
    ability.abilityIndex = abilityIndex;
    ability.expectedZoneChangeGeneration = 9;
    ability.selectedMode = mode;
    EXPECT_TRUE(ruledPendingAbilitySourceStillCurrent(state, ability));

    state.activatedAbilityModesByAbility.remove(abilityKey);
    EXPECT_FALSE(ruledPendingAbilitySourceStillCurrent(state, ability));
}

TEST(RuledActivatedAbilityModeTest, SubmittedActivationSurvivesReplacementOfTheLiveModeOffer)
{
    FakeHost host;
    RuledClientState state(&host);
    constexpr quint32 sourceOid = 0x517u;
    constexpr int abilityIndex = 4;
    constexpr quint64 sourceGeneration = 12;
    RuledModalSpellOption mode;
    mode.modeIndex = 2;
    mode.label = QStringLiteral("Return a tapped creature");
    mode.selectable = true;

    PendingActivatedAbility ability;
    ability.valid = true;
    ASSERT_TRUE(ability.beginCommit());
    ability.permanentOid = sourceOid;
    ability.abilityIndex = abilityIndex;
    ability.expectedZoneChangeGeneration = sourceGeneration;
    ability.selectedMode = mode;
    EXPECT_TRUE(ruledPendingAbilitySourceStillCurrent(state, ability));

    ruled::v1::PendingAbilityActivation pending;
    pending.set_transaction_id(81);
    pending.set_source_object_id(sourceOid);
    pending.set_source_zone_change_generation(sourceGeneration);
    pending.set_ability_index(abilityIndex);
    pending.set_stage(ruled::v1::ABILITY_ACTIVATION_STAGE_PAYMENT);
    state.pendingAbilityActivation = pending;
    ability.applyEngineSnapshot(*state.pendingAbilityActivation);
    EXPECT_TRUE(ruledPendingAbilitySourceStillCurrent(state, ability));

    state.pendingAbilityActivation->set_transaction_id(82);
    EXPECT_FALSE(ruledPendingAbilitySourceStillCurrent(state, ability));

    state.pendingAbilityActivation->set_transaction_id(81);
    ability.applyEngineSnapshot(*state.pendingAbilityActivation);
    EXPECT_TRUE(ruledPendingAbilitySourceStillCurrent(state, ability));
}

TEST(RuledActivatedAbilityModeTest, ChangedTargetGenerationInvalidatesTheCapturedModeReceipt)
{
    FakeHost host;
    RuledClientState state(&host);
    constexpr quint32 sourceOid = 0x522u;
    constexpr int abilityIndex = 3;
    constexpr quint32 targetOid = 0x523u;
    constexpr quint64 capturedGeneration = 19;
    constexpr quint64 latestGeneration = 20;
    state.abilitySourceGenerationByOid.insert(sourceOid, 11);
    state.zoneAbilityIndicesByOid[sourceOid].insert(abilityIndex);
    state.battlefieldGenerationByOid.insert(targetOid, latestGeneration);

    RuledTargetGroupData latestGroup;
    latestGroup.groupIndex = 0;
    latestGroup.minTargets = 1;
    latestGroup.maxTargets = 1;
    latestGroup.validPermanentIds.insert(targetOid);
    latestGroup.validPermanentGenerations.insert(targetOid, latestGeneration);
    RuledModalSpellOption latestMode;
    latestMode.modeIndex = 2;
    latestMode.label = QStringLiteral("Return a tapped creature");
    latestMode.selectable = true;
    latestMode.needsTarget = true;
    latestMode.targets.groups.append(latestGroup);
    RuledActivatedAbilityModeOffer offer;
    offer.minModes = 1;
    offer.maxModes = 1;
    offer.modes.append(latestMode);
    state.activatedAbilityModesByAbility.insert(RuledClientState::abilityTargetKey(sourceOid, abilityIndex), offer);

    RuledModalSpellOption capturedMode = latestMode;
    capturedMode.targets.groups[0].validPermanentGenerations.insert(targetOid, capturedGeneration);
    PendingActivatedAbility ability;
    ability.valid = true;
    ability.waitingForTarget = false;
    ability.targetingCostApplied = true;
    ability.permanentOid = sourceOid;
    ability.abilityIndex = abilityIndex;
    ability.expectedZoneChangeGeneration = 11;
    ability.selectedMode = capturedMode;
    PendingActivatedAbility::Target selected;
    selected.ref.set_object_id(targetOid);
    selected.ref.set_group_index(0);
    selected.ref.set_kind(ruled::v1::TARGET_REF_KIND_PERMANENT);
    selected.ref.set_expected_zone_change_generation(capturedGeneration);
    selected.zoneChangeGeneration = capturedGeneration;
    ability.selectedTargets.append(selected);

    PendingRuledSpellCast spell;
    EXPECT_TRUE(reconcileRuledPendingTargets(spell, ability, state, kLocalPlayer));
    EXPECT_TRUE(ability.selectedTargets.isEmpty());
    EXPECT_TRUE(ability.waitingForTarget);
    EXPECT_TRUE(ability.restartAfterTargetInvalidation);
    ASSERT_TRUE(ability.selectedMode.has_value());
    EXPECT_EQ(ability.selectedMode->targets.groups[0].validPermanentGenerations.value(targetOid),
              capturedGeneration);
}

TEST(RuledXTargetChoiceTest, AbilityMenuDisablesAnEnginePublishedEmptyChoiceSet)
{
    FakeHost host;
    RuledClientState state(&host);
    RuledAbilityEntry ability;
    ability.text = QStringLiteral("{X}, {T}: Copy target artifact.");
    ability.manaCost = QStringLiteral("X");
    ability.activatable = true;
    state.activatedAbilitiesByOid.insert(44, RuledAbilityEntries{ability});

    RuledSpellTargetData targets;
    RuledTargetGroupData group;
    group.hasXTargetChoices = true;
    targets.groups.append(group);
    state.validTargetsByAbility.insert(RuledClientState::abilityTargetKey(44, 0), targets);
    auto options = RuledPendingCast::cardActionMenuOptions({}, state, 44);
    ASSERT_EQ(options.size(), 1);
    EXPECT_FALSE(options.first().enabled);

    RuledXTargetChoiceData choice;
    choice.xValue = 0;
    choice.candidateGenerations.insert(42, 7);
    targets.groups[0].xTargetChoices.append(choice);
    state.validTargetsByAbility.insert(RuledClientState::abilityTargetKey(44, 0), targets);
    options = RuledPendingCast::cardActionMenuOptions({}, state, 44);
    ASSERT_EQ(options.size(), 1);
    EXPECT_TRUE(options.first().enabled);
}

TEST(RuledPendingTargetTest, CastCostObjectEligibilityUsesThePublishedOptionCandidates)
{
    PendingRuledSpellCast spell;
    spell.valid = true;
    spell.waitingForCastCostObject = true;
    spell.nextCastCostGroup = 0;
    spell.activeCastCostOption = 3;

    RuledCastCostOption behold;
    behold.optionIndex = 3;
    behold.kind = RuledCastCostOptionKind::Behold;
    behold.selectable = true;
    behold.validHandIndices = {7};
    behold.validPermanentIds = {900};
    RuledCastCostGroup group;
    group.options = {behold};
    spell.castCostGroups = {group};

    EXPECT_EQ(ruledCastCostObjectEligibility(spell, RuledCastCostCandidateKind::Hand, 7),
              RuledTargetClickEligibility::Legal);
    EXPECT_EQ(ruledCastCostObjectEligibility(spell, RuledCastCostCandidateKind::Hand, 8),
              RuledTargetClickEligibility::Illegal);
    EXPECT_EQ(ruledCastCostObjectEligibility(spell, RuledCastCostCandidateKind::Permanent, 900),
              RuledTargetClickEligibility::Legal);

    spell.waitingForCastCostObject = false;
    EXPECT_EQ(ruledCastCostObjectEligibility(spell, RuledCastCostCandidateKind::Hand, 7),
              RuledTargetClickEligibility::NotTargeting);
}

TEST(RuledPendingTargetTest, DiscardCastCostAcceptsOnlyPublishedHandCandidates)
{
    PendingRuledSpellCast spell;
    spell.valid = true;
    spell.waitingForCastCostObject = true;
    spell.nextCastCostGroup = 0;
    spell.activeCastCostOption = 4;

    RuledCastCostOption discard;
    discard.optionIndex = 4;
    discard.kind = RuledCastCostOptionKind::DiscardCard;
    discard.selectable = true;
    discard.validHandIndices = {7};
    discard.validPermanentIds = {900};
    RuledCastCostGroup group;
    group.options = {discard};
    spell.castCostGroups = {group};

    EXPECT_TRUE(ruledCastCostUsesHandChoice(discard.kind));
    EXPECT_TRUE(ruledCastCostUsesObjectChoice(discard.kind));
    EXPECT_EQ(ruledCastCostObjectEligibility(spell, RuledCastCostCandidateKind::Hand, 7),
              RuledTargetClickEligibility::Legal);
    EXPECT_EQ(ruledCastCostObjectEligibility(spell, RuledCastCostCandidateKind::Hand, 8),
              RuledTargetClickEligibility::Illegal);
    EXPECT_EQ(ruledCastCostObjectEligibility(spell, RuledCastCostCandidateKind::Permanent, 900),
              RuledTargetClickEligibility::Illegal);
    EXPECT_FALSE(ruledCastCostUsesObjectChoice(RuledCastCostOptionKind::PayLife));
}

TEST(RuledPendingTargetTest, HarmonizeCastCostOnlyAcceptsPublishedPermanentCandidates)
{
    PendingRuledSpellCast spell;
    spell.valid = true;
    spell.waitingForCastCostObject = true;
    spell.nextCastCostGroup = 0;
    spell.activeCastCostOption = 0;

    RuledCastCostOption harmonize;
    harmonize.optionIndex = 0;
    harmonize.kind = RuledCastCostOptionKind::TapPermanentForGenericReduction;
    harmonize.selectable = true;
    harmonize.validHandIndices = {7};
    harmonize.validPermanentIds = {900};
    RuledCastCostGroup group;
    group.options = {harmonize};
    spell.castCostGroups = {group};

    EXPECT_EQ(ruledCastCostObjectEligibility(spell, RuledCastCostCandidateKind::Hand, 7),
              RuledTargetClickEligibility::Illegal);
    EXPECT_EQ(ruledCastCostObjectEligibility(spell, RuledCastCostCandidateKind::Permanent, 900),
              RuledTargetClickEligibility::Legal);
    EXPECT_EQ(ruledCastCostObjectEligibility(spell, RuledCastCostCandidateKind::Permanent, 901),
              RuledTargetClickEligibility::Illegal);
}

TEST(RuledPendingTargetTest, HarmonizeCostIsNotFinalizedWhileItsOptionChoiceIsPending)
{
    PendingRuledSpellCast spell;
    spell.valid = true;
    spell.remainingCost[QLatin1Char('X')] = 5;
    spell.nextCastCostGroup = 0;

    RuledCastCostOption harmonize;
    harmonize.optionIndex = 0;
    harmonize.kind = RuledCastCostOptionKind::TapPermanentForGenericReduction;
    harmonize.selectable = true;
    RuledCastCostGroup group;
    group.options = {harmonize};
    spell.castCostGroups = {group};

    EXPECT_FALSE(ruledCastCostGroupsComplete(spell));

    spell.castCostGenericReduction = 2;
    spell.nextCastCostGroup = 1;
    EXPECT_TRUE(ruledCastCostGroupsComplete(spell));
    EXPECT_EQ(ruledFinalGenericCost(spell.remainingCost.value(QLatin1Char('X')), 0, spell.castCostGenericReduction), 3);
}

TEST(RuledPendingTargetTest, OptionalGroupCanSkipButDoesNotAutoAdvanceWithoutASelection)
{
    PendingRuledSpellCast spell;
    spell.valid = true;
    RuledCastCostGroup group;
    group.groupIndex = 7;
    group.min = 0;
    group.max = 1;
    spell.castCostGroups = {group};
    EXPECT_TRUE(ruledCastCostGroupCanConfirm(spell, group));
    EXPECT_FALSE(ruledCastCostGroupSelectionCompletesImmediately(spell, group));
    EXPECT_FALSE(ruledCastCostGroupsComplete(spell));
    spell.nextCastCostGroup = 1;
    EXPECT_TRUE(ruledCastCostGroupsComplete(spell));
}

TEST(RuledPendingPaymentTest, ClearingAnInteractionRemovesItsSubmissionAndCostStaging)
{
    RuledPendingCast pending;
    auto &spell = pending.beginSpell();
    spell.submissionPending = true;
    spell.waitingForCastCostObject = true;
    spell.castingPermissionId = 77;
    spell.castCostSelections.append({2, 3});
    pending.clearSpell();
    EXPECT_EQ(pending.activeInteraction(), RuledPendingCast::InteractionKind::None);
    EXPECT_FALSE(pending.spell.submissionPending);
    EXPECT_FALSE(pending.spell.waitingForCastCostObject);
    EXPECT_TRUE(pending.spell.castCostSelections.isEmpty());
    EXPECT_EQ(pending.spell.castingPermissionId, 0u);
    auto &ability = pending.beginAbility();
    ability.waitingForCost = true;
    ability.costSelections.append({4, RuledCostChoiceZone::Battlefield, {42}, {9}});
    pending.clearAbility();
    EXPECT_FALSE(pending.ability.waitingForCost);
    EXPECT_TRUE(pending.ability.costSelections.isEmpty());
    EXPECT_EQ(pending.activeInteraction(), RuledPendingCast::InteractionKind::None);
}

TEST(RuledPendingTargetTest, BoundedCastCostGroupStaysOpenUntilItsSelectionCountIsConfirmed)
{
    PendingRuledSpellCast spell;
    spell.valid = true;
    spell.nextCastCostGroup = 0;

    RuledCastCostGroup group;
    group.groupIndex = 4;
    group.min = 1;
    group.max = 2;
    group.options = {{10, QStringLiteral("First"), RuledCastCostOptionKind::Mana},
                     {11, QStringLiteral("Second"), RuledCastCostOptionKind::Mana}};
    spell.castCostGroups = {group};

    EXPECT_EQ(ruledCastCostGroupSelectionCount(spell, 4), 0);
    EXPECT_FALSE(ruledCastCostGroupCanConfirm(spell, group));

    spell.castCostSelections.append({4, 10, RuledPendingCastCostSelection::ObjectKind::None, 0, 0, 0});
    EXPECT_EQ(ruledCastCostGroupSelectionCount(spell, 4), 1);
    EXPECT_TRUE(ruledCastCostOptionAlreadySelected(spell, 4, 10));
    EXPECT_TRUE(ruledCastCostGroupCanConfirm(spell, group));
    EXPECT_FALSE(ruledCastCostGroupSelectionCompletesImmediately(spell, group));
    EXPECT_FALSE(ruledCastCostGroupsComplete(spell));

    spell.castCostSelections.append({4, 11, RuledPendingCastCostSelection::ObjectKind::None, 0, 0, 0});
    EXPECT_EQ(ruledCastCostGroupSelectionCount(spell, 4), 2);
    EXPECT_TRUE(ruledCastCostGroupCanConfirm(spell, group));
    EXPECT_FALSE(ruledCastCostGroupSelectionCompletesImmediately(spell, group));

    spell.nextCastCostGroup = 1;
    EXPECT_TRUE(ruledCastCostGroupsComplete(spell));
}

TEST(RuledPendingTargetTest, SingleCastCostChoiceCompletesImmediately)
{
    PendingRuledSpellCast spell;
    spell.valid = true;

    RuledCastCostGroup group;
    group.groupIndex = 7;
    group.min = 1;
    group.max = 1;
    spell.castCostSelections.append({7, 1, RuledPendingCastCostSelection::ObjectKind::None, 0, 0, 0});

    EXPECT_TRUE(ruledCastCostGroupSelectionCompletesImmediately(spell, group));
}

TEST(RuledPendingTargetTest, SingleRequiredCastCostObjectCompletesOnItsClick)
{
    RuledCastCostOption sacrifice;
    sacrifice.kind = RuledCastCostOptionKind::SacrificePermanent;
    sacrifice.objectMin = 1;
    sacrifice.objectMax = 1;

    EXPECT_FALSE(ruledCastCostObjectSelectionCompletesImmediately(sacrifice, 0, true));
    EXPECT_FALSE(ruledCastCostObjectSelectionCompletesImmediately(sacrifice, 1, false));
    EXPECT_TRUE(ruledCastCostObjectSelectionCompletesImmediately(sacrifice, 1, true));

    sacrifice.objectMax = 2;
    EXPECT_FALSE(ruledCastCostObjectSelectionCompletesImmediately(sacrifice, 1, true));
}

TEST(RuledPendingTargetTest, PermanentChoicePromptKeepsEngineSpecificityAndAddsClickGuidance)
{
    EXPECT_EQ(formatRuledPermanentChoicePrompt(QStringLiteral("Choose a creature you control.")),
              QStringLiteral("Choose a creature you control.\nClick a highlighted permanent on the battlefield."));
}

TEST(RuledPendingTargetTest, ReconcileDropsTargetsMissingFromLatestLegalSnapshot)
{
    FakeHost host;
    RuledClientState state(&host);
    PendingRuledSpellCast spell;
    spell.valid = true;
    spell.waitingForTarget = true;
    spell.handIndex = 6;
    spell.activeTargetGroupPosition = 0;
    spell.selectedTargetOids = {10, 20};
    spell.selectedTargetDamages = {1, 2};
    spell.selectedTargetOidsByGroup = {{10, 20}};
    spell.selectedTargetDamagesByGroup = {{1, 2}};
    spell.targetDamageAllocations = {1, 2};
    RuledSpellTargetData current;
    current.validPermanentIds.insert(20);
    current.groups.append(static_cast<const RuledTargetGroupData &>(current));
    state.validTargetsByHandSlot.insert(RuledClientState::spellTargetKey(6, 0), current);

    PendingActivatedAbility ability;
    EXPECT_TRUE(reconcileRuledPendingTargets(spell, ability, state, 0));
    EXPECT_EQ(spell.selectedTargetOids, QVector<quint32>({20}));
    EXPECT_EQ(spell.selectedTargetDamages, QVector<quint32>({2}));
    EXPECT_EQ(spell.targetDamageAllocations, QVector<int>({2}));
}

TEST(RuledPendingTargetTest, ReconcileRepairsBothLegacyPendingFamiliesInOneLegalRefresh)
{
    FakeHost host;
    RuledClientState state(&host);

    PendingRuledSpellCast spell;
    spell.valid = true;
    spell.handIndex = 2;
    spell.activeTargetGroupPosition = 0;
    spell.selectedTargetOids = {10};
    spell.selectedTargetOidsByGroup = {{10}};

    PendingActivatedAbility ability;
    ability.valid = true;
    ability.permanentOid = 30;
    ability.abilityIndex = 1;
    ability.selectedTargets.append(PendingActivatedAbility::Target{});
    ability.selectedTargets.last().ref.set_object_id(40);

    RuledSpellTargetData noSpellCandidates;
    noSpellCandidates.groups.append(static_cast<const RuledTargetGroupData &>(noSpellCandidates));
    state.validTargetsByHandSlot.insert(RuledClientState::spellTargetKey(2, 0), noSpellCandidates);
    state.validTargetsByAbility.insert(RuledClientState::abilityTargetKey(30, 1), {});

    EXPECT_TRUE(reconcileRuledPendingTargets(spell, ability, state, 0));
    EXPECT_TRUE(spell.selectedTargetOids.isEmpty());
    EXPECT_TRUE(ability.selectedTargets.isEmpty());
}

TEST(RuledPendingTargetTest, EngineOwnedActivationSurvivesSuppressedOrdinaryOffers)
{
    FakeHost host;
    RuledClientState state(&host);
    auto &engine = state.pendingAbilityActivation.emplace();
    engine.set_transaction_id(91);
    engine.set_revision(3);
    engine.set_actor_player_id(kLocalPlayer);
    engine.set_source_object_id(30);
    engine.set_source_zone_change_generation(4);
    engine.set_ability_index(1);
    engine.set_stage(ruled::v1::ABILITY_ACTIVATION_STAGE_PAYMENT);
    PendingActivatedAbility ability;
    ability.valid = true;
    ability.permanentOid = 30;
    ability.expectedZoneChangeGeneration = 4;
    ability.abilityIndex = 1;
    ability.applyEngineSnapshot(*state.pendingAbilityActivation);
    ability.selectedTargets.append(PendingActivatedAbility::Target{});
    ability.selectedTargets.last().ref.set_object_id(40);
    EXPECT_TRUE(ruledPendingAbilitySourceStillCurrent(state, ability));
    PendingRuledSpellCast spell;
    EXPECT_FALSE(reconcileRuledPendingTargets(spell, ability, state, kLocalPlayer));
    ASSERT_EQ(ability.selectedTargets.size(), 1);
    EXPECT_EQ(ability.selectedTargets.first().ref.object_id(), 40u);
}

TEST(RuledPendingTargetTest, WaitingActivationCannotAutoSubmitAnEmptyLocalCost)
{
    PendingActivatedAbility ability;
    ability.valid = true;
    ability.chosenOpponentTargets = true;
    ability.applyEngineSnapshot(localWaitingActivationSnapshot(91, 3));
    EXPECT_EQ(readyRuledPendingPaymentAction({}, ability), RuledPendingPaymentAction::None);
}

TEST(RuledPendingTargetTest, AbilityTargetInvalidationRewindsItsRequiredGroup)
{
    FakeHost host;
    RuledClientState state(&host);
    RuledSpellTargetData data;
    data.groups.append(RuledTargetGroupData{});
    data.groups[0].validPermanentIds = {41};
    state.validTargetsByAbility.insert(RuledClientState::abilityTargetKey(30, 1), data);
    PendingActivatedAbility ability;
    ability.valid = true;
    ability.permanentOid = 30;
    ability.abilityIndex = 1;
    ability.activeTargetGroupPosition = 1;
    ability.selectedTargets.append(PendingActivatedAbility::Target{});
    ability.selectedTargets.last().ref.set_object_id(40);
    PendingRuledSpellCast spell;
    ASSERT_TRUE(reconcileRuledPendingTargets(spell, ability, state, kLocalPlayer));
    EXPECT_EQ(ability.activeTargetGroupPosition, 0);
    EXPECT_TRUE(ability.waitingForTarget);
    EXPECT_EQ(
        ruledTargetClickEligibility(spell, ability, state, RuledTargetCandidateKind::Battlefield, 41, kLocalPlayer),
        RuledTargetClickEligibility::Legal);
}

TEST(RuledPendingTargetTest, RejectedNonModalActivationRetriesOnlyWhenRetargetingIsNotRequired)
{
    FakeHost host;
    RuledClientState state(&host);
    constexpr quint32 sourceOid = 30;
    constexpr int abilityIndex = 1;
    constexpr quint32 offeredTargetOid = 41;
    state.abilitySourceGenerationByOid.insert(sourceOid, 7);
    state.zoneAbilityIndicesByOid[sourceOid].insert(abilityIndex);

    RuledSpellTargetData data;
    data.groups.append(RuledTargetGroupData{});
    data.groups[0].groupIndex = 0;
    data.groups[0].minTargets = 1;
    data.groups[0].maxTargets = 1;
    data.groups[0].validPermanentIds.insert(offeredTargetOid);
    state.validTargetsByAbility.insert(RuledClientState::abilityTargetKey(sourceOid, abilityIndex), data);

    const auto pendingWithTarget = [&](quint32 targetOid) {
        PendingActivatedAbility ability;
        ability.valid = true;
        ability.permanentOid = sourceOid;
        ability.expectedZoneChangeGeneration = 7;
        ability.abilityIndex = abilityIndex;
        ability.activeTargetGroupPosition = 0;
        PendingActivatedAbility::Target target;
        target.ref.set_object_id(targetOid);
        target.ref.set_group_index(0);
        target.ref.set_kind(ruled::v1::TARGET_REF_KIND_PERMANENT);
        ability.selectedTargets.append(target);
        return ability;
    };

    PendingRuledSpellCast spell;
    auto stale = pendingWithTarget(offeredTargetOid + 1);
    ASSERT_TRUE(reconcileRuledPendingTargets(spell, stale, state, kLocalPlayer));
    ASSERT_TRUE(stale.waitingForTarget);
    EXPECT_FALSE(ruledRejectedActivationCanRetry(stale));

    auto stillLegal = pendingWithTarget(offeredTargetOid);
    EXPECT_FALSE(reconcileRuledPendingTargets(spell, stillLegal, state, kLocalPlayer));
    EXPECT_TRUE(ruledRejectedActivationCanRetry(stillLegal));

    PendingActivatedAbility targetless;
    targetless.valid = true;
    EXPECT_TRUE(ruledRejectedActivationCanRetry(targetless));
}

TEST(RuledPendingTargetTest, ActivationOwnTargetEligibilityDoesNotUseTheOpponentsGroup)
{
    FakeHost host;
    RuledClientState state(&host);
    RuledSpellTargetData data;
    data.groups.append(RuledTargetGroupData{});
    data.groups.append(RuledTargetGroupData{});
    data.groups[0].validPermanentIds = {40};
    data.groups[1].validPermanentIds = {41};
    data.groups[1].groupIndex = 1;
    data.groups[1].chosenByOpponent = true;
    data.validPermanentIds = {40, 41};
    state.validTargetsByAbility.insert(RuledClientState::abilityTargetKey(30, 1), data);
    PendingActivatedAbility ability;
    ability.valid = true;
    ability.waitingForTarget = true;
    ability.permanentOid = 30;
    ability.abilityIndex = 1;
    EXPECT_EQ(ruledTargetClickEligibility({}, ability, state, RuledTargetCandidateKind::Battlefield, 40, kLocalPlayer),
              RuledTargetClickEligibility::Legal);
    EXPECT_EQ(ruledTargetClickEligibility({}, ability, state, RuledTargetCandidateKind::Battlefield, 41, kLocalPlayer),
              RuledTargetClickEligibility::Illegal);
}

TEST(RuledPendingTargetTest, ActivationChoiceUsesOnlyTheOfferedSeatRevisionAndGeneration)
{
    FakeHost host;
    RuledClientState state(&host);
    auto &engine = state.pendingAbilityActivation.emplace();
    engine.set_transaction_id(91);
    engine.set_revision(3);
    engine.set_actor_player_id(kLocalPlayer);
    engine.set_deciding_player_id(kOpponent);
    engine.set_stage(ruled::v1::ABILITY_ACTIVATION_STAGE_OPPONENT_TARGET);
    auto *target = engine.add_target_candidates();
    target->set_object_id(40);
    target->set_group_index(1);
    target->set_zone_change_generation(8);
    state.battlefieldGenerationByOid.insert(40, 19);
    const auto command = ruledActivationChoice(state, kOpponent, 91, 3, std::nullopt, 40);
    ASSERT_TRUE(command);
    ASSERT_TRUE(command->has_submit_ability_activation_choice());
    EXPECT_EQ(command->submit_ability_activation_choice().target().zone_change_generation(), 8u);
    EXPECT_EQ(command->submit_ability_activation_choice().target().group_index(), 1u);
    EXPECT_FALSE(command->has_submit_resolution_choice());
    EXPECT_FALSE(ruledActivationChoice(state, kLocalPlayer, 91, 3, std::nullopt, 40));
    EXPECT_FALSE(ruledActivationChoice(state, kOpponent, 90, 3, std::nullopt, 40));
    EXPECT_FALSE(ruledActivationChoice(state, kOpponent, 91, 2, std::nullopt, 40));
    EXPECT_FALSE(ruledActivationChoice(state, kOpponent, 91, 3, std::nullopt, 41));
    EXPECT_FALSE(ruledActivationChoice(state, kOpponent, 91, 3, 1, 40));
    engine.set_stage(ruled::v1::ABILITY_ACTIVATION_STAGE_CHOOSE_OPPONENT);
    engine.set_deciding_player_id(kLocalPlayer);
    engine.add_valid_opponent_ids(kOpponent);
    EXPECT_TRUE(ruledActivationChoice(state, kLocalPlayer, 91, 3, kOpponent, std::nullopt));
    EXPECT_FALSE(ruledActivationChoice(state, kLocalPlayer, 91, 3, 2, std::nullopt));
    EXPECT_FALSE(ruledActivationChoice(state, kLocalPlayer, 91, 3, std::nullopt, 40));
}

TEST(RuledPendingTargetTest, ActivationReconnectUsesPublicTargetsAndLockedPayment)
{
    FakeHost host;
    RuledClientState state(&host);
    auto &engine = state.pendingAbilityActivation.emplace();
    engine.set_transaction_id(91);
    engine.set_revision(3);
    engine.set_actor_player_id(kLocalPlayer);
    engine.set_source_object_id(30);
    engine.set_source_zone_change_generation(4);
    engine.set_stage(ruled::v1::ABILITY_ACTIVATION_STAGE_PAYMENT);
    engine.set_locked_total_cost("3");
    for (int group = 0; group < 2; ++group) {
        auto *target = engine.add_announced_targets();
        target->set_object_id(40 + group);
        target->set_group_index(group);
        target->set_zone_change_generation(8 + group);
    }
    PendingActivatedAbility local;
    ruledApplyActivationView(local, engine);
    ASSERT_EQ(local.selectedTargets.size(), 2);
    EXPECT_EQ(local.selectedTargets.last().ref.group_index(), 1u);
    EXPECT_EQ(local.selectedTargets.last().zoneChangeGeneration, 9u);
    EXPECT_EQ(local.remainingCost.value(QChar('X')), 3);
    EXPECT_TRUE(ruledActivationCanPay(state, kLocalPlayer));
    EXPECT_FALSE(ruledActivationCanPay(state, kOpponent));
    local.remainingCost[QChar('X')] = 1;
    ruledApplyActivationView(local, engine);
    EXPECT_EQ(local.remainingCost.value(QChar('X')), 1);
    state.choiceWaitingPlayerId = kOpponent;
    EXPECT_FALSE(ruledActivationCanPay(state, kLocalPlayer));
}

TEST(RuledPendingTargetTest, HandTappedCreatureActivationRetainsOnlyCurrentGenerationSelection)
{
    ruled::v1::PendingAbilityActivation engine;
    engine.set_transaction_id(92);
    engine.set_revision(1);
    engine.set_actor_player_id(kLocalPlayer);
    engine.set_source_object_id(500);
    engine.set_source_zone_change_generation(8);
    engine.set_source_zone(ruled::v1::ABILITY_SOURCE_ZONE_HAND);
    engine.set_return_tapped_creature_cost_index(1);
    auto *candidate = engine.add_return_tapped_creature_candidates();
    candidate->set_object_id(900);
    candidate->set_zone_change_generation(12);

    PendingActivatedAbility local;
    ruledApplyActivationView(local, engine);
    ASSERT_TRUE(local.deferredReturnTappedCreature);
    EXPECT_FALSE(local.needsTarget);
    ASSERT_EQ(local.costChoices.size(), 1);
    EXPECT_EQ(local.costChoices.first().costIndex, 1);
    EXPECT_EQ(local.costChoices.first().kind, RuledCostChoiceKind::ReturnTappedCreature);
    EXPECT_EQ(local.costChoices.first().candidateGenerations.value(900), 12u);

    local.costSelections = {{1, RuledCostChoiceZone::Battlefield, {900}, {12}}};
    local.nextCostChoice = 0;
    local.waitingForCost = true;
    ruledApplyActivationView(local, engine);
    ASSERT_EQ(local.costSelections.size(), 1);
    EXPECT_EQ(local.nextCostChoice, 0);
    EXPECT_TRUE(local.waitingForCost);
    EXPECT_TRUE(ruledReturnTappedCreatureSelection(local).has_value());

    local.waitingForCost = false;
    ruledApplyActivationView(local, engine);
    EXPECT_EQ(local.nextCostChoice, 1);
    EXPECT_FALSE(local.waitingForCost);
    engine.set_revision(2);
    engine.mutable_return_tapped_creature_candidates(0)->set_zone_change_generation(13);
    local.waitingForCost = true;
    ruledApplyActivationView(local, engine);
    EXPECT_TRUE(local.costSelections.isEmpty());
    EXPECT_EQ(local.nextCostChoice, 0);
    EXPECT_TRUE(local.waitingForCost);
    EXPECT_FALSE(ruledReturnTappedCreatureSelection(local).has_value());
}

TEST(RuledPendingTargetTest, ActivationCancelRaceInitializesPaymentWithoutDroppingTheAckPhase)
{
    ruled::v1::PendingAbilityActivation engine;
    engine.set_transaction_id(91);
    engine.set_stage(ruled::v1::ABILITY_ACTIVATION_STAGE_OPPONENT_TARGET);
    PendingActivatedAbility local;
    ruledApplyActivationView(local, engine);
    ASSERT_TRUE(local.beginCancel());
    engine.set_stage(ruled::v1::ABILITY_ACTIVATION_STAGE_PAYMENT);
    engine.set_locked_total_cost("3");
    ruledApplyActivationView(local, engine);
    EXPECT_EQ(local.stage(), PendingActivatedAbility::Stage::CancelPending);
    EXPECT_EQ(local.remainingCost.value(QChar('X')), 3);
    local.rejectCancel();
    local.remainingCost[QChar('X')] = 1;
    auto commitPending = local;
    ASSERT_TRUE(commitPending.beginCommit());
    ruledApplyActivationView(commitPending, engine);
    EXPECT_EQ(commitPending.stage(), PendingActivatedAbility::Stage::CommitPending);
    EXPECT_EQ(commitPending.remainingCost.value(QChar('X')), 1);

    auto cancelPending = local;
    ASSERT_TRUE(cancelPending.beginCancel());
    ruledApplyActivationView(cancelPending, engine);
    EXPECT_EQ(cancelPending.stage(), PendingActivatedAbility::Stage::CancelPending);
    EXPECT_EQ(cancelPending.remainingCost.value(QChar('X')), 1);
}

TEST_F(RuledClientTest, AggregateActivationReconnectRestoresPrivateGroupAndFixedPipsAfterSourceLoss)
{
    constexpr quint32 sourceOid = 0x730u;
    constexpr quint64 sourceGeneration = 8;
    constexpr int abilityIndex = 1;
    constexpr int counterCostIndex = 1;
    constexpr quint32 firstCounterOid = 901u;
    constexpr quint64 firstCounterGeneration = 0;
    constexpr quint32 secondCounterOid = 902u;
    constexpr quint64 secondCounterGeneration = 17;
    constexpr quint64 draftId = 0xabcdu;

    const QVector<RuledPendingCounterContribution> acceptedRows = {
        {counterCostIndex, firstCounterOid, firstCounterGeneration, kNightCounterOptionId, 1},
        {counterCostIndex, firstCounterOid, firstCounterGeneration, kChargeCounterOptionId, 1},
        {counterCostIndex, secondCounterOid, secondCounterGeneration, kNightCounterOptionId, 1},
    };
    const QString mixedLockedCost = QStringLiteral("{1}{U} + 2 life");
    const QString bothLifeLockedCost = QStringLiteral("{1} + 4 life");
    const QStringList matchingOfferLabels = {
        QStringLiteral("Night"), QStringLiteral("Charge"), QStringLiteral("Night on object 902")};

    const auto makeAcceptedView = [&](quint64 transactionId,
                                     quint64 revision,
                                     const QString &lockedCost,
                                     bool firstPipPaysLife,
                                     bool secondPipPaysLife) {
        ruled::v1::PendingAbilityActivation engine;
        engine.set_transaction_id(transactionId);
        engine.set_revision(revision);
        engine.set_actor_player_id(kLocalPlayer);
        engine.set_deciding_player_id(kLocalPlayer);
        engine.set_stage(ruled::v1::ABILITY_ACTIVATION_STAGE_PAYMENT);
        engine.set_source_object_id(sourceOid);
        engine.set_source_zone_change_generation(sourceGeneration);
        engine.set_source_zone(ruled::v1::ABILITY_SOURCE_ZONE_BATTLEFIELD);
        engine.set_ability_index(abilityIndex);
        engine.set_source_description("Tekuthal, Inquiry Dominus");
        engine.set_locked_total_cost(lockedCost.toStdString());

        auto *selection = engine.add_cost_selections();
        selection->set_cost_index(counterCostIndex);
        auto *group = selection->mutable_counter_removal_group();
        for (const auto &row : acceptedRows) {
            auto *contribution = group->add_contributions();
            contribution->mutable_object()->set_object_id(row.objectId);
            contribution->mutable_object()->set_zone_change_generation(row.zoneChangeGeneration);
            contribution->set_option_id(row.optionId);
            contribution->set_count(row.count);
        }
        auto *firstPip = engine.add_flex_payments();
        firstPip->set_pip_index(1);
        firstPip->set_pay_life(firstPipPaysLife);
        auto *secondPip = engine.add_flex_payments();
        secondPip->set_pip_index(2);
        secondPip->set_pay_life(secondPipPaysLife);
        return engine;
    };

    const auto addMatchingCounterOffers = [&](PendingActivatedAbility &local) {
        RuledCostChoice choice;
        choice.costIndex = counterCostIndex;
        choice.zone = RuledCostChoiceZone::Battlefield;
        choice.kind = RuledCostChoiceKind::RemoveCountersFromPermanents;
        RuledCounterRemovalGroup group;
        group.requiredCount = 3;

        RuledCounterRemovalGroupCandidate first;
        first.objectId = firstCounterOid;
        first.zoneChangeGeneration = firstCounterGeneration;
        first.options.append({kNightCounterOptionId, QStringLiteral("Night"), 2});
        first.options.append({kChargeCounterOptionId, QStringLiteral("Charge"), 2});
        group.candidates.append(first);

        RuledCounterRemovalGroupCandidate second;
        second.objectId = secondCounterOid;
        second.zoneChangeGeneration = secondCounterGeneration;
        // Distinct fixture text makes same-option label matching across objects observable.
        second.options.append({kNightCounterOptionId, QStringLiteral("Night on object 902"), 2});
        group.candidates.append(second);

        choice.counterRemovalGroup = group;
        local.costChoices.append(choice);
    };

    const auto labeledRows = [&](const QVector<RuledPendingCounterContribution> &rows,
                                 const QStringList &labels) {
        auto result = rows;
        for (int i = 0; i < result.size() && i < labels.size(); ++i)
            result[i].counterLabel = labels.at(i);
        return result;
    };

    const auto expectRestoredRows = [&](const PendingActivatedAbility &local,
                                        const QVector<RuledPendingCounterContribution> &expectedRows) {
        EXPECT_EQ(local.counterGroupContributions.size(), expectedRows.size());
        for (int i = 0; i < expectedRows.size() && i < local.counterGroupContributions.size(); ++i) {
            const auto &actual = local.counterGroupContributions.at(i);
            const auto &expected = expectedRows.at(i);
            EXPECT_EQ(actual.costIndex, expected.costIndex);
            EXPECT_EQ(actual.objectId, expected.objectId);
            EXPECT_EQ(actual.zoneChangeGeneration, expected.zoneChangeGeneration);
            EXPECT_EQ(actual.optionId, expected.optionId);
            EXPECT_EQ(actual.count, expected.count);
            EXPECT_EQ(actual.counterLabel, expected.counterLabel);
        }
    };

    const auto expectRestoredPayment =
        [&](const PendingActivatedAbility &local, const ruled::v1::PendingAbilityActivation &engine,
            const QVector<RuledPendingCounterContribution> &expectedRows, quint64 expectedDraftId) {
            EXPECT_EQ(local.transactionId(), engine.transaction_id());
            EXPECT_EQ(local.revision(), engine.revision());
            EXPECT_EQ(local.permanentOid, sourceOid);
            EXPECT_EQ(local.expectedZoneChangeGeneration, sourceGeneration);
            EXPECT_EQ(local.abilityIndex, abilityIndex);
            EXPECT_EQ(local.abilityText, QString::fromStdString(engine.source_description()));
            EXPECT_EQ(local.draftId, expectedDraftId);
            EXPECT_TRUE(local.hasInitializedPayment());
            EXPECT_TRUE(local.waitingForMana);
            EXPECT_FALSE(local.waitingForCost);
            EXPECT_TRUE(local.costChoices.isEmpty());
            EXPECT_TRUE(local.costSelections.isEmpty());
            EXPECT_TRUE(local.selectedTargets.isEmpty());
            EXPECT_FALSE(local.needsTarget);
            EXPECT_FALSE(local.chosenOpponentTargets);
            EXPECT_FALSE(local.waitingForTarget);
            EXPECT_TRUE(local.remainingCost.isEmpty());
            EXPECT_TRUE(local.flexPips.isEmpty());
            EXPECT_TRUE(local.lifePipIndices.isEmpty());
            EXPECT_EQ(local.engineLockedCostLabel, QString::fromStdString(engine.locked_total_cost()));
            expectRestoredRows(local, expectedRows);

            EXPECT_EQ(local.fixedFlexPayments.size(), engine.flex_payments_size());
            for (int i = 0; i < engine.flex_payments_size() && i < local.fixedFlexPayments.size(); ++i) {
                EXPECT_EQ(local.fixedFlexPayments.at(i).pip_index(), engine.flex_payments(i).pip_index());
                EXPECT_EQ(local.fixedFlexPayments.at(i).pay_life(), engine.flex_payments(i).pay_life());
            }
        };

    auto mixedView = makeAcceptedView(91, 1, mixedLockedCost, false, true);
    ASSERT_EQ(mixedView.cost_selections_size(), 1);
    ASSERT_TRUE(mixedView.cost_selections(0).has_counter_removal_group());
    ASSERT_EQ(mixedView.cost_selections(0).counter_removal_group().contributions_size(), acceptedRows.size());
    ASSERT_EQ(mixedView.flex_payments_size(), 2);
    EXPECT_EQ(mixedView.flex_payments(0).pip_index(), 1u);
    EXPECT_FALSE(mixedView.flex_payments(0).pay_life());
    EXPECT_EQ(mixedView.flex_payments(1).pip_index(), 2u);
    EXPECT_TRUE(mixedView.flex_payments(1).pay_life());
    EXPECT_FALSE(mixedView.has_chosen_opponent_id());
    EXPECT_EQ(mixedView.announced_targets_size(), 0);

    PendingActivatedAbility local;
    local.valid = true;
    local.permanentOid = sourceOid;
    local.expectedZoneChangeGeneration = sourceGeneration;
    local.sourceZone = ruled::v1::ABILITY_SOURCE_ZONE_BATTLEFIELD;
    local.abilityIndex = abilityIndex;
    local.draftId = draftId;
    local.counterGroupContributions = acceptedRows;
    local.counterGroupConfirmed = true;
    local.remainingCost[QChar('X')] = 1;
    local.lifePipIndices.append(2);
    RuledFlexPip firstLocalPip;
    firstLocalPip.pipIndex = 1;
    firstLocalPip.phyrexian = true;
    RuledFlexPip secondLocalPip;
    secondLocalPip.pipIndex = 2;
    secondLocalPip.phyrexian = true;
    local.flexPips = {firstLocalPip, secondLocalPip};
    addMatchingCounterOffers(local);

    // Tekuthal's {1}{U/P}{U/P} uses pip indices 1 and 2. The mixed choice is false/true.
    // The live source is absent from client state; prior typed offers remain only in this draft.
    EXPECT_TRUE(state->abilityCostChoices(sourceOid, abilityIndex).isEmpty());

    for (int mismatch = 0; mismatch < 4; ++mismatch) {
        auto mismatchedOffer = local;
        switch (mismatch) {
            case 0:
                ++mismatchedOffer.permanentOid;
                break;
            case 1:
                ++mismatchedOffer.expectedZoneChangeGeneration;
                break;
            case 2:
                mismatchedOffer.sourceZone = ruled::v1::ABILITY_SOURCE_ZONE_HAND;
                break;
            default:
                ++mismatchedOffer.abilityIndex;
                break;
        }
        ruledApplyActivationView(mismatchedOffer, mixedView);
        expectRestoredRows(mismatchedOffer, acceptedRows);
    }

    ruledApplyActivationView(local, mixedView);
    EXPECT_EQ(local.stage(), PendingActivatedAbility::Stage::Paying);
    expectRestoredPayment(local, mixedView, labeledRows(acceptedRows, matchingOfferLabels), draftId);

    auto refreshedView = mixedView;
    refreshedView.set_revision(2);
    refreshedView.mutable_cost_selections(0)->mutable_counter_removal_group()->mutable_contributions()->SwapElements(0, 2);
    const QVector<RuledPendingCounterContribution> refreshedRows = {
        labeledRows(acceptedRows, matchingOfferLabels).at(2),
        labeledRows(acceptedRows, matchingOfferLabels).at(1),
        labeledRows(acceptedRows, matchingOfferLabels).at(0),
    };
    for (const auto stage : {PendingActivatedAbility::Stage::CommitPending,
                             PendingActivatedAbility::Stage::CancelPending}) {
        auto sameTransaction = local;
        sameTransaction.costChoices.clear();
        sameTransaction.costSelections.clear();
        const bool submitting = stage == PendingActivatedAbility::Stage::CommitPending ? sameTransaction.beginCommit()
                                                                                       : sameTransaction.beginCancel();
        ASSERT_TRUE(submitting);
        ruledApplyActivationView(sameTransaction, refreshedView);
        EXPECT_EQ(sameTransaction.stage(), stage);
        expectRestoredPayment(sameTransaction, refreshedView, refreshedRows, draftId);
    }

    auto stagedPayment = local;
    const QMap<QChar, int> stagedRemainingCost = {{QChar('U'), 1}};
    stagedPayment.remainingCost = stagedRemainingCost;
    ruledApplyActivationView(stagedPayment, refreshedView);
    EXPECT_EQ(stagedPayment.stage(), PendingActivatedAbility::Stage::Paying);
    EXPECT_EQ(stagedPayment.remainingCost, stagedRemainingCost);

    auto bothLifeView = makeAcceptedView(92, 1, bothLifeLockedCost, true, true);
    ASSERT_EQ(bothLifeView.flex_payments_size(), 2);
    EXPECT_TRUE(bothLifeView.flex_payments(0).pay_life());
    EXPECT_TRUE(bothLifeView.flex_payments(1).pay_life());
    PendingActivatedAbility freshReconnect;
    freshReconnect.draftId = draftId + 1;
    freshReconnect.permanentOid = sourceOid + 1;
    freshReconnect.expectedZoneChangeGeneration = sourceGeneration + 1;
    freshReconnect.sourceZone = ruled::v1::ABILITY_SOURCE_ZONE_HAND;
    freshReconnect.abilityIndex = abilityIndex + 1;
    freshReconnect.counterGroupContributions = labeledRows(
        acceptedRows, {QStringLiteral("stale Night"), QStringLiteral("stale Charge"), QStringLiteral("stale remote Night")});
    freshReconnect.fixedFlexPayments.append(ruled::v1::FlexPipPayment{});
    freshReconnect.fixedFlexPayments.last().set_pip_index(99);
    freshReconnect.engineLockedCostLabel = QStringLiteral("stale label");
    freshReconnect.remainingCost[QChar('X')] = 5;
    freshReconnect.lifePipIndices.append(99);
    RuledFlexPip stalePip;
    stalePip.pipIndex = 99;
    stalePip.phyrexian = true;
    freshReconnect.flexPips.append(stalePip);
    EXPECT_TRUE(freshReconnect.costChoices.isEmpty());
    EXPECT_TRUE(state->abilityCostChoices(sourceOid, abilityIndex).isEmpty());

    ruledApplyActivationView(freshReconnect, bothLifeView);
    EXPECT_EQ(freshReconnect.stage(), PendingActivatedAbility::Stage::Paying);
    expectRestoredPayment(freshReconnect, bothLifeView, acceptedRows, draftId + 1);
}

TEST_F(RuledClientTest, StagedActivationRetainsPaymentWhileNestedReplacementOwnsInput)
{
    auto &engine = state->pendingAbilityActivation.emplace();
    engine.set_actor_player_id(kLocalPlayer);
    engine.set_stage(ruled::v1::ABILITY_ACTIVATION_STAGE_PAYMENT);
    state->payment.begin();
    state->payment.selection.mutable_mana()->set_c(1);
    RuledClientState::RuledPendingChoice replacement;
    replacement.kind = RuledClientState::ChoiceKind::ReplacementEffect;
    state->setPendingChoice(replacement);
    EXPECT_TRUE(state->payment.active);
    EXPECT_EQ(state->payment.selection.mana().c(), 1u);
    EXPECT_FALSE(ruledActivationCanPay(*state, kLocalPlayer));
    state->clearPendingChoice();
    EXPECT_TRUE(ruledActivationCanPay(*state, kLocalPlayer));
    EXPECT_EQ(state->payment.selection.mana().c(), 1u);
}

TEST_F(RuledClientTest, RetiringNestedReplacementPreservesStagedAbilityPayment)
{
    constexpr quint64 promptId = 9303;
    auto *activation = &state->pendingAbilityActivation.emplace();
    activation->set_transaction_id(92);
    activation->set_revision(4);
    activation->set_actor_player_id(kLocalPlayer);
    activation->set_stage(ruled::v1::ABILITY_ACTIVATION_STAGE_PAYMENT);

    state->payment.begin();
    ASSERT_TRUE(state->payment.payMana('U', 0, 51));

    ruled::v1::RuledEventBatch replacement;
    auto *snapshot = replacement.mutable_resolution_prompt_state();
    snapshot->set_active(true);
    snapshot->set_deciding_player_id(kLocalPlayer);
    snapshot->set_resolution_prompt_id(promptId);
    auto *choice = replacement.add_events()->mutable_resolution_choice_required();
    choice->set_deciding_player_id(kLocalPlayer);
    choice->set_choice_kind(ruled::v1::CHOICE_KIND_REPLACEMENT_EFFECT);
    choice->set_resolution_prompt_id(promptId);
    choice->set_min(1);
    choice->set_max(1);
    choice->add_candidate_object_ids(7001);
    auto *option = choice->add_replacement_options();
    option->set_application_id(7001);
    option->set_effect_summary("Mana damage replacement");
    applyRaw(replacement);

    ASSERT_TRUE(state->hasPendingReplacementEffect());
    ASSERT_EQ(state->pendingChoice->resolutionPromptId, promptId);
    EXPECT_TRUE(state->payment.active);
    EXPECT_EQ(state->payment.selection.mana().u(), 1u);
    EXPECT_EQ(state->payment.optimisticManaCounterSpendCount(51), 1);

    state->submitReplacementEffect(0, state->pendingChoiceRevision);
    ASSERT_EQ(host.sentCommands.size(), 1);
    const quint64 submittedRevision = state->pendingChoiceRevision;

    ruled::v1::RuledEventBatch inactive;
    inactive.mutable_resolution_prompt_state()->set_active(false);
    inactive.mutable_resolution_prompt_state()->set_deciding_player_id(0);
    applyRaw(inactive);

    EXPECT_GT(state->pendingChoiceRevision, submittedRevision);
    EXPECT_FALSE(state->hasPendingReplacementEffect());
    EXPECT_FALSE(state->pendingChoice.has_value());
    EXPECT_TRUE(state->payment.active);
    EXPECT_EQ(state->payment.selection.mana().u(), 1u);
    EXPECT_EQ(state->payment.optimisticManaCounterSpendCount(51), 1);

    host.answerPendingAck(false);
    EXPECT_FALSE(state->hasPendingReplacementEffect());
    EXPECT_FALSE(state->pendingChoice.has_value());
    EXPECT_TRUE(state->payment.active);
    EXPECT_EQ(state->payment.selection.mana().u(), 1u);
    EXPECT_EQ(state->payment.optimisticManaCounterSpendCount(51), 1);
    ASSERT_TRUE(state->pendingAbilityActivation.has_value());
    EXPECT_EQ(state->pendingAbilityActivation->stage(), ruled::v1::ABILITY_ACTIVATION_STAGE_PAYMENT);
}

TEST_F(RuledClientTest, HandSlotAndPublicZoneMapsAreQueryable)
{
    ruled::v1::RuledEventBatch batch;
    auto *hs = batch.add_events()->mutable_hand_slot_map();
    auto *he = hs->add_entries();
    he->set_player_id(kOpponent);
    he->set_server_card_id(42);
    he->set_hand_index(3);
    auto *gy = batch.add_events()->mutable_graveyard_object_map();
    auto *ge = gy->add_entries();
    ge->set_player_id(kLocalPlayer);
    ge->set_engine_object_id(500);
    ge->set_server_card_id(11);
    auto *xe = batch.add_events()->mutable_exile_object_map()->add_entries();
    xe->set_player_id(kOpponent);
    xe->set_engine_object_id(700);
    xe->set_server_card_id(13);
    apply(batch);

    EXPECT_EQ(state->engineHandSlotForServerCard(kOpponent, 42), 3);
    EXPECT_EQ(state->engineHandSlotForServerCard(kLocalPlayer, 42), -1);
    EXPECT_EQ(state->graveyardEngineOidForOwnedCard(kLocalPlayer, 11), 500u);
    EXPECT_EQ(state->graveyardEngineOidForOwnedCard(kLocalPlayer, 12), 0u);
    // Card ids repeat across players' zones, so the owner has to be part of the key.
    EXPECT_EQ(state->graveyardEngineOidForOwnedCard(kOpponent, 11), 0u);
    EXPECT_EQ(state->exileEngineOidForOwnedCard(kOpponent, 13), 700u);
    EXPECT_EQ(state->exileEngineOidForOwnedCard(kLocalPlayer, 13), 0u);
}

TEST_F(RuledClientTest, HandSlotMapPersistsUntilReplaced)
{
    ruled::v1::RuledEventBatch first;
    auto *he = first.add_events()->mutable_hand_slot_map()->add_entries();
    he->set_player_id(kLocalPlayer);
    he->set_server_card_id(5);
    he->set_hand_index(0);
    apply(first);
    ASSERT_EQ(state->engineHandSlotForServerCard(kLocalPlayer, 5), 0);

    // Servatrice omits the map on batches with no hand change, so an absent map means "unchanged"
    // and the slot has to survive.
    apply(phaseBatch(ruled::v1::PHASE_ID_MAIN1, kLocalPlayer));
    EXPECT_EQ(state->engineHandSlotForServerCard(kLocalPlayer, 5), 0);

    // A present map is a full replacement: card 5 left the hand, card 7 took slot 0.
    ruled::v1::RuledEventBatch second;
    auto *he2 = second.add_events()->mutable_hand_slot_map()->add_entries();
    he2->set_player_id(kLocalPlayer);
    he2->set_server_card_id(7);
    he2->set_hand_index(0);
    apply(second);
    EXPECT_EQ(state->engineHandSlotForServerCard(kLocalPlayer, 5), -1);
    EXPECT_EQ(state->engineHandSlotForServerCard(kLocalPlayer, 7), 0);
}

// ---------------------------------------------------------------------------------------
// Structured legal hand actions — one case per kind
// ---------------------------------------------------------------------------------------

TEST_F(RuledClientTest, LegalHandSlotRequiresStableServerCardIdentity)
{
    ruled::v1::RuledEventBatch legalOnly;
    auto &actions = (*legalOnly.mutable_legal_by_player())[kLocalPlayer];
    addHandAction(actions, ruled::v1::HAND_ACTION_PLAY_LAND, 0, "Forest");
    apply(legalOnly);

    // A newly visible dev-conjured card can arrive before its HandSlotMap refresh. Its visual
    // position must never be treated as engine slot 0, or clicking it plays the existing Forest.
    EXPECT_EQ(state->legalHandSlotForServerCard(ruled::v1::HAND_ACTION_PLAY_LAND, kLocalPlayer, 99), -1);

    ruled::v1::RuledEventBatch mapped;
    auto &mappedActions = (*mapped.mutable_legal_by_player())[kLocalPlayer];
    addHandAction(mappedActions, ruled::v1::HAND_ACTION_PLAY_LAND, 0, "Forest");
    auto *forest = mapped.add_events()->mutable_hand_slot_map()->add_entries();
    forest->set_player_id(kLocalPlayer);
    forest->set_server_card_id(5);
    forest->set_hand_index(0);
    apply(mapped);

    EXPECT_EQ(state->legalHandSlotForServerCard(ruled::v1::HAND_ACTION_PLAY_LAND, kLocalPlayer, 5), 0);
    EXPECT_EQ(state->legalHandSlotForServerCard(ruled::v1::HAND_ACTION_PLAY_LAND, kLocalPlayer, 99), -1);
}

TEST_F(RuledClientTest, AppliesStructuredLandActionsIncludingMdfcFaces)
{
    ruled::v1::RuledEventBatch batch;
    auto &actions = (*batch.mutable_legal_by_player())[kLocalPlayer];
    addHandAction(actions, ruled::v1::HAND_ACTION_PLAY_LAND, 2, "Forest");
    addHandAction(actions, ruled::v1::HAND_ACTION_PLAY_LAND, 4, "Cragcrown Pathway", 0);
    addHandAction(actions, ruled::v1::HAND_ACTION_PLAY_LAND, 4, "Timbercrown Pathway", 1);
    addHandAction(actions, ruled::v1::HAND_ACTION_CAST_SPELL, 1, "Lightning Bolt", 0, true);
    apply(batch);

    constexpr auto kLand = ruled::v1::HAND_ACTION_PLAY_LAND;
    EXPECT_TRUE(state->isHandActionLegal(kLand, 2));
    EXPECT_TRUE(state->isHandActionLegal(kLand, 4));
    EXPECT_FALSE(state->isHandActionLegal(kLand, 3));
    EXPECT_EQ(state->handActionIndicesForCardName(kLand, "Forest"), QList<int>({2}));
    // A cast action never lands in the PlayLand set.
    EXPECT_FALSE(state->isHandActionLegal(kLand, 1));
    EXPECT_TRUE(state->isHandActionLegal(ruled::v1::HAND_ACTION_CAST_SPELL, 1));

    // CR 712: one hand slot, two playable faces, sorted by face index.
    const QVector<RuledFaceOption> faces = state->handActionFaceOptions(kLand, 4);
    ASSERT_EQ(faces.size(), 2);
    EXPECT_EQ(faces[0].faceIndex, 0);
    EXPECT_EQ(faces[0].faceName, QStringLiteral("Cragcrown Pathway"));
    EXPECT_EQ(faces[1].faceIndex, 1);
    EXPECT_EQ(faces[1].faceName, QStringLiteral("Timbercrown Pathway"));
    // A single-face land still reports exactly one option, at face 0.
    ASSERT_EQ(state->handActionFaceOptions(kLand, 2).size(), 1);
    EXPECT_EQ(state->handActionFaceOptions(kLand, 2)[0].faceIndex, 0);
}

TEST_F(RuledClientTest, AppliesStructuredCastActionsAndTargetRequirement)
{
    ruled::v1::RuledEventBatch batch;
    auto &actions = (*batch.mutable_legal_by_player())[kLocalPlayer];
    addHandAction(actions, ruled::v1::HAND_ACTION_CAST_SPELL, 1, "Lightning Bolt", 0, true);
    addHandAction(actions, ruled::v1::HAND_ACTION_CAST_SPELL, 3, "Llanowar Elves");
    apply(batch);

    constexpr auto kCast = ruled::v1::HAND_ACTION_CAST_SPELL;
    EXPECT_TRUE(state->isHandActionLegal(kCast, 1));
    EXPECT_TRUE(state->handActionNeedsTarget(kCast, 1));
    EXPECT_TRUE(state->isHandActionLegal(kCast, 3));
    EXPECT_FALSE(state->handActionNeedsTarget(kCast, 3));
    EXPECT_EQ(state->handActionIndexForCard(kCast, "Llanowar Elves", 99), 3);
    EXPECT_EQ(state->handActionIndexForCard(kCast, "Nonexistent", 0), -1);
}

TEST_F(RuledClientTest, LibraryTopViewAndOffersAreAppliedAndClearedWithTheZoneSnapshot)
{
    QSignalSpy topChanged(state, &RuledClientState::privateLibraryTopChanged);
    ruled::v1::RuledEventBatch batch;
    auto *zoneView = batch.add_events()->mutable_zone_view();
    auto *localView = zoneView->add_per_player();
    localView->set_player_id(kLocalPlayer);
    auto *top = localView->mutable_private_library_top();
    top->set_object_id(800u);
    top->set_zone_change_generation(12u);
    top->set_card_id("akoum_warrior_akoum_teeth");
    top->set_card_name("Akoum Warrior");
    zoneView->add_per_player()->set_player_id(kOpponent);

    auto &actions = (*batch.mutable_legal_by_player())[kLocalPlayer];
    auto *cast = actions.add_zone_cast_actions();
    cast->set_source_zone(ruled::v1::CAST_SOURCE_ZONE_LIBRARY);
    cast->set_object_id(800u);
    cast->set_card_name("Akoum Warrior");
    cast->set_face_index(0u);
    cast->set_cost("{1}{U}");
    cast->set_zone_change_generation(12u);
    cast->set_cast_method(ruled::v1::CAST_METHOD_NORMAL);
    auto *land = actions.add_zone_land_actions();
    land->set_source_zone(ruled::v1::CAST_SOURCE_ZONE_LIBRARY);
    land->set_object_id(800u);
    land->set_card_name("Akoum Teeth");
    land->set_face_index(1u);
    land->set_zone_change_generation(12u);
    apply(batch);

    ASSERT_EQ(topChanged.count(), 1);
    ASSERT_TRUE(state->privateLibraryTop.has_value());
    EXPECT_EQ(state->privateLibraryTop->objectId, 800u);
    EXPECT_EQ(state->privateLibraryTop->zoneChangeGeneration, 12u);
    EXPECT_EQ(state->privateLibraryTop->cardName, QStringLiteral("Akoum Warrior"));
    EXPECT_TRUE(state->isZoneCastActionLegal(800u, 0, RuledCastSource::Library,
                                             ruled::v1::CAST_METHOD_NORMAL, 0, 12u));
    const auto landFaces = state->zoneLandFaceOptions(800u, RuledCastSource::Library);
    ASSERT_EQ(landFaces.size(), 1);
    EXPECT_EQ(landFaces.first().faceIndex, 1);
    EXPECT_EQ(landFaces.first().faceName, QStringLiteral("Akoum Teeth"));

    ruled::v1::RuledEventBatch cleared;
    auto *clearedView = cleared.add_events()->mutable_zone_view();
    clearedView->add_per_player()->set_player_id(kLocalPlayer);
    clearedView->add_per_player()->set_player_id(kOpponent);
    apply(cleared);
    EXPECT_EQ(topChanged.count(), 2);
    EXPECT_FALSE(state->privateLibraryTop.has_value());
    EXPECT_FALSE(state->isZoneCastActionLegal(800u, 0, RuledCastSource::Library,
                                              ruled::v1::CAST_METHOD_NORMAL, 0, 12u));
    EXPECT_FALSE(state->isZoneLandActionLegal(800u, RuledCastSource::Library));
}

TEST_F(RuledClientTest, AdventureCastFacesCarryEngineNamesAndCosts)
{
    ruled::v1::RuledEventBatch batch;
    auto &actions = (*batch.mutable_legal_by_player())[kLocalPlayer];
    addHandAction(actions, ruled::v1::HAND_ACTION_CAST_SPELL, 6, "Bonecrusher Giant", 0, false, "{2}{R}");
    addHandAction(actions, ruled::v1::HAND_ACTION_CAST_SPELL, 6, "Stomp", 1, true, "{1}{R}");
    apply(batch);

    const QVector<RuledFaceOption> faces = state->handActionFaceOptions(ruled::v1::HAND_ACTION_CAST_SPELL, 6);
    ASSERT_EQ(faces.size(), 2);
    EXPECT_EQ(faces[0].faceIndex, 0);
    EXPECT_EQ(faces[0].faceName, QStringLiteral("Bonecrusher Giant"));
    EXPECT_EQ(faces[0].manaCost, QStringLiteral("{2}{R}"));
    EXPECT_FALSE(state->handActionNeedsTarget(ruled::v1::HAND_ACTION_CAST_SPELL, 6, 0));
    EXPECT_EQ(faces[1].faceIndex, 1);
    EXPECT_EQ(faces[1].faceName, QStringLiteral("Stomp"));
    EXPECT_EQ(faces[1].manaCost, QStringLiteral("{1}{R}"));
    EXPECT_TRUE(state->handActionNeedsTarget(ruled::v1::HAND_ACTION_CAST_SPELL, 6, 1));
}

TEST_F(RuledClientTest, OmenFacesOptionalTargetAndStackAnnotationStayEngineAuthored)
{
    ruled::v1::RuledEventBatch legalBatch;
    auto &actions = (*legalBatch.mutable_legal_by_player())[kLocalPlayer];
    addHandAction(actions, ruled::v1::HAND_ACTION_CAST_SPELL, 6, "Dirgur Island Dragon", 0, false, "{5}{U}");
    addHandAction(actions, ruled::v1::HAND_ACTION_CAST_SPELL, 6, "Skimming Strike", 1, true, "{1}{U}");
    auto &targets = (*actions.mutable_valid_targets_by_hand_slot())[(6u << 8) | 1u];
    auto *group = targets.add_groups();
    group->set_group_index(0);
    group->set_prompt_text("Choose up to one target creature");
    group->set_min(0);
    group->set_max(1);
    group->add_valid_permanent_ids(101u);
    apply(legalBatch);

    const QVector<RuledFaceOption> faces = state->handActionFaceOptions(ruled::v1::HAND_ACTION_CAST_SPELL, 6);
    ASSERT_EQ(faces.size(), 2);
    EXPECT_EQ(faces[0].faceName, QStringLiteral("Dirgur Island Dragon"));
    EXPECT_EQ(faces[0].manaCost, QStringLiteral("{5}{U}"));
    EXPECT_EQ(faces[1].faceName, QStringLiteral("Skimming Strike"));
    EXPECT_EQ(faces[1].manaCost, QStringLiteral("{1}{U}"));
    ASSERT_EQ(state->spellTargetData(6, 1).groups.size(), 1);
    EXPECT_TRUE(ruledTargetGroupUsesExplicitConfirmation(state->spellTargetData(6, 1).groups.first()));

    ruled::v1::RuledEventBatch stackBatch;
    auto *push = stackBatch.add_events()->mutable_stack_pushed();
    push->set_object_id(700u);
    push->set_card_id("dirgur_island_dragon_skimming_strike");
    push->set_description("Skimming Strike");
    push->set_ability_annotation("Skimming Strike");
    apply(stackBatch);
    EXPECT_EQ(state->stackAnnotation(700u), QStringLiteral("Skimming Strike"));
    EXPECT_EQ(state->getStackOidOrder(), QList<quint32>({700u}));

    ruled::v1::RuledEventBatch resolvedBatch;
    auto *resolved = resolvedBatch.add_events()->mutable_stack_resolved();
    resolved->set_object_id(700u);
    resolved->set_destination(ruled::v1::STACK_RESOLVE_DESTINATION_LIBRARY);
    apply(resolvedBatch);
    EXPECT_FALSE(state->hasStackItems());
    EXPECT_TRUE(state->stackAnnotation(700u).isEmpty());
}

TEST_F(RuledClientTest, PhysicalSpellPrimaryPresentationDoesNotBecomeStackAnnotation)
{
    ruled::v1::RuledEventBatch ordinaryBatch;
    auto *ordinary = ordinaryBatch.add_events()->mutable_stack_pushed();
    ordinary->set_object_id(701u);
    ordinary->set_card_id("serra_angel");
    ordinary->set_description("Serra Angel");
    ordinary->mutable_primary_presentation()->set_fallback_text("Serra Angel");
    apply(ordinaryBatch);

    EXPECT_TRUE(state->stackAnnotation(701u).isEmpty());

    ruled::v1::RuledEventBatch multiFaceBatch;
    auto *multiFace = multiFaceBatch.add_events()->mutable_stack_pushed();
    multiFace->set_object_id(702u);
    multiFace->set_card_id("fire_ice");
    multiFace->set_description("Fire");
    multiFace->set_ability_annotation("Fire");
    multiFace->mutable_primary_presentation()->set_fallback_text("Fire deals 2 damage.");
    apply(multiFaceBatch);

    EXPECT_EQ(state->stackAnnotation(702u), QStringLiteral("Fire"));
}

TEST_F(RuledClientTest, PrototypePublicStateTracksStackResolutionAndBattlefieldReplacement)
{
    ruled::v1::RuledEventBatch cast;
    auto *push = cast.add_events()->mutable_stack_pushed();
    push->set_object_id(9820u);
    push->set_card_id("skitterbeam_battalion");
    push->set_prototyped(true);
    apply(cast);
    EXPECT_TRUE(state->isPrototyped(9820u));
    EXPECT_EQ(state->stackAnnotation(9820u), QStringLiteral("Prototype"));

    ruled::v1::RuledEventBatch resolved;
    resolved.add_events()->mutable_stack_resolved()->set_object_id(9820u);
    apply(resolved);
    EXPECT_FALSE(state->isPrototyped(9820u));

    ruled::v1::RuledEventBatch battlefield;
    auto *zoneView = battlefield.add_events()->mutable_zone_view();
    auto *playerView = zoneView->add_per_player();
    auto *object = playerView->add_battlefield_objects();
    object->set_object_id(9821u);
    object->set_power(2);
    object->set_toughness(2);
    object->set_prototyped(true);
    apply(battlefield);
    EXPECT_TRUE(state->isPrototyped(9821u));
    EXPECT_EQ(state->battlefieldPowerToughness(9821u), QStringLiteral("2/2"));

    ruled::v1::RuledEventBatch facedown;
    auto *replacementView = facedown.add_events()->mutable_zone_view()->add_per_player();
    auto *hidden = replacementView->add_battlefield_objects();
    hidden->set_object_id(9821u);
    hidden->set_face_down(true);
    hidden->set_prototyped(true);
    apply(facedown);
    EXPECT_FALSE(state->isPrototyped(9821u));
}

TEST_F(RuledClientTest, StackAnnotationKeepsAbilityAndStructuredChoicePresentation)
{
    ruled::v1::RuledEventBatch abilityBatch;
    auto *ability = abilityBatch.add_events()->mutable_stack_pushed();
    ability->set_object_id(703u);
    ability->set_description("Triggered ability");
    ability->set_ability_annotation("Engine fallback ability");
    ability->mutable_primary_presentation()->set_fallback_text("Resolved ability text");
    apply(abilityBatch);

    EXPECT_EQ(state->stackAnnotation(703u), QStringLiteral("Resolved ability text"));

    ruled::v1::RuledEventBatch choicesBatch;
    auto *choices = choicesBatch.add_events()->mutable_stack_pushed();
    choices->set_object_id(704u);
    choices->set_card_id("boros_charm");
    choices->set_description("Boros Charm");
    choices->set_ability_annotation("Fallback mode\nFallback cost");
    choices->mutable_primary_presentation()->set_fallback_text("Boros Charm");
    choices->add_chosen_mode_presentations()->set_fallback_text("Resolved mode text");
    choices->add_chosen_cast_cost_presentations()->set_fallback_text("Resolved cost text");
    apply(choicesBatch);

    EXPECT_EQ(state->stackAnnotation(704u), QStringLiteral("Resolved mode text\nResolved cost text"));

    ruled::v1::RuledEventBatch markerBatch;
    auto *marker = markerBatch.add_events()->mutable_stack_pushed();
    marker->set_object_id(705u);
    marker->set_card_id("fireball");
    marker->set_description("Fireball");
    marker->set_ability_annotation("X = 4");
    marker->mutable_primary_presentation()->set_fallback_text("Fireball");
    apply(markerBatch);

    EXPECT_EQ(state->stackAnnotation(705u), QStringLiteral("X = 4"));

    for (const auto &x : QVector<QPair<quint32, QString>>{{706u, QStringLiteral("X = 0")},
                                                          {707u, QStringLiteral("X = 7")}}) {
        ruled::v1::RuledEventBatch loyaltyXBatch;
        auto *loyaltyX = loyaltyXBatch.add_events()->mutable_stack_pushed();
        loyaltyX->set_object_id(x.first);
        loyaltyX->set_ability_annotation(x.second.toStdString());
        loyaltyX->mutable_primary_presentation()->set_fallback_text("Sorin -X");
        apply(loyaltyXBatch);
        EXPECT_TRUE(state->stackAnnotation(x.first).contains(x.second));
    }
}

TEST_F(RuledClientTest, AppliesAuthoritativeModalModeDataPerFace)
{
    ruled::v1::RuledEventBatch batch;
    auto &actions = (*batch.mutable_legal_by_player())[kLocalPlayer];
    auto *cast = addHandAction(actions, ruled::v1::HAND_ACTION_CAST_SPELL, 5, "Boros Charm");
    cast->set_min_modes(1);
    cast->set_max_modes(1);
    auto *damage = cast->add_modes();
    damage->set_mode_index(0);
    damage->set_label("Deal 4 damage");
    damage->set_selectable(true);
    damage->set_needs_target(true);
    auto *damageGroup = damage->mutable_targets()->add_groups();
    damageGroup->set_group_index(0);
    damageGroup->set_prompt_text("Choose target player");
    damageGroup->set_min(1);
    damageGroup->set_max(1);
    damageGroup->set_can_target_opponent(true);
    auto *disabled = cast->add_modes();
    disabled->set_mode_index(2);
    disabled->set_label("Creature gains double strike");
    disabled->set_selectable(false);
    disabled->set_needs_target(true);
    apply(batch);

    const auto &set = state->handActions[ruled::v1::HAND_ACTION_CAST_SPELL];
    const auto key = RuledClientState::handCastActionKey(5, 0, ruled::v1::CAST_METHOD_NORMAL);
    ASSERT_TRUE(set.modalOptionsByCastKey.contains(key));
    EXPECT_EQ(set.modalMinModesByCastKey.value(key), 1);
    EXPECT_EQ(set.modalMaxModesByCastKey.value(key), 1);
    const auto modes = set.modalOptionsByCastKey.value(key);
    ASSERT_EQ(modes.size(), 2);
    EXPECT_EQ(modes[0].modeIndex, 0);
    EXPECT_TRUE(modes[0].targets.canTargetOpponent);
    EXPECT_FALSE(modes[1].selectable);
}

TEST_F(RuledClientTest, AppliesAuthoritativeActivatedAbilityModeDataPerAbility)
{
    constexpr quint32 sourceOid = 0x102u;
    constexpr int abilityIndex = 3;
    constexpr quint32 targetOid = 0x205u;
    constexpr quint64 targetGeneration = 0x123456789abcdef0ull;
    const quint64 abilityKey = RuledClientState::abilityTargetKey(sourceOid, abilityIndex);

    ruled::v1::RuledEventBatch batch;
    auto &actions = (*batch.mutable_legal_by_player())[kLocalPlayer];
    auto &offer = (*actions.mutable_activated_ability_modes())[abilityKey];
    offer.set_min_modes(1);
    offer.set_max_modes(1);

    auto *targetless = offer.add_modes();
    targetless->set_mode_index(0);
    targetless->set_label("Targetless mode");
    targetless->set_selectable(true);
    targetless->set_needs_target(false);

    auto *targeted = offer.add_modes();
    targeted->set_mode_index(2);
    targeted->mutable_presentation()->set_fallback_text("Targeted mode");
    targeted->set_selectable(true);
    targeted->set_needs_target(true);
    auto *group = targeted->mutable_targets()->add_groups();
    group->set_group_index(0);
    group->set_min(1);
    group->set_max(1);
    group->add_valid_permanent_ids(targetOid);
    auto *candidate = group->add_valid_permanent_targets();
    candidate->set_object_id(targetOid);
    candidate->set_zone_change_generation(targetGeneration);

    apply(batch);

    ASSERT_TRUE(state->activatedAbilityModesByAbility.contains(abilityKey));
    const auto parsedOffer = state->activatedAbilityModesByAbility.value(abilityKey);
    EXPECT_EQ(parsedOffer.minModes, 1);
    EXPECT_EQ(parsedOffer.maxModes, 1);
    ASSERT_EQ(parsedOffer.modes.size(), 2);
    EXPECT_EQ(parsedOffer.modes[0].modeIndex, 0);
    EXPECT_EQ(parsedOffer.modes[0].label, QStringLiteral("Targetless mode"));
    EXPECT_TRUE(parsedOffer.modes[0].selectable);
    EXPECT_FALSE(parsedOffer.modes[0].needsTarget);
    EXPECT_TRUE(parsedOffer.modes[0].targets.groups.isEmpty());
    EXPECT_EQ(parsedOffer.modes[1].modeIndex, 2);
    EXPECT_EQ(parsedOffer.modes[1].label, QStringLiteral("Targeted mode"));
    EXPECT_TRUE(parsedOffer.modes[1].selectable);
    EXPECT_TRUE(parsedOffer.modes[1].needsTarget);
    ASSERT_EQ(parsedOffer.modes[1].targets.groups.size(), 1);
    const auto &parsedGroup = parsedOffer.modes[1].targets.groups[0];
    EXPECT_TRUE(parsedGroup.validPermanentIds.contains(targetOid));
    EXPECT_EQ(parsedGroup.validPermanentGenerations.value(targetOid), targetGeneration);

    ruled::v1::RuledEventBatch refreshed;
    (*refreshed.mutable_legal_by_player())[kLocalPlayer];
    apply(refreshed);
    EXPECT_FALSE(state->activatedAbilityModesByAbility.contains(abilityKey));
}

TEST_F(RuledClientTest, ActivatedAbilityModePreviewCarriesOnlyModeTargetsAndGeneration)
{
    constexpr quint32 sourceOid = 0x302u;
    constexpr int abilityIndex = 4;
    constexpr quint32 targetOid = 0x405u;
    constexpr quint64 targetGeneration = 0x23456789abcdef01ull;
    const quint64 abilityKey = RuledClientState::abilityTargetKey(sourceOid, abilityIndex);

    ruled::v1::RuledEventBatch batch;
    auto &actions = (*batch.mutable_legal_by_player())[kLocalPlayer];
    auto &offer = (*actions.mutable_activated_ability_modes())[abilityKey];
    offer.set_min_modes(1);
    offer.set_max_modes(1);
    auto *targeted = offer.add_modes();
    targeted->set_mode_index(2);
    targeted->set_label("Return a tapped creature");
    targeted->set_selectable(true);
    targeted->set_needs_target(true);
    auto *group = targeted->mutable_targets()->add_groups();
    group->set_group_index(0);
    group->set_min(1);
    group->set_max(1);
    group->add_valid_permanent_ids(targetOid);
    auto *candidate = group->add_valid_permanent_targets();
    candidate->set_object_id(targetOid);
    candidate->set_zone_change_generation(targetGeneration);
    apply(batch);

    ASSERT_TRUE(state->activatedAbilityModesByAbility.contains(abilityKey));
    const auto offerData = state->activatedAbilityModesByAbility.value(abilityKey);
    ASSERT_EQ(offerData.modes.size(), 1);
    const auto selectedMode = offerData.modes.first(); // Result of the shared one-mode picker.
    ASSERT_EQ(selectedMode.targets.groups.size(), 1);
    const auto &targetGroup = selectedMode.targets.groups.first();
    ASSERT_EQ(targetGroup.validPermanentGenerations.value(targetOid), targetGeneration);

    PendingActivatedAbility pending;
    pending.permanentOid = sourceOid;
    pending.abilityIndex = abilityIndex;
    pending.expectedZoneChangeGeneration = 7;
    pending.selectedMode = selectedMode;
    pending.valid = true;
    pending.waitingForTarget = true;
    PendingActivatedAbility::Target selectedTarget;
    selectedTarget.ref.set_group_index(static_cast<quint32>(targetGroup.groupIndex));
    selectedTarget.ref.set_object_id(targetOid);
    selectedTarget.ref.set_kind(ruled::v1::TARGET_REF_KIND_PERMANENT);
    selectedTarget.ref.set_expected_zone_change_generation(targetGroup.validPermanentGenerations.value(targetOid));
    selectedTarget.zoneChangeGeneration = targetGroup.validPermanentGenerations.value(targetOid);
    EXPECT_EQ(ruledStageActivatedAbilityTarget(pending, targetGroup, selectedTarget),
              RuledActivatedTargetStageResult::Selected);

    ruled::v1::RuledCommand command;
    pending.writeActivationHeader(*command.mutable_activate_ability());
    RuledPayment payment;
    payment.begin();
    const auto preview = payment.requestAction(command);

    ASSERT_TRUE(preview.has_activate_ability());
    EXPECT_EQ(preview.transaction_id(), payment.transaction());
    EXPECT_EQ(preview.revision(), 1u);
    ASSERT_EQ(preview.activate_ability().selected_modes_size(), 1);
    const auto &serializedMode = preview.activate_ability().selected_modes(0);
    EXPECT_EQ(serializedMode.mode_index(), 2u);
    ASSERT_EQ(serializedMode.targets_size(), 1);
    EXPECT_EQ(serializedMode.targets(0).group_index(), 0u);
    EXPECT_EQ(serializedMode.targets(0).object_id(), targetOid);
    ASSERT_TRUE(serializedMode.targets(0).has_expected_zone_change_generation());
    EXPECT_EQ(serializedMode.targets(0).expected_zone_change_generation(), targetGeneration);
    EXPECT_EQ(preview.activate_ability().targets_size(), 0);

    ruled::v1::PaymentPreview staleReply;
    staleReply.set_transaction_id(preview.transaction_id());
    staleReply.set_revision(preview.revision() + 1);
    staleReply.set_valid(true);
    staleReply.set_complete(true);
    EXPECT_FALSE(payment.apply(staleReply));

    ruled::v1::PaymentPreview acceptedReply;
    acceptedReply.set_transaction_id(preview.transaction_id());
    acceptedReply.set_revision(preview.revision());
    acceptedReply.set_valid(true);
    acceptedReply.set_complete(true);
    EXPECT_TRUE(payment.apply(acceptedReply));
    EXPECT_EQ(payment.nextPreviewAction(true), RuledPayment::PreviewAction::Submit);

    ruled::v1::RuledCommand submitted;
    *submitted.mutable_activate_ability() = command.activate_ability();
    payment.writePayment(submitted);
    ASSERT_TRUE(submitted.activate_ability().has_payment());
    ASSERT_EQ(submitted.activate_ability().selected_modes_size(), 1);
    EXPECT_EQ(submitted.activate_ability().selected_modes(0).mode_index(), 2u);
}

TEST_F(RuledClientTest, ModalDecodingPreservesPresentationTargetsAndCostLinksAcrossCastSources)
{
    ruled::v1::RuledEventBatch batch;
    auto &actions = (*batch.mutable_legal_by_player())[kLocalPlayer];
    auto *hand = addHandAction(actions, ruled::v1::HAND_ACTION_CAST_SPELL, 5, "Modal spell", 1);
    auto *zone = actions.add_zone_cast_actions();
    zone->set_object_id(77);
    zone->set_face_index(1);
    zone->set_source_zone(ruled::v1::CAST_SOURCE_ZONE_EXILE);
    zone->set_casting_permission_id(8);
    zone->set_cast_method(ruled::v1::CAST_METHOD_PERMISSION);
    zone->set_zone_change_generation(12);
    const auto addModes = [](auto *action) {
        action->set_min_modes(1);
        action->set_max_modes(2);
        action->mutable_all_modes_cast_cost()->set_group_index(4);
        action->mutable_all_modes_cast_cost()->set_option_index(6);
        auto *mode = action->add_modes();
        mode->set_mode_index(3);
        mode->set_label("Unused label");
        mode->mutable_presentation()->set_fallback_text("Presented mode");
        mode->set_selectable(true);
        mode->set_needs_target(true);
        mode->mutable_linked_cast_cost()->set_group_index(2);
        mode->mutable_linked_cast_cost()->set_option_index(7);
        auto *group = mode->mutable_targets()->add_groups();
        group->set_group_index(0);
        group->set_min(1);
        group->set_max(1);
        group->set_can_target_opponent(true);
        auto *disabled = action->add_modes();
        disabled->set_mode_index(9);
        disabled->set_label("Disabled mode");
    };
    addModes(hand);
    addModes(zone);
    apply(batch);

    const RuledCastActionKey handKey{5, 1, RuledCastSource::Hand, ruled::v1::CAST_METHOD_NORMAL, 0};
    const RuledCastActionKey zoneKey{77, 1, RuledCastSource::Exile, ruled::v1::CAST_METHOD_PERMISSION, 8};
    const auto checkModes = [](const RuledHandActionSet &set, const RuledCastActionKey &key) {
        EXPECT_EQ(set.modalMinModesByCastKey.value(key), 1);
        EXPECT_EQ(set.modalMaxModesByCastKey.value(key), 2);
        EXPECT_EQ(set.allModesCastCostByCastKey.value(key), qMakePair(4, 6));
        const auto modes = set.modalOptionsByCastKey.value(key);
        ASSERT_EQ(modes.size(), 2);
        EXPECT_EQ(modes[0].modeIndex, 3);
        EXPECT_EQ(modes[0].label, QStringLiteral("Presented mode"));
        EXPECT_TRUE(modes[0].selectable);
        EXPECT_TRUE(modes[0].needsTarget);
        EXPECT_TRUE(modes[0].targets.canTargetOpponent);
        EXPECT_EQ(modes[0].linkedCastCostGroupIndex, 2);
        EXPECT_EQ(modes[0].linkedCastCostOptionIndex, 7);
        EXPECT_EQ(modes[1].modeIndex, 9);
        EXPECT_EQ(modes[1].label, QStringLiteral("Disabled mode"));
        EXPECT_FALSE(modes[1].selectable);
        EXPECT_FALSE(modes[1].needsTarget);
        EXPECT_FALSE(modes[1].targets.canTargetOpponent);
        EXPECT_EQ(modes[1].linkedCastCostGroupIndex, -1);
        EXPECT_EQ(modes[1].linkedCastCostOptionIndex, -1);
    };
    checkModes(state->handActions[ruled::v1::HAND_ACTION_CAST_SPELL], handKey);
    checkModes(state->zoneCastActions, zoneKey);
    EXPECT_FALSE(state->zoneCastActions.modalOptionsByCastKey.contains(handKey));
    EXPECT_EQ(state->zoneCastActions.faceOptionsByIndex.value(77).first().zoneChangeGeneration, 12u);
    apply(ruled::v1::RuledEventBatch{});
    EXPECT_TRUE(state->zoneCastActions.modalOptionsByCastKey.isEmpty());
    EXPECT_TRUE(state->handActions.isEmpty());
}

TEST_F(RuledClientTest, AppliesStructuredCleanupDiscardActionsAndRequiredCount)
{
    ruled::v1::RuledEventBatch batch;
    auto &actions = (*batch.mutable_legal_by_player())[kLocalPlayer];
    // CR 514.1: nine cards in hand means two must be discarded.
    for (int i = 0; i < 9; ++i) {
        addHandAction(actions, ruled::v1::HAND_ACTION_CLEANUP_DISCARD, i, "Card" + std::to_string(i));
    }
    apply(batch);

    EXPECT_TRUE(state->localPlayerMustCleanupDiscard());
    EXPECT_EQ(state->cleanupDiscardRequiredCount(), 2);
    EXPECT_EQ(state->cleanupDiscardSelectedCount(), 0);
    // Cleanup clicks resolve by authoritative hand-slot identity. A client display-name mismatch
    // must not empty the candidate list and make the required discard unclickable.
    EXPECT_EQ(state->handActionClickCandidates(ruled::v1::HAND_ACTION_CLEANUP_DISCARD, "Different display name"),
              QList<int>({0, 1, 2, 3, 4, 5, 6, 7, 8}));

    state->toggleCleanupDiscardHandIndex(0);
    state->toggleCleanupDiscardHandIndex(4);
    EXPECT_EQ(state->cleanupDiscardSelectedIndicesSorted(), QList<int>({0, 4}));
    // Selection is capped at the required count.
    state->toggleCleanupDiscardHandIndex(6);
    EXPECT_EQ(state->cleanupDiscardSelectedCount(), 2);
    // Toggling off frees a slot again.
    state->toggleCleanupDiscardHandIndex(0);
    state->toggleCleanupDiscardHandIndex(6);
    EXPECT_EQ(state->cleanupDiscardSelectedIndicesSorted(), QList<int>({4, 6}));
    // Illegal slots are never selectable.
    state->clearCleanupDiscardSelection();
    state->toggleCleanupDiscardHandIndex(50);
    EXPECT_EQ(state->cleanupDiscardSelectedCount(), 0);
}

TEST_F(RuledClientTest, OpeningLabelsAloneNeverAuthorizeActions)
{
    ruled::v1::RuledEventBatch batch;
    auto &actions = (*batch.mutable_legal_by_player())[kLocalPlayer];
    actions.add_labels("You start (opening pick)");
    actions.add_labels("Keep opening hand (opening)");
    apply(batch);
    EXPECT_EQ(state->getOpeningUiKind(), RuledClientState::RuledOpeningUiKind::None);
    state->openingPickFirstSeat(kOpponent);
    state->openingMulliganKeep();
    state->openingMulliganRedraw();
    EXPECT_TRUE(host.sentCommands.empty());
    EXPECT_EQ(state->getOpeningMulliganCount(), 0);
}

TEST_F(RuledClientTest, StructuredOpeningSurvivesEmptyLabelsAndRefresh)
{
    ruled::v1::RuledEventBatch batch;
    auto &actions = (*batch.mutable_legal_by_player())[kLocalPlayer];
    auto *opening = actions.mutable_opening();
    opening->set_stage(ruled::v1::OPENING_STAGE_CHOOSE_STARTING_PLAYER);
    opening->set_deciding_player_id(kLocalPlayer);
    for (int seat : {7, 19, 42})
        opening->add_eligible_starting_player_ids(seat);
    apply(batch);
    EXPECT_EQ(state->getOpeningUiKind(), RuledClientState::RuledOpeningUiKind::ChooseFirst);
    EXPECT_EQ(state->getOpeningPickSeatIds(), QVector<int>({7, 19, 42}));
    state->openingPickFirstSeat(42);
    state->openingPickFirstSeat(99);
    ASSERT_EQ(host.sentCommands.size(), 1);
    EXPECT_EQ(host.sentCommands.back().choose_starting_player().starting_player_id(), 42);

    opening->Clear();
    opening->set_stage(ruled::v1::OPENING_STAGE_MULLIGAN);
    opening->set_deciding_player_id(kLocalPlayer);
    opening->set_can_keep(true);
    opening->set_can_redraw(true);
    opening->set_mulligans_taken(2);
    actions.add_labels("Wording may change freely");
    apply(batch);
    EXPECT_EQ(state->getOpeningUiKind(), RuledClientState::RuledOpeningUiKind::MulliganChoice);
    state->openingMulliganRedraw();
    EXPECT_EQ(state->getOpeningMulliganCount(), 2);
    apply(batch); // rejected redraw or reconnect: use the same authoritative count
    EXPECT_EQ(state->getOpeningMulliganCount(), 2);
    opening->set_can_redraw(false);
    apply(batch);
    host.sentCommands.clear();
    state->openingMulliganRedraw();
    EXPECT_TRUE(host.sentCommands.empty());
    state->openingMulliganKeep();
    ASSERT_EQ(host.sentCommands.size(), 1);

    opening->set_stage(ruled::v1::OPENING_STAGE_BOTTOM);
    opening->set_bottom_cards_remaining(1); // a reconnect after the first bottom command
    addHandAction(actions, ruled::v1::HAND_ACTION_OPENING_BOTTOM, 0, "Forest");
    apply(batch);
    EXPECT_EQ(state->getOpeningUiKind(), RuledClientState::RuledOpeningUiKind::BottomLibrary);
    EXPECT_EQ(state->openingBottomRequiredCount(), 1);
    EXPECT_EQ(state->getOpeningMulliganCount(), 2);

    opening->set_deciding_player_id(kOpponent);
    apply(batch);
    EXPECT_EQ(state->getOpeningUiKind(), RuledClientState::RuledOpeningUiKind::None);
    EXPECT_TRUE(state->isWaitingForChoice());
    EXPECT_EQ(state->choiceWaitingPlayer(), kOpponent);
    opening->set_stage(static_cast<ruled::v1::OpeningStage>(999));
    apply(batch);
    EXPECT_FALSE(state->isWaitingForChoice());
    EXPECT_EQ(state->getOpeningUiKind(), RuledClientState::RuledOpeningUiKind::None);
    actions.clear_opening();
    apply(batch);
    EXPECT_EQ(state->getOpeningMulliganCount(), 0);
    EXPECT_EQ(state->openingBottomRequiredCount(), 0);
}

TEST_F(RuledClientTest, OpeningSeatDisplayStartsWithYouAndFollowsEngineTurnOrder)
{
    ruled::v1::RuledEventBatch batch;
    auto *opening = (*batch.mutable_legal_by_player())[kLocalPlayer].mutable_opening();
    opening->set_stage(ruled::v1::OPENING_STAGE_CHOOSE_STARTING_PLAYER);
    opening->set_deciding_player_id(kLocalPlayer);
    for (const int seat : {7, kLocalPlayer, 19})
        opening->add_eligible_starting_player_ids(seat);
    apply(batch);
    EXPECT_EQ(state->getOpeningPickSeatIds(), QVector<int>({kLocalPlayer, 19, 7}));
    for (const int seat : state->getOpeningPickSeatIds())
        state->openingPickFirstSeat(seat);
    ASSERT_EQ(host.sentCommands.size(), 3);
    EXPECT_EQ(host.sentCommands[0].choose_starting_player().starting_player_id(), kLocalPlayer);
    EXPECT_EQ(host.sentCommands[1].choose_starting_player().starting_player_id(), 19);
    EXPECT_EQ(host.sentCommands[2].choose_starting_player().starting_player_id(), 7);
}

TEST_F(RuledClientTest, ParsesTargetingTablesForHandSlotsAndAbilities)
{
    ruled::v1::RuledEventBatch batch;
    auto &actions = (*batch.mutable_legal_by_player())[kLocalPlayer];
    // Hand slot 1, face 0 — the composite key the engine emits.
    auto &slotTargets = (*actions.mutable_valid_targets_by_hand_slot())[(1u << 8) | 0u];
    slotTargets.set_is_damage_targets(true);
    slotTargets.set_fixed_damage(4);
    slotTargets.set_damage_division(ruled::v1::DAMAGE_DIVISION_PER_TARGET);
    slotTargets.set_extra_mana_per_target(1);
    auto *group = slotTargets.add_groups();
    group->set_group_index(0);
    group->set_prompt_text("Choose up to two target creatures");
    group->set_min(0);
    group->set_max(2);
    group->add_valid_permanent_ids(101);
    group->add_valid_stack_ids(300);
    group->add_valid_graveyard_ids(500);
    group->set_can_target_opponent(true);
    auto *secondGroup = slotTargets.add_groups();
    secondGroup->set_group_index(1);
    secondGroup->set_prompt_text("Choose another target");
    secondGroup->set_min(1);
    secondGroup->set_max(1);
    secondGroup->add_valid_permanent_ids(202);
    secondGroup->add_distinct_from_group_indices(0);
    // Ability index 2 on permanent 100.
    auto &abilityTargets = (*actions.mutable_valid_targets_by_ability())[(quint64(100) << 32) | 2u];
    auto *abilityGroup = abilityTargets.add_groups();
    abilityGroup->set_group_index(0);
    abilityGroup->set_prompt_text("Choose target");
    abilityGroup->set_min(1);
    abilityGroup->set_max(1);
    abilityGroup->add_valid_permanent_ids(200);
    abilityGroup->set_can_target_self(true);
    auto *xTargetChoices = abilityGroup->mutable_x_target_choices();
    auto *xTargetChoice = xTargetChoices->add_choices();
    xTargetChoice->set_x_value(4);
    auto *xTargetCandidate = xTargetChoice->add_candidates();
    xTargetCandidate->set_object_id(200);
    xTargetCandidate->set_zone_change_generation(7);
    apply(batch);

    EXPECT_FALSE(state->isValidSpellTarget(1, 0, 100));
    EXPECT_TRUE(state->isValidSpellTarget(1, 0, 101));
    EXPECT_EQ(state->spellTargetData(1, 0).damageDivision, ruled::v1::DAMAGE_DIVISION_PER_TARGET);
    EXPECT_EQ(state->spellTargetData(1, 0).minTargets, 0);
    EXPECT_EQ(state->spellTargetData(1, 0).maxTargets, 2);
    EXPECT_EQ(state->spellTargetData(1, 0).promptText, "Choose up to two target creatures");
    ASSERT_EQ(state->spellTargetData(1, 0).groups.size(), 2);
    EXPECT_EQ(state->spellTargetData(1, 0).groups.at(1).validPermanentIds, QSet<quint32>({202}));
    EXPECT_EQ(state->spellTargetData(1, 0).groups.at(1).distinctFromGroupIndices, QVector<int>({0}));
    const auto parsedAbilityTargets = state->abilityTargetData(100, 2);
    ASSERT_EQ(parsedAbilityTargets.groups.size(), 1);
    EXPECT_TRUE(parsedAbilityTargets.groups.first().hasXTargetChoices);
    EXPECT_EQ(ruledAvailableXTargetValues(parsedAbilityTargets.groups.first()), QVector<quint32>({4}));
    EXPECT_EQ(ruledXTargetCandidateGeneration(parsedAbilityTargets.groups.first(), 4, 200),
              std::optional<quint64>(7));
    // A different face of the same slot carries its own (here: empty) target set.
    EXPECT_FALSE(state->isValidSpellTarget(1, 1, 100));
    EXPECT_TRUE(state->isValidSpellStackTarget(1, 0, 300));
    EXPECT_TRUE(state->isValidSpellGraveyardTarget(1, 0, 500));
    EXPECT_TRUE(state->canSpellTargetOpponent(1, 0));
    EXPECT_FALSE(state->canSpellTargetSelf(1, 0));
    EXPECT_TRUE(state->spellIsDamageTargets(1, 0));
    EXPECT_EQ(state->spellMaxTargets(1, 0), 2);
    EXPECT_EQ(state->spellFixedDamage(1, 0), 4);
    EXPECT_EQ(state->spellExtraManaPerTarget(1, 0), 1);

    EXPECT_TRUE(state->abilityNeedsTarget(100, 2));
    EXPECT_FALSE(state->abilityNeedsTarget(100, 0));
    EXPECT_TRUE(state->isValidAbilityTarget(100, 2, 200));
    EXPECT_TRUE(state->canAbilityTargetSelf(100, 2));
}

TEST_F(RuledClientTest, MultiZoneSearchCarriesSourceAnnotationsAndMalformedMetadataFailsClosed)
{
    QSignalSpy started(state, &RuledClientState::librarySearchPickStarted);
    ruled::v1::RuledEventBatch batch;
    auto *choice = batch.add_events()->mutable_resolution_choice_required();
    choice->set_deciding_player_id(kLocalPlayer);
    choice->set_choice_kind(ruled::v1::CHOICE_KIND_ZONE_SEARCH);
    choice->set_prompt_text("Search the selected zones for Altanak");
    choice->set_min(0);
    choice->set_max(7);
    for (const quint32 oid : {71u, 72u, 73u, 74u, 75u, 76u, 77u})
        choice->add_candidate_object_ids(oid);
    for (const int scid : {0, 1, 2, 3, 4, 5, 6})
        choice->add_candidate_server_card_ids(scid);
    for (const char *name : {"A hand card", "A graveyard card", "A library card", "A battlefield card",
                             "An exiled card", "A stack spell", "A commander card"})
        choice->add_candidate_names(name);
    choice->add_candidate_source_zones(ruled::v1::CHOICE_CANDIDATE_SOURCE_ZONE_HAND);
    choice->add_candidate_source_zones(ruled::v1::CHOICE_CANDIDATE_SOURCE_ZONE_GRAVEYARD);
    choice->add_candidate_source_zones(ruled::v1::CHOICE_CANDIDATE_SOURCE_ZONE_LIBRARY);
    choice->add_candidate_source_zones(ruled::v1::CHOICE_CANDIDATE_SOURCE_ZONE_BATTLEFIELD);
    choice->add_candidate_source_zones(ruled::v1::CHOICE_CANDIDATE_SOURCE_ZONE_EXILE);
    choice->add_candidate_source_zones(ruled::v1::CHOICE_CANDIDATE_SOURCE_ZONE_STACK);
    choice->add_candidate_source_zones(ruled::v1::CHOICE_CANDIDATE_SOURCE_ZONE_COMMAND);
    apply(batch);

    ASSERT_TRUE(state->isResolutionHandPickActive());
    EXPECT_EQ(state->resolutionHandPickViewTitle(), QStringLiteral("Search selected zones"));
    EXPECT_FALSE(state->resolutionHandPickShowViewControls());
    EXPECT_EQ(state->resolutionHandPickCandidateAnnotations(),
              QStringList({QStringLiteral("Hand"), QStringLiteral("Graveyard"), QStringLiteral("Library"),
                           QStringLiteral("Battlefield"), QStringLiteral("Exile"), QStringLiteral("Stack"),
                           QStringLiteral("Command")}));
    ASSERT_TRUE(state->pendingChoice.has_value());
    for (int i = 0; i < 7; ++i) {
        EXPECT_EQ(state->pendingChoice->serverCardIdToOid.value(i), static_cast<quint32>(71 + i));
    }
    ASSERT_EQ(started.count(), 1);

    state->clearPendingChoice();
    ruled::v1::RuledEventBatch malformed;
    auto *bad = malformed.add_events()->mutable_resolution_choice_required();
    bad->CopyFrom(*choice);
    bad->mutable_candidate_source_zones()->RemoveLast();
    apply(malformed);
    EXPECT_FALSE(state->isResolutionHandPickActive());
}

TEST_F(RuledClientTest, BeholdUsesAnAnnotatedImagePickerAndMalformedMetadataFailsClosed)
{
    QSignalSpy started(state, &RuledClientState::librarySearchPickStarted);
    ruled::v1::RuledEventBatch batch;
    auto *choice = batch.add_events()->mutable_resolution_choice_required();
    choice->set_deciding_player_id(kLocalPlayer);
    choice->set_choice_kind(ruled::v1::CHOICE_KIND_BEHOLD);
    choice->set_prompt_text("You may behold an Elf");
    choice->set_min(0);
    choice->set_max(1);
    choice->add_candidate_object_ids(71);
    choice->add_candidate_object_ids(72);
    choice->add_candidate_server_card_ids(0);
    choice->add_candidate_server_card_ids(1);
    choice->add_candidate_names("Llanowar Elves");
    choice->add_candidate_names("Safewright Cavalry");
    choice->add_candidate_source_zones(ruled::v1::CHOICE_CANDIDATE_SOURCE_ZONE_HAND);
    choice->add_candidate_source_zones(ruled::v1::CHOICE_CANDIDATE_SOURCE_ZONE_BATTLEFIELD);
    apply(batch);

    ASSERT_TRUE(state->isResolutionHandPickActive());
    EXPECT_EQ(state->resolutionHandPickViewTitle(), QStringLiteral("Behold"));
    EXPECT_FALSE(state->resolutionHandPickShowViewControls());
    EXPECT_EQ(state->resolutionHandPickCandidateAnnotations(),
              QStringList({QStringLiteral("Hand"), QStringLiteral("Battlefield")}));
    EXPECT_EQ(host.dialogRequests, 0);
    ASSERT_EQ(started.count(), 1);

    state->clearPendingChoice();
    ruled::v1::RuledEventBatch malformed;
    auto *bad = malformed.add_events()->mutable_resolution_choice_required();
    bad->CopyFrom(*choice);
    bad->mutable_candidate_source_zones()->RemoveLast();
    apply(malformed);
    EXPECT_FALSE(state->isResolutionHandPickActive());
    EXPECT_EQ(started.count(), 1);
}

TEST_F(RuledClientTest, ParsesAuthoritativeActivatedCostChoicesAndPayability)
{
    ruled::v1::RuledEventBatch batch;
    auto &actions = (*batch.mutable_legal_by_player())[kLocalPlayer];
    const quint64 key = (quint64(100) << 32) | 2u;
    auto &costs = (*actions.mutable_cost_choices_by_ability())[key];
    costs.set_non_mana_costs_payable(false);
    auto *discard = costs.add_choices();
    discard->set_cost_index(0);
    discard->set_zone(ruled::v1::COST_CHOICE_ZONE_HAND);
    discard->add_candidate_ids(1);
    discard->add_candidate_ids(3);
    auto *sacrifice = costs.add_choices();
    sacrifice->set_cost_index(2);
    sacrifice->set_zone(ruled::v1::COST_CHOICE_ZONE_BATTLEFIELD);
    sacrifice->set_kind(ruled::v1::COST_CHOICE_KIND_SACRIFICE);
    sacrifice->add_candidate_ids(100);
    auto *sacrificeRef = sacrifice->add_candidate_objects();
    sacrificeRef->mutable_object()->set_object_id(100);
    sacrificeRef->mutable_object()->set_zone_change_generation(7);
    auto *graveyard = costs.add_choices();
    graveyard->set_cost_index(3);
    graveyard->set_zone(ruled::v1::COST_CHOICE_ZONE_GRAVEYARD);
    graveyard->set_min(2);
    graveyard->set_max(2);
    graveyard->add_candidate_ids(501);
    graveyard->add_candidate_ids(502);
    apply(batch);

    EXPECT_FALSE(state->abilityActivatable(100, 2));
    const auto choices = state->abilityCostChoices(100, 2);
    ASSERT_EQ(choices.size(), 3);
    EXPECT_EQ(choices.at(0).costIndex, 0);
    EXPECT_EQ(choices.at(0).zone, RuledCostChoiceZone::Hand);
    EXPECT_EQ(choices.at(0).candidateIds, QSet<quint32>({1, 3}));
    EXPECT_EQ(choices.at(1).zone, RuledCostChoiceZone::Battlefield);
    EXPECT_EQ(choices.at(1).kind, RuledCostChoiceKind::Sacrifice);
    EXPECT_TRUE(choices.at(1).candidateIds.contains(100));
    EXPECT_EQ(choices.at(1).candidateGenerations.value(100), 7u);
    EXPECT_EQ(choices.at(2).zone, RuledCostChoiceZone::Graveyard);
    EXPECT_EQ(choices.at(2).min, 2);
    EXPECT_EQ(choices.at(2).max, 2);
    EXPECT_EQ(choices.at(2).candidateIds, QSet<quint32>({501, 502}));
}

TEST_F(RuledClientTest, AggregateCounterOffersPreserveKindsQuantitiesAndIncarnations)
{
    auto batch = unsupportedCostBatch(static_cast<int>(ruled::v1::COST_CHOICE_KIND_REMOVE_COUNTERS_FROM_PERMANENTS));
    auto &actions = (*batch.mutable_legal_by_player())[kLocalPlayer];
    const quint64 key = (static_cast<quint64>(kUnsupportedCostSourceOid) << 32) |
                        static_cast<quint32>(kUnsupportedCostAbilityIndex);
    auto &costs = (*actions.mutable_cost_choices_by_ability())[key];
    auto *choice = costs.mutable_choices(0);
    auto *group = choice->mutable_counter_removal_group();
    group->set_required_count(3);
    group->clear_candidates();

    auto *first = group->add_candidates();
    first->mutable_object()->set_object_id(901);
    first->mutable_object()->set_zone_change_generation(13);
    auto *night = first->add_options();
    night->set_option_id(0xf00dbaad);
    night->set_label("Night");
    night->set_available_count(2);
    auto *firstCharge = first->add_options();
    firstCharge->set_option_id(0x51a7e001);
    firstCharge->set_label("Charge");
    firstCharge->set_available_count(3);

    auto *second = group->add_candidates();
    second->mutable_object()->set_object_id(902);
    second->mutable_object()->set_zone_change_generation(17);
    auto *secondCharge = second->add_options();
    secondCharge->set_option_id(0x51a7e001);
    secondCharge->set_label("Charge");
    secondCharge->set_available_count(2);

    applyRaw(batch);

    ASSERT_TRUE(state->activatedAbilityForOid(kUnsupportedCostSourceOid, kUnsupportedCostAbilityIndex));
    const auto stateKey = RuledClientState::abilityTargetKey(kUnsupportedCostSourceOid, kUnsupportedCostAbilityIndex);
    ASSERT_TRUE(state->abilityCostData.contains(stateKey));
    EXPECT_TRUE(state->abilityCostData.value(stateKey).nonManaCostsPayable);
    EXPECT_TRUE(state->abilityActivatable(kUnsupportedCostSourceOid, kUnsupportedCostAbilityIndex));
    const auto choices = state->abilityCostChoices(kUnsupportedCostSourceOid, kUnsupportedCostAbilityIndex);
    ASSERT_EQ(choices.size(), 1);
    EXPECT_EQ(choices.front().kind, RuledCostChoiceKind::RemoveCountersFromPermanents);
    ASSERT_TRUE(choices.front().counterRemovalGroup.has_value());

    const auto &parsed = *choices.front().counterRemovalGroup;
    EXPECT_EQ(parsed.requiredCount, 3u);
    ASSERT_EQ(parsed.candidates.size(), 2);
    EXPECT_EQ(parsed.candidates.at(0).objectId, 901u);
    EXPECT_EQ(parsed.candidates.at(0).zoneChangeGeneration, 13u);
    ASSERT_EQ(parsed.candidates.at(0).options.size(), 2);
    EXPECT_EQ(parsed.candidates.at(0).options.at(0).optionId, 0xf00dbaadu);
    EXPECT_EQ(parsed.candidates.at(0).options.at(0).label, QStringLiteral("Night"));
    EXPECT_EQ(parsed.candidates.at(0).options.at(0).availableCount, 2u);
    EXPECT_EQ(parsed.candidates.at(0).options.at(1).optionId, 0x51a7e001u);
    EXPECT_EQ(parsed.candidates.at(0).options.at(1).label, QStringLiteral("Charge"));
    EXPECT_EQ(parsed.candidates.at(0).options.at(1).availableCount, 3u);
    EXPECT_EQ(parsed.candidates.at(1).objectId, 902u);
    EXPECT_EQ(parsed.candidates.at(1).zoneChangeGeneration, 17u);
    ASSERT_EQ(parsed.candidates.at(1).options.size(), 1);
    EXPECT_EQ(parsed.candidates.at(1).options.at(0).optionId, 0x51a7e001u);
    EXPECT_EQ(parsed.candidates.at(1).options.at(0).label, QStringLiteral("Charge"));
    EXPECT_EQ(parsed.candidates.at(1).options.at(0).availableCount, 2u);
}

TEST_F(RuledClientTest, AggregateCounterOffersRejectMalformedGroupsAtomically)
{
    const auto checkRejected = [&](ruled::v1::RuledEventBatch batch) {
        applyRaw(batch);
        const auto choices = state->abilityCostChoices(kUnsupportedCostSourceOid, kUnsupportedCostAbilityIndex);
        ASSERT_EQ(choices.size(), 1);
        EXPECT_EQ(choices.front().kind, RuledCostChoiceKind::Unspecified);
        EXPECT_FALSE(choices.front().counterRemovalGroup.has_value());
        const auto key = RuledClientState::abilityTargetKey(kUnsupportedCostSourceOid, kUnsupportedCostAbilityIndex);
        ASSERT_TRUE(state->abilityCostData.contains(key));
        EXPECT_FALSE(state->abilityCostData.value(key).nonManaCostsPayable);
        EXPECT_FALSE(state->abilityActivatable(kUnsupportedCostSourceOid, kUnsupportedCostAbilityIndex));
    };

    for (int malformed = 0; malformed < 10; ++malformed) {
        auto batch = unsupportedCostBatch(static_cast<int>(ruled::v1::COST_CHOICE_KIND_REMOVE_COUNTERS_FROM_PERMANENTS));
        auto &actions = (*batch.mutable_legal_by_player())[kLocalPlayer];
        const quint64 wireKey = (static_cast<quint64>(kUnsupportedCostSourceOid) << 32) |
                                static_cast<quint32>(kUnsupportedCostAbilityIndex);
        auto *choice = (*actions.mutable_cost_choices_by_ability())[wireKey].mutable_choices(0);
        auto *group = choice->mutable_counter_removal_group();
        auto *candidate = group->mutable_candidates(0);
        auto *option = candidate->mutable_options(0);
        const auto existingOptionId = option->option_id();
        switch (malformed) {
            case 0:
                choice->clear_counter_removal_group();
                break;
            case 1:
                group->set_required_count(0);
                break;
            case 2:
                candidate->clear_object();
                break;
            case 3:
                candidate->mutable_object()->set_object_id(0);
                break;
            case 4: {
                auto *duplicate = group->add_candidates();
                duplicate->mutable_object()->set_object_id(901);
                duplicate->mutable_object()->set_zone_change_generation(99);
                auto *duplicateOption = duplicate->add_options();
                duplicateOption->set_option_id(2);
                duplicateOption->set_available_count(1);
                break;
            }
            case 5:
                candidate->clear_options();
                break;
            case 6: {
                auto *duplicateOption = candidate->add_options();
                duplicateOption->set_option_id(existingOptionId);
                duplicateOption->set_available_count(1);
                break;
            }
            case 7:
                option->set_option_id(0);
                break;
            case 8:
                option->set_available_count(0);
                break;
            case 9: {
                const auto *zoneField = choice->GetDescriptor()->FindFieldByName("zone");
                choice->GetReflection()->SetEnumValue(choice, zoneField, 777);
                break;
            }
        }
        checkRejected(std::move(batch));
    }

    auto unknownKind = unsupportedCostBatch(static_cast<int>(ruled::v1::COST_CHOICE_KIND_REMOVE_COUNTERS_FROM_PERMANENTS));
    auto &actions = (*unknownKind.mutable_legal_by_player())[kLocalPlayer];
    const quint64 wireKey = (static_cast<quint64>(kUnsupportedCostSourceOid) << 32) |
                            static_cast<quint32>(kUnsupportedCostAbilityIndex);
    auto *choice = (*actions.mutable_cost_choices_by_ability())[wireKey].mutable_choices(0);
    setWireCostChoiceKind(choice, 777);
    checkRejected(std::move(unknownKind));
}

TEST_F(RuledClientTest, AggregateCounterOfferAcceptsZeroIncarnationGeneration)
{
    auto batch = unsupportedCostBatch(static_cast<int>(ruled::v1::COST_CHOICE_KIND_REMOVE_COUNTERS_FROM_PERMANENTS));
    auto &actions = (*batch.mutable_legal_by_player())[kLocalPlayer];
    const quint64 wireKey = (static_cast<quint64>(kUnsupportedCostSourceOid) << 32) |
                            static_cast<quint32>(kUnsupportedCostAbilityIndex);
    auto *choice = (*actions.mutable_cost_choices_by_ability())[wireKey].mutable_choices(0);
    choice->mutable_counter_removal_group()->mutable_candidates(0)->mutable_object()->set_zone_change_generation(0);
    applyRaw(batch);

    const auto choices = state->abilityCostChoices(kUnsupportedCostSourceOid, kUnsupportedCostAbilityIndex);
    ASSERT_EQ(choices.size(), 1);
    EXPECT_EQ(choices.front().kind, RuledCostChoiceKind::RemoveCountersFromPermanents);
    ASSERT_TRUE(choices.front().counterRemovalGroup.has_value());
    ASSERT_EQ(choices.front().counterRemovalGroup->candidates.size(), 1);
    EXPECT_EQ(choices.front().counterRemovalGroup->candidates.front().zoneChangeGeneration, 0u);
    EXPECT_TRUE(state->abilityActivatable(kUnsupportedCostSourceOid, kUnsupportedCostAbilityIndex));
}

TEST_F(RuledClientTest, CounterGroupBeginRechecksEngineNonManaPayability)
{
    auto batch = aggregateCounterOfferBatch();
    const quint64 wireKey = (static_cast<quint64>(kUnsupportedCostSourceOid) << 32) |
                            static_cast<quint32>(kUnsupportedCostAbilityIndex);
    auto &actions = (*batch.mutable_legal_by_player())[kLocalPlayer];
    (*actions.mutable_cost_choices_by_ability())[wireKey].set_non_mana_costs_payable(false);
    applyRaw(batch);

    const auto rawAbility = state->activatedAbilityForOid(kUnsupportedCostSourceOid, kUnsupportedCostAbilityIndex);
    ASSERT_TRUE(rawAbility.has_value());
    EXPECT_TRUE(rawAbility->activatable);
    EXPECT_FALSE(state->abilityCostData.value(wireKey).nonManaCostsPayable);
    EXPECT_FALSE(state->abilityActivatable(kUnsupportedCostSourceOid, kUnsupportedCostAbilityIndex));

    auto directDraft = makeAggregateCounterDraft(*state);
    ASSERT_TRUE(ruledStageCounterContributions(directDraft, 1, 901, 13, aggregateAcceptedCounterRows()));
    ASSERT_TRUE(ruledConfirmCounterContributions(directDraft));
    EXPECT_FALSE(ruledBuildCounterGroupBegin(*state, directDraft, aggregatePhiPayments(directDraft)).has_value());

    RuledPendingCast owner;
    auto &ownedDraft = prepareOwnedAggregateCounterDraft(owner, *state);
    ASSERT_TRUE(ruledStageCounterContributions(ownedDraft, 1, 901, 13, aggregateAcceptedCounterRows()));
    ASSERT_TRUE(ruledConfirmCounterContributions(ownedDraft));
    EXPECT_FALSE(owner.prepareCounterGroupBegin(*state, kLocalPlayer, aggregatePhiPayments(ownedDraft)).has_value());
    EXPECT_EQ(ownedDraft.ownedBegin(), nullptr);
    EXPECT_EQ(ownedDraft.stage(), PendingActivatedAbility::Stage::Announcing);
    EXPECT_FALSE(state->pendingAbilityActivation.has_value());
    EXPECT_FALSE(ruledCounterQuantityContext(*state, ownedDraft, 1, 901, 13, QStringLiteral("eligible permanent"))
                     .has_value());
    EXPECT_TRUE(host.sentCommands.isEmpty());
}

TEST_F(RuledClientTest, AggregateCounterSelectionSerializesTwoKindsOnOnePermanentAndSplitCards)
{
    applyRaw(aggregateCounterOfferBatch());
    EXPECT_TRUE(state->abilityActivatable(kUnsupportedCostSourceOid, kUnsupportedCostAbilityIndex));

    const auto makeDraft = [&]() {
        return makeAggregateCounterDraft(*state);
    };
    const QVector<QVector<RuledPendingCounterContribution>> patterns = {
        {{1, 901, 13, kNightCounterOptionId, 2}, {1, 901, 13, kChargeCounterOptionId, 1}},
        {{1, 901, 13, kNightCounterOptionId, 1}, {1, 902, 17, kChargeCounterOptionId, 2}},
        {{1, 901, 13, kChargeCounterOptionId, 3}},
        {{1, 901, 13, kChargeCounterOptionId, 1},
         {1, 902, 17, kChargeCounterOptionId, 1},
         {1, 903, 21, kChargeCounterOptionId, 1}},
    };

    for (int patternIndex = 0; patternIndex < patterns.size(); ++patternIndex) {
        auto pending = makeDraft();
        QVector<QPair<quint32, quint64>> receipts;
        for (const auto &row : patterns.at(patternIndex)) {
            const QPair<quint32, quint64> receipt{row.objectId, row.zoneChangeGeneration};
            if (!receipts.contains(receipt)) {
                receipts.append(receipt);
            }
        }
        for (const auto &receipt : receipts) {
            QVector<RuledPendingCounterContribution> receiptRows;
            for (const auto &row : patterns.at(patternIndex)) {
                if (row.objectId == receipt.first && row.zoneChangeGeneration == receipt.second) {
                    receiptRows.append(row);
                }
            }
            EXPECT_TRUE(ruledStageCounterContributions(pending, 1, receipt.first, receipt.second, receiptRows))
                << "pattern " << patternIndex;
        }
        EXPECT_TRUE(ruledConfirmCounterContributions(pending)) << "pattern " << patternIndex;

        ASSERT_EQ(pending.flexPips.size(), 2);
        auto flexPayments = aggregatePhiPayments(pending, patternIndex % 2 != 0);
        const auto generated = ruledBuildCounterGroupBegin(*state, pending, flexPayments);
        EXPECT_TRUE(generated.has_value()) << "pattern " << patternIndex;
        if (generated) {
            std::string bytes;
            ASSERT_TRUE(generated->SerializeToString(&bytes));
            ruled::v1::BeginAbilityActivation roundTrip;
            ASSERT_TRUE(roundTrip.ParseFromString(bytes));
            EXPECT_EQ(roundTrip.source_object_id(), kUnsupportedCostSourceOid);
            EXPECT_EQ(roundTrip.expected_zone_change_generation(), kUnsupportedCostSourceGeneration);
            EXPECT_EQ(roundTrip.source_zone(), ruled::v1::ABILITY_SOURCE_ZONE_BATTLEFIELD);
            EXPECT_EQ(roundTrip.ability_index(), static_cast<quint32>(kUnsupportedCostAbilityIndex));
            EXPECT_FALSE(roundTrip.has_own_target());
            EXPECT_FALSE(roundTrip.has_opponent_player_id());
            ASSERT_EQ(roundTrip.cost_selections_size(), 1);
            const auto &selection = roundTrip.cost_selections(0);
            EXPECT_EQ(selection.cost_index(), 1u);
            ASSERT_TRUE(selection.has_counter_removal_group());
            ASSERT_EQ(selection.counter_removal_group().contributions_size(), patterns.at(patternIndex).size());
            auto expectedRows = patterns.at(patternIndex);
            std::sort(expectedRows.begin(), expectedRows.end(), [](const auto &left, const auto &right) {
                return std::tie(left.costIndex, left.objectId, left.zoneChangeGeneration, left.optionId) <
                       std::tie(right.costIndex, right.objectId, right.zoneChangeGeneration, right.optionId);
            });
            for (int rowIndex = 0; rowIndex < expectedRows.size(); ++rowIndex) {
                const auto &expected = expectedRows.at(rowIndex);
                const auto &actual = selection.counter_removal_group().contributions(rowIndex);
                EXPECT_EQ(actual.object().object_id(), expected.objectId);
                EXPECT_EQ(actual.object().zone_change_generation(), expected.zoneChangeGeneration);
                EXPECT_EQ(actual.option_id(), expected.optionId);
                EXPECT_EQ(actual.count(), expected.count);
            }
            ASSERT_EQ(roundTrip.flex_payments_size(), 2);
            EXPECT_EQ(roundTrip.flex_payments(0).pip_index(), pending.flexPips.at(0).pipIndex);
            EXPECT_EQ(roundTrip.flex_payments(1).pip_index(), pending.flexPips.at(1).pipIndex);
            EXPECT_EQ(roundTrip.flex_payments(0).pay_life(), patternIndex % 2 != 0);
            EXPECT_EQ(roundTrip.flex_payments(1).pay_life(), patternIndex % 2 == 0);
        }
    }
    EXPECT_TRUE(host.sentCommands.isEmpty());
}

TEST_F(RuledClientTest, AggregateCounterModalResultRejectsCanceledAndIdenticallyRebegunDraft)
{
    applyRaw(aggregateCounterOfferBatch());

    const auto sameContributionRows = [](const QVector<RuledPendingCounterContribution> &left,
                                         const QVector<RuledPendingCounterContribution> &right) {
        if (left.size() != right.size()) {
            return false;
        }
        for (int i = 0; i < left.size(); ++i) {
            const auto &a = left.at(i);
            const auto &b = right.at(i);
            if (a.costIndex != b.costIndex || a.objectId != b.objectId ||
                a.zoneChangeGeneration != b.zoneChangeGeneration || a.optionId != b.optionId || a.count != b.count) {
                return false;
            }
        }
        return true;
    };

    const QVector<RuledPendingCounterContribution> existingRows = {{1, 901, 13, kChargeCounterOptionId, 3}};
    const QVector<RuledPendingCounterContribution> modalRows = {
        {1, 901, 13, kNightCounterOptionId, 2}, {1, 901, 13, kChargeCounterOptionId, 1}};

    RuledPendingCast draftSequence;
    const quint64 firstDraftId = draftSequence.beginAbility().draftId;
    draftSequence.clearAbility();
    const quint64 clearedByCancelDraftId = draftSequence.ability.draftId;
    const quint64 secondDraftId = draftSequence.beginAbility().draftId;
    draftSequence.beginSpell();
    const quint64 clearedBySpellDraftId = draftSequence.ability.draftId;
    const quint64 thirdDraftId = draftSequence.beginAbility().draftId;
    EXPECT_NE(firstDraftId, 0u);
    EXPECT_NE(secondDraftId, 0u);
    EXPECT_NE(thirdDraftId, 0u);
    EXPECT_EQ(clearedByCancelDraftId, 0u);
    EXPECT_NE(firstDraftId, secondDraftId);
    EXPECT_NE(secondDraftId, thirdDraftId);
    EXPECT_NE(firstDraftId, thirdDraftId);
    EXPECT_EQ(clearedBySpellDraftId, 0u);

    RuledPendingCast owner;
    prepareOwnedAggregateCounterDraft(owner, *state);
    const bool existingRowsStaged =
        ruledStageCounterContributions(owner.ability, 1, 901, 13, existingRows);
    const bool originalDraftConfirmed = ruledConfirmCounterContributions(owner.ability);
    EXPECT_TRUE(existingRowsStaged);
    EXPECT_TRUE(originalDraftConfirmed);
    const PendingActivatedAbility capturedDraft = owner.ability;
    EXPECT_NE(capturedDraft.draftId, 0u);

    owner.clearAbility();
    auto &replacementDraft = owner.beginAbility();
    const quint64 replacementDraftId = replacementDraft.draftId;
    replacementDraft = capturedDraft;
    replacementDraft.draftId = replacementDraftId;
    EXPECT_NE(replacementDraftId, 0u);
    EXPECT_NE(replacementDraftId, capturedDraft.draftId);
    EXPECT_EQ(replacementDraft.draftId, replacementDraftId);
    const auto replacementRowsBefore = replacementDraft.counterGroupContributions;
    const bool replacementConfirmedBefore = replacementDraft.counterGroupConfirmed;

    const bool staleResultAccepted = owner.stageCounterContributionsAfterPrompt(
        *state, capturedDraft, 1, 901, 13, modalRows);
    EXPECT_FALSE(staleResultAccepted) << "a canceled draft's modal result must not stage into its identical replacement";
    EXPECT_TRUE(sameContributionRows(owner.ability.counterGroupContributions, replacementRowsBefore));
    EXPECT_EQ(owner.ability.counterGroupConfirmed, replacementConfirmedBefore);
    EXPECT_TRUE(host.sentCommands.isEmpty());

    RuledPendingCast positiveOwner;
    prepareOwnedAggregateCounterDraft(positiveOwner, *state);
    EXPECT_TRUE(state->abilityActivatable(kUnsupportedCostSourceOid, kUnsupportedCostAbilityIndex));
    const bool positiveRowsStaged =
        ruledStageCounterContributions(positiveOwner.ability, 1, 901, 13, existingRows);
    const bool positiveDraftConfirmed = ruledConfirmCounterContributions(positiveOwner.ability);
    EXPECT_TRUE(positiveRowsStaged);
    EXPECT_TRUE(positiveDraftConfirmed);
    const PendingActivatedAbility currentDraftCapture = positiveOwner.ability;
    const bool currentResultAccepted = positiveOwner.stageCounterContributionsAfterPrompt(
        *state, currentDraftCapture, 1, 901, 13, modalRows);
    EXPECT_TRUE(currentResultAccepted);
    EXPECT_TRUE(sameContributionRows(positiveOwner.ability.counterGroupContributions, modalRows));
    EXPECT_FALSE(positiveOwner.ability.counterGroupConfirmed);
    EXPECT_TRUE(host.sentCommands.isEmpty());
}

TEST_F(RuledClientTest, AggregateCounterModalResultRevalidatesCurrentOfferAndPendingContext)
{
    const QVector<RuledPendingCounterContribution> existingRows = {{1, 901, 13, kChargeCounterOptionId, 3}};
    const QVector<RuledPendingCounterContribution> modalRows = {
        {1, 901, 13, kNightCounterOptionId, 2}, {1, 901, 13, kChargeCounterOptionId, 1}};
    const QVector<RuledPendingCounterContribution> reentrantRows = {
        {1, 901, 13, kNightCounterOptionId, 1}, {1, 901, 13, kChargeCounterOptionId, 2}};
    const auto sameContributionRows = [](const QVector<RuledPendingCounterContribution> &left,
                                         const QVector<RuledPendingCounterContribution> &right) {
        if (left.size() != right.size()) {
            return false;
        }
        for (int i = 0; i < left.size(); ++i) {
            const auto &a = left.at(i);
            const auto &b = right.at(i);
            if (a.costIndex != b.costIndex || a.objectId != b.objectId ||
                a.zoneChangeGeneration != b.zoneChangeGeneration || a.optionId != b.optionId || a.count != b.count) {
                return false;
            }
        }
        return true;
    };
    const auto captureConfirmedDraft = [&](RuledPendingCast &owner) {
        prepareOwnedAggregateCounterDraft(owner, *state);
        const bool staged = ruledStageCounterContributions(owner.ability, 1, 901, 13, existingRows);
        const bool confirmed = ruledConfirmCounterContributions(owner.ability);
        EXPECT_TRUE(staged);
        EXPECT_TRUE(confirmed);
        return owner.ability;
    };
    const auto expectRejectedWithoutMutation = [&](RuledPendingCast &owner,
                                                   const PendingActivatedAbility &captured,
                                                   quint32 objectId,
                                                   quint64 zoneChangeGeneration,
                                                   const QVector<RuledPendingCounterContribution> &rows,
                                                   const char *reason) {
        const auto rowsBefore = owner.ability.counterGroupContributions;
        const bool confirmedBefore = owner.ability.counterGroupConfirmed;
        const bool accepted = owner.stageCounterContributionsAfterPrompt(
            *state, captured, 1, objectId, zoneChangeGeneration, rows);
        EXPECT_FALSE(accepted) << reason;
        EXPECT_TRUE(sameContributionRows(owner.ability.counterGroupContributions, rowsBefore)) << reason;
        EXPECT_EQ(owner.ability.counterGroupConfirmed, confirmedBefore) << reason;
    };
    const auto rejectAfterPendingChange = [&](const auto &change, const char *reason) {
        applyRaw(aggregateCounterOfferBatch());
        RuledPendingCast owner;
        const PendingActivatedAbility captured = captureConfirmedDraft(owner);
        change(owner.ability);
        expectRejectedWithoutMutation(owner, captured, 901, 13, modalRows, reason);
    };

    applyRaw(aggregateCounterOfferBatch());
    RuledPendingCast changedSourceOwner;
    const PendingActivatedAbility sourceCapture = captureConfirmedDraft(changedSourceOwner);
    auto changedSourceBatch = aggregateCounterOfferBatch();
    auto *source = changedSourceBatch.mutable_events(0)
                       ->mutable_zone_view()
                       ->mutable_per_player(0)
                       ->mutable_battlefield_objects(0);
    source->set_zone_change_generation(kUnsupportedCostSourceGeneration + 1);
    applyRaw(changedSourceBatch);
    expectRejectedWithoutMutation(
        changedSourceOwner, sourceCapture, 901, 13, modalRows, "the source generation changed while the prompt was open");

    applyRaw(aggregateCounterOfferBatch());
    RuledPendingCast inactiveOfferOwner;
    const PendingActivatedAbility inactiveOfferCapture = captureConfirmedDraft(inactiveOfferOwner);
    auto inactiveOfferBatch = aggregateCounterOfferBatch();
    auto *inactiveSource = inactiveOfferBatch.mutable_events(0)
                               ->mutable_zone_view()
                               ->mutable_per_player(0)
                               ->mutable_battlefield_objects(0);
    inactiveSource->mutable_activated_abilities(0)->set_activatable(false);
    applyRaw(inactiveOfferBatch);
    expectRejectedWithoutMutation(inactiveOfferOwner,
                                  inactiveOfferCapture,
                                  901,
                                  13,
                                  modalRows,
                                  "the raw engine ability is no longer activatable");

    rejectAfterPendingChange(
        [](PendingActivatedAbility &pending) { pending.sourceZone = ruled::v1::ABILITY_SOURCE_ZONE_GRAVEYARD; },
        "the local source zone changed");
    rejectAfterPendingChange(
        [](PendingActivatedAbility &pending) { ++pending.expectedZoneChangeGeneration; },
        "the local source generation changed");
    rejectAfterPendingChange([](PendingActivatedAbility &pending) { ++pending.abilityIndex; },
                             "the activated ability slot changed");
    rejectAfterPendingChange([](PendingActivatedAbility &pending) { pending.beginOrdinaryAnnouncement(); },
                             "the ability left the unaccepted announcing stage");
    rejectAfterPendingChange([](PendingActivatedAbility &pending) { ++pending.nextCostChoice; },
                             "the active cost position changed");

    applyRaw(aggregateCounterOfferBatch());
    RuledPendingCast cachedInventoryOwner;
    const PendingActivatedAbility cachedInventoryCapture = captureConfirmedDraft(cachedInventoryOwner);
    cachedInventoryOwner.ability.costChoices[0].counterRemovalGroup->candidates[0].options[1].availableCount = 4;
    expectRejectedWithoutMutation(cachedInventoryOwner,
                                  cachedInventoryCapture,
                                  901,
                                  13,
                                  modalRows,
                                  "the cached counter inventory changed");

    applyRaw(aggregateCounterOfferBatch());
    RuledPendingCast freshInventoryOwner;
    const PendingActivatedAbility freshInventoryCapture = captureConfirmedDraft(freshInventoryOwner);
    auto changedInventoryBatch = aggregateCounterOfferBatch();
    const quint64 wireKey = (static_cast<quint64>(kUnsupportedCostSourceOid) << 32) |
                            static_cast<quint32>(kUnsupportedCostAbilityIndex);
    auto &changedActions = (*changedInventoryBatch.mutable_legal_by_player())[kLocalPlayer];
    auto &changedChoices = (*changedActions.mutable_cost_choices_by_ability())[wireKey];
    auto *changedGroup = changedChoices.mutable_choices(0)->mutable_counter_removal_group();
    changedGroup->mutable_candidates(0)->mutable_options(0)->set_available_count(3);
    applyRaw(changedInventoryBatch);
    expectRejectedWithoutMutation(freshInventoryOwner,
                                  freshInventoryCapture,
                                  901,
                                  13,
                                  modalRows,
                                  "the freshly offered counter inventory changed");

    applyRaw(aggregateCounterOfferBatch());
    RuledPendingCast changedContributorOwner;
    const PendingActivatedAbility changedContributorCapture = captureConfirmedDraft(changedContributorOwner);
    auto changedContributorBatch = aggregateCounterOfferBatch();
    auto &changedContributorActions = (*changedContributorBatch.mutable_legal_by_player())[kLocalPlayer];
    auto &changedContributorChoices = (*changedContributorActions.mutable_cost_choices_by_ability())[wireKey];
    auto *changedContributorChoice = changedContributorChoices.mutable_choices(0);
    changedContributorChoice->mutable_candidate_objects(0)->mutable_object()->set_zone_change_generation(14);
    auto *changedContributorGroup = changedContributorChoice->mutable_counter_removal_group();
    changedContributorGroup->mutable_candidates(0)->mutable_object()->set_zone_change_generation(14);
    applyRaw(changedContributorBatch);
    expectRejectedWithoutMutation(changedContributorOwner,
                                  changedContributorCapture,
                                  901,
                                  13,
                                  modalRows,
                                  "the clicked contributor receipt is no longer offered at its captured generation");

    applyRaw(aggregateCounterOfferBatch());
    RuledPendingCast reentrantOwner;
    const PendingActivatedAbility reentrantCapture = captureConfirmedDraft(reentrantOwner);
    const bool reentrantStageAccepted =
        ruledStageCounterContributions(reentrantOwner.ability, 1, 901, 13, reentrantRows);
    EXPECT_TRUE(reentrantStageAccepted);
    expectRejectedWithoutMutation(reentrantOwner,
                                  reentrantCapture,
                                  901,
                                  13,
                                  modalRows,
                                  "reentrant staging changed rows while the prompt was open");

    applyRaw(aggregateCounterOfferBatch());
    RuledPendingCast cosmeticOwner;
    const PendingActivatedAbility cosmeticCapture = captureConfirmedDraft(cosmeticOwner);
    auto cosmeticBatch = aggregateCounterOfferBatch();
    auto &cosmeticActions = (*cosmeticBatch.mutable_legal_by_player())[kLocalPlayer];
    auto &cosmeticChoices = (*cosmeticActions.mutable_cost_choices_by_ability())[wireKey];
    auto *cosmeticGroup = cosmeticChoices.mutable_choices(0)->mutable_counter_removal_group();
    cosmeticGroup->mutable_candidates(0)->mutable_options()->SwapElements(0, 1);
    cosmeticGroup->mutable_candidates(0)->mutable_options(0)->set_label("Charge labels may change");
    cosmeticGroup->mutable_candidates(0)->mutable_options(1)->set_label("Night labels may change");
    cosmeticGroup->mutable_candidates()->SwapElements(0, 1);
    applyRaw(cosmeticBatch);
    const bool cosmeticResultAccepted = cosmeticOwner.stageCounterContributionsAfterPrompt(
        *state, cosmeticCapture, 1, 901, 13, modalRows);
    EXPECT_TRUE(cosmeticResultAccepted);
    EXPECT_TRUE(sameContributionRows(cosmeticOwner.ability.counterGroupContributions, modalRows));
    EXPECT_FALSE(cosmeticOwner.ability.counterGroupConfirmed);

    applyRaw(aggregateCounterOfferBatch());
    auto zeroContributorGenerationBatch = aggregateCounterOfferBatch();
    auto &zeroGenerationActions = (*zeroContributorGenerationBatch.mutable_legal_by_player())[kLocalPlayer];
    auto &zeroGenerationChoices = (*zeroGenerationActions.mutable_cost_choices_by_ability())[wireKey];
    auto *zeroGenerationChoice = zeroGenerationChoices.mutable_choices(0);
    auto *zeroGenerationGroup = zeroGenerationChoice->mutable_counter_removal_group();
    zeroGenerationGroup->mutable_candidates(0)->mutable_object()->set_zone_change_generation(0);
    zeroGenerationChoice->mutable_candidate_objects(0)->mutable_object()->set_zone_change_generation(0);
    applyRaw(zeroContributorGenerationBatch);
    RuledPendingCast zeroGenerationOwner;
    prepareOwnedAggregateCounterDraft(zeroGenerationOwner, *state);
    const QVector<RuledPendingCounterContribution> zeroGenerationRows = {
        {1, 901, 0, kChargeCounterOptionId, 3}};
    const bool zeroGenerationStaged =
        ruledStageCounterContributions(zeroGenerationOwner.ability, 1, 901, 0, zeroGenerationRows);
    const bool zeroGenerationConfirmed = ruledConfirmCounterContributions(zeroGenerationOwner.ability);
    EXPECT_TRUE(zeroGenerationStaged);
    EXPECT_TRUE(zeroGenerationConfirmed);
    const PendingActivatedAbility zeroGenerationCapture = zeroGenerationOwner.ability;
    const QVector<RuledPendingCounterContribution> zeroGenerationModalRows = {
        {1, 901, 0, kNightCounterOptionId, 2}, {1, 901, 0, kChargeCounterOptionId, 1}};
    const bool zeroGenerationAccepted = zeroGenerationOwner.stageCounterContributionsAfterPrompt(
        *state, zeroGenerationCapture, 1, 901, 0, zeroGenerationModalRows);
    EXPECT_TRUE(zeroGenerationAccepted);
    EXPECT_TRUE(sameContributionRows(zeroGenerationOwner.ability.counterGroupContributions, zeroGenerationModalRows));
    EXPECT_TRUE(host.sentCommands.isEmpty());
}

TEST_F(RuledClientTest, AggregateCounterContributionsRejectIncompleteMalformedAndOverfullDrafts)
{
    applyRaw(aggregateCounterOfferBatch());
    auto flexPayments = aggregatePhiPayments(makeAggregateCounterDraft(*state));

    auto incomplete = makeAggregateCounterDraft(*state);
    const QVector<RuledPendingCounterContribution> twoCounters = {{1, 901, 13, kNightCounterOptionId, 2}};
    ASSERT_TRUE(ruledStageCounterContributions(incomplete, 1, 901, 13, twoCounters));
    EXPECT_FALSE(ruledConfirmCounterContributions(incomplete));
    EXPECT_FALSE(ruledBuildCounterGroupBegin(*state, incomplete, flexPayments).has_value());

    auto oversized = makeAggregateCounterDraft(*state);
    const QVector<RuledPendingCounterContribution> fourCounters = {{1, 901, 13, kChargeCounterOptionId, 4}};
    EXPECT_FALSE(ruledStageCounterContributions(oversized, 1, 901, 13, fourCounters));
    EXPECT_TRUE(oversized.counterGroupContributions.isEmpty());

    const QVector<QVector<RuledPendingCounterContribution>> malformedRows = {
        {{1, 901, 13, 0xdeadbeefu, 1}},
        {{1, 901, 13, kChargeCounterOptionId, 1}, {1, 901, 13, kChargeCounterOptionId, 1}},
        {{2, 901, 13, kChargeCounterOptionId, 1}},
        {{1, 901, 14, kChargeCounterOptionId, 1}},
    };
    for (const auto &rows : malformedRows) {
        auto malformed = makeAggregateCounterDraft(*state);
        EXPECT_FALSE(ruledStageCounterContributions(malformed, 1, 901, 13, rows));
        EXPECT_TRUE(malformed.counterGroupContributions.isEmpty());
        EXPECT_FALSE(malformed.counterGroupConfirmed);
    }

    auto complete = makeAggregateCounterDraft(*state);
    ASSERT_EQ(complete.costChoices.size(), 1);
    ASSERT_TRUE(complete.costChoices.front().counterRemovalGroup.has_value());
    ASSERT_EQ(complete.costChoices.front().counterRemovalGroup->requiredCount, 3u);
    ASSERT_TRUE(ruledStageCounterContributions(
        complete, 1, 901, 13, {{1, 901, 13, kChargeCounterOptionId, 3}}));
    ASSERT_TRUE(ruledConfirmCounterContributions(complete));
    EXPECT_TRUE(complete.counterGroupConfirmed);
    ASSERT_TRUE(ruledBuildCounterGroupBegin(*state, complete, aggregatePhiPayments(complete)).has_value());

    auto rawZero = complete;
    rawZero.counterGroupContributions.append({1, 901, 13, kNightCounterOptionId, 0});
    const auto rawZeroRowsBeforeBuild = rawZero.counterGroupContributions;
    const bool rawZeroConfirmedBeforeBuild = rawZero.counterGroupConfirmed;
    ASSERT_EQ(rawZeroRowsBeforeBuild.size(), 2);
    EXPECT_EQ(rawZeroRowsBeforeBuild.at(0).optionId, kChargeCounterOptionId);
    EXPECT_EQ(rawZeroRowsBeforeBuild.at(0).count, 3u);
    EXPECT_EQ(rawZeroRowsBeforeBuild.at(1).optionId, kNightCounterOptionId);
    EXPECT_EQ(rawZeroRowsBeforeBuild.at(1).count, 0u);
    EXPECT_FALSE(ruledBuildCounterGroupBegin(*state, rawZero, aggregatePhiPayments(rawZero)).has_value());
    EXPECT_EQ(rawZero.counterGroupContributions.size(), rawZeroRowsBeforeBuild.size());
    EXPECT_EQ(rawZero.counterGroupConfirmed, rawZeroConfirmedBeforeBuild);
    for (int rowIndex = 0; rowIndex < rawZeroRowsBeforeBuild.size(); ++rowIndex) {
        EXPECT_EQ(rawZero.counterGroupContributions.at(rowIndex).costIndex,
                  rawZeroRowsBeforeBuild.at(rowIndex).costIndex);
        EXPECT_EQ(rawZero.counterGroupContributions.at(rowIndex).objectId,
                  rawZeroRowsBeforeBuild.at(rowIndex).objectId);
        EXPECT_EQ(rawZero.counterGroupContributions.at(rowIndex).zoneChangeGeneration,
                  rawZeroRowsBeforeBuild.at(rowIndex).zoneChangeGeneration);
        EXPECT_EQ(rawZero.counterGroupContributions.at(rowIndex).optionId,
                  rawZeroRowsBeforeBuild.at(rowIndex).optionId);
        EXPECT_EQ(rawZero.counterGroupContributions.at(rowIndex).count,
                  rawZeroRowsBeforeBuild.at(rowIndex).count);
    }

    EXPECT_TRUE(ruledStageCounterContributions(complete, 1, 901, 13,
                                               QVector<RuledPendingCounterContribution>{}));
    EXPECT_TRUE(complete.counterGroupContributions.isEmpty());
    EXPECT_FALSE(complete.counterGroupConfirmed);
    EXPECT_TRUE(host.sentCommands.isEmpty());
}

TEST_F(RuledClientTest, AggregateCounterZeroRowsDeselectOnlyTheirReceiptAndRejectMalformedRows)
{
    auto batch = aggregateCounterOfferBatch();
    const quint64 wireKey = (static_cast<quint64>(kUnsupportedCostSourceOid) << 32) |
                            static_cast<quint32>(kUnsupportedCostAbilityIndex);
    auto &actions = (*batch.mutable_legal_by_player())[kLocalPlayer];
    auto &costs = (*actions.mutable_cost_choices_by_ability())[wireKey];
    costs.mutable_choices(0)->mutable_counter_removal_group()->set_required_count(6);
    applyRaw(batch);

    auto pending = makeAggregateCounterDraft(*state);
    ASSERT_TRUE(ruledStageCounterContributions(
        pending, 1, 901, 13, {{1, 901, 13, kNightCounterOptionId, 1}, {1, 901, 13, kChargeCounterOptionId, 2}}));
    ASSERT_TRUE(ruledStageCounterContributions(pending, 1, 902, 17, {{1, 902, 17, kChargeCounterOptionId, 2}}));
    ASSERT_TRUE(ruledStageCounterContributions(pending, 1, 903, 21, {{1, 903, 21, kChargeCounterOptionId, 1}}));
    ASSERT_TRUE(ruledConfirmCounterContributions(pending));

    const bool partialZeroAccepted = ruledStageCounterContributions(
        pending, 1, 901, 13, {{1, 901, 13, kNightCounterOptionId, 0}, {1, 901, 13, kChargeCounterOptionId, 2}});
    EXPECT_TRUE(partialZeroAccepted);
    if (partialZeroAccepted) {
        EXPECT_FALSE(pending.counterGroupConfirmed);
        ASSERT_EQ(pending.counterGroupContributions.size(), 3);
        EXPECT_EQ(std::count_if(pending.counterGroupContributions.cbegin(), pending.counterGroupContributions.cend(),
                                [](const auto &row) { return row.objectId == 901; }),
                  1);
        const auto selectedCharge = std::find_if(pending.counterGroupContributions.cbegin(),
                                                 pending.counterGroupContributions.cend(), [](const auto &row) {
                                                     return row.objectId == 901;
                                                 });
        ASSERT_NE(selectedCharge, pending.counterGroupContributions.cend());
        EXPECT_EQ(selectedCharge->optionId, kChargeCounterOptionId);
        EXPECT_EQ(selectedCharge->count, 2u);
        EXPECT_EQ(std::count_if(pending.counterGroupContributions.cbegin(), pending.counterGroupContributions.cend(),
                                [](const auto &row) { return row.objectId == 902 || row.objectId == 903; }),
                  2);
    }

    const bool allZeroAccepted = ruledStageCounterContributions(
        pending, 1, 901, 13, {{1, 901, 13, kNightCounterOptionId, 0}, {1, 901, 13, kChargeCounterOptionId, 0}});
    EXPECT_TRUE(allZeroAccepted);
    if (allZeroAccepted) {
        EXPECT_FALSE(pending.counterGroupConfirmed);
        ASSERT_EQ(pending.counterGroupContributions.size(), 2);
        EXPECT_TRUE(std::none_of(pending.counterGroupContributions.cbegin(), pending.counterGroupContributions.cend(),
                                 [](const auto &row) { return row.objectId == 901; }));
        EXPECT_TRUE(std::any_of(pending.counterGroupContributions.cbegin(), pending.counterGroupContributions.cend(),
                                [](const auto &row) { return row.objectId == 902 && row.count == 2; }));
        EXPECT_TRUE(std::any_of(pending.counterGroupContributions.cbegin(), pending.counterGroupContributions.cend(),
                                [](const auto &row) { return row.objectId == 903 && row.count == 1; }));
    }

    costs.mutable_choices(0)->mutable_counter_removal_group()->set_required_count(3);
    applyRaw(batch);
    auto confirmed = makeAggregateCounterDraft(*state);
    ASSERT_TRUE(ruledStageCounterContributions(confirmed, 1, 901, 13, {{1, 901, 13, kChargeCounterOptionId, 3}}));
    ASSERT_TRUE(ruledConfirmCounterContributions(confirmed));
    const QVector<QVector<RuledPendingCounterContribution>> malformedZeroRows = {
        {{2, 901, 13, kNightCounterOptionId, 0}},
        {{1, 902, 17, kNightCounterOptionId, 0}},
        {{1, 901, 99, kNightCounterOptionId, 0}},
        {{1, 901, 13, 0, 0}},
        {{1, 901, 13, 0xdeadbeefu, 0}},
        {{1, 901, 13, kNightCounterOptionId, 0}, {1, 901, 13, kNightCounterOptionId, 0}},
        {{1, 901, 13, kNightCounterOptionId, 0}, {1, 901, 13, kChargeCounterOptionId, 4}},
    };
    for (const auto &rows : malformedZeroRows) {
        auto malformed = confirmed;
        EXPECT_FALSE(ruledStageCounterContributions(malformed, 1, 901, 13, rows));
        EXPECT_TRUE(malformed.counterGroupConfirmed);
        ASSERT_EQ(malformed.counterGroupContributions.size(), 1);
        EXPECT_EQ(malformed.counterGroupContributions.front().objectId, 901u);
        EXPECT_EQ(malformed.counterGroupContributions.front().optionId, kChargeCounterOptionId);
        EXPECT_EQ(malformed.counterGroupContributions.front().count, 3u);
    }
    EXPECT_TRUE(host.sentCommands.isEmpty());
}

TEST_F(RuledClientTest, AggregateCounterGroupAvailabilityIsSharedAcrossCostIndices)
{
    auto batch = aggregateCounterOfferBatch();
    auto &actions = (*batch.mutable_legal_by_player())[kLocalPlayer];
    const quint64 wireKey = (static_cast<quint64>(kUnsupportedCostSourceOid) << 32) |
                            static_cast<quint32>(kUnsupportedCostAbilityIndex);
    auto &costs = (*actions.mutable_cost_choices_by_ability())[wireKey];
    auto *first = costs.mutable_choices(0);
    first->mutable_counter_removal_group()->set_required_count(2);
    ruled::v1::LegalCostChoice second;
    second.CopyFrom(*first);
    second.set_cost_index(4);
    second.mutable_counter_removal_group()->set_required_count(2);
    *costs.add_choices() = second;
    applyRaw(batch);

    auto staged = makeAggregateCounterDraft(*state);
    staged.costChoices = state->abilityCostChoices(kUnsupportedCostSourceOid, kUnsupportedCostAbilityIndex);
    const QVector<RuledPendingCounterContribution> firstUse = {{1, 901, 13, kChargeCounterOptionId, 2}};
    const QVector<RuledPendingCounterContribution> secondUse = {{4, 901, 13, kChargeCounterOptionId, 2}};
    ASSERT_TRUE(ruledStageCounterContributions(staged, 1, 901, 13, firstUse));
    EXPECT_FALSE(ruledStageCounterContributions(staged, 4, 901, 13, secondUse));
    ASSERT_EQ(staged.counterGroupContributions.size(), 1);

    auto invalidCombined = makeAggregateCounterDraft(*state);
    invalidCombined.costChoices = staged.costChoices;
    invalidCombined.counterGroupContributions = {firstUse.front(), secondUse.front()};
    invalidCombined.counterGroupConfirmed = true;
    EXPECT_FALSE(ruledConfirmCounterContributions(invalidCombined));
    invalidCombined.counterGroupConfirmed = true;
    auto flexPayments = aggregatePhiPayments(invalidCombined);
    EXPECT_FALSE(ruledBuildCounterGroupBegin(*state, invalidCombined, flexPayments).has_value());
}

TEST_F(RuledClientTest, AggregateCounterQuantityContextReservesOtherGroupsAndUsesSharedMinimum)
{
    applyRaw(aggregateCounterTwoGroupOfferBatch());
    const auto offers = state->abilityCostChoices(kUnsupportedCostSourceOid, kUnsupportedCostAbilityIndex);
    ASSERT_EQ(offers.size(), 2);
    ASSERT_EQ(offers.at(0).costIndex, 1);
    ASSERT_EQ(offers.at(1).costIndex, 4);
    ASSERT_TRUE(offers.at(0).counterRemovalGroup.has_value());
    ASSERT_TRUE(offers.at(1).counterRemovalGroup.has_value());
    ASSERT_EQ(offers.at(0).counterRemovalGroup->requiredCount, 3u);
    ASSERT_EQ(offers.at(1).counterRemovalGroup->requiredCount, 1u);
    const auto optionAvailable = [](const RuledCostChoice &choice, quint32 objectId, quint32 optionId) {
        const auto candidate = std::find_if(choice.counterRemovalGroup->candidates.cbegin(),
                                            choice.counterRemovalGroup->candidates.cend(),
                                            [objectId](const auto &entry) { return entry.objectId == objectId; });
        if (candidate == choice.counterRemovalGroup->candidates.cend()) {
            return quint32{0};
        }
        const auto option = std::find_if(candidate->options.cbegin(),
                                         candidate->options.cend(),
                                         [optionId](const auto &entry) { return entry.optionId == optionId; });
        return option == candidate->options.cend() ? quint32{0} : option->availableCount;
    };
    ASSERT_EQ(optionAvailable(offers.at(0), 901, kChargeCounterOptionId), 3u);
    ASSERT_EQ(optionAvailable(offers.at(1), 901, kChargeCounterOptionId), 2u);

    auto pending = makeAggregateCounterDraft(*state);
    ASSERT_TRUE(ruledStageCounterContributions(
        pending, 1, 902, 17, {{1, 902, 17, kChargeCounterOptionId, 1}}));
    ASSERT_TRUE(ruledStageCounterContributions(
        pending, 1, 901, 13, {{1, 901, 13, kNightCounterOptionId, 1}}));
    ASSERT_TRUE(ruledStageCounterContributions(
        pending, 4, 901, 13, {{4, 901, 13, kChargeCounterOptionId, 1}}));

    const auto context = ruledCounterQuantityContext(
        *state, pending, 1, 901, 13, QStringLiteral("Test permanent"));
    ASSERT_TRUE(context.has_value());
    EXPECT_EQ(context->costIndex, 1u);
    EXPECT_EQ(context->objectId, 901u);
    EXPECT_EQ(context->zoneChangeGeneration, 13u);
    EXPECT_EQ(context->remainingForThisObject, 2u);
    ASSERT_EQ(context->offeredOptions.size(), 2);
    const auto charge = std::find_if(context->offeredOptions.cbegin(),
                                     context->offeredOptions.cend(),
                                     [](const auto &option) { return option.optionId == kChargeCounterOptionId; });
    const auto night = std::find_if(context->offeredOptions.cbegin(),
                                    context->offeredOptions.cend(),
                                    [](const auto &option) { return option.optionId == kNightCounterOptionId; });
    ASSERT_NE(charge, context->offeredOptions.cend());
    ASSERT_NE(night, context->offeredOptions.cend());
    EXPECT_EQ(charge->label, QStringLiteral("Charge"));
    EXPECT_EQ(charge->availableCount, 1u);
    EXPECT_EQ(night->availableCount, 2u);
    EXPECT_EQ(context->initialQuantities.value(kNightCounterOptionId), 1u);
    EXPECT_EQ(context->initialQuantities.value(kChargeCounterOptionId), 0u);

    applyRaw(aggregateCounterSharedReservationOfferBatch(false));
    auto sharedPending = makeAggregateCounterDraft(*state);
    ASSERT_TRUE(ruledStageCounterContributions(
        sharedPending, 4, 901, 13, {{4, 901, 13, kChargeCounterOptionId, 1}}));
    const auto sharedContext = ruledCounterQuantityContext(
        *state, sharedPending, 1, 901, 13, QStringLiteral("Shared receipt"));
    ASSERT_TRUE(sharedContext.has_value());
    ASSERT_EQ(sharedContext->offeredOptions.size(), 1);
    EXPECT_EQ(sharedContext->offeredOptions.front().optionId, kNightCounterOptionId);
    EXPECT_EQ(sharedContext->offeredOptions.front().availableCount, 2u);
    EXPECT_FALSE(sharedContext->initialQuantities.contains(kChargeCounterOptionId));

    RuledCounterQuantityDialog::Result exhaustedOptionResult;
    exhaustedOptionResult.costIndex = sharedContext->costIndex;
    exhaustedOptionResult.objectId = sharedContext->objectId;
    exhaustedOptionResult.zoneChangeGeneration = sharedContext->zoneChangeGeneration;
    exhaustedOptionResult.selections.append({kChargeCounterOptionId, 1});
    EXPECT_FALSE(ruledCounterQuantityRows(*sharedContext, exhaustedOptionResult).has_value());

    RuledCounterQuantityDialog::Result usableOptionResult;
    usableOptionResult.costIndex = sharedContext->costIndex;
    usableOptionResult.objectId = sharedContext->objectId;
    usableOptionResult.zoneChangeGeneration = sharedContext->zoneChangeGeneration;
    usableOptionResult.selections.append({kNightCounterOptionId, 1});
    const auto usableRows = ruledCounterQuantityRows(*sharedContext, usableOptionResult);
    ASSERT_TRUE(usableRows.has_value());
    ASSERT_EQ(usableRows->size(), 1);
    EXPECT_EQ(usableRows->front().costIndex, 1);
    EXPECT_EQ(usableRows->front().objectId, 901u);
    EXPECT_EQ(usableRows->front().zoneChangeGeneration, 13u);
    EXPECT_EQ(usableRows->front().optionId, kNightCounterOptionId);
    const auto otherGroupRows = sharedPending.counterGroupContributions;
    ASSERT_TRUE(ruledStageCounterContributions(sharedPending, 1, 901, 13, *usableRows));
    EXPECT_EQ(sharedPending.counterGroupContributions.size(), 2);
    EXPECT_TRUE(std::any_of(sharedPending.counterGroupContributions.cbegin(),
                            sharedPending.counterGroupContributions.cend(),
                            [](const auto &row) {
                                return row.costIndex == 4 && row.objectId == 901 &&
                                       row.zoneChangeGeneration == 13 && row.optionId == kChargeCounterOptionId &&
                                       row.count == 1;
                            }));
    EXPECT_EQ(otherGroupRows.size(), 1);

    applyRaw(aggregateCounterSharedReservationOfferBatch(true));
    auto exhaustedPending = makeAggregateCounterDraft(*state);
    ASSERT_TRUE(ruledStageCounterContributions(exhaustedPending,
                                               4,
                                               901,
                                               13,
                                               {{4, 901, 13, kChargeCounterOptionId, 1},
                                                {4, 901, 13, kNightCounterOptionId, 1}}));
    EXPECT_FALSE(ruledCounterQuantityContext(
                     *state, exhaustedPending, 1, 901, 13, QStringLiteral("No unreserved options"))
                     .has_value());

    applyRaw(aggregateCounterTwoGroupOfferBatch());
    auto baseline = makeAggregateCounterDraft(*state);
    ASSERT_TRUE(ruledCounterQuantityContext(*state, baseline, 1, 901, 13, QStringLiteral("Live source")).has_value());
    auto staleStage = baseline;
    staleStage.applyEngineSnapshot(localPaymentSnapshot(91, 1));
    EXPECT_FALSE(ruledCounterQuantityContext(*state, staleStage, 1, 901, 13, QStringLiteral("Stale stage"))
                     .has_value());
    auto malformedRows = baseline;
    malformedRows.counterGroupContributions = {{1, 901, 99, kChargeCounterOptionId, 1}};
    EXPECT_FALSE(ruledCounterQuantityContext(*state, malformedRows, 1, 901, 13, QStringLiteral("Stale row"))
                     .has_value());
    EXPECT_FALSE(ruledCounterQuantityContext(*state, baseline, 1, 901, 14, QStringLiteral("Stale receipt"))
                     .has_value());

    auto changedInventory = aggregateCounterTwoGroupOfferBatch();
    auto &changedActions = (*changedInventory.mutable_legal_by_player())[kLocalPlayer];
    const quint64 wireKey = (static_cast<quint64>(kUnsupportedCostSourceOid) << 32) |
                            static_cast<quint32>(kUnsupportedCostAbilityIndex);
    (*changedActions.mutable_cost_choices_by_ability())[wireKey]
        .mutable_choices(0)
        ->mutable_counter_removal_group()
        ->mutable_candidates(0)
        ->mutable_options(0)
        ->set_available_count(4);
    applyRaw(changedInventory);
    EXPECT_FALSE(ruledCounterQuantityContext(*state, baseline, 1, 901, 13, QStringLiteral("Changed offer"))
                     .has_value());

    auto inactiveSource = aggregateCounterTwoGroupOfferBatch();
    inactiveSource.mutable_events(0)
        ->mutable_zone_view()
        ->mutable_per_player(0)
        ->mutable_battlefield_objects(0)
        ->mutable_activated_abilities(0)
        ->set_activatable(false);
    applyRaw(inactiveSource);
    EXPECT_FALSE(ruledCounterQuantityContext(*state, baseline, 1, 901, 13, QStringLiteral("Inactive ability"))
                     .has_value());

    auto changedSource = aggregateCounterTwoGroupOfferBatch();
    changedSource.mutable_events(0)
        ->mutable_zone_view()
        ->mutable_per_player(0)
        ->mutable_battlefield_objects(0)
        ->set_zone_change_generation(kUnsupportedCostSourceGeneration + 1);
    applyRaw(changedSource);
    EXPECT_FALSE(ruledCounterQuantityContext(*state, baseline, 1, 901, 13, QStringLiteral("Changed source"))
                     .has_value());
}

TEST_F(RuledClientTest, AggregateCounterQuantityRowsRejectOverBudgetAndDuplicateOptions)
{
    auto batch = aggregateCounterOfferBatch();
    const quint64 wireKey = (static_cast<quint64>(kUnsupportedCostSourceOid) << 32) |
                            static_cast<quint32>(kUnsupportedCostAbilityIndex);
    auto &actions = (*batch.mutable_legal_by_player())[kLocalPlayer];
    auto &costs = (*actions.mutable_cost_choices_by_ability())[wireKey];
    auto *choice = costs.mutable_choices(0);
    choice->mutable_candidate_objects(0)->mutable_object()->set_zone_change_generation(0);
    choice->mutable_counter_removal_group()->mutable_candidates(0)->mutable_object()->set_zone_change_generation(0);
    applyRaw(batch);

    const auto pending = makeAggregateCounterDraft(*state);
    const auto context = ruledCounterQuantityContext(
        *state, pending, 1, 901, 0, QStringLiteral("Zero-generation permanent"));
    ASSERT_TRUE(context.has_value());
    ASSERT_EQ(context->costIndex, 1u);
    ASSERT_EQ(context->objectId, 901u);
    ASSERT_EQ(context->zoneChangeGeneration, 0u);
    ASSERT_EQ(context->offeredOptions.size(), 2);

    RuledCounterQuantityDialog::Result validResult;
    validResult.costIndex = context->costIndex;
    validResult.objectId = context->objectId;
    validResult.zoneChangeGeneration = context->zoneChangeGeneration;
    validResult.selections.append({kChargeCounterOptionId, 2});
    const auto rows = ruledCounterQuantityRows(*context, validResult);
    ASSERT_TRUE(rows.has_value());
    ASSERT_EQ(rows->size(), 1);
    EXPECT_EQ(rows->front().costIndex, 1);
    EXPECT_EQ(rows->front().objectId, 901u);
    EXPECT_EQ(rows->front().zoneChangeGeneration, 0u);
    EXPECT_EQ(rows->front().optionId, kChargeCounterOptionId);
    EXPECT_EQ(rows->front().count, 2u);
    EXPECT_EQ(rows->front().counterLabel, QStringLiteral("Charge"));

    const auto originalContext = *context;
    auto overBudget = validResult;
    overBudget.selections[0].count = 4;
    EXPECT_FALSE(ruledCounterQuantityRows(*context, overBudget).has_value());

    auto overGroupTotal = validResult;
    overGroupTotal.selections.clear();
    overGroupTotal.selections.append({kChargeCounterOptionId, 3});
    overGroupTotal.selections.append({kNightCounterOptionId, 1});
    EXPECT_FALSE(ruledCounterQuantityRows(*context, overGroupTotal).has_value());

    auto duplicateOptions = validResult;
    duplicateOptions.selections.clear();
    duplicateOptions.selections.append({kChargeCounterOptionId, 1});
    duplicateOptions.selections.append({kChargeCounterOptionId, 1});
    EXPECT_FALSE(ruledCounterQuantityRows(*context, duplicateOptions).has_value());

    auto unknownOption = validResult;
    unknownOption.selections.clear();
    unknownOption.selections.append({0xdeadbeefu, 1});
    EXPECT_FALSE(ruledCounterQuantityRows(*context, unknownOption).has_value());

    auto allZero = validResult;
    allZero.selections.clear();
    allZero.selections.append({kChargeCounterOptionId, 0});
    allZero.selections.append({kNightCounterOptionId, 0});
    const auto emptyRows = ruledCounterQuantityRows(*context, allZero);
    ASSERT_TRUE(emptyRows.has_value());
    EXPECT_TRUE(emptyRows->isEmpty());

    auto mismatchedReceipt = validResult;
    mismatchedReceipt.zoneChangeGeneration = 1;
    EXPECT_FALSE(ruledCounterQuantityRows(*context, mismatchedReceipt).has_value());
    mismatchedReceipt = validResult;
    mismatchedReceipt.objectId = 902;
    EXPECT_FALSE(ruledCounterQuantityRows(*context, mismatchedReceipt).has_value());
    mismatchedReceipt = validResult;
    ++mismatchedReceipt.costIndex;
    EXPECT_FALSE(ruledCounterQuantityRows(*context, mismatchedReceipt).has_value());

    EXPECT_EQ(context->costIndex, originalContext.costIndex);
    EXPECT_EQ(context->objectId, originalContext.objectId);
    EXPECT_EQ(context->zoneChangeGeneration, originalContext.zoneChangeGeneration);
    EXPECT_EQ(context->offeredOptions.size(), originalContext.offeredOptions.size());
    EXPECT_EQ(context->initialQuantities.size(), originalContext.initialQuantities.size());

    auto maximumBatch = aggregateCounterOfferBatch();
    const quint64 maximumWireKey = (static_cast<quint64>(kUnsupportedCostSourceOid) << 32) |
                                   static_cast<quint32>(kUnsupportedCostAbilityIndex);
    auto &maximumActions = (*maximumBatch.mutable_legal_by_player())[kLocalPlayer];
    auto *maximumChoice = (*maximumActions.mutable_cost_choices_by_ability())[maximumWireKey].mutable_choices(0);
    auto *maximumGroup = maximumChoice->mutable_counter_removal_group();
    const quint32 maximum = std::numeric_limits<quint32>::max();
    maximumGroup->set_required_count(maximum);
    auto *maximumCandidate = maximumGroup->mutable_candidates(0);
    maximumCandidate->mutable_options(0)->set_available_count(1);
    maximumCandidate->mutable_options(1)->set_available_count(maximum);
    applyRaw(maximumBatch);
    const auto maximumPending = makeAggregateCounterDraft(*state);
    const auto maximumContext = ruledCounterQuantityContext(
        *state, maximumPending, 1, 901, 13, QStringLiteral("Maximum count"));
    ASSERT_TRUE(maximumContext.has_value());
    ASSERT_EQ(maximumContext->remainingForThisObject, maximum);
    const auto maximumCharge = std::find_if(maximumContext->offeredOptions.cbegin(),
                                            maximumContext->offeredOptions.cend(),
                                            [](const auto &option) {
                                                return option.optionId == kChargeCounterOptionId;
                                            });
    ASSERT_NE(maximumCharge, maximumContext->offeredOptions.cend());
    EXPECT_EQ(maximumCharge->availableCount, maximum);
    RuledCounterQuantityDialog::Result maximumResult;
    maximumResult.costIndex = maximumContext->costIndex;
    maximumResult.objectId = maximumContext->objectId;
    maximumResult.zoneChangeGeneration = maximumContext->zoneChangeGeneration;
    maximumResult.selections.append({kChargeCounterOptionId, maximum});
    const auto maximumRows = ruledCounterQuantityRows(*maximumContext, maximumResult);
    ASSERT_TRUE(maximumRows.has_value());
    ASSERT_EQ(maximumRows->size(), 1);
    EXPECT_EQ(maximumRows->front().count, maximum);

    auto widerTotal = maximumResult;
    widerTotal.selections.append({kNightCounterOptionId, 1});
    EXPECT_FALSE(ruledCounterQuantityRows(*maximumContext, widerTotal).has_value());
}

TEST_F(RuledClientTest, AggregateCounterComponentNavigationRejectsStaleDraftAndPreservesRows)
{
    applyRaw(aggregateCounterTwoGroupOfferBatch());
    const auto offers = state->abilityCostChoices(kUnsupportedCostSourceOid, kUnsupportedCostAbilityIndex);
    ASSERT_EQ(offers.size(), 2);
    ASSERT_EQ(offers.at(0).costIndex, 1);
    ASSERT_EQ(offers.at(1).costIndex, 4);

    const QVector<RuledPendingCounterContribution> completeRows = {
        {1, 902, 17, kChargeCounterOptionId, 1},
        {1, 901, 13, kNightCounterOptionId, 1},
        {1, 901, 13, kChargeCounterOptionId, 1},
        {4, 901, 13, kChargeCounterOptionId, 1},
    };
    const auto prepareCompleteDraft = [&](RuledPendingCast &owner) -> PendingActivatedAbility & {
        auto &draft = owner.beginAbility();
        const quint64 newDraftId = draft.draftId;
        draft = makeAggregateCounterDraft(*state);
        draft.draftId = newDraftId;
        draft.waitingForCost = true;
        return draft;
    };
    const auto stageAndConfirm = [&](PendingActivatedAbility &draft) {
        return ruledStageCounterContributions(draft, 1, 902, 17, {completeRows.at(0)}) &&
               ruledStageCounterContributions(draft, 1, 901, 13, {completeRows.at(1), completeRows.at(2)}) &&
               ruledStageCounterContributions(draft, 4, 901, 13, {completeRows.at(3)}) &&
               ruledConfirmCounterContributions(draft);
    };
    const auto sameRows = [](const QVector<RuledPendingCounterContribution> &left,
                             const QVector<RuledPendingCounterContribution> &right) {
        if (left.size() != right.size()) {
            return false;
        }
        for (int index = 0; index < left.size(); ++index) {
            const auto &a = left.at(index);
            const auto &b = right.at(index);
            if (a.costIndex != b.costIndex || a.objectId != b.objectId ||
                a.zoneChangeGeneration != b.zoneChangeGeneration || a.optionId != b.optionId || a.count != b.count) {
                return false;
            }
        }
        return true;
    };
    const auto expectRejectedWithoutOwnerMutation = [&](RuledPendingCast &testOwner,
                                                        const PendingActivatedAbility &captured,
                                                        int requestedCostIndex,
                                                        quint32 objectId,
                                                        quint64 generation) {
        const auto rowsBefore = testOwner.ability.counterGroupContributions;
        const int positionBefore = testOwner.ability.nextCostChoice;
        const bool confirmationBefore = testOwner.ability.counterGroupConfirmed;
        EXPECT_FALSE(testOwner.selectCounterGroupAfterPrompt(
            *state, captured, requestedCostIndex, objectId, generation));
        EXPECT_EQ(testOwner.ability.nextCostChoice, positionBefore);
        EXPECT_TRUE(sameRows(testOwner.ability.counterGroupContributions, rowsBefore));
        EXPECT_EQ(testOwner.ability.counterGroupConfirmed, confirmationBefore);
    };

    RuledPendingCast owner;
    auto &original = prepareCompleteDraft(owner);
    ASSERT_TRUE(stageAndConfirm(original));
    ASSERT_EQ(original.nextCostChoice, 0);
    const PendingActivatedAbility staleCapture = original;

    owner.clearAbility();
    auto &replacement = prepareCompleteDraft(owner);
    ASSERT_NE(replacement.draftId, staleCapture.draftId);
    ASSERT_TRUE(stageAndConfirm(replacement));
    ASSERT_EQ(replacement.nextCostChoice, 0);
    const auto replacementRows = replacement.counterGroupContributions;
    const bool staleSelectionAccepted = owner.selectCounterGroupAfterPrompt(*state, staleCapture, 4, 901, 13);
    EXPECT_FALSE(staleSelectionAccepted);
    EXPECT_EQ(owner.ability.nextCostChoice, 0);
    EXPECT_TRUE(sameRows(owner.ability.counterGroupContributions, replacementRows));
    EXPECT_TRUE(owner.ability.counterGroupConfirmed);

    RuledPendingCast currentOwner;
    auto &current = prepareCompleteDraft(currentOwner);
    ASSERT_TRUE(stageAndConfirm(current));
    ASSERT_EQ(current.nextCostChoice, 0);
    const PendingActivatedAbility currentCapture = current;
    const auto currentRows = current.counterGroupContributions;
    const bool currentSelectionAccepted = currentOwner.selectCounterGroupAfterPrompt(*state, currentCapture, 4, 901, 13);
    EXPECT_TRUE(currentSelectionAccepted);
    EXPECT_EQ(currentOwner.ability.nextCostChoice, 1);
    EXPECT_TRUE(sameRows(currentOwner.ability.counterGroupContributions, currentRows));
    EXPECT_TRUE(currentOwner.ability.counterGroupConfirmed);

    RuledPendingCast reentrantRowsOwner;
    auto &reentrantRowsDraft = prepareCompleteDraft(reentrantRowsOwner);
    ASSERT_TRUE(stageAndConfirm(reentrantRowsDraft));
    const PendingActivatedAbility reentrantRowsCapture = reentrantRowsDraft;
    reentrantRowsDraft.counterGroupContributions = {
        {1, 902, 17, kChargeCounterOptionId, 1},
        {1, 901, 13, kNightCounterOptionId, 2},
        {4, 901, 13, kChargeCounterOptionId, 1},
    };
    ASSERT_TRUE(ruledConfirmCounterContributions(reentrantRowsDraft));
    expectRejectedWithoutOwnerMutation(reentrantRowsOwner, reentrantRowsCapture, 4, 901, 13);

    RuledPendingCast confirmationOwner;
    auto &confirmationDraft = prepareCompleteDraft(confirmationOwner);
    ASSERT_TRUE(stageAndConfirm(confirmationDraft));
    const PendingActivatedAbility confirmedCapture = confirmationDraft;
    confirmationDraft.counterGroupConfirmed = false;
    expectRejectedWithoutOwnerMutation(confirmationOwner, confirmedCapture, 4, 901, 13);

    RuledPendingCast positionOwner;
    auto &positionDraft = prepareCompleteDraft(positionOwner);
    ASSERT_TRUE(stageAndConfirm(positionDraft));
    const PendingActivatedAbility originalPositionCapture = positionDraft;
    positionDraft.nextCostChoice = 1;
    expectRejectedWithoutOwnerMutation(positionOwner, originalPositionCapture, 1, 901, 13);

    RuledPendingCast inventoryOwner;
    auto &inventoryDraft = prepareCompleteDraft(inventoryOwner);
    ASSERT_TRUE(stageAndConfirm(inventoryDraft));
    const PendingActivatedAbility inventoryCapture = inventoryDraft;
    auto changedInventory = aggregateCounterTwoGroupOfferBatch();
    const quint64 wireKey = (static_cast<quint64>(kUnsupportedCostSourceOid) << 32) |
                            static_cast<quint32>(kUnsupportedCostAbilityIndex);
    auto &changedActions = (*changedInventory.mutable_legal_by_player())[kLocalPlayer];
    (*changedActions.mutable_cost_choices_by_ability())[wireKey]
        .mutable_choices(0)
        ->mutable_counter_removal_group()
        ->mutable_candidates(0)
        ->mutable_options(1)
        ->set_available_count(4);
    applyRaw(changedInventory);
    expectRejectedWithoutOwnerMutation(inventoryOwner, inventoryCapture, 4, 901, 13);

    applyRaw(aggregateCounterTwoGroupOfferBatch());
    RuledPendingCast sourceOwner;
    auto &sourceDraft = prepareCompleteDraft(sourceOwner);
    ASSERT_TRUE(stageAndConfirm(sourceDraft));
    const PendingActivatedAbility sourceCapture = sourceDraft;
    auto changedSource = aggregateCounterTwoGroupOfferBatch();
    changedSource.mutable_events(0)
        ->mutable_zone_view()
        ->mutable_per_player(0)
        ->mutable_battlefield_objects(0)
        ->set_zone_change_generation(kUnsupportedCostSourceGeneration + 1);
    applyRaw(changedSource);
    expectRejectedWithoutOwnerMutation(sourceOwner, sourceCapture, 4, 901, 13);
}

TEST_F(RuledClientTest, AggregateCounterContributionCountsUseFullUnsignedRange)
{
    auto batch = aggregateCounterOfferBatch();
    auto &actions = (*batch.mutable_legal_by_player())[kLocalPlayer];
    const quint64 wireKey = (static_cast<quint64>(kUnsupportedCostSourceOid) << 32) |
                            static_cast<quint32>(kUnsupportedCostAbilityIndex);
    auto *choice = (*actions.mutable_cost_choices_by_ability())[wireKey].mutable_choices(0);
    auto *group = choice->mutable_counter_removal_group();
    const quint32 maximum = std::numeric_limits<quint32>::max();
    group->set_required_count(maximum);
    group->clear_candidates();
    auto *candidate = group->add_candidates();
    candidate->mutable_object()->set_object_id(901);
    candidate->mutable_object()->set_zone_change_generation(13);
    auto *charge = candidate->add_options();
    charge->set_option_id(kChargeCounterOptionId);
    charge->set_label("Charge");
    charge->set_available_count(maximum);
    auto *night = candidate->add_options();
    night->set_option_id(kNightCounterOptionId);
    night->set_label("Night");
    night->set_available_count(1);
    applyRaw(batch);

    auto pending = makeAggregateCounterDraft(*state);
    const QVector<RuledPendingCounterContribution> exactMaximum = {
        {1, 901, 13, kChargeCounterOptionId, maximum}};
    ASSERT_TRUE(ruledStageCounterContributions(pending, 1, 901, 13, exactMaximum));
    ASSERT_TRUE(ruledConfirmCounterContributions(pending));
    auto flexPayments = aggregatePhiPayments(pending);
    const auto generated = ruledBuildCounterGroupBegin(*state, pending, flexPayments);
    ASSERT_TRUE(generated.has_value());
    ASSERT_EQ(generated->cost_selections_size(), 1);
    ASSERT_EQ(generated->cost_selections(0).counter_removal_group().contributions_size(), 1);
    EXPECT_EQ(generated->cost_selections(0).counter_removal_group().contributions(0).count(), maximum);

    const QVector<RuledPendingCounterContribution> overMaximum = {
        {1, 901, 13, kChargeCounterOptionId, maximum}, {1, 901, 13, kNightCounterOptionId, 1}};
    EXPECT_FALSE(ruledStageCounterContributions(pending, 1, 901, 13, overMaximum));
    ASSERT_EQ(pending.counterGroupContributions.size(), 1);
    EXPECT_EQ(pending.counterGroupContributions.front().count, maximum);
    EXPECT_TRUE(pending.counterGroupConfirmed);
}

TEST_F(RuledClientTest, AggregateCounterBuilderRechecksLiveSourceAndCompleteUniquePhiChoices)
{
    applyRaw(aggregateCounterOfferBatch());
    auto pending = makeAggregateCounterDraft(*state);
    const QVector<RuledPendingCounterContribution> exactRows = {
        {1, 901, 13, kNightCounterOptionId, 1}, {1, 901, 13, kChargeCounterOptionId, 2}};
    ASSERT_TRUE(ruledStageCounterContributions(pending, 1, 901, 13, exactRows));
    ASSERT_TRUE(ruledConfirmCounterContributions(pending));

    ASSERT_EQ(pending.flexPips.size(), 2);
    auto completePhi = aggregatePhiPayments(pending);
    ASSERT_EQ(completePhi.size(), 2);
    EXPECT_FALSE(completePhi.front().pay_life());
    EXPECT_TRUE(completePhi.back().pay_life());
    EXPECT_TRUE(ruledBuildCounterGroupBegin(*state, pending, completePhi).has_value());

    QVector<ruled::v1::FlexPipPayment> missingPhi{completePhi.front()};
    EXPECT_FALSE(ruledBuildCounterGroupBegin(*state, pending, missingPhi).has_value());
    QVector<ruled::v1::FlexPipPayment> duplicatePhi = completePhi;
    duplicatePhi.append(ruled::v1::FlexPipPayment{});
    duplicatePhi.back().set_pip_index(completePhi.front().pip_index());
    EXPECT_FALSE(ruledBuildCounterGroupBegin(*state, pending, duplicatePhi).has_value());
    QVector<ruled::v1::FlexPipPayment> unknownPhi = completePhi;
    unknownPhi.append(ruled::v1::FlexPipPayment{});
    unknownPhi.back().set_pip_index(77);
    EXPECT_FALSE(ruledBuildCounterGroupBegin(*state, pending, unknownPhi).has_value());

    auto changedInventory = aggregateCounterOfferBatch();
    auto &changedActions = (*changedInventory.mutable_legal_by_player())[kLocalPlayer];
    const quint64 wireKey = (static_cast<quint64>(kUnsupportedCostSourceOid) << 32) |
                            static_cast<quint32>(kUnsupportedCostAbilityIndex);
    auto *changedChoice = (*changedActions.mutable_cost_choices_by_ability())[wireKey].mutable_choices(0);
    changedChoice->mutable_counter_removal_group()->mutable_candidates(0)->mutable_options(1)->set_available_count(2);
    applyRaw(changedInventory);
    EXPECT_FALSE(ruledBuildCounterGroupBegin(*state, pending, completePhi).has_value());
    EXPECT_TRUE(pending.counterGroupConfirmed);
    ASSERT_EQ(pending.counterGroupContributions.size(), 2);
    EXPECT_EQ(pending.counterGroupContributions.front().zoneChangeGeneration, 13u);

    auto changedSource = aggregateCounterOfferBatch();
    auto *zoneView = changedSource.mutable_events(0)->mutable_zone_view();
    zoneView->mutable_per_player(0)->mutable_battlefield_objects(0)->set_zone_change_generation(99);
    applyRaw(changedSource);
    EXPECT_FALSE(ruledBuildCounterGroupBegin(*state, pending, completePhi).has_value());
    EXPECT_TRUE(pending.counterGroupConfirmed);
    EXPECT_EQ(pending.expectedZoneChangeGeneration, kUnsupportedCostSourceGeneration);
    EXPECT_TRUE(host.sentCommands.isEmpty());
}

TEST_F(RuledClientTest, AggregateCounterBeginUsesTheCurrentPublishedManaCostWithoutReconcile)
{
    applyRaw(aggregateCounterOfferBatch());
    auto pending = makeAggregateCounterDraft(*state);
    const auto published = state->activatedAbilityForOid(kUnsupportedCostSourceOid, kUnsupportedCostAbilityIndex);
    ASSERT_TRUE(published.has_value());
    EXPECT_EQ(published->manaCost, QStringLiteral("{B/P}{W/P}"));
    const auto parsedCurrentPips = RuledPendingCast::parseFlexPips(published->manaCost);
    ASSERT_EQ(parsedCurrentPips.size(), 2);
    ASSERT_EQ(pending.flexPips.size(), parsedCurrentPips.size());
    ASSERT_TRUE(pending.flexPips.at(0).phyrexian);
    ASSERT_TRUE(pending.flexPips.at(1).phyrexian);
    EXPECT_EQ(pending.flexPips.at(0).pipIndex, parsedCurrentPips.at(0).pipIndex);
    EXPECT_EQ(pending.flexPips.at(1).pipIndex, parsedCurrentPips.at(1).pipIndex);

    const QVector<RuledPendingCounterContribution> exactRows = {
        {1, 901, 13, kNightCounterOptionId, 1}, {1, 901, 13, kChargeCounterOptionId, 2}};
    ASSERT_TRUE(ruledStageCounterContributions(pending, 1, 901, 13, exactRows));
    ASSERT_TRUE(ruledConfirmCounterContributions(pending));
    auto currentCostPayments = aggregatePhiPayments(pending);
    ASSERT_EQ(currentCostPayments.size(), parsedCurrentPips.size());
    EXPECT_EQ(currentCostPayments.at(0).pip_index(), parsedCurrentPips.at(0).pipIndex);
    EXPECT_EQ(currentCostPayments.at(1).pip_index(), parsedCurrentPips.at(1).pipIndex);
    EXPECT_FALSE(currentCostPayments.at(0).pay_life());
    EXPECT_TRUE(currentCostPayments.at(1).pay_life());
    const auto baseline = ruledBuildCounterGroupBegin(*state, pending, currentCostPayments);
    ASSERT_TRUE(baseline.has_value());
    ASSERT_EQ(baseline->flex_payments_size(), 2);
    EXPECT_FALSE(baseline->flex_payments(0).pay_life());
    EXPECT_TRUE(baseline->flex_payments(1).pay_life());

    auto clearedCache = pending;
    clearedCache.flexPips.clear();
    EXPECT_FALSE(ruledBuildCounterGroupBegin(*state, clearedCache, {}).has_value());

    auto inventedIndices = pending;
    inventedIndices.flexPips[0].pipIndex = 3;
    inventedIndices.flexPips[1].pipIndex = 9;
    const auto inventedPayments = aggregatePhiPayments(inventedIndices);
    EXPECT_FALSE(ruledBuildCounterGroupBegin(*state, inventedIndices, inventedPayments).has_value());

    auto refreshedSource = aggregateCounterOfferBatch();
    auto *refreshedAbility = refreshedSource.mutable_events(0)
                                 ->mutable_zone_view()
                                 ->mutable_per_player(0)
                                 ->mutable_battlefield_objects(0)
                                 ->mutable_activated_abilities(0);
    refreshedAbility->set_mana_cost("{1}{B/P}{W/P}");
    applyRaw(refreshedSource);
    const auto currentAbility = state->activatedAbilityForOid(kUnsupportedCostSourceOid, kUnsupportedCostAbilityIndex);
    ASSERT_TRUE(currentAbility.has_value());
    EXPECT_EQ(currentAbility->manaCost, QStringLiteral("{1}{B/P}{W/P}"));
    // Deliberately skip counter-draft reconciliation: the builder must notice that its cached
    // Phi indices no longer match the current engine-published ability cost.
    EXPECT_FALSE(ruledBuildCounterGroupBegin(*state, pending, currentCostPayments).has_value());
    EXPECT_TRUE(host.sentCommands.isEmpty());
}

TEST_F(RuledClientTest, AggregateActivationOwnedBeginWaitsForAckAndMatchingSnapshotBothOrders)
{
    applyRaw(aggregateCounterOfferBatch());
    ASSERT_FALSE(state->pendingAbilityActivation.has_value());

    const QVector<RuledPendingCounterContribution> acceptedRows = {
        {1, 901, 13, kNightCounterOptionId, 1},
        {1, 901, 13, kChargeCounterOptionId, 2},
    };
    const auto prepareOwnedBegin = [&](RuledPendingCast &owner) {
        auto &draft = prepareOwnedAggregateCounterDraft(owner, *state);
        if (!ruledStageCounterContributions(draft, 1, 901, 13, acceptedRows) ||
            !ruledConfirmCounterContributions(draft)) {
            return std::optional<RuledOwnedCounterGroupBegin>{};
        }
        draft.waitingForCost = false;
        return owner.prepareCounterGroupBegin(*state, kLocalPlayer, aggregatePhiPayments(draft));
    };

    const auto makePaymentSnapshot = [](const RuledOwnedCounterGroupBegin &captured, quint64 transactionId) {
        ruled::v1::PendingAbilityActivation current;
        current.set_transaction_id(transactionId);
        current.set_revision(1);
        current.set_stage(ruled::v1::ABILITY_ACTIVATION_STAGE_PAYMENT);
        current.set_actor_player_id(captured.actorPlayerId);
        current.set_deciding_player_id(captured.actorPlayerId);
        current.set_source_object_id(captured.announcement.source_object_id());
        current.set_source_zone_change_generation(captured.announcement.expected_zone_change_generation());
        current.set_source_zone(captured.announcement.source_zone());
        current.set_ability_index(captured.announcement.ability_index());
        current.set_reserved_object_id(kUnsupportedCostSourceOid + 5000);
        current.set_source_description("Aggregate counter activation");
        current.set_locked_total_cost("{B} + 2 life");
        for (const auto &selection : captured.announcement.cost_selections())
            current.add_cost_selections()->CopyFrom(selection);
        for (const auto &payment : captured.announcement.flex_payments())
            current.add_flex_payments()->CopyFrom(payment);
        return current;
    };

    const auto expectAcceptedState = [&](const RuledPendingCast &owner,
                                         const RuledOwnedCounterGroupBegin &captured,
                                         const ruled::v1::PendingAbilityActivation &current) {
        const auto &pending = owner.ability;
        EXPECT_EQ(pending.draftId, captured.draftId);
        EXPECT_EQ(pending.permanentOid, captured.announcement.source_object_id());
        EXPECT_NE(current.reserved_object_id(), pending.permanentOid);
        EXPECT_EQ(pending.transactionId(), current.transaction_id());
        EXPECT_EQ(pending.stage(), PendingActivatedAbility::Stage::Paying);
        EXPECT_TRUE(pending.hasInitializedPayment());
        EXPECT_TRUE(pending.waitingForMana);
        EXPECT_FALSE(pending.waitingForCost);
        EXPECT_FALSE(pending.waitingForTarget);
        EXPECT_FALSE(pending.chosenOpponentTargets);
        EXPECT_FALSE(pending.needsTarget);
        EXPECT_TRUE(pending.selectedTargets.isEmpty());
        EXPECT_TRUE(pending.costChoices.isEmpty());
        EXPECT_TRUE(pending.costSelections.isEmpty());
        EXPECT_TRUE(pending.flexPips.isEmpty());
        EXPECT_TRUE(pending.lifePipIndices.isEmpty());
        EXPECT_EQ(pending.engineLockedCostLabel, QString::fromStdString(current.locked_total_cost()));
        EXPECT_EQ(pending.ownedBegin(), nullptr);

        int expectedRowCount = 0;
        for (const auto &selection : captured.announcement.cost_selections())
            expectedRowCount += selection.counter_removal_group().contributions_size();
        EXPECT_EQ(pending.counterGroupContributions.size(), expectedRowCount);
        for (const auto &selection : captured.announcement.cost_selections()) {
            for (const auto &contribution : selection.counter_removal_group().contributions()) {
                const auto restored = std::find_if(
                    pending.counterGroupContributions.cbegin(), pending.counterGroupContributions.cend(),
                    [&](const auto &row) {
                        return row.costIndex == static_cast<int>(selection.cost_index()) &&
                               row.objectId == contribution.object().object_id() &&
                               row.zoneChangeGeneration == contribution.object().zone_change_generation() &&
                               row.optionId == contribution.option_id();
                    });
                EXPECT_NE(restored, pending.counterGroupContributions.cend());
                if (restored != pending.counterGroupContributions.cend())
                    EXPECT_EQ(restored->count, contribution.count());
            }
        }

        EXPECT_EQ(pending.fixedFlexPayments.size(), captured.announcement.flex_payments_size());
        for (const auto &expected : captured.announcement.flex_payments()) {
            const auto restored = std::find_if(
                pending.fixedFlexPayments.cbegin(), pending.fixedFlexPayments.cend(),
                [&expected](const auto &payment) { return payment.pip_index() == expected.pip_index(); });
            EXPECT_NE(restored, pending.fixedFlexPayments.cend());
            if (restored != pending.fixedFlexPayments.cend())
                EXPECT_EQ(restored->pay_life(), expected.pay_life());
        }
    };

    RuledPendingCast snapshotFirstOwner;
    const auto snapshotFirstCapture = prepareOwnedBegin(snapshotFirstOwner);
    ASSERT_TRUE(snapshotFirstCapture.has_value());
    EXPECT_NE(snapshotFirstCapture->draftId, 0u);
    EXPECT_EQ(snapshotFirstCapture->actorPlayerId, kLocalPlayer);
    EXPECT_EQ(snapshotFirstCapture->announcement.flex_payments_size(), 2);
    EXPECT_EQ(snapshotFirstCapture->announcement.flex_payments(0).pip_index(), 0u);
    EXPECT_FALSE(snapshotFirstCapture->announcement.flex_payments(0).pay_life());
    EXPECT_EQ(snapshotFirstCapture->announcement.flex_payments(1).pip_index(), 1u);
    EXPECT_TRUE(snapshotFirstCapture->announcement.flex_payments(1).pay_life());
    EXPECT_EQ(snapshotFirstOwner.ability.stage(), PendingActivatedAbility::Stage::BeginPending);
    EXPECT_FALSE(snapshotFirstOwner.ability.waitingForCost);
    EXPECT_FALSE(snapshotFirstOwner.ability.waitingForMana);
    ASSERT_EQ(snapshotFirstOwner.ability.fixedFlexPayments.size(), 2);
    EXPECT_EQ(readyRuledPendingPaymentAction({}, snapshotFirstOwner.ability), RuledPendingPaymentAction::None);
    EXPECT_FALSE(snapshotFirstOwner.prepareCounterGroupBegin(*state, kLocalPlayer, {}).has_value());

    auto snapshotFirstView = makePaymentSnapshot(*snapshotFirstCapture, 91);
    ASSERT_NE(snapshotFirstView.transaction_id(), 0u);
    ASSERT_EQ(snapshotFirstView.actor_player_id(), kLocalPlayer);
    ASSERT_EQ(snapshotFirstView.deciding_player_id(), kLocalPlayer);
    ASSERT_EQ(snapshotFirstView.stage(), ruled::v1::ABILITY_ACTIVATION_STAGE_PAYMENT);
    ASSERT_EQ(snapshotFirstView.cost_selections_size(), snapshotFirstCapture->announcement.cost_selections_size());
    ASSERT_EQ(snapshotFirstView.flex_payments_size(), snapshotFirstCapture->announcement.flex_payments_size());
    EXPECT_FALSE(snapshotFirstView.has_chosen_opponent_id());
    EXPECT_EQ(snapshotFirstView.announced_targets_size(), 0);

    const auto snapshotFirstUpdate = snapshotFirstOwner.reconcileCounterGroupBegin(&snapshotFirstView);
    EXPECT_EQ(snapshotFirstUpdate, RuledOwnedCounterGroupBeginUpdate::Waiting);
    EXPECT_EQ(snapshotFirstOwner.ability.stage(), PendingActivatedAbility::Stage::BeginPending);
    EXPECT_EQ(snapshotFirstOwner.ability.transactionId(), 0u);
    EXPECT_FALSE(snapshotFirstOwner.ability.hasInitializedPayment());
    EXPECT_FALSE(snapshotFirstOwner.ability.waitingForMana);
    EXPECT_NE(snapshotFirstOwner.ability.ownedBegin(), nullptr);
    EXPECT_EQ(readyRuledPendingPaymentAction({}, snapshotFirstOwner.ability), RuledPendingPaymentAction::None);

    const auto snapshotFirstAck = snapshotFirstOwner.acknowledgeCounterGroupBegin(
        *snapshotFirstCapture, true, &snapshotFirstView);
    EXPECT_EQ(snapshotFirstAck, RuledOwnedCounterGroupBeginUpdate::Accepted);
    expectAcceptedState(snapshotFirstOwner, *snapshotFirstCapture, snapshotFirstView);
    EXPECT_EQ(snapshotFirstOwner.acknowledgeCounterGroupBegin(
                  *snapshotFirstCapture, true, &snapshotFirstView),
              RuledOwnedCounterGroupBeginUpdate::Ignored);

    RuledPendingCast ackFirstOwner;
    const auto ackFirstCapture = prepareOwnedBegin(ackFirstOwner);
    ASSERT_TRUE(ackFirstCapture.has_value());
    EXPECT_EQ(ackFirstOwner.ability.stage(), PendingActivatedAbility::Stage::BeginPending);
    EXPECT_FALSE(ackFirstOwner.ability.waitingForMana);
    EXPECT_FALSE(ackFirstOwner.ability.waitingForCost);

    const auto ackFirstUpdate = ackFirstOwner.acknowledgeCounterGroupBegin(*ackFirstCapture, true, nullptr);
    EXPECT_EQ(ackFirstUpdate, RuledOwnedCounterGroupBeginUpdate::Waiting);
    EXPECT_EQ(ackFirstOwner.ability.stage(), PendingActivatedAbility::Stage::BeginPending);
    EXPECT_EQ(ackFirstOwner.ability.transactionId(), 0u);
    EXPECT_FALSE(ackFirstOwner.ability.hasInitializedPayment());
    EXPECT_FALSE(ackFirstOwner.ability.waitingForMana);
    ASSERT_NE(ackFirstOwner.ability.ownedBegin(), nullptr);
    EXPECT_TRUE(ackFirstOwner.ability.ownedBegin()->acknowledged);

    auto ackFirstView = makePaymentSnapshot(*ackFirstCapture, 92);
    ASSERT_NE(ackFirstView.transaction_id(), 0u);
    EXPECT_EQ(ackFirstView.actor_player_id(), kLocalPlayer);
    EXPECT_EQ(ackFirstView.deciding_player_id(), kLocalPlayer);
    EXPECT_FALSE(ackFirstView.has_chosen_opponent_id());
    EXPECT_EQ(ackFirstView.announced_targets_size(), 0);
    const auto ackFirstSnapshotUpdate = ackFirstOwner.reconcileCounterGroupBegin(&ackFirstView);
    EXPECT_EQ(ackFirstSnapshotUpdate, RuledOwnedCounterGroupBeginUpdate::Accepted);
    expectAcceptedState(ackFirstOwner, *ackFirstCapture, ackFirstView);
    EXPECT_EQ(ackFirstOwner.acknowledgeCounterGroupBegin(
                  *ackFirstCapture, true, &ackFirstView),
              RuledOwnedCounterGroupBeginUpdate::Ignored);
    EXPECT_TRUE(host.sentCommands.isEmpty());
}

TEST_F(RuledClientTest, AggregateActivationOwnedBeginPreparationIsSingleAndRequiresUnownedAnnouncingState)
{
    applyRaw(aggregateCounterOfferBatch());
    const auto stageConfirmedDraft = [&](RuledPendingCast &owner) {
        auto &draft = prepareOwnedAggregateCounterDraft(owner, *state);
        if (!ruledStageCounterContributions(draft, 1, 901, 13, aggregateAcceptedCounterRows()) ||
            !ruledConfirmCounterContributions(draft)) {
            return QVector<ruled::v1::FlexPipPayment>{};
        }
        draft.waitingForCost = false;
        return aggregatePhiPayments(draft);
    };

    RuledPendingCast owner;
    auto payments = stageConfirmedDraft(owner);
    ASSERT_EQ(payments.size(), 2);
    EXPECT_FALSE(owner.prepareCounterGroupBegin(*state, -1, payments).has_value());
    EXPECT_EQ(owner.ability.stage(), PendingActivatedAbility::Stage::Announcing);
    EXPECT_EQ(owner.ability.ownedBegin(), nullptr);

    const auto captured = owner.prepareCounterGroupBegin(*state, kLocalPlayer, payments);
    ASSERT_TRUE(captured.has_value());
    EXPECT_NE(captured->draftId, 0u);
    EXPECT_EQ(owner.ability.stage(), PendingActivatedAbility::Stage::BeginPending);
    EXPECT_FALSE(owner.ability.waitingForMana);
    EXPECT_FALSE(owner.prepareCounterGroupBegin(*state, kLocalPlayer, payments).has_value());
    ASSERT_NE(owner.ability.ownedBegin(), nullptr);

    RuledPendingCast blockedOwner;
    const auto blockedPayments = stageConfirmedDraft(blockedOwner);
    ASSERT_EQ(blockedPayments.size(), 2);
    state->pendingAbilityActivation = makeOwnedAggregateCounterPaymentSnapshot(*captured, 91);
    EXPECT_FALSE(blockedOwner.prepareCounterGroupBegin(*state, kLocalPlayer, blockedPayments).has_value());
    EXPECT_EQ(blockedOwner.ability.stage(), PendingActivatedAbility::Stage::Announcing);
    EXPECT_EQ(blockedOwner.ability.ownedBegin(), nullptr);
}

TEST_F(RuledClientTest, AggregateActivationOwnedBeginRejectsMalformedSnapshotsWithoutLatching)
{
    applyRaw(aggregateCounterOfferBatch());

    const auto expectRejected = [&](const char *caseName, const auto &mutate) {
        SCOPED_TRACE(caseName);
        RuledPendingCast owner;
        const auto captured = prepareOwnedAggregateCounterBegin(owner, *state);
        ASSERT_TRUE(captured.has_value());

        auto malformed = makeOwnedAggregateCounterPaymentSnapshot(*captured, 91);
        mutate(malformed);
        EXPECT_EQ(owner.reconcileCounterGroupBegin(&malformed), RuledOwnedCounterGroupBeginUpdate::Waiting);
        ASSERT_NE(owner.ability.ownedBegin(), nullptr);
        EXPECT_EQ(owner.ability.stage(), PendingActivatedAbility::Stage::BeginPending);
        EXPECT_EQ(owner.ability.transactionId(), 0u);
        EXPECT_FALSE(owner.ability.hasInitializedPayment());
        EXPECT_FALSE(owner.ability.waitingForMana);
        EXPECT_EQ(owner.ability.ownedBegin()->matchedTransactionId, 0u);

        const auto matching = makeOwnedAggregateCounterPaymentSnapshot(*captured, 91);
        EXPECT_EQ(owner.acknowledgeCounterGroupBegin(*captured, true, &matching),
                  RuledOwnedCounterGroupBeginUpdate::Accepted);
        EXPECT_EQ(owner.ability.transactionId(), 91u);
    };

    expectRejected("wrong actor", [](auto &current) { current.set_actor_player_id(kOpponent); });
    expectRejected("wrong deciding player", [](auto &current) { current.set_deciding_player_id(kOpponent); });
    expectRejected("wrong source object", [](auto &current) { current.set_source_object_id(kUnsupportedCostSourceOid + 1); });
    expectRejected("wrong source generation", [](auto &current) {
        current.set_source_zone_change_generation(current.source_zone_change_generation() + 1);
    });
    expectRejected("wrong source zone", [](auto &current) {
        current.set_source_zone(ruled::v1::ABILITY_SOURCE_ZONE_HAND);
    });
    expectRejected("wrong ability slot", [](auto &current) { current.set_ability_index(current.ability_index() + 1); });
    expectRejected("zero transaction", [](auto &current) { current.set_transaction_id(0); });
    expectRejected("nonpayment stage", [](auto &current) {
        current.set_stage(ruled::v1::ABILITY_ACTIVATION_STAGE_CHOOSE_OPPONENT);
    });
    expectRejected("chosen opponent", [](auto &current) { current.set_chosen_opponent_id(kOpponent); });
    expectRejected("announced target", [](auto &current) { current.add_announced_targets()->set_object_id(901); });
    expectRejected("valid opponent offer", [](auto &current) { current.add_valid_opponent_ids(kOpponent); });
    expectRejected("target group shape", [](auto &current) { current.mutable_target_group()->set_min(1); });
    expectRejected("target candidate shape", [](auto &current) { current.add_target_candidates()->set_object_id(901); });
    expectRejected("return candidate shape", [](auto &current) {
        current.add_return_tapped_creature_candidates()->set_object_id(901);
    });
    expectRejected("return cost index shape", [](auto &current) { current.set_return_tapped_creature_cost_index(1); });
    expectRejected("wrong cost index", [](auto &current) {
        current.mutable_cost_selections(0)->set_cost_index(current.cost_selections(0).cost_index() + 1);
    });
    expectRejected("wrong contributor generation", [](auto &current) {
        auto *contribution = current.mutable_cost_selections(0)->mutable_counter_removal_group()->mutable_contributions(0);
        contribution->mutable_object()->set_zone_change_generation(contribution->object().zone_change_generation() + 1);
    });
    expectRejected("wrong counter option", [](auto &current) {
        auto *contribution = current.mutable_cost_selections(0)->mutable_counter_removal_group()->mutable_contributions(0);
        contribution->set_option_id(contribution->option_id() + 1);
    });
    expectRejected("wrong counter count", [](auto &current) {
        auto *contribution = current.mutable_cost_selections(0)->mutable_counter_removal_group()->mutable_contributions(0);
        contribution->set_count(contribution->count() + 1);
    });
    expectRejected("missing counter rows", [](auto &current) {
        current.mutable_cost_selections(0)->mutable_counter_removal_group()->clear_contributions();
    });
    expectRejected("duplicate counter row", [](auto &current) {
        auto *group = current.mutable_cost_selections(0)->mutable_counter_removal_group();
        const auto duplicate = group->contributions(0);
        group->add_contributions()->CopyFrom(duplicate);
    });
    expectRejected("extra valid counter row", [](auto &current) {
        auto *contribution = current.mutable_cost_selections(0)->mutable_counter_removal_group()->add_contributions();
        contribution->mutable_object()->set_object_id(903);
        contribution->mutable_object()->set_zone_change_generation(21);
        contribution->set_option_id(kChargeCounterOptionId);
        contribution->set_count(1);
    });
    expectRejected("duplicate cost group", [](auto &current) {
        const auto duplicate = current.cost_selections(0);
        current.add_cost_selections()->CopyFrom(duplicate);
    });
    expectRejected("unexpected cost variant", [](auto &current) {
        current.clear_cost_selections();
        auto *selection = current.add_cost_selections();
        selection->set_cost_index(1);
        selection->set_hand_index(0);
    });
    expectRejected("missing fixed flex payment", [](auto &current) { current.clear_flex_payments(); });
    expectRejected("extra fixed flex payment", [](auto &current) {
        auto *extra = current.add_flex_payments();
        extra->set_pip_index(99);
        extra->set_pay_life(false);
    });
    expectRejected("duplicate fixed flex payment", [](auto &current) {
        const auto duplicate = current.flex_payments(0);
        current.add_flex_payments()->CopyFrom(duplicate);
    });
    expectRejected("false phyrexian choice changed", [](auto &current) {
        current.mutable_flex_payments(0)->set_pay_life(true);
    });
}

TEST_F(RuledClientTest, AggregateActivationOwnedBeginLatchesFirstMatchingTransaction)
{
    applyRaw(aggregateCounterOfferBatch());
    RuledPendingCast owner;
    const auto captured = prepareOwnedAggregateCounterBegin(owner, *state);
    ASSERT_TRUE(captured.has_value());

    auto transactionA = makeOwnedAggregateCounterPaymentSnapshot(*captured, 91);
    auto transactionB = makeOwnedAggregateCounterPaymentSnapshot(*captured, 92);
    EXPECT_EQ(owner.reconcileCounterGroupBegin(&transactionA), RuledOwnedCounterGroupBeginUpdate::Waiting);
    ASSERT_NE(owner.ability.ownedBegin(), nullptr);
    EXPECT_EQ(owner.ability.ownedBegin()->matchedTransactionId, 91u);
    EXPECT_EQ(owner.reconcileCounterGroupBegin(nullptr), RuledOwnedCounterGroupBeginUpdate::Waiting);
    EXPECT_EQ(owner.ability.ownedBegin()->matchedTransactionId, 91u);
    EXPECT_EQ(owner.reconcileCounterGroupBegin(&transactionB), RuledOwnedCounterGroupBeginUpdate::Waiting);
    EXPECT_EQ(owner.ability.transactionId(), 0u);
    EXPECT_EQ(owner.ability.ownedBegin()->matchedTransactionId, 91u);
    EXPECT_EQ(owner.acknowledgeCounterGroupBegin(*captured, true, &transactionB),
              RuledOwnedCounterGroupBeginUpdate::Waiting);
    EXPECT_TRUE(owner.ability.ownedBegin()->acknowledged);
    EXPECT_EQ(owner.ability.ownedBegin()->matchedTransactionId, 91u);
    EXPECT_EQ(owner.reconcileCounterGroupBegin(&transactionA), RuledOwnedCounterGroupBeginUpdate::Accepted);
    EXPECT_EQ(owner.ability.transactionId(), 91u);
    EXPECT_NE(owner.ability.transactionId(), transactionB.transaction_id());
}

TEST_F(RuledClientTest, AggregateActivationOwnedBeginRejectRenewsTokenAndPreservesChoices)
{
    applyRaw(aggregateCounterOfferBatch());
    RuledPendingCast owner;
    const auto rejectedCapture = prepareOwnedAggregateCounterBegin(owner, *state);
    ASSERT_TRUE(rejectedCapture.has_value());
    const auto originalDraftId = owner.ability.draftId;
    const auto originalSource = owner.ability.permanentOid;
    const auto originalGeneration = owner.ability.expectedZoneChangeGeneration;
    const auto originalZone = owner.ability.sourceZone;
    const auto originalSlot = owner.ability.abilityIndex;
    const auto originalRows = owner.ability.counterGroupContributions;
    const auto originalFlexPayments = owner.ability.fixedFlexPayments;
    ASSERT_TRUE(owner.ability.counterGroupConfirmed);
    const auto matchingSnapshot = makeOwnedAggregateCounterPaymentSnapshot(*rejectedCapture, 91);

    EXPECT_EQ(owner.reconcileCounterGroupBegin(&matchingSnapshot), RuledOwnedCounterGroupBeginUpdate::Waiting);
    ASSERT_NE(owner.ability.ownedBegin(), nullptr);
    EXPECT_EQ(owner.ability.ownedBegin()->matchedTransactionId, 91u);
    EXPECT_EQ(owner.ability.transactionId(), 0u);
    EXPECT_FALSE(owner.ability.hasInitializedPayment());
    EXPECT_EQ(owner.acknowledgeCounterGroupBegin(*rejectedCapture, false, &matchingSnapshot),
              RuledOwnedCounterGroupBeginUpdate::Rejected);
    EXPECT_TRUE(owner.ability.valid);
    EXPECT_NE(owner.ability.draftId, 0u);
    EXPECT_NE(owner.ability.draftId, originalDraftId);
    EXPECT_EQ(owner.ability.permanentOid, originalSource);
    EXPECT_EQ(owner.ability.expectedZoneChangeGeneration, originalGeneration);
    EXPECT_EQ(owner.ability.sourceZone, originalZone);
    EXPECT_EQ(owner.ability.abilityIndex, originalSlot);
    EXPECT_EQ(owner.ability.counterGroupContributions.size(), originalRows.size());
    for (int i = 0; i < originalRows.size() && i < owner.ability.counterGroupContributions.size(); ++i) {
        EXPECT_EQ(owner.ability.counterGroupContributions.at(i).costIndex, originalRows.at(i).costIndex);
        EXPECT_EQ(owner.ability.counterGroupContributions.at(i).objectId, originalRows.at(i).objectId);
        EXPECT_EQ(owner.ability.counterGroupContributions.at(i).zoneChangeGeneration,
                  originalRows.at(i).zoneChangeGeneration);
        EXPECT_EQ(owner.ability.counterGroupContributions.at(i).optionId, originalRows.at(i).optionId);
        EXPECT_EQ(owner.ability.counterGroupContributions.at(i).count, originalRows.at(i).count);
    }
    EXPECT_TRUE(owner.ability.counterGroupConfirmed);
    EXPECT_EQ(owner.ability.ownedBegin(), nullptr);
    EXPECT_EQ(owner.ability.stage(), PendingActivatedAbility::Stage::Announcing);
    EXPECT_EQ(owner.ability.transactionId(), 0u);
    EXPECT_FALSE(owner.ability.hasInitializedPayment());
    EXPECT_FALSE(owner.ability.waitingForMana);
    ASSERT_EQ(owner.ability.fixedFlexPayments.size(), originalFlexPayments.size());
    for (int i = 0; i < originalFlexPayments.size(); ++i) {
        EXPECT_EQ(owner.ability.fixedFlexPayments.at(i).pip_index(), originalFlexPayments.at(i).pip_index());
        EXPECT_EQ(owner.ability.fixedFlexPayments.at(i).pay_life(), originalFlexPayments.at(i).pay_life());
    }

    const auto retryCapture = owner.prepareCounterGroupBegin(*state, kLocalPlayer, owner.ability.fixedFlexPayments);
    ASSERT_TRUE(retryCapture.has_value());
    EXPECT_NE(retryCapture->draftId, rejectedCapture->draftId);
    EXPECT_EQ(owner.acknowledgeCounterGroupBegin(*rejectedCapture, true, &matchingSnapshot),
              RuledOwnedCounterGroupBeginUpdate::Ignored);
    EXPECT_EQ(owner.ability.stage(), PendingActivatedAbility::Stage::BeginPending);
    ASSERT_NE(owner.ability.ownedBegin(), nullptr);
    EXPECT_FALSE(owner.ability.ownedBegin()->acknowledged);

    auto retrySnapshot = makeOwnedAggregateCounterPaymentSnapshot(*retryCapture, 92);
    EXPECT_EQ(owner.acknowledgeCounterGroupBegin(*retryCapture, true, &retrySnapshot),
              RuledOwnedCounterGroupBeginUpdate::Accepted);
    EXPECT_EQ(owner.ability.transactionId(), retrySnapshot.transaction_id());
}

TEST_F(RuledClientTest, AggregateActivationOwnedBeginIgnoresStaleCallbacksAndWholeOwnerCopies)
{
    applyRaw(aggregateCounterOfferBatch());
    RuledPendingCast original;
    const auto captured = prepareOwnedAggregateCounterBegin(original, *state);
    ASSERT_TRUE(captured.has_value());

    RuledPendingCast copiedOwner = original;
    ASSERT_NE(copiedOwner.ability.ownedBegin(), nullptr);
    auto capturedWithTransientFields = *captured;
    capturedWithTransientFields.acknowledged = true;
    capturedWithTransientFields.matchedTransactionId = 777;
    auto snapshot = makeOwnedAggregateCounterPaymentSnapshot(capturedWithTransientFields, 91);
    EXPECT_EQ(copiedOwner.acknowledgeCounterGroupBegin(capturedWithTransientFields, true, nullptr),
              RuledOwnedCounterGroupBeginUpdate::Waiting);
    ASSERT_NE(copiedOwner.ability.ownedBegin(), nullptr);
    EXPECT_TRUE(copiedOwner.ability.ownedBegin()->acknowledged);
    EXPECT_EQ(copiedOwner.ability.ownedBegin()->matchedTransactionId, 0u);
    EXPECT_EQ(copiedOwner.acknowledgeCounterGroupBegin(*captured, true, &snapshot),
              RuledOwnedCounterGroupBeginUpdate::Ignored);
    EXPECT_EQ(copiedOwner.acknowledgeCounterGroupBegin(*captured, false, &snapshot),
              RuledOwnedCounterGroupBeginUpdate::Ignored);
    auto alteredCapture = *captured;
    alteredCapture.announcement.set_expected_zone_change_generation(
        alteredCapture.announcement.expected_zone_change_generation() + 1);
    EXPECT_EQ(original.acknowledgeCounterGroupBegin(alteredCapture, false, &snapshot),
              RuledOwnedCounterGroupBeginUpdate::Ignored);
    ASSERT_NE(original.ability.ownedBegin(), nullptr);
    EXPECT_FALSE(original.ability.ownedBegin()->acknowledged);
    EXPECT_EQ(copiedOwner.reconcileCounterGroupBegin(&snapshot), RuledOwnedCounterGroupBeginUpdate::Accepted);
    EXPECT_EQ(copiedOwner.ability.transactionId(), 91u);
    EXPECT_EQ(original.ability.stage(), PendingActivatedAbility::Stage::BeginPending);
    EXPECT_NE(original.ability.ownedBegin(), nullptr);

    const auto expectOldCallbacksIgnored = [&](const char *caseName, const auto &invalidate) {
        SCOPED_TRACE(caseName);
        RuledPendingCast owner;
        const auto oldCapture = prepareOwnedAggregateCounterBegin(owner, *state);
        ASSERT_TRUE(oldCapture.has_value());
        const auto oldDraftId = oldCapture->draftId;
        invalidate(owner);
        const auto oldSnapshot = makeOwnedAggregateCounterPaymentSnapshot(*oldCapture, 91);
        EXPECT_EQ(owner.acknowledgeCounterGroupBegin(*oldCapture, true, &oldSnapshot),
                  RuledOwnedCounterGroupBeginUpdate::Ignored);
        EXPECT_EQ(owner.acknowledgeCounterGroupBegin(*oldCapture, false, &oldSnapshot),
                  RuledOwnedCounterGroupBeginUpdate::Ignored);
        EXPECT_EQ(owner.reconcileCounterGroupBegin(&oldSnapshot), RuledOwnedCounterGroupBeginUpdate::Ignored);
        EXPECT_NE(owner.ability.draftId, oldDraftId);
        EXPECT_EQ(owner.ability.ownedBegin(), nullptr);
        EXPECT_EQ(owner.ability.transactionId(), 0u);
    };
    expectOldCallbacksIgnored("clearAbility", [](RuledPendingCast &owner) {
        owner.clearAbility();
        owner.beginAbility();
    });
    expectOldCallbacksIgnored("beginAbility replacement", [](RuledPendingCast &owner) { owner.beginAbility(); });
    expectOldCallbacksIgnored("beginSpell", [](RuledPendingCast &owner) { owner.beginSpell(); });
    expectOldCallbacksIgnored("sessionReset clear sequence", [](RuledPendingCast &owner) {
        owner.clearSpell();
        owner.clearAbility();
    });
}

TEST_F(RuledClientTest, AggregateActivationOwnedBeginSessionResetPreventsOldCallbacksOnIdenticalNewBegin)
{
    applyRaw(aggregateCounterOfferBatch());
    RuledPendingCast owner;
    const auto oldCapture = prepareOwnedAggregateCounterBegin(owner, *state);
    ASSERT_TRUE(oldCapture.has_value());
    const auto oldDraftId = oldCapture->draftId;

    // This mirrors RuledPaymentUi's sessionReset handler; the owner and its draft allocator survive.
    owner.clearSpell();
    owner.clearAbility();
    const auto newCapture = prepareOwnedAggregateCounterBegin(owner, *state);
    ASSERT_TRUE(newCapture.has_value());
    EXPECT_NE(newCapture->draftId, oldDraftId);

    auto newSnapshot = makeOwnedAggregateCounterPaymentSnapshot(*newCapture, 92);
    EXPECT_EQ(owner.acknowledgeCounterGroupBegin(*oldCapture, true, &newSnapshot),
              RuledOwnedCounterGroupBeginUpdate::Ignored);
    EXPECT_EQ(owner.acknowledgeCounterGroupBegin(*oldCapture, false, &newSnapshot),
              RuledOwnedCounterGroupBeginUpdate::Ignored);
    ASSERT_NE(owner.ability.ownedBegin(), nullptr);
    EXPECT_EQ(owner.ability.ownedBegin()->draftId, newCapture->draftId);
    EXPECT_FALSE(owner.ability.ownedBegin()->acknowledged);
    EXPECT_EQ(owner.ability.transactionId(), 0u);
    EXPECT_EQ(owner.acknowledgeCounterGroupBegin(*newCapture, true, &newSnapshot),
              RuledOwnedCounterGroupBeginUpdate::Accepted);
    EXPECT_EQ(owner.ability.transactionId(), 92u);
}

TEST_F(RuledClientTest, AggregateActivationOwnedBeginAcceptsReorderedReceiptAndPreservesSubmittingRefresh)
{
    applyRaw(aggregateCounterOfferBatch());
    RuledPendingCast owner;
    const auto captured = prepareOwnedAggregateCounterBegin(owner, *state);
    ASSERT_TRUE(captured.has_value());
    auto snapshot = makeOwnedAggregateCounterPaymentSnapshot(*captured, 91);
    ASSERT_EQ(snapshot.cost_selections_size(), 1);
    auto *group = snapshot.mutable_cost_selections(0)->mutable_counter_removal_group();
    ASSERT_EQ(group->contributions_size(), 2);
    group->mutable_contributions()->SwapElements(0, 1);
    snapshot.mutable_flex_payments()->SwapElements(0, 1);
    snapshot.set_source_description("changed public label");
    snapshot.set_locked_total_cost("another opaque cost label");
    state->zoneAbilitySourceByOid.insert(kUnsupportedCostSourceOid, static_cast<ruled::v1::AbilitySourceZone>(99));

    EXPECT_EQ(owner.reconcileCounterGroupBegin(&snapshot), RuledOwnedCounterGroupBeginUpdate::Waiting);
    EXPECT_EQ(owner.acknowledgeCounterGroupBegin(*captured, true, &snapshot), RuledOwnedCounterGroupBeginUpdate::Accepted);
    EXPECT_EQ(owner.ability.transactionId(), 91u);
    EXPECT_EQ(owner.ability.permanentOid, captured->announcement.source_object_id());

    ASSERT_TRUE(owner.ability.beginCommit());
    owner.ability.remainingCost.insert(QLatin1Char('B'), 1);
    const auto revisionBeforeRefresh = owner.ability.revision();
    EXPECT_EQ(owner.reconcileCounterGroupBegin(&snapshot), RuledOwnedCounterGroupBeginUpdate::Ignored);
    EXPECT_EQ(owner.ability.stage(), PendingActivatedAbility::Stage::CommitPending);
    EXPECT_EQ(owner.ability.remainingCost.value(QLatin1Char('B')), 1);
    EXPECT_EQ(owner.ability.revision(), revisionBeforeRefresh);

    snapshot.set_revision(snapshot.revision() + 1);
    ruledApplyActivationView(owner.ability, snapshot);
    EXPECT_EQ(owner.ability.stage(), PendingActivatedAbility::Stage::CommitPending);
    EXPECT_EQ(owner.ability.remainingCost.value(QLatin1Char('B')), 1);
    auto cancelOwner = owner;
    cancelOwner.ability.rejectCommit(false);
    ASSERT_TRUE(cancelOwner.ability.beginCancel());
    ruledApplyActivationView(cancelOwner.ability, snapshot);
    EXPECT_EQ(cancelOwner.ability.stage(), PendingActivatedAbility::Stage::CancelPending);
}

TEST_F(RuledClientTest, AggregateActivationOwnedBeginAcceptsZeroContributorGeneration)
{
    applyRaw(aggregateCounterOfferBatch());
    auto batch = aggregateCounterOfferBatch();
    auto &actions = (*batch.mutable_legal_by_player())[kLocalPlayer];
    const quint64 wireKey = (static_cast<quint64>(kUnsupportedCostSourceOid) << 32) |
                            static_cast<quint32>(kUnsupportedCostAbilityIndex);
    auto &choices = (*actions.mutable_cost_choices_by_ability())[wireKey];
    auto *choice = choices.mutable_choices(0);
    choice->mutable_candidate_objects(0)->mutable_object()->set_zone_change_generation(0);
    choice->mutable_counter_removal_group()->mutable_candidates(0)->mutable_object()->set_zone_change_generation(0);
    applyRaw(batch);

    RuledPendingCast owner;
    auto &draft = prepareOwnedAggregateCounterDraft(owner, *state);
    const QVector<RuledPendingCounterContribution> zeroGenerationRows = {
        {1, 901, 0, kNightCounterOptionId, 1}, {1, 901, 0, kChargeCounterOptionId, 2}};
    ASSERT_TRUE(ruledStageCounterContributions(draft, 1, 901, 0, zeroGenerationRows));
    ASSERT_TRUE(ruledConfirmCounterContributions(draft));
    draft.waitingForCost = false;
    const auto captured = owner.prepareCounterGroupBegin(*state, kLocalPlayer, aggregatePhiPayments(draft));
    ASSERT_TRUE(captured.has_value());
    auto snapshot = makeOwnedAggregateCounterPaymentSnapshot(*captured, 91);
    EXPECT_EQ(owner.acknowledgeCounterGroupBegin(*captured, true, &snapshot),
              RuledOwnedCounterGroupBeginUpdate::Accepted);
    EXPECT_EQ(owner.ability.transactionId(), 91u);
}

TEST_F(RuledClientTest, AggregateCounterPaymentRetryMatchesAcceptedReceiptAndSurvivesSourceReplacement)
{
    state->payment.clear();
    applyRaw(aggregateCounterOfferBatch());
    // The accepted transaction is tied to the original generation, even when the currently
    // published source map has since lost that object or now names its returned incarnation.
    state->abilitySourceGenerationByOid.insert(kUnsupportedCostSourceOid, 0);

    RuledPendingCast owner;
    const auto captured = prepareOwnedAggregateCounterBegin(owner, *state);
    ASSERT_TRUE(captured.has_value());
    auto accepted = makeOwnedAggregateCounterPaymentSnapshot(*captured, 91);
    accepted.set_revision(0); // Revisions are opaque counters; zero is a valid receipt.
    ASSERT_EQ(owner.acknowledgeCounterGroupBegin(*captured, true, &accepted),
              RuledOwnedCounterGroupBeginUpdate::Accepted);
    state->pendingAbilityActivation = accepted;
    ASSERT_TRUE(owner.ability.markPaymentFailed());
    state->payment.begin(true);

    ruled::v1::RuledCommand failedCommit;
    failedCommit.mutable_commit_ability_activation()->set_transaction_id(91);
    failedCommit.mutable_commit_ability_activation()->set_expected_revision(0);
    const auto failedRequest = state->payment.requestAction(failedCommit);
    ruled::v1::PaymentPreview invalidPreview;
    invalidPreview.set_transaction_id(failedRequest.transaction_id());
    invalidPreview.set_revision(failedRequest.revision());
    invalidPreview.set_error("Preview was rejected before it could be rebuilt.");
    ASSERT_TRUE(state->payment.apply(invalidPreview));
    EXPECT_FALSE(state->payment.view.valid());
    EXPECT_FALSE(state->payment.pending);

    state->abilitySourceGenerationByOid.insert(kUnsupportedCostSourceOid, 99);
    state->zoneAbilityIndicesByOid[kUnsupportedCostSourceOid].insert(kUnsupportedCostAbilityIndex);
    const auto acceptedProjection = state->pendingAbilityActivation->SerializeAsString();
    const auto acceptedRows = owner.ability.counterGroupContributions;
    const auto acceptedFlex = owner.ability.fixedFlexPayments;

    ASSERT_TRUE(ruledRetryCounterGroupPayment(*state, owner.ability, kLocalPlayer, 91, 0));
    EXPECT_FALSE(owner.ability.paymentFailed());
    EXPECT_EQ(state->pendingAbilityActivation->SerializeAsString(), acceptedProjection);
    EXPECT_EQ(owner.ability.permanentOid, captured->announcement.source_object_id());
    EXPECT_EQ(owner.ability.expectedZoneChangeGeneration, captured->announcement.expected_zone_change_generation());
    EXPECT_EQ(owner.ability.sourceZone, captured->announcement.source_zone());
    EXPECT_EQ(owner.ability.abilityIndex, static_cast<int>(captured->announcement.ability_index()));
    EXPECT_EQ(owner.ability.counterGroupContributions.size(), acceptedRows.size());
    EXPECT_EQ(owner.ability.fixedFlexPayments.size(), acceptedFlex.size());
    EXPECT_TRUE(state->payment.active);
    EXPECT_TRUE(state->payment.pending);
    EXPECT_FALSE(state->payment.submitting);
    EXPECT_FALSE(state->payment.view.valid()); // The helper does not require a valid old preview.

    ruled::v1::RuledCommand retryCommit;
    retryCommit.mutable_commit_ability_activation()->set_transaction_id(91);
    retryCommit.mutable_commit_ability_activation()->set_expected_revision(
        state->pendingAbilityActivation->revision());
    const auto retryRequest = state->payment.requestAction(retryCommit);
    ASSERT_TRUE(retryRequest.has_commit_ability_activation());
    EXPECT_EQ(retryRequest.commit_ability_activation().transaction_id(), 91u);
    EXPECT_EQ(retryRequest.commit_ability_activation().expected_revision(), 0u);
    EXPECT_EQ(retryRequest.revision(), failedRequest.revision() + 2);
    EXPECT_EQ(retryRequest.commit_ability_activation().payment().SerializeAsString(),
              state->payment.selection.SerializeAsString());
    EXPECT_EQ(state->pendingAbilityActivation->SerializeAsString(), acceptedProjection);
    ASSERT_EQ(state->pendingAbilityActivation->cost_selections_size(), 1);
    EXPECT_TRUE(state->pendingAbilityActivation->cost_selections(0).has_counter_removal_group());
    EXPECT_EQ(state->pendingAbilityActivation->flex_payments_size(), 2);
    EXPECT_EQ(owner.ability.fixedFlexPayments.size(), acceptedFlex.size());
}

TEST_F(RuledClientTest, AggregateCounterPaymentRetryRejectsStaleOrMalformedStateWithoutMutation)
{
    enum class Rejection
    {
        StaleTransaction,
        StaleRevision,
        WrongRequestedActor,
        WrongEngineActor,
        WrongDecider,
        EngineLock,
        NestedChoice,
        InactivePayment,
        PendingPreview,
        SubmittingPayment,
        MissingFailureLatch,
        WrongLocalStage,
        LocalRowMismatch,
        LocalFlexMismatch,
        MixedEngineCost,
        DuplicateEngineCost,
        InvalidEngineObject,
        TargetedProjection,
        MissingEngineProjection,
    };

    for (const auto rejection : {Rejection::StaleTransaction, Rejection::StaleRevision,
                                 Rejection::WrongRequestedActor, Rejection::WrongEngineActor,
                                 Rejection::WrongDecider, Rejection::EngineLock, Rejection::NestedChoice,
                                 Rejection::InactivePayment, Rejection::PendingPreview,
                                 Rejection::SubmittingPayment, Rejection::MissingFailureLatch,
                                 Rejection::WrongLocalStage, Rejection::LocalRowMismatch,
                                 Rejection::LocalFlexMismatch, Rejection::MixedEngineCost,
                                 Rejection::DuplicateEngineCost, Rejection::InvalidEngineObject,
                                 Rejection::TargetedProjection, Rejection::MissingEngineProjection}) {
        SCOPED_TRACE(static_cast<int>(rejection));
        state->payment.clear();
        state->pendingAbilityActivation.reset();
        state->pendingChoice.reset();
        state->finishEngineCommand();
        applyRaw(aggregateCounterOfferBatch());
        RuledPendingCast owner;
        const auto captured = prepareOwnedAggregateCounterBegin(owner, *state);
        ASSERT_TRUE(captured.has_value());
        auto accepted = makeOwnedAggregateCounterPaymentSnapshot(*captured, 91);
        accepted.set_revision(7);
        ASSERT_EQ(owner.acknowledgeCounterGroupBegin(*captured, true, &accepted),
                  RuledOwnedCounterGroupBeginUpdate::Accepted);
        state->pendingAbilityActivation = accepted;
        ASSERT_TRUE(owner.ability.markPaymentFailed());
        state->payment.begin(true);

        int requestedActor = kLocalPlayer;
        quint64 requestedTransaction = 91;
        quint64 requestedRevision = 7;
        switch (rejection) {
            case Rejection::StaleTransaction:
                requestedTransaction = 92;
                break;
            case Rejection::StaleRevision:
                requestedRevision = 6;
                break;
            case Rejection::WrongRequestedActor:
                requestedActor = kOpponent;
                break;
            case Rejection::WrongEngineActor:
                state->pendingAbilityActivation->set_actor_player_id(kOpponent);
                break;
            case Rejection::WrongDecider:
                state->pendingAbilityActivation->set_deciding_player_id(kOpponent);
                break;
            case Rejection::EngineLock:
                ASSERT_TRUE(state->beginEngineCommand());
                break;
            case Rejection::NestedChoice:
                state->pendingChoice.emplace();
                break;
            case Rejection::InactivePayment:
                state->payment.active = false;
                break;
            case Rejection::PendingPreview:
                state->payment.pending = true;
                break;
            case Rejection::SubmittingPayment:
                state->payment.submitting = true;
                break;
            case Rejection::MissingFailureLatch:
                ASSERT_TRUE(owner.ability.retryPayment());
                break;
            case Rejection::WrongLocalStage:
                ASSERT_TRUE(owner.ability.beginCancel());
                break;
            case Rejection::LocalRowMismatch:
                ++owner.ability.counterGroupContributions.front().count;
                break;
            case Rejection::LocalFlexMismatch:
                owner.ability.fixedFlexPayments.front().set_pay_life(
                    !owner.ability.fixedFlexPayments.front().pay_life());
                break;
            case Rejection::MixedEngineCost:
                state->pendingAbilityActivation->add_cost_selections()->set_cost_index(4);
                break;
            case Rejection::DuplicateEngineCost:
                state->pendingAbilityActivation->add_cost_selections()->CopyFrom(
                    state->pendingAbilityActivation->cost_selections(0));
                break;
            case Rejection::InvalidEngineObject:
                state->pendingAbilityActivation->mutable_cost_selections(0)
                    ->mutable_counter_removal_group()->mutable_contributions(0)->mutable_object()->set_object_id(0);
                break;
            case Rejection::TargetedProjection:
                state->pendingAbilityActivation->set_chosen_opponent_id(kOpponent);
                break;
            case Rejection::MissingEngineProjection:
                state->pendingAbilityActivation.reset();
                break;
        }

        const bool latchBefore = owner.ability.paymentFailed();
        const auto rowsBefore = owner.ability.counterGroupContributions;
        const auto flexBefore = owner.ability.fixedFlexPayments;
        const auto paymentSelectionBefore = state->payment.selection.SerializeAsString();
        const auto paymentViewBefore = state->payment.view.SerializeAsString();
        const auto engineBefore = state->pendingAbilityActivation
                                      ? state->pendingAbilityActivation->SerializeAsString()
                                      : std::string{};
        const bool activeBefore = state->payment.active;
        const bool pendingBefore = state->payment.pending;
        const bool submittingBefore = state->payment.submitting;

        EXPECT_FALSE(ruledRetryCounterGroupPayment(*state, owner.ability, requestedActor,
                                                   requestedTransaction, requestedRevision));
        EXPECT_EQ(owner.ability.paymentFailed(), latchBefore);
        EXPECT_EQ(owner.ability.counterGroupContributions.size(), rowsBefore.size());
        EXPECT_EQ(owner.ability.fixedFlexPayments.size(), flexBefore.size());
        EXPECT_EQ(state->payment.selection.SerializeAsString(), paymentSelectionBefore);
        EXPECT_EQ(state->payment.view.SerializeAsString(), paymentViewBefore);
        EXPECT_EQ(state->payment.active, activeBefore);
        EXPECT_EQ(state->payment.pending, pendingBefore);
        EXPECT_EQ(state->payment.submitting, submittingBefore);
        EXPECT_EQ(state->pendingAbilityActivation
                      ? state->pendingAbilityActivation->SerializeAsString()
                      : std::string{},
                  engineBefore);
    }
}

TEST_F(RuledClientTest, AggregateCounterPaymentRetryDoesNotRearmSanitizedPayment)
{
    state->payment.clear();
    applyRaw(aggregateCounterOfferBatch());
    RuledPendingCast owner;
    const auto captured = prepareOwnedAggregateCounterBegin(owner, *state);
    ASSERT_TRUE(captured.has_value());
    auto accepted = makeOwnedAggregateCounterPaymentSnapshot(*captured, 91);
    accepted.set_revision(2);
    ASSERT_EQ(owner.acknowledgeCounterGroupBegin(*captured, true, &accepted),
              RuledOwnedCounterGroupBeginUpdate::Accepted);
    state->pendingAbilityActivation = accepted;
    ASSERT_TRUE(owner.ability.markPaymentFailed());
    state->payment.begin(true);

    ruled::v1::RuledCommand commit;
    commit.mutable_commit_ability_activation()->set_transaction_id(91);
    commit.mutable_commit_ability_activation()->set_expected_revision(2);
    auto query = state->payment.requestAction(commit);
    ruled::v1::PaymentPreview sanitized;
    sanitized.set_transaction_id(query.transaction_id());
    sanitized.set_revision(query.revision());
    sanitized.set_valid(true);
    sanitized.set_selection_changed(true);
    *sanitized.mutable_selection() = query.commit_ability_activation().payment();
    ASSERT_TRUE(state->payment.apply(sanitized));
    EXPECT_FALSE(state->payment.beginSubmission());

    ASSERT_TRUE(ruledRetryCounterGroupPayment(*state, owner.ability, kLocalPlayer, 91, 2));
    query = state->payment.requestAction(commit);
    ASSERT_TRUE(query.has_commit_ability_activation());
    EXPECT_EQ(query.commit_ability_activation().transaction_id(), 91u);
    EXPECT_EQ(query.commit_ability_activation().expected_revision(), 2u);
    ruled::v1::PaymentPreview clean;
    clean.set_transaction_id(query.transaction_id());
    clean.set_revision(query.revision());
    clean.set_valid(true);
    clean.set_complete(true);
    ASSERT_TRUE(state->payment.apply(clean));
    EXPECT_EQ(state->payment.nextPreviewAction(true), RuledPayment::PreviewAction::None);

    ASSERT_TRUE(state->payment.stageMana(QLatin1Char('B'))); // A fresh user payment choice arms it.
    query = state->payment.requestAction(commit);
    clean.set_revision(query.revision());
    *clean.mutable_selection() = query.commit_ability_activation().payment();
    ASSERT_TRUE(state->payment.apply(clean));
    EXPECT_EQ(state->payment.nextPreviewAction(true), RuledPayment::PreviewAction::Submit);
}

TEST_F(RuledClientTest, AggregateCounterCancelRequiresExactAcceptedProjectionButNotLiveSource)
{
    state->payment.clear();
    auto sourceGenerationZero = aggregateCounterOfferBatch();
    sourceGenerationZero.mutable_events(0)
        ->mutable_zone_view()
        ->mutable_per_player(0)
        ->mutable_battlefield_objects(0)
        ->set_zone_change_generation(0);
    applyRaw(sourceGenerationZero);

    RuledPendingCast owner;
    auto accepted = prepareAcceptedAggregateCounterPayment(owner, *state, 91, 7);
    ASSERT_TRUE(accepted.has_value());
    EXPECT_EQ(accepted->source_zone_change_generation(), 0u);
    state->pendingAbilityActivation = *accepted;
    state->payment.clear(); // Inactive model and invalid view do not prevent canceling the exact transaction.
    state->zoneAbilitySourceByOid.remove(kUnsupportedCostSourceOid);
    state->abilitySourceGenerationByOid.remove(kUnsupportedCostSourceOid);
    state->zoneAbilityIndicesByOid.remove(kUnsupportedCostSourceOid);
    state->activatedAbilitiesByOid.remove(kUnsupportedCostSourceOid);
    const auto acceptedProjection = state->pendingAbilityActivation->SerializeAsString();

    const auto cancel = ruledCancelCounterGroupPayment(*state, owner.ability, kLocalPlayer);
    ASSERT_TRUE(cancel.has_value());
    ASSERT_TRUE(cancel->has_cancel_ability_activation());
    EXPECT_EQ(cancel->cancel_ability_activation().transaction_id(), 91u);
    EXPECT_EQ(cancel->cancel_ability_activation().expected_revision(), 7u);
    EXPECT_EQ(owner.ability.stage(), PendingActivatedAbility::Stage::CancelPending);
    EXPECT_EQ(owner.ability.permanentOid, kUnsupportedCostSourceOid);
    EXPECT_EQ(owner.ability.expectedZoneChangeGeneration, 0u);
    EXPECT_EQ(owner.ability.abilityIndex, kUnsupportedCostAbilityIndex);
    EXPECT_EQ(state->pendingAbilityActivation->SerializeAsString(), acceptedProjection);

    enum class Rejection
    {
        StaleTransaction,
        StaleRevision,
        WrongActor,
        WrongEngineStage,
        WrongLocalStage,
        EngineLock,
        NestedChoice,
        MissingCounterReceipt,
        ChangedFixedPhi,
    };
    for (const auto rejection : {Rejection::StaleTransaction,
                                 Rejection::StaleRevision,
                                 Rejection::WrongActor,
                                 Rejection::WrongEngineStage,
                                 Rejection::WrongLocalStage,
                                 Rejection::EngineLock,
                                 Rejection::NestedChoice,
                                 Rejection::MissingCounterReceipt,
                                 Rejection::ChangedFixedPhi}) {
        SCOPED_TRACE(static_cast<int>(rejection));
        state->payment.clear();
        state->pendingChoice.reset();
        state->finishEngineCommand();
        applyRaw(aggregateCounterOfferBatch());
        RuledPendingCast rejectedOwner;
        auto rejectedAccepted = prepareAcceptedAggregateCounterPayment(rejectedOwner, *state, 91, 7);
        ASSERT_TRUE(rejectedAccepted.has_value());
        state->pendingAbilityActivation = *rejectedAccepted;
        int actorPlayerId = kLocalPlayer;
        switch (rejection) {
            case Rejection::StaleTransaction:
                state->pendingAbilityActivation->set_transaction_id(92);
                break;
            case Rejection::StaleRevision:
                state->pendingAbilityActivation->set_revision(8);
                break;
            case Rejection::WrongActor:
                actorPlayerId = kOpponent;
                break;
            case Rejection::WrongEngineStage:
                state->pendingAbilityActivation->set_stage(ruled::v1::ABILITY_ACTIVATION_STAGE_CHOOSE_OPPONENT);
                break;
            case Rejection::WrongLocalStage:
                ASSERT_TRUE(rejectedOwner.ability.beginCommit());
                break;
            case Rejection::EngineLock:
                ASSERT_TRUE(state->beginEngineCommand());
                break;
            case Rejection::NestedChoice:
                state->pendingChoice.emplace();
                break;
            case Rejection::MissingCounterReceipt:
                state->pendingAbilityActivation->clear_cost_selections();
                break;
            case Rejection::ChangedFixedPhi:
                state->pendingAbilityActivation->mutable_flex_payments(0)->set_pay_life(
                    !state->pendingAbilityActivation->flex_payments(0).pay_life());
                break;
        }

        const auto localStageBefore = rejectedOwner.ability.stage();
        const auto localTransactionBefore = rejectedOwner.ability.transactionId();
        const auto localRevisionBefore = rejectedOwner.ability.revision();
        const auto localRowsBefore = rejectedOwner.ability.counterGroupContributions;
        const auto localFlexBefore = rejectedOwner.ability.fixedFlexPayments;
        const auto engineBefore = state->pendingAbilityActivation->SerializeAsString();
        const auto paymentSelectionBefore = state->payment.selection.SerializeAsString();
        const auto paymentViewBefore = state->payment.view.SerializeAsString();
        const bool paymentActiveBefore = state->payment.active;
        const bool paymentPendingBefore = state->payment.pending;
        const bool commandPendingBefore = state->isEngineCommandPending();
        const bool nestedChoiceBefore = state->pendingChoice.has_value();

        EXPECT_FALSE(ruledCancelCounterGroupPayment(*state, rejectedOwner.ability, actorPlayerId).has_value());
        EXPECT_EQ(rejectedOwner.ability.stage(), localStageBefore);
        EXPECT_EQ(rejectedOwner.ability.transactionId(), localTransactionBefore);
        EXPECT_EQ(rejectedOwner.ability.revision(), localRevisionBefore);
        EXPECT_TRUE(sameAggregateCounterRows(rejectedOwner.ability.counterGroupContributions, localRowsBefore));
        EXPECT_TRUE(sameAggregateFlexPayments(rejectedOwner.ability.fixedFlexPayments, localFlexBefore));
        EXPECT_EQ(state->pendingAbilityActivation->SerializeAsString(), engineBefore);
        EXPECT_EQ(state->payment.selection.SerializeAsString(), paymentSelectionBefore);
        EXPECT_EQ(state->payment.view.SerializeAsString(), paymentViewBefore);
        EXPECT_EQ(state->payment.active, paymentActiveBefore);
        EXPECT_EQ(state->payment.pending, paymentPendingBefore);
        EXPECT_EQ(state->isEngineCommandPending(), commandPendingBefore);
        EXPECT_EQ(state->pendingChoice.has_value(), nestedChoiceBefore);
    }
}

TEST_F(RuledClientTest, AggregateCounterPreviewFailureRecordsOnlyCurrentCorrelatedReplies)
{
    state->payment.clear();
    applyRaw(aggregateCounterOfferBatch());
    RuledPendingCast owner;
    auto accepted = prepareAcceptedAggregateCounterPayment(owner, *state, 91, 7);
    ASSERT_TRUE(accepted.has_value());
    state->pendingAbilityActivation = *accepted;
    const auto acceptedProjection = state->pendingAbilityActivation->SerializeAsString();

    state->payment.begin(true);
    ASSERT_TRUE(state->payment.stageMana(QLatin1Char('B')));
    ruled::v1::RuledCommand commit;
    commit.mutable_commit_ability_activation()->set_transaction_id(91);
    commit.mutable_commit_ability_activation()->set_expected_revision(7);
    const auto query = state->payment.requestAction(commit);
    ruled::v1::PaymentPreview rejected;
    rejected.set_transaction_id(query.transaction_id());
    rejected.set_revision(query.revision());
    rejected.set_error("The current aggregate counter payment was rejected.");
    const auto stagedSelection = state->payment.selection.SerializeAsString();

    EXPECT_TRUE(ruledRecordCounterGroupPreviewFailure(*state, owner.ability, kLocalPlayer, &rejected));
    EXPECT_TRUE(owner.ability.paymentFailed());
    EXPECT_TRUE(state->payment.active);
    EXPECT_FALSE(state->payment.pending);
    EXPECT_FALSE(state->payment.submitting);
    EXPECT_FALSE(state->payment.view.valid());
    EXPECT_EQ(state->payment.view.error(), rejected.error());
    EXPECT_EQ(state->payment.selection.SerializeAsString(), stagedSelection);
    EXPECT_EQ(state->pendingAbilityActivation->SerializeAsString(), acceptedProjection);

    enum class StaleOuterProjection
    {
        Transaction,
        Revision,
    };
    for (const auto stale : {StaleOuterProjection::Transaction, StaleOuterProjection::Revision}) {
        SCOPED_TRACE(static_cast<int>(stale));
        state->payment.clear();
        state->pendingChoice.reset();
        state->finishEngineCommand();
        applyRaw(aggregateCounterOfferBatch());
        RuledPendingCast staleOwner;
        auto staleAccepted = prepareAcceptedAggregateCounterPayment(staleOwner, *state, 91, 7);
        ASSERT_TRUE(staleAccepted.has_value());
        state->pendingAbilityActivation = *staleAccepted;
        state->payment.begin(true);
        ASSERT_TRUE(state->payment.stageMana(QLatin1Char('B')));
        const auto staleQuery = state->payment.requestAction(commit);
        ruled::v1::PaymentPreview staleReply;
        staleReply.set_transaction_id(staleQuery.transaction_id());
        staleReply.set_revision(staleQuery.revision());
        staleReply.set_error("Correlated payment error from the changed outer projection.");
        if (stale == StaleOuterProjection::Transaction)
            state->pendingAbilityActivation->set_transaction_id(92);
        else
            state->pendingAbilityActivation->set_revision(8);

        const auto localRowsBefore = staleOwner.ability.counterGroupContributions;
        const auto localFlexBefore = staleOwner.ability.fixedFlexPayments;
        const auto localStageBefore = staleOwner.ability.stage();
        const auto outerBefore = state->pendingAbilityActivation->SerializeAsString();
        const auto selectionBefore = state->payment.selection.SerializeAsString();
        const auto viewBefore = state->payment.view.SerializeAsString();
        const auto paymentTransactionBefore = state->payment.transaction();

        EXPECT_FALSE(ruledRecordCounterGroupPreviewFailure(*state, staleOwner.ability, kLocalPlayer, &staleReply));
        EXPECT_FALSE(staleOwner.ability.paymentFailed());
        EXPECT_EQ(staleOwner.ability.stage(), localStageBefore);
        EXPECT_TRUE(sameAggregateCounterRows(staleOwner.ability.counterGroupContributions, localRowsBefore));
        EXPECT_TRUE(sameAggregateFlexPayments(staleOwner.ability.fixedFlexPayments, localFlexBefore));
        EXPECT_EQ(state->pendingAbilityActivation->SerializeAsString(), outerBefore);
        EXPECT_EQ(state->payment.transaction(), paymentTransactionBefore);
        EXPECT_TRUE(state->payment.active);
        EXPECT_TRUE(state->payment.pending);
        EXPECT_FALSE(state->payment.submitting);
        EXPECT_EQ(state->payment.selection.SerializeAsString(), selectionBefore);
        EXPECT_EQ(state->payment.view.SerializeAsString(), viewBefore);
    }

    enum class StalePaymentReply
    {
        Transaction,
        Revision,
    };
    for (const auto stale : {StalePaymentReply::Transaction, StalePaymentReply::Revision}) {
        SCOPED_TRACE(stale == StalePaymentReply::Transaction ? "stale payment-preview transaction" :
                                                              "stale payment-preview revision");
        state->payment.clear();
        state->pendingChoice.reset();
        state->finishEngineCommand();
        applyRaw(aggregateCounterOfferBatch());
        RuledPendingCast replyOwner;
        auto replyAccepted = prepareAcceptedAggregateCounterPayment(replyOwner, *state, 91, 7);
        ASSERT_TRUE(replyAccepted.has_value());
        state->pendingAbilityActivation = *replyAccepted;
        const auto outerProjection = state->pendingAbilityActivation->SerializeAsString();

        state->payment.begin(true);
        ASSERT_TRUE(state->payment.stageMana(QLatin1Char('B')));
        const auto earlierQuery = state->payment.requestAction(commit);
        ruled::v1::PaymentPreview earlierResponse;
        earlierResponse.set_transaction_id(earlierQuery.transaction_id());
        earlierResponse.set_revision(earlierQuery.revision());
        earlierResponse.set_valid(true);
        *earlierResponse.mutable_selection() = earlierQuery.commit_ability_activation().payment();
        ASSERT_TRUE(state->payment.apply(earlierResponse));

        if (stale == StalePaymentReply::Transaction)
            state->payment.begin(true);
        else
            ASSERT_TRUE(state->payment.stageMana(QLatin1Char('R')));
        if (stale == StalePaymentReply::Transaction)
            ASSERT_TRUE(state->payment.stageMana(QLatin1Char('R')));
        const auto currentQuery = state->payment.requestAction(commit);
        ruled::v1::PaymentPreview staleReply;
        staleReply.set_transaction_id(earlierQuery.transaction_id());
        staleReply.set_revision(earlierQuery.revision());
        staleReply.set_error("A payment failure arrived for an earlier model query.");

        const auto selectionBefore = state->payment.selection.SerializeAsString();
        const auto viewBefore = state->payment.view.SerializeAsString();
        const auto modelTransactionBefore = state->payment.transaction();
        const auto localRowsBefore = replyOwner.ability.counterGroupContributions;
        const auto localFlexBefore = replyOwner.ability.fixedFlexPayments;
        if (stale == StalePaymentReply::Transaction) {
            EXPECT_NE(currentQuery.transaction_id(), earlierQuery.transaction_id());
        } else {
            EXPECT_EQ(currentQuery.transaction_id(), earlierQuery.transaction_id());
            EXPECT_GT(currentQuery.revision(), earlierQuery.revision());
        }
        EXPECT_FALSE(ruledRecordCounterGroupPreviewFailure(*state, replyOwner.ability, kLocalPlayer, &staleReply));
        EXPECT_FALSE(replyOwner.ability.paymentFailed());
        EXPECT_TRUE(state->payment.active);
        EXPECT_TRUE(state->payment.pending);
        EXPECT_FALSE(state->payment.submitting);
        EXPECT_EQ(state->payment.transaction(), modelTransactionBefore);
        EXPECT_EQ(state->payment.selection.SerializeAsString(), selectionBefore);
        EXPECT_EQ(state->payment.view.SerializeAsString(), viewBefore);
        EXPECT_EQ(state->pendingAbilityActivation->SerializeAsString(), outerProjection);
        EXPECT_TRUE(sameAggregateCounterRows(replyOwner.ability.counterGroupContributions, localRowsBefore));
        EXPECT_TRUE(sameAggregateFlexPayments(replyOwner.ability.fixedFlexPayments, localFlexBefore));
    }

    state->payment.clear();
    state->pendingChoice.reset();
    state->finishEngineCommand();
    applyRaw(aggregateCounterOfferBatch());
    RuledPendingCast observedOwner;
    auto observedAccepted = prepareAcceptedAggregateCounterPayment(observedOwner, *state, 91, 7);
    ASSERT_TRUE(observedAccepted.has_value());
    state->pendingAbilityActivation = *observedAccepted;
    state->payment.begin(true);
    ASSERT_TRUE(state->payment.stageMana(QLatin1Char('B')));
    const auto observedQuery = state->payment.requestAction(commit);
    ruled::v1::PaymentPreview alreadyAppliedFailure;
    alreadyAppliedFailure.set_transaction_id(observedQuery.transaction_id());
    alreadyAppliedFailure.set_revision(observedQuery.revision());
    alreadyAppliedFailure.set_error("The invalid preview was already applied by the dispatcher.");
    ASSERT_TRUE(state->payment.apply(alreadyAppliedFailure));
    ASSERT_FALSE(state->payment.pending);
    ASSERT_FALSE(state->payment.view.valid());
    ASSERT_TRUE(state->beginEngineCommand());
    state->pendingChoice.emplace();
    const auto observedProjection = state->pendingAbilityActivation->SerializeAsString();
    const auto observedView = state->payment.view.SerializeAsString();
    const auto observedSelection = state->payment.selection.SerializeAsString();

    EXPECT_TRUE(ruledRecordCounterGroupPreviewFailure(*state, observedOwner.ability, kLocalPlayer, nullptr));
    EXPECT_TRUE(observedOwner.ability.paymentFailed());
    EXPECT_FALSE(state->payment.pending);
    EXPECT_EQ(state->pendingAbilityActivation->SerializeAsString(), observedProjection);
    EXPECT_EQ(state->payment.view.SerializeAsString(), observedView);
    EXPECT_EQ(state->payment.selection.SerializeAsString(), observedSelection);
    EXPECT_TRUE(state->isEngineCommandPending());
    EXPECT_TRUE(state->pendingChoice.has_value());
}

TEST_F(RuledClientTest, AggregateCounterCancelRejectedSnapshotPreservesFailedPaymentUntilRetry)
{
    state->payment.clear();
    applyRaw(aggregateCounterOfferBatch());
    RuledPendingCast owner;
    auto accepted = prepareAcceptedAggregateCounterPayment(owner, *state, 91, 7);
    ASSERT_TRUE(accepted.has_value());
    state->pendingAbilityActivation = *accepted;
    ruledApplyActivationView(owner.ability, *state->pendingAbilityActivation);
    ASSERT_TRUE(owner.ability.hasInitializedPayment());
    ASSERT_TRUE(owner.ability.counterGroupConfirmed);

    state->payment.begin(true);
    ASSERT_TRUE(state->payment.stageMana(QLatin1Char('B')));
    ruled::v1::RuledCommand commit;
    commit.mutable_commit_ability_activation()->set_transaction_id(91);
    commit.mutable_commit_ability_activation()->set_expected_revision(7);
    const auto failedQuery = state->payment.requestAction(commit);
    ruled::v1::PaymentPreview failedReply;
    failedReply.set_transaction_id(failedQuery.transaction_id());
    failedReply.set_revision(failedQuery.revision());
    failedReply.set_error("The accepted payment preview was rejected.");
    ASSERT_TRUE(ruledRecordCounterGroupPreviewFailure(*state, owner.ability, kLocalPlayer, &failedReply));
    ASSERT_TRUE(owner.ability.paymentFailed());

    const auto acceptedProjection = state->pendingAbilityActivation->SerializeAsString();
    const auto acceptedRows = owner.ability.counterGroupContributions;
    const auto acceptedFlex = owner.ability.fixedFlexPayments;
    const auto paymentSelection = state->payment.selection.SerializeAsString();
    const auto paymentView = state->payment.view.SerializeAsString();
    const auto paymentTransaction = state->payment.transaction();
    ASSERT_TRUE(state->payment.active);
    ASSERT_FALSE(state->payment.pending);
    ASSERT_FALSE(state->payment.submitting);

    const auto cancel = ruledCancelCounterGroupPayment(*state, owner.ability, kLocalPlayer);
    ASSERT_TRUE(cancel.has_value());
    ASSERT_TRUE(cancel->has_cancel_ability_activation());
    EXPECT_EQ(cancel->cancel_ability_activation().transaction_id(), 91u);
    EXPECT_EQ(cancel->cancel_ability_activation().expected_revision(), 7u);
    EXPECT_EQ(owner.ability.stage(), PendingActivatedAbility::Stage::CancelPending);
    EXPECT_FALSE(owner.ability.paymentFailed());
    EXPECT_FALSE(ruledRetryCounterGroupPayment(*state, owner.ability, kLocalPlayer, 91, 7));
    EXPECT_FALSE(ruledInvalidateCounterGroupPaymentForRefresh(*state, owner.ability, kLocalPlayer));

    ruledApplyActivationView(owner.ability, *state->pendingAbilityActivation);
    EXPECT_EQ(owner.ability.stage(), PendingActivatedAbility::Stage::CancelPending);
    EXPECT_FALSE(owner.ability.paymentFailed());
    EXPECT_FALSE(ruledRetryCounterGroupPayment(*state, owner.ability, kLocalPlayer, 91, 7));
    EXPECT_FALSE(ruledInvalidateCounterGroupPaymentForRefresh(*state, owner.ability, kLocalPlayer));
    EXPECT_EQ(owner.ability.transactionId(), 91u);
    EXPECT_EQ(owner.ability.revision(), 7u);
    EXPECT_TRUE(sameAggregateCounterRows(owner.ability.counterGroupContributions, acceptedRows));
    EXPECT_TRUE(sameAggregateFlexPayments(owner.ability.fixedFlexPayments, acceptedFlex));
    EXPECT_EQ(state->pendingAbilityActivation->SerializeAsString(), acceptedProjection);
    EXPECT_EQ(state->payment.selection.SerializeAsString(), paymentSelection);
    EXPECT_EQ(state->payment.view.SerializeAsString(), paymentView);
    EXPECT_EQ(state->payment.transaction(), paymentTransaction);
    EXPECT_TRUE(state->payment.active);
    EXPECT_FALSE(state->payment.pending);
    EXPECT_FALSE(state->payment.submitting);

    owner.ability.rejectCancel(); // The negative cancel ACK transition used by RuledPaymentUi.
    ruledApplyActivationView(owner.ability, *state->pendingAbilityActivation);
    EXPECT_EQ(owner.ability.stage(), PendingActivatedAbility::Stage::Paying);
    EXPECT_TRUE(owner.ability.paymentFailed());
    EXPECT_EQ(owner.ability.transactionId(), 91u);
    EXPECT_EQ(owner.ability.revision(), 7u);
    EXPECT_TRUE(sameAggregateCounterRows(owner.ability.counterGroupContributions, acceptedRows));
    EXPECT_TRUE(sameAggregateFlexPayments(owner.ability.fixedFlexPayments, acceptedFlex));
    EXPECT_EQ(state->pendingAbilityActivation->SerializeAsString(), acceptedProjection);
    EXPECT_FALSE(ruledInvalidateCounterGroupPaymentForRefresh(*state, owner.ability, kLocalPlayer));
    EXPECT_EQ(state->payment.selection.SerializeAsString(), paymentSelection);
    EXPECT_EQ(state->payment.view.SerializeAsString(), paymentView);
    EXPECT_EQ(state->payment.transaction(), paymentTransaction);
    EXPECT_TRUE(state->payment.active);
    EXPECT_FALSE(state->payment.pending);
    EXPECT_FALSE(state->payment.submitting);
    EXPECT_EQ(state->payment.nextPreviewAction(true), RuledPayment::PreviewAction::None);

    EXPECT_TRUE(ruledRetryCounterGroupPayment(*state, owner.ability, kLocalPlayer, 91, 7));
    EXPECT_FALSE(owner.ability.paymentFailed());
    EXPECT_TRUE(state->payment.pending); // Only an explicit Retry requests a fresh preview.
}

TEST_F(RuledClientTest, AggregateCounterRefreshPreservesFailedPaymentAndInvalidatesOrdinaryPendingPreview)
{
    state->payment.clear();
    applyRaw(aggregateCounterOfferBatch());
    RuledPendingCast failedOwner;
    auto failedAccepted = prepareAcceptedAggregateCounterPayment(failedOwner, *state, 91, 7);
    ASSERT_TRUE(failedAccepted.has_value());
    state->pendingAbilityActivation = *failedAccepted;
    const auto acceptedProjection = state->pendingAbilityActivation->SerializeAsString();
    state->payment.begin(true);
    ASSERT_TRUE(state->payment.stageMana(QLatin1Char('B')));

    ruled::v1::RuledCommand commit;
    commit.mutable_commit_ability_activation()->set_transaction_id(91);
    commit.mutable_commit_ability_activation()->set_expected_revision(7);
    const auto failedQuery = state->payment.requestAction(commit);
    ruled::v1::PaymentPreview failedReply;
    failedReply.set_transaction_id(failedQuery.transaction_id());
    failedReply.set_revision(failedQuery.revision());
    failedReply.set_error("Payment preview failed before the accepted activation could continue.");
    const auto failedSelection = state->payment.selection.SerializeAsString();
    ASSERT_TRUE(ruledRecordCounterGroupPreviewFailure(*state, failedOwner.ability, kLocalPlayer, &failedReply));
    ASSERT_TRUE(failedOwner.ability.paymentFailed());

    const auto failedView = state->payment.view.SerializeAsString();
    const auto failedError = state->payment.view.error();
    const auto paymentTransaction = state->payment.transaction();
    EXPECT_FALSE(ruledInvalidateCounterGroupPaymentForRefresh(*state, failedOwner.ability, kLocalPlayer));
    EXPECT_TRUE(failedOwner.ability.paymentFailed());
    EXPECT_TRUE(state->payment.active);
    EXPECT_FALSE(state->payment.pending);
    EXPECT_FALSE(state->payment.submitting);
    EXPECT_EQ(state->payment.transaction(), paymentTransaction);
    EXPECT_EQ(state->payment.view.SerializeAsString(), failedView);
    EXPECT_EQ(state->payment.view.error(), failedError);
    EXPECT_EQ(state->payment.selection.SerializeAsString(), failedSelection);
    EXPECT_EQ(state->pendingAbilityActivation->SerializeAsString(), acceptedProjection);

    ASSERT_TRUE(ruledRetryCounterGroupPayment(*state, failedOwner.ability, kLocalPlayer, 91, 7));
    EXPECT_FALSE(failedOwner.ability.paymentFailed());
    EXPECT_TRUE(state->payment.active);
    EXPECT_TRUE(state->payment.pending);
    EXPECT_FALSE(state->payment.submitting);
    EXPECT_FALSE(state->payment.view.valid());
    EXPECT_EQ(state->payment.selection.SerializeAsString(), failedSelection);
    EXPECT_EQ(state->pendingAbilityActivation->SerializeAsString(), acceptedProjection);

    state->payment.clear();
    state->pendingChoice.reset();
    state->finishEngineCommand();
    applyRaw(aggregateCounterOfferBatch());
    RuledPendingCast ordinaryOwner;
    auto ordinaryAccepted = prepareAcceptedAggregateCounterPayment(ordinaryOwner, *state, 91, 7);
    ASSERT_TRUE(ordinaryAccepted.has_value());
    state->pendingAbilityActivation = *ordinaryAccepted;
    state->payment.begin(true);
    ASSERT_TRUE(state->payment.stageMana(QLatin1Char('B')));
    const auto oldQuery = state->payment.requestAction(commit);
    const auto ordinarySelection = state->payment.selection.SerializeAsString();
    const auto ordinaryProjection = state->pendingAbilityActivation->SerializeAsString();

    EXPECT_TRUE(ruledInvalidateCounterGroupPaymentForRefresh(*state, ordinaryOwner.ability, kLocalPlayer));
    EXPECT_FALSE(ordinaryOwner.ability.paymentFailed());
    EXPECT_TRUE(state->payment.active);
    EXPECT_TRUE(state->payment.pending);
    EXPECT_FALSE(state->payment.submitting);
    EXPECT_EQ(state->payment.transaction(), oldQuery.transaction_id());
    EXPECT_EQ(state->payment.selection.SerializeAsString(), ordinarySelection);
    EXPECT_EQ(state->pendingAbilityActivation->SerializeAsString(), ordinaryProjection);

    ruled::v1::PaymentPreview retiredReply;
    retiredReply.set_transaction_id(oldQuery.transaction_id());
    retiredReply.set_revision(oldQuery.revision());
    retiredReply.set_valid(true);
    *retiredReply.mutable_selection() = oldQuery.commit_ability_activation().payment();
    EXPECT_FALSE(state->payment.apply(retiredReply));
    EXPECT_TRUE(state->payment.pending);
    EXPECT_EQ(state->payment.selection.SerializeAsString(), ordinarySelection);
    EXPECT_EQ(state->pendingAbilityActivation->SerializeAsString(), ordinaryProjection);
}

TEST_F(RuledClientTest, AggregateCounterFlexibleChoiceRequiresTheCapturedPromptContext)
{
    const quint64 wireKey = (static_cast<quint64>(kUnsupportedCostSourceOid) << 32) |
                            static_cast<quint32>(kUnsupportedCostAbilityIndex);
    const auto prepareConfirmedDraft = [&](RuledPendingCast &owner) {
        auto &draft = prepareOwnedAggregateCounterDraft(owner, *state);
        if (!ruledStageCounterContributions(draft, 1, 901, 13, aggregateAcceptedCounterRows()) ||
            !ruledConfirmCounterContributions(draft)) {
            return false;
        }
        draft.waitingForCost = false;
        return true;
    };

    for (const bool payLifeForFirstPhi : {false, true}) {
        SCOPED_TRACE(payLifeForFirstPhi ? "pay life for the first Phyrexian pip" :
                                         "pay mana for the first Phyrexian pip");
        applyRaw(aggregateCounterOfferBatch());
        RuledPendingCast owner;
        ASSERT_TRUE(prepareConfirmedDraft(owner));
        const auto captured = owner.ability;
        const auto payments = aggregatePhiPayments(captured, payLifeForFirstPhi);
        ASSERT_EQ(payments.size(), 2);
        EXPECT_EQ(payments.at(0).pay_life(), payLifeForFirstPhi);
        EXPECT_EQ(payments.at(1).pay_life(), !payLifeForFirstPhi);
        const std::optional<QVector<ruled::v1::FlexPipPayment>> result{payments};

        EXPECT_TRUE(owner.applyCounterGroupFlexibleChoiceAfterPrompt(*state, captured, result));
        EXPECT_TRUE(sameAggregateFlexPayments(owner.ability.fixedFlexPayments, payments));
        EXPECT_TRUE(sameAggregateCounterRows(owner.ability.counterGroupContributions,
                                             captured.counterGroupContributions));
        EXPECT_TRUE(owner.ability.counterGroupConfirmed);
        EXPECT_EQ(owner.ability.draftId, captured.draftId);
        EXPECT_EQ(owner.ability.stage(), PendingActivatedAbility::Stage::Announcing);
    }

    applyRaw(aggregateCounterOfferBatch());
    RuledPendingCast cancelOwner;
    ASSERT_TRUE(prepareConfirmedDraft(cancelOwner));
    const auto cancelCapture = cancelOwner.ability;
    const auto cancelRows = cancelOwner.ability.counterGroupContributions;
    const auto cancelFixed = cancelOwner.ability.fixedFlexPayments;
    const std::optional<QVector<ruled::v1::FlexPipPayment>> cancelResult;
    EXPECT_TRUE(cancelOwner.applyCounterGroupFlexibleChoiceAfterPrompt(*state, cancelCapture, cancelResult));
    EXPECT_TRUE(sameAggregateCounterRows(cancelOwner.ability.counterGroupContributions, cancelRows));
    EXPECT_TRUE(sameAggregateFlexPayments(cancelOwner.ability.fixedFlexPayments, cancelFixed));
    EXPECT_EQ(cancelOwner.ability.draftId, cancelCapture.draftId);
    EXPECT_TRUE(cancelOwner.ability.counterGroupConfirmed);

    applyRaw(aggregateCounterOfferBatch());
    RuledPendingCast cosmeticOwner;
    ASSERT_TRUE(prepareConfirmedDraft(cosmeticOwner));
    const auto cosmeticCapture = cosmeticOwner.ability;
    auto cosmeticOffers = aggregateCounterOfferBatch();
    auto &cosmeticActions = (*cosmeticOffers.mutable_legal_by_player())[kLocalPlayer];
    auto &cosmeticCosts = (*cosmeticActions.mutable_cost_choices_by_ability())[wireKey];
    auto *cosmeticGroup = cosmeticCosts.mutable_choices(0)->mutable_counter_removal_group();
    cosmeticGroup->mutable_candidates(0)->mutable_options()->SwapElements(0, 1);
    cosmeticGroup->mutable_candidates(0)->mutable_options(0)->set_label("Charge label refreshed");
    cosmeticGroup->mutable_candidates(0)->mutable_options(1)->set_label("Night label refreshed");
    cosmeticGroup->mutable_candidates()->SwapElements(0, 1);
    applyRaw(cosmeticOffers);
    const auto cosmeticPayments = aggregatePhiPayments(cosmeticCapture, true);
    const std::optional<QVector<ruled::v1::FlexPipPayment>> cosmeticResult{cosmeticPayments};
    EXPECT_TRUE(cosmeticOwner.applyCounterGroupFlexibleChoiceAfterPrompt(*state, cosmeticCapture, cosmeticResult));
    EXPECT_TRUE(sameAggregateFlexPayments(cosmeticOwner.ability.fixedFlexPayments, cosmeticPayments));
    EXPECT_TRUE(sameAggregateCounterRows(cosmeticOwner.ability.counterGroupContributions,
                                         cosmeticCapture.counterGroupContributions));

    enum class StaleContext
    {
        IdenticalReplacementDraft,
        SameTokenRestageAndReconfirm,
        MaterialOfferChange,
        SourceGenerationChange,
        SourceZoneChange,
        MissingFreshAbilitySlot,
        FlexiblePipChange,
        PriorFlexibleChoiceChange,
    };
    for (const auto stale : {StaleContext::IdenticalReplacementDraft,
                             StaleContext::SameTokenRestageAndReconfirm,
                             StaleContext::MaterialOfferChange,
                             StaleContext::SourceGenerationChange,
                             StaleContext::SourceZoneChange,
                             StaleContext::MissingFreshAbilitySlot,
                             StaleContext::FlexiblePipChange,
                             StaleContext::PriorFlexibleChoiceChange}) {
        SCOPED_TRACE(static_cast<int>(stale));
        applyRaw(aggregateCounterOfferBatch());
        RuledPendingCast owner;
        ASSERT_TRUE(prepareConfirmedDraft(owner));
        const auto captured = owner.ability;
        const auto stalePayments = aggregatePhiPayments(captured);
        const std::optional<QVector<ruled::v1::FlexPipPayment>> staleResult{stalePayments};

        switch (stale) {
            case StaleContext::IdenticalReplacementDraft: {
                auto &replacement = prepareOwnedAggregateCounterDraft(owner, *state);
                ASSERT_TRUE(ruledStageCounterContributions(
                    replacement, 1, 901, 13, aggregateAcceptedCounterRows()));
                ASSERT_TRUE(ruledConfirmCounterContributions(replacement));
                replacement.waitingForCost = false;
                ASSERT_NE(replacement.draftId, captured.draftId);
                ASSERT_TRUE(sameAggregateCounterRows(replacement.counterGroupContributions,
                                                     captured.counterGroupContributions));
                break;
            }
            case StaleContext::SameTokenRestageAndReconfirm:
                ASSERT_TRUE(ruledStageCounterContributions(
                    owner.ability, 1, 901, 13,
                    {{1, 901, 13, kNightCounterOptionId, 2}, {1, 901, 13, kChargeCounterOptionId, 1}}));
                ASSERT_TRUE(ruledConfirmCounterContributions(owner.ability));
                ASSERT_EQ(owner.ability.draftId, captured.draftId);
                ASSERT_NE(sameAggregateCounterRows(owner.ability.counterGroupContributions,
                                                   captured.counterGroupContributions), true);
                break;
            case StaleContext::MaterialOfferChange: {
                auto changedOffer = aggregateCounterOfferBatch();
                auto &actions = (*changedOffer.mutable_legal_by_player())[kLocalPlayer];
                auto &costs = (*actions.mutable_cost_choices_by_ability())[wireKey];
                costs.mutable_choices(0)->mutable_counter_removal_group()->set_required_count(4);
                applyRaw(changedOffer);
                break;
            }
            case StaleContext::SourceGenerationChange:
                state->abilitySourceGenerationByOid.insert(kUnsupportedCostSourceOid,
                                                           kUnsupportedCostSourceGeneration + 1);
                break;
            case StaleContext::SourceZoneChange:
                state->zoneAbilitySourceByOid.insert(kUnsupportedCostSourceOid,
                                                     ruled::v1::ABILITY_SOURCE_ZONE_HAND);
                break;
            case StaleContext::MissingFreshAbilitySlot:
                state->zoneAbilityIndicesByOid[kUnsupportedCostSourceOid].clear();
                break;
            case StaleContext::FlexiblePipChange: {
                auto changedPip = aggregateCounterOfferBatch();
                changedPip.mutable_events(0)
                    ->mutable_zone_view()
                    ->mutable_per_player(0)
                    ->mutable_battlefield_objects(0)
                    ->mutable_activated_abilities(0)
                    ->set_mana_cost("{U/P}{W/P}");
                applyRaw(changedPip);
                break;
            }
            case StaleContext::PriorFlexibleChoiceChange: {
                ruled::v1::FlexPipPayment previousChoice;
                previousChoice.set_pip_index(captured.flexPips.front().pipIndex);
                previousChoice.set_pay_life(true);
                owner.ability.fixedFlexPayments.append(previousChoice);
                break;
            }
        }

        const auto currentRows = owner.ability.counterGroupContributions;
        const auto currentFixed = owner.ability.fixedFlexPayments;
        const auto currentStage = owner.ability.stage();
        const auto currentDraftId = owner.ability.draftId;
        const auto currentConfirmed = owner.ability.counterGroupConfirmed;
        EXPECT_FALSE(owner.applyCounterGroupFlexibleChoiceAfterPrompt(*state, captured, staleResult));
        EXPECT_TRUE(sameAggregateCounterRows(owner.ability.counterGroupContributions, currentRows));
        EXPECT_TRUE(sameAggregateFlexPayments(owner.ability.fixedFlexPayments, currentFixed));
        EXPECT_EQ(owner.ability.stage(), currentStage);
        EXPECT_EQ(owner.ability.draftId, currentDraftId);
        EXPECT_EQ(owner.ability.counterGroupConfirmed, currentConfirmed);
        EXPECT_FALSE(owner.applyCounterGroupFlexibleChoiceAfterPrompt(*state, captured, cancelResult));
        EXPECT_TRUE(sameAggregateCounterRows(owner.ability.counterGroupContributions, currentRows));
        EXPECT_TRUE(sameAggregateFlexPayments(owner.ability.fixedFlexPayments, currentFixed));
        EXPECT_EQ(owner.ability.stage(), currentStage);
        EXPECT_EQ(owner.ability.draftId, currentDraftId);
        EXPECT_EQ(owner.ability.counterGroupConfirmed, currentConfirmed);
    }

    applyRaw(aggregateCounterOfferBatch());
    RuledPendingCast notPayableOwner;
    ASSERT_TRUE(prepareConfirmedDraft(notPayableOwner));
    const auto notPayableCapture = notPayableOwner.ability;
    auto nonPayableOffer = aggregateCounterOfferBatch();
    auto &nonPayableActions = (*nonPayableOffer.mutable_legal_by_player())[kLocalPlayer];
    const quint64 abilityKey = (static_cast<quint64>(kUnsupportedCostSourceOid) << 32) |
                               static_cast<quint32>(kUnsupportedCostAbilityIndex);
    (*nonPayableActions.mutable_cost_choices_by_ability())[abilityKey].set_non_mana_costs_payable(false);
    applyRaw(nonPayableOffer);
    const auto nonPayableRows = notPayableOwner.ability.counterGroupContributions;
    const auto nonPayableFixed = notPayableOwner.ability.fixedFlexPayments;
    EXPECT_TRUE(notPayableOwner.applyCounterGroupFlexibleChoiceAfterPrompt(*state, notPayableCapture, cancelResult));
    const auto deniedPayments = aggregatePhiPayments(notPayableCapture, true);
    const std::optional<QVector<ruled::v1::FlexPipPayment>> deniedResult{deniedPayments};
    EXPECT_FALSE(notPayableOwner.applyCounterGroupFlexibleChoiceAfterPrompt(*state, notPayableCapture, deniedResult));
    EXPECT_TRUE(sameAggregateCounterRows(notPayableOwner.ability.counterGroupContributions, nonPayableRows));
    EXPECT_TRUE(sameAggregateFlexPayments(notPayableOwner.ability.fixedFlexPayments, nonPayableFixed));
    EXPECT_TRUE(notPayableOwner.ability.counterGroupConfirmed);
}

TEST_F(RuledClientTest, AggregateCounterBeginRejectsCurrentTargetModalAndXShapes)
{
    const auto makeConfirmedPending = [&]() -> PendingActivatedAbility {
        auto pending = makeAggregateCounterDraft(*state);
        const QVector<RuledPendingCounterContribution> exactRows = {
            {1, 901, 13, kNightCounterOptionId, 1}, {1, 901, 13, kChargeCounterOptionId, 2}};
        EXPECT_TRUE(ruledStageCounterContributions(pending, 1, 901, 13, exactRows));
        EXPECT_TRUE(ruledConfirmCounterContributions(pending));
        return pending;
    };
    const auto key = RuledClientState::abilityTargetKey(kUnsupportedCostSourceOid, kUnsupportedCostAbilityIndex);

    applyRaw(aggregateCounterOfferBatch());
    auto targeted = makeConfirmedPending();
    const auto phiPayments = aggregatePhiPayments(targeted);
    EXPECT_TRUE(ruledBuildCounterGroupBegin(*state, targeted, phiPayments).has_value());
    state->validTargetsByAbility.insert(key, RuledClientState::SpellTargetData{});
    EXPECT_FALSE(ruledBuildCounterGroupBegin(*state, targeted, phiPayments).has_value());

    applyRaw(aggregateCounterOfferBatch());
    auto modal = makeConfirmedPending();
    auto payments = aggregatePhiPayments(modal);
    RuledActivatedAbilityModeOffer modeOffer;
    modeOffer.minModes = 1;
    modeOffer.maxModes = 1;
    RuledModalSpellOption mode;
    mode.modeIndex = 0;
    mode.selectable = true;
    modeOffer.modes.append(mode);
    state->activatedAbilityModesByAbility.insert(key, modeOffer);
    EXPECT_FALSE(ruledBuildCounterGroupBegin(*state, modal, payments).has_value());

    applyRaw(aggregateCounterOfferBatch());
    auto counterX = makeConfirmedPending();
    payments = aggregatePhiPayments(counterX);
    auto &abilityEntries = state->activatedAbilitiesByOid[kUnsupportedCostSourceOid];
    ASSERT_EQ(abilityEntries.size(), kUnsupportedCostAbilityIndex + 1);
    ASSERT_TRUE(abilityEntries.at(kUnsupportedCostAbilityIndex).has_value());
    abilityEntries[kUnsupportedCostAbilityIndex]->xCounterManaChoice =
        RuledXCounterManaChoice{QStringLiteral("charge"), 2, QStringLiteral("B"), QStringLiteral("G")};
    EXPECT_FALSE(ruledBuildCounterGroupBegin(*state, counterX, payments).has_value());

    applyRaw(aggregateCounterOfferBatch());
    auto loyaltyX = makeConfirmedPending();
    payments = aggregatePhiPayments(loyaltyX);
    auto &loyaltyEntries = state->activatedAbilitiesByOid[kUnsupportedCostSourceOid];
    ASSERT_EQ(loyaltyEntries.size(), kUnsupportedCostAbilityIndex + 1);
    ASSERT_TRUE(loyaltyEntries.at(kUnsupportedCostAbilityIndex).has_value());
    loyaltyEntries[kUnsupportedCostAbilityIndex]->loyaltyXChoice = RuledLoyaltyXChoice{0, 5};
    EXPECT_FALSE(ruledBuildCounterGroupBegin(*state, loyaltyX, payments).has_value());

    applyRaw(aggregateCounterOfferBatch());
    auto manaX = makeConfirmedPending();
    auto &manaXEntries = state->activatedAbilitiesByOid[kUnsupportedCostSourceOid];
    ASSERT_EQ(manaXEntries.size(), kUnsupportedCostAbilityIndex + 1);
    ASSERT_TRUE(manaXEntries.at(kUnsupportedCostAbilityIndex).has_value());
    manaXEntries[kUnsupportedCostAbilityIndex]->manaCost = QStringLiteral("{X}{B/P}{W/P}");
    manaX.flexPips = RuledPendingCast::parseFlexPips(manaXEntries.at(kUnsupportedCostAbilityIndex)->manaCost);
    payments = aggregatePhiPayments(manaX);
    EXPECT_FALSE(ruledBuildCounterGroupBegin(*state, manaX, payments).has_value());
    EXPECT_TRUE(host.sentCommands.isEmpty());
}

TEST_F(RuledClientTest, AggregateCounterBeginRejectsUnknownCurrentSourceZoneEvenWhenCached)
{
    applyRaw(aggregateCounterOfferBatch());
    auto pending = makeAggregateCounterDraft(*state);
    const QVector<RuledPendingCounterContribution> exactRows = {
        {1, 901, 13, kNightCounterOptionId, 1}, {1, 901, 13, kChargeCounterOptionId, 2}};
    ASSERT_TRUE(ruledStageCounterContributions(pending, 1, 901, 13, exactRows));
    ASSERT_TRUE(ruledConfirmCounterContributions(pending));

    const auto unknownZone = static_cast<ruled::v1::AbilitySourceZone>(99);
    state->zoneAbilitySourceByOid.insert(kUnsupportedCostSourceOid, unknownZone);
    pending.sourceZone = unknownZone;
    EXPECT_FALSE(ruledBuildCounterGroupBegin(*state, pending, aggregatePhiPayments(pending)).has_value());
    EXPECT_TRUE(host.sentCommands.isEmpty());
}

TEST_F(RuledClientTest, AggregateCounterDraftRefreshRequiresNewConfirmationWithoutRebindingGeneration)
{
    applyRaw(aggregateCounterOfferBatch());
    PendingActivatedAbility pending;
    pending.valid = true;
    pending.permanentOid = kUnsupportedCostSourceOid;
    pending.sourceZone = state->abilitySourceZone(kUnsupportedCostSourceOid);
    pending.expectedZoneChangeGeneration = kUnsupportedCostSourceGeneration;
    pending.abilityIndex = kUnsupportedCostAbilityIndex;
    pending.costChoices = state->abilityCostChoices(kUnsupportedCostSourceOid, kUnsupportedCostAbilityIndex);
    pending.counterGroupContributions = {{1, 901, 13, kChargeCounterOptionId, 3}};
    pending.counterGroupConfirmed = true;

    auto changed = aggregateCounterOfferBatch();
    auto &actions = (*changed.mutable_legal_by_player())[kLocalPlayer];
    const quint64 wireKey = (static_cast<quint64>(kUnsupportedCostSourceOid) << 32) |
                            static_cast<quint32>(kUnsupportedCostAbilityIndex);
    auto *choice = (*actions.mutable_cost_choices_by_ability())[wireKey].mutable_choices(0);
    choice->mutable_counter_removal_group()->mutable_candidates(0)->mutable_object()->set_zone_change_generation(99);
    applyRaw(changed);
    const auto changedChoices = state->abilityCostChoices(kUnsupportedCostSourceOid, kUnsupportedCostAbilityIndex);
    EXPECT_FALSE(ruledReconcileCounterContributions(pending, changedChoices));
    EXPECT_TRUE(pending.counterGroupContributions.isEmpty());
    EXPECT_FALSE(pending.counterGroupConfirmed);
    ASSERT_EQ(pending.costChoices.size(), 1);
    ASSERT_TRUE(pending.costChoices.front().counterRemovalGroup.has_value());
    EXPECT_EQ(pending.costChoices.front().counterRemovalGroup->candidates.front().zoneChangeGeneration, 99u);

    pending.counterGroupContributions = {{1, 901, 99, kChargeCounterOptionId, 3}};
    pending.counterGroupConfirmed = true;
    auto cosmetic = aggregateCounterOfferBatch();
    auto &cosmeticActions = (*cosmetic.mutable_legal_by_player())[kLocalPlayer];
    auto *cosmeticChoice = (*cosmeticActions.mutable_cost_choices_by_ability())[wireKey].mutable_choices(0);
    auto *cosmeticGroup = cosmeticChoice->mutable_counter_removal_group();
    cosmeticGroup->mutable_candidates(0)->mutable_object()->set_zone_change_generation(99);
    cosmeticGroup->mutable_candidates(0)->mutable_options(0)->set_label("Night counters");
    cosmeticGroup->mutable_candidates()->SwapElements(0, 1);
    cosmeticGroup->mutable_candidates(1)->mutable_options()->SwapElements(0, 1);
    applyRaw(cosmetic);
    const auto cosmeticChoices = state->abilityCostChoices(kUnsupportedCostSourceOid, kUnsupportedCostAbilityIndex);
    EXPECT_TRUE(ruledReconcileCounterContributions(pending, cosmeticChoices));
    ASSERT_EQ(pending.counterGroupContributions.size(), 1);
    EXPECT_EQ(pending.counterGroupContributions.front().zoneChangeGeneration, 99u);
    EXPECT_TRUE(pending.counterGroupConfirmed);

    for (int materialChange = 0; materialChange < 3; ++materialChange) {
        applyRaw(aggregateCounterOfferBatch());
        auto draft = makeAggregateCounterDraft(*state);
        if (materialChange == 2) {
            ASSERT_TRUE(ruledStageCounterContributions(
                draft, 1, 901, 13, {{1, 901, 13, kNightCounterOptionId, 1}}));
            ASSERT_TRUE(ruledStageCounterContributions(
                draft, 1, 902, 17, {{1, 902, 17, kChargeCounterOptionId, 2}}));
        } else {
            ASSERT_TRUE(ruledStageCounterContributions(
                draft, 1, 901, 13, {{1, 901, 13, kNightCounterOptionId, 1},
                                    {1, 901, 13, kChargeCounterOptionId, 2}}));
        }
        ASSERT_TRUE(ruledConfirmCounterContributions(draft));
        const auto oldRows = draft.counterGroupContributions;
        const auto oldGeneration = oldRows.front().zoneChangeGeneration;

        auto refreshed = aggregateCounterOfferBatch();
        auto &refreshedActions = (*refreshed.mutable_legal_by_player())[kLocalPlayer];
        auto *refreshedChoice =
            (*refreshedActions.mutable_cost_choices_by_ability())[wireKey].mutable_choices(0);
        auto *refreshedGroup = refreshedChoice->mutable_counter_removal_group();
        if (materialChange == 0) {
            refreshedGroup->mutable_candidates(0)->mutable_options(1)->set_available_count(2);
        } else if (materialChange == 1) {
            refreshedChoice->set_kind(ruled::v1::COST_CHOICE_KIND_TAP);
        } else {
            refreshedGroup->mutable_candidates()->DeleteSubrange(1, 1);
        }
        applyRaw(refreshed);

        auto flexPayments = aggregatePhiPayments(draft);
        EXPECT_FALSE(ruledBuildCounterGroupBegin(*state, draft, flexPayments).has_value());
        EXPECT_TRUE(draft.counterGroupConfirmed);
        ASSERT_EQ(draft.counterGroupContributions.size(), oldRows.size());
        for (int rowIndex = 0; rowIndex < oldRows.size(); ++rowIndex) {
            EXPECT_EQ(draft.counterGroupContributions.at(rowIndex).objectId, oldRows.at(rowIndex).objectId);
            EXPECT_EQ(draft.counterGroupContributions.at(rowIndex).zoneChangeGeneration,
                      oldRows.at(rowIndex).zoneChangeGeneration);
            EXPECT_EQ(draft.counterGroupContributions.at(rowIndex).optionId, oldRows.at(rowIndex).optionId);
            EXPECT_EQ(draft.counterGroupContributions.at(rowIndex).count, oldRows.at(rowIndex).count);
        }
        EXPECT_EQ(draft.counterGroupContributions.front().zoneChangeGeneration, oldGeneration);

        const auto refreshedChoices = state->abilityCostChoices(kUnsupportedCostSourceOid, kUnsupportedCostAbilityIndex);
        EXPECT_FALSE(ruledReconcileCounterContributions(draft, refreshedChoices));
        EXPECT_TRUE(draft.counterGroupContributions.isEmpty());
        EXPECT_FALSE(draft.counterGroupConfirmed);
        ASSERT_EQ(draft.costChoices.size(), 1);
        if (materialChange == 1) {
            EXPECT_EQ(draft.costChoices.front().kind, RuledCostChoiceKind::Tap);
            EXPECT_FALSE(draft.costChoices.front().counterRemovalGroup.has_value());
        } else {
            ASSERT_TRUE(draft.costChoices.front().counterRemovalGroup.has_value());
            if (materialChange == 0) {
                EXPECT_EQ(draft.costChoices.front().counterRemovalGroup->candidates.front().options.at(1).availableCount,
                          2u);
            } else {
                EXPECT_FALSE(std::any_of(draft.costChoices.front().counterRemovalGroup->candidates.cbegin(),
                                         draft.costChoices.front().counterRemovalGroup->candidates.cend(),
                                         [](const auto &candidate) { return candidate.objectId == 902; }));
            }
        }
    }
}

TEST_F(RuledClientTest, AggregateCounterLiveReconciliationRefreshesMaterialOffersAndRemainsEditable)
{
    const quint64 wireKey = (static_cast<quint64>(kUnsupportedCostSourceOid) << 32) |
                            static_cast<quint32>(kUnsupportedCostAbilityIndex);
    auto initialBatch = aggregateCounterTwoGroupOfferBatch();
    const auto &publishedAbility = initialBatch.events(0)
                                      .zone_view()
                                      .per_player(0)
                                      .battlefield_objects(0)
                                      .activated_abilities(0);
    ASSERT_TRUE(publishedAbility.activatable());
    applyRaw(initialBatch);

    const auto published = state->activatedAbilityForOid(kUnsupportedCostSourceOid,
                                                          kUnsupportedCostAbilityIndex);
    ASSERT_TRUE(published.has_value());
    EXPECT_TRUE(published->activatable);
    EXPECT_EQ(state->abilitySourceGeneration(kUnsupportedCostSourceOid), kUnsupportedCostSourceGeneration);

    const auto initialOffers = state->abilityCostChoices(kUnsupportedCostSourceOid,
                                                          kUnsupportedCostAbilityIndex);
    ASSERT_EQ(initialOffers.size(), 2);
    EXPECT_EQ(initialOffers.at(0).costIndex, 1u);
    EXPECT_EQ(initialOffers.at(1).costIndex, 4u);
    ASSERT_TRUE(initialOffers.at(0).counterRemovalGroup.has_value());
    ASSERT_TRUE(initialOffers.at(1).counterRemovalGroup.has_value());
    EXPECT_EQ(initialOffers.at(0).counterRemovalGroup->requiredCount, 3u);
    EXPECT_EQ(initialOffers.at(1).counterRemovalGroup->requiredCount, 1u);
    ASSERT_EQ(initialOffers.at(0).counterRemovalGroup->candidates.size(), 3);
    ASSERT_EQ(initialOffers.at(1).counterRemovalGroup->candidates.size(), 1);
    ASSERT_EQ(initialOffers.at(0).counterRemovalGroup->candidates.front().options.size(), 2);
    EXPECT_EQ(initialOffers.at(0).counterRemovalGroup->candidates.front().objectId, 901u);
    EXPECT_EQ(initialOffers.at(0).counterRemovalGroup->candidates.front().zoneChangeGeneration, 13u);
    EXPECT_EQ(initialOffers.at(0).counterRemovalGroup->candidates.front().options.at(0).optionId,
              kNightCounterOptionId);
    EXPECT_EQ(initialOffers.at(0).counterRemovalGroup->candidates.front().options.at(0).availableCount, 2u);
    EXPECT_EQ(initialOffers.at(0).counterRemovalGroup->candidates.front().options.at(1).optionId,
              kChargeCounterOptionId);
    EXPECT_EQ(initialOffers.at(0).counterRemovalGroup->candidates.front().options.at(1).availableCount, 3u);
    EXPECT_EQ(initialOffers.at(1).counterRemovalGroup->candidates.front().objectId, 901u);
    EXPECT_EQ(initialOffers.at(1).counterRemovalGroup->candidates.front().zoneChangeGeneration, 13u);
    ASSERT_EQ(initialOffers.at(1).counterRemovalGroup->candidates.front().options.size(), 1);
    EXPECT_EQ(initialOffers.at(1).counterRemovalGroup->candidates.front().options.front().optionId,
              kChargeCounterOptionId);
    EXPECT_EQ(initialOffers.at(1).counterRemovalGroup->candidates.front().options.front().availableCount, 2u);

    auto refreshedBatch = aggregateCounterTwoGroupOfferBatch();
    auto &refreshedActions = (*refreshedBatch.mutable_legal_by_player())[kLocalPlayer];
    auto &refreshedCosts = (*refreshedActions.mutable_cost_choices_by_ability())[wireKey];
    ASSERT_EQ(refreshedCosts.choices_size(), 2);
    auto *refreshedSecondGroup =
        refreshedCosts.mutable_choices(1)->mutable_counter_removal_group();
    ASSERT_EQ(refreshedSecondGroup->candidates_size(), 1);
    ASSERT_EQ(refreshedSecondGroup->candidates(0).options_size(), 1);
    refreshedSecondGroup->mutable_candidates(0)->mutable_options(0)->set_available_count(1);
    EXPECT_EQ(refreshedSecondGroup->candidates(0).options(0).available_count(), 1u);

    RuledPendingCast owner;
    auto &draft = prepareOwnedAggregateCounterDraft(owner, *state);
    ASSERT_TRUE(draft.valid);
    ASSERT_TRUE(draft.waitingForCost);
    ASSERT_EQ(draft.costChoices.size(), 2);
    ASSERT_EQ(draft.nextCostChoice, 0);
    ASSERT_TRUE(ruledStageCounterContributions(
        draft,
        1,
        901,
        13,
        {{1, 901, 13, kNightCounterOptionId, 2}, {1, 901, 13, kChargeCounterOptionId, 1}}));
    ASSERT_TRUE(ruledStageCounterContributions(
        draft, 4, 901, 13, {{4, 901, 13, kChargeCounterOptionId, 1}}));
    ASSERT_TRUE(ruledConfirmCounterContributions(draft));
    EXPECT_EQ(draft.counterGroupContributions.size(), 3);
    EXPECT_TRUE(draft.counterGroupConfirmed);

    EXPECT_TRUE(owner.reconcileAbilityCosts(*state, kLocalPlayer));
    EXPECT_TRUE(draft.valid);
    EXPECT_TRUE(draft.waitingForCost);
    EXPECT_EQ(draft.counterGroupContributions.size(), 3);
    EXPECT_TRUE(draft.counterGroupConfirmed);

    applyRaw(refreshedBatch);
    const auto liveOffers = state->abilityCostChoices(kUnsupportedCostSourceOid,
                                                       kUnsupportedCostAbilityIndex);
    EXPECT_EQ(liveOffers.size(), 2);
    if (liveOffers.size() == 2 && liveOffers.at(1).counterRemovalGroup.has_value() &&
        !liveOffers.at(1).counterRemovalGroup->candidates.isEmpty() &&
        !liveOffers.at(1).counterRemovalGroup->candidates.front().options.isEmpty()) {
        EXPECT_EQ(liveOffers.at(1).counterRemovalGroup->candidates.front().options.front().availableCount, 1u);
    }

    EXPECT_TRUE(owner.reconcileAbilityCosts(*state, kLocalPlayer));
    EXPECT_TRUE(draft.valid);
    EXPECT_TRUE(draft.waitingForCost);
    EXPECT_TRUE(draft.counterGroupContributions.isEmpty());
    EXPECT_FALSE(draft.counterGroupConfirmed);
    EXPECT_EQ(draft.costChoices.size(), 2);
    if (draft.costChoices.size() == 2) {
        EXPECT_EQ(draft.costChoices.at(0).costIndex, 1u);
        EXPECT_EQ(draft.costChoices.at(1).costIndex, 4u);
        if (draft.costChoices.at(1).counterRemovalGroup.has_value() &&
            !draft.costChoices.at(1).counterRemovalGroup->candidates.isEmpty() &&
            !draft.costChoices.at(1).counterRemovalGroup->candidates.front().options.isEmpty()) {
            EXPECT_EQ(draft.costChoices.at(1).counterRemovalGroup->candidates.front().options.front().availableCount,
                      1u);
        }
        EXPECT_LT(draft.nextCostChoice, draft.costChoices.size());
        if (draft.nextCostChoice < draft.costChoices.size()) {
            EXPECT_EQ(draft.costChoices.at(draft.nextCostChoice).costIndex, 1u);
        }
    }

    const auto actualContext = ruledCounterQuantityContext(
        *state, draft, 1, 901, 13, QStringLiteral("Live refreshed offer"));
    EXPECT_TRUE(actualContext.has_value());
    if (actualContext.has_value()) {
        EXPECT_EQ(actualContext->costIndex, 1u);
        RuledCounterQuantityDialog::Result nightSelection;
        nightSelection.costIndex = actualContext->costIndex;
        nightSelection.objectId = actualContext->objectId;
        nightSelection.zoneChangeGeneration = actualContext->zoneChangeGeneration;
        nightSelection.selections.append({kNightCounterOptionId, 1});
        const auto actualRows = ruledCounterQuantityRows(*actualContext, nightSelection);
        EXPECT_TRUE(actualRows.has_value());
        if (actualRows.has_value()) {
            EXPECT_EQ(actualRows->size(), 1);
            EXPECT_TRUE(ruledStageCounterContributions(
                draft, 1, 901, 13, *actualRows));
            EXPECT_TRUE(draft.valid);
            EXPECT_TRUE(draft.waitingForCost);
            EXPECT_FALSE(draft.counterGroupConfirmed);
            EXPECT_EQ(draft.counterGroupContributions.size(), 1);
            if (draft.counterGroupContributions.size() == 1) {
                const auto &row = draft.counterGroupContributions.front();
                EXPECT_EQ(row.costIndex, 1u);
                EXPECT_EQ(row.objectId, 901u);
                EXPECT_EQ(row.zoneChangeGeneration, 13u);
                EXPECT_EQ(row.optionId, kNightCounterOptionId);
                EXPECT_EQ(row.count, 1u);
            }
        }
    }
}

TEST_F(RuledClientTest, AggregateCounterLiveReconcilePreservesCosmeticRowsAndActiveCostIndex)
{
    const quint64 wireKey = (static_cast<quint64>(kUnsupportedCostSourceOid) << 32) |
                            static_cast<quint32>(kUnsupportedCostAbilityIndex);
    applyRaw(aggregateCounterTwoGroupOfferBatch());

    RuledPendingCast owner;
    auto &draft = prepareOwnedAggregateCounterDraft(owner, *state);
    ASSERT_EQ(draft.costChoices.size(), 2);
    ASSERT_TRUE(ruledStageCounterContributions(
        draft, 1, 901, 13, {{1, 901, 13, kNightCounterOptionId, 2}, {1, 901, 13, kChargeCounterOptionId, 1}}));
    ASSERT_TRUE(ruledStageCounterContributions(draft, 4, 901, 13, {{4, 901, 13, kChargeCounterOptionId, 1}}));
    ASSERT_TRUE(ruledConfirmCounterContributions(draft));
    draft.nextCostChoice = 1; // The active cost is identified by costIndex 4, not by its old vector position.

    auto cosmeticBatch = aggregateCounterTwoGroupOfferBatch();
    auto &actions = (*cosmeticBatch.mutable_legal_by_player())[kLocalPlayer];
    auto &costs = (*actions.mutable_cost_choices_by_ability())[wireKey];
    ASSERT_EQ(costs.choices_size(), 2);

    auto *firstGroup = costs.mutable_choices(0)->mutable_counter_removal_group();
    ASSERT_EQ(firstGroup->candidates_size(), 3);
    auto *firstCandidate = firstGroup->mutable_candidates(0);
    ASSERT_EQ(firstCandidate->object().object_id(), 901u);
    ASSERT_EQ(firstCandidate->options_size(), 2);
    firstCandidate->mutable_options()->SwapElements(0, 1);
    firstCandidate->mutable_options(0)->set_label("Charge, refreshed");
    firstCandidate->mutable_options(1)->set_label("Night, refreshed");
    firstGroup->mutable_candidates()->SwapElements(0, 1);
    costs.mutable_choices(1)
        ->mutable_counter_removal_group()
        ->mutable_candidates(0)
        ->mutable_options(0)
        ->set_label("Charge for cost 4");
    costs.mutable_choices()->SwapElements(0, 1);
    applyRaw(cosmeticBatch);

    EXPECT_TRUE(owner.reconcileAbilityCosts(*state, kLocalPlayer));
    EXPECT_TRUE(draft.valid);
    EXPECT_TRUE(draft.waitingForCost);
    EXPECT_TRUE(draft.counterGroupConfirmed);
    ASSERT_EQ(draft.counterGroupContributions.size(), 3);
    ASSERT_EQ(draft.costChoices.size(), 2);
    EXPECT_EQ(draft.costChoices.at(0).costIndex, 4);
    EXPECT_EQ(draft.costChoices.at(1).costIndex, 1);
    EXPECT_EQ(draft.nextCostChoice, 0);

    EXPECT_EQ(draft.counterGroupContributions.at(0).costIndex, 1);
    EXPECT_EQ(draft.counterGroupContributions.at(0).objectId, 901u);
    EXPECT_EQ(draft.counterGroupContributions.at(0).zoneChangeGeneration, 13u);
    EXPECT_EQ(draft.counterGroupContributions.at(0).optionId, kNightCounterOptionId);
    EXPECT_EQ(draft.counterGroupContributions.at(0).count, 2u);
    EXPECT_EQ(draft.counterGroupContributions.at(0).counterLabel, QStringLiteral("Night, refreshed"));
    EXPECT_EQ(draft.counterGroupContributions.at(1).costIndex, 1);
    EXPECT_EQ(draft.counterGroupContributions.at(1).objectId, 901u);
    EXPECT_EQ(draft.counterGroupContributions.at(1).zoneChangeGeneration, 13u);
    EXPECT_EQ(draft.counterGroupContributions.at(1).optionId, kChargeCounterOptionId);
    EXPECT_EQ(draft.counterGroupContributions.at(1).count, 1u);
    EXPECT_EQ(draft.counterGroupContributions.at(1).counterLabel, QStringLiteral("Charge, refreshed"));
    EXPECT_EQ(draft.counterGroupContributions.at(2).costIndex, 4);
    EXPECT_EQ(draft.counterGroupContributions.at(2).objectId, 901u);
    EXPECT_EQ(draft.counterGroupContributions.at(2).zoneChangeGeneration, 13u);
    EXPECT_EQ(draft.counterGroupContributions.at(2).optionId, kChargeCounterOptionId);
    EXPECT_EQ(draft.counterGroupContributions.at(2).count, 1u);
    EXPECT_EQ(draft.counterGroupContributions.at(2).counterLabel, QStringLiteral("Charge for cost 4"));
}

TEST_F(RuledClientTest, AggregateCounterLiveReconcileNormalizesRemovedGroupsAndRejectsMissingLastGroup)
{
    const quint64 wireKey = (static_cast<quint64>(kUnsupportedCostSourceOid) << 32) |
                            static_cast<quint32>(kUnsupportedCostAbilityIndex);
    applyRaw(aggregateCounterTwoGroupOfferBatch());

    RuledPendingCast owner;
    auto &draft = prepareOwnedAggregateCounterDraft(owner, *state);
    ASSERT_TRUE(ruledStageCounterContributions(
        draft, 1, 901, 13, {{1, 901, 13, kNightCounterOptionId, 2}, {1, 901, 13, kChargeCounterOptionId, 1}}));
    ASSERT_TRUE(ruledStageCounterContributions(draft, 4, 901, 13, {{4, 901, 13, kChargeCounterOptionId, 1}}));
    ASSERT_TRUE(ruledConfirmCounterContributions(draft));
    draft.nextCostChoice = 1;

    auto removedActiveGroup = aggregateCounterTwoGroupOfferBatch();
    auto &actions = (*removedActiveGroup.mutable_legal_by_player())[kLocalPlayer];
    auto &costs = (*actions.mutable_cost_choices_by_ability())[wireKey];
    ASSERT_EQ(costs.choices_size(), 2);
    ASSERT_EQ(costs.choices(1).cost_index(), 4u);
    costs.mutable_choices()->DeleteSubrange(1, 1);
    applyRaw(removedActiveGroup);

    EXPECT_TRUE(owner.reconcileAbilityCosts(*state, kLocalPlayer));
    EXPECT_TRUE(draft.valid);
    EXPECT_TRUE(draft.waitingForCost);
    EXPECT_FALSE(draft.counterGroupConfirmed);
    EXPECT_TRUE(draft.counterGroupContributions.isEmpty());
    ASSERT_EQ(draft.costChoices.size(), 1);
    EXPECT_EQ(draft.costChoices.front().costIndex, 1);
    EXPECT_EQ(draft.nextCostChoice, 0);

    auto missingLastGroup = aggregateCounterTwoGroupOfferBatch();
    auto &missingActions = (*missingLastGroup.mutable_legal_by_player())[kLocalPlayer];
    missingActions.mutable_cost_choices_by_ability()->erase(wireKey);
    applyRaw(missingLastGroup);

    EXPECT_FALSE(owner.reconcileAbilityCosts(*state, kLocalPlayer));
    EXPECT_TRUE(draft.valid);
    EXPECT_TRUE(draft.waitingForCost);
    EXPECT_TRUE(draft.counterGroupContributions.isEmpty());
    EXPECT_FALSE(draft.counterGroupConfirmed);
    ASSERT_EQ(draft.costChoices.size(), 1);
    EXPECT_EQ(draft.costChoices.front().costIndex, 1);
}

TEST_F(RuledClientTest, AggregateCounterLiveReconcileClearsAllRowsForMaterialOfferChanges)
{
    const quint64 wireKey = (static_cast<quint64>(kUnsupportedCostSourceOid) << 32) |
                            static_cast<quint32>(kUnsupportedCostAbilityIndex);
    const std::array<const char *, 7> changeNames = {"required count",
                                                     "shared candidate generation",
                                                     "shared option id",
                                                     "candidate addition",
                                                     "candidate removal",
                                                     "cost group addition",
                                                     "cost group removal"};

    for (int materialChange = 0; materialChange < static_cast<int>(changeNames.size()); ++materialChange) {
        SCOPED_TRACE(changeNames.at(static_cast<std::size_t>(materialChange)));
        applyRaw(aggregateCounterTwoGroupOfferBatch());

        RuledPendingCast owner;
        auto &draft = prepareOwnedAggregateCounterDraft(owner, *state);
        ASSERT_TRUE(ruledStageCounterContributions(
            draft, 1, 901, 13, {{1, 901, 13, kNightCounterOptionId, 2}, {1, 901, 13, kChargeCounterOptionId, 1}}));
        ASSERT_TRUE(ruledStageCounterContributions(draft, 4, 901, 13, {{4, 901, 13, kChargeCounterOptionId, 1}}));
        ASSERT_TRUE(ruledConfirmCounterContributions(draft));
        ASSERT_EQ(draft.counterGroupContributions.size(), 3);

        auto changedBatch = aggregateCounterTwoGroupOfferBatch();
        auto &actions = (*changedBatch.mutable_legal_by_player())[kLocalPlayer];
        auto &costs = (*actions.mutable_cost_choices_by_ability())[wireKey];
        ASSERT_EQ(costs.choices_size(), 2);
        switch (materialChange) {
            case 0:
                costs.mutable_choices(1)->mutable_counter_removal_group()->set_required_count(2);
                break;
            case 1:
                for (auto &choice : *costs.mutable_choices()) {
                    for (auto &legacy : *choice.mutable_candidate_objects()) {
                        if (legacy.has_object() && legacy.object().object_id() == 901)
                            legacy.mutable_object()->set_zone_change_generation(14);
                    }
                    auto *group = choice.mutable_counter_removal_group();
                    for (auto &candidate : *group->mutable_candidates()) {
                        if (candidate.object().object_id() == 901)
                            candidate.mutable_object()->set_zone_change_generation(14);
                    }
                }
                break;
            case 2: {
                const quint32 replacementOptionId = kChargeCounterOptionId ^ 0x0000ffffu;
                for (auto &choice : *costs.mutable_choices()) {
                    auto *group = choice.mutable_counter_removal_group();
                    for (auto &candidate : *group->mutable_candidates()) {
                        for (auto &option : *candidate.mutable_options()) {
                            if (option.option_id() == kChargeCounterOptionId)
                                option.set_option_id(replacementOptionId);
                        }
                    }
                }
                break;
            }
            case 3: {
                auto *candidate = costs.mutable_choices(0)->mutable_counter_removal_group()->add_candidates();
                candidate->mutable_object()->set_object_id(904);
                candidate->mutable_object()->set_zone_change_generation(29);
                auto *charge = candidate->add_options();
                charge->set_option_id(kChargeCounterOptionId);
                charge->set_label("Charge");
                charge->set_available_count(1);
                break;
            }
            case 4: {
                costs.mutable_choices(0)
                    ->mutable_counter_removal_group()
                    ->mutable_candidates()
                    ->DeleteSubrange(2, 1);
                break;
            }
            case 5: {
                const auto addedGroup = costs.choices(1);
                auto *choice = costs.add_choices();
                choice->CopyFrom(addedGroup);
                choice->set_cost_index(5);
                break;
            }
            case 6:
                costs.mutable_choices()->DeleteSubrange(0, 1);
                break;
        }
        applyRaw(changedBatch);

        EXPECT_TRUE(owner.reconcileAbilityCosts(*state, kLocalPlayer));
        EXPECT_TRUE(draft.valid);
        EXPECT_TRUE(draft.waitingForCost);
        EXPECT_FALSE(draft.counterGroupConfirmed);
        EXPECT_TRUE(draft.counterGroupContributions.isEmpty());
        const int expectedGroupCount = materialChange == 5 ? 3 : (materialChange == 6 ? 1 : 2);
        ASSERT_EQ(draft.costChoices.size(), expectedGroupCount);
        if (materialChange == 6) {
            EXPECT_EQ(draft.costChoices.front().costIndex, 4);
            EXPECT_EQ(draft.nextCostChoice, 0);
        }
    }
}

TEST_F(RuledClientTest, AggregateCounterLiveReconcilePreservesOwnedAndAcceptedTransactions)
{
    const quint64 wireKey = (static_cast<quint64>(kUnsupportedCostSourceOid) << 32) |
                            static_cast<quint32>(kUnsupportedCostAbilityIndex);
    const auto sameRows = [](const QVector<RuledPendingCounterContribution> &left,
                             const QVector<RuledPendingCounterContribution> &right) {
        if (left.size() != right.size())
            return false;
        for (int i = 0; i < left.size(); ++i) {
            const auto &a = left.at(i);
            const auto &b = right.at(i);
            if (a.costIndex != b.costIndex || a.objectId != b.objectId ||
                a.zoneChangeGeneration != b.zoneChangeGeneration || a.optionId != b.optionId || a.count != b.count ||
                a.counterLabel != b.counterLabel) {
                return false;
            }
        }
        return true;
    };
    const auto sameFlexPayments = [](const QVector<ruled::v1::FlexPipPayment> &left,
                                     const QVector<ruled::v1::FlexPipPayment> &right) {
        if (left.size() != right.size())
            return false;
        for (int i = 0; i < left.size(); ++i) {
            if (left.at(i).pip_index() != right.at(i).pip_index() ||
                left.at(i).pay_life() != right.at(i).pay_life()) {
                return false;
            }
        }
        return true;
    };
    const auto prepareTwoGroupBegin = [&](RuledPendingCast &owner) {
        auto &draft = prepareOwnedAggregateCounterDraft(owner, *state);
        const bool stagedFirst = ruledStageCounterContributions(
            draft, 1, 901, 13, {{1, 901, 13, kNightCounterOptionId, 2}, {1, 901, 13, kChargeCounterOptionId, 1}});
        const bool stagedSecond = ruledStageCounterContributions(draft, 4, 901, 13, {{4, 901, 13, kChargeCounterOptionId, 1}});
        const bool confirmed = ruledConfirmCounterContributions(draft);
        draft.waitingForCost = false;
        if (!stagedFirst || !stagedSecond || !confirmed)
            return std::optional<RuledOwnedCounterGroupBegin>{};
        return owner.prepareCounterGroupBegin(*state, kLocalPlayer, aggregatePhiPayments(draft));
    };

    applyRaw(aggregateCounterTwoGroupOfferBatch());
    RuledPendingCast ownedOwner;
    const auto ownedCapture = prepareTwoGroupBegin(ownedOwner);
    ASSERT_TRUE(ownedCapture.has_value());
    ASSERT_NE(ownedOwner.ability.ownedBegin(), nullptr);
    const auto ownedDraftId = ownedOwner.ability.draftId;
    const auto ownedAnnouncement = ownedOwner.ability.ownedBegin()->announcement.SerializeAsString();
    const auto ownedRows = ownedOwner.ability.counterGroupContributions;
    const auto ownedFlex = ownedOwner.ability.fixedFlexPayments;
    const auto ownedSourceId = ownedOwner.ability.permanentOid;
    const auto ownedSourceGeneration = ownedOwner.ability.expectedZoneChangeGeneration;
    const auto ownedSourceZone = ownedOwner.ability.sourceZone;
    const auto ownedAbilityIndex = ownedOwner.ability.abilityIndex;

    auto noSourceOrOffers = aggregateCounterTwoGroupOfferBatch();
    noSourceOrOffers.mutable_events(0)
        ->mutable_zone_view()
        ->mutable_per_player(0)
        ->clear_battlefield_objects();
    auto &emptyActions = (*noSourceOrOffers.mutable_legal_by_player())[kLocalPlayer];
    emptyActions.mutable_cost_choices_by_ability()->erase(wireKey);
    applyRaw(noSourceOrOffers);
    EXPECT_FALSE(state->activatedAbilityForOid(kUnsupportedCostSourceOid, kUnsupportedCostAbilityIndex).has_value());
    EXPECT_TRUE(state->abilityCostChoices(kUnsupportedCostSourceOid, kUnsupportedCostAbilityIndex).isEmpty());

    EXPECT_TRUE(ownedOwner.reconcileAbilityCosts(*state, kLocalPlayer));
    EXPECT_NE(ownedOwner.ability.ownedBegin(), nullptr);
    EXPECT_EQ(ownedOwner.ability.stage(), PendingActivatedAbility::Stage::BeginPending);
    EXPECT_EQ(ownedOwner.ability.draftId, ownedDraftId);
    EXPECT_EQ(ownedOwner.ability.ownedBegin()->draftId, ownedDraftId);
    EXPECT_EQ(ownedOwner.ability.ownedBegin()->announcement.SerializeAsString(), ownedAnnouncement);
    EXPECT_TRUE(sameRows(ownedOwner.ability.counterGroupContributions, ownedRows));
    EXPECT_TRUE(sameFlexPayments(ownedOwner.ability.fixedFlexPayments, ownedFlex));
    EXPECT_EQ(ownedOwner.ability.permanentOid, ownedSourceId);
    EXPECT_EQ(ownedOwner.ability.expectedZoneChangeGeneration, ownedSourceGeneration);
    EXPECT_EQ(ownedOwner.ability.sourceZone, ownedSourceZone);
    EXPECT_EQ(ownedOwner.ability.abilityIndex, ownedAbilityIndex);
    ASSERT_EQ(ownedOwner.ability.costChoices.size(), 2);

    // A begin already sent to the host has a short source-removal gap before its engine echo.
    RuledPendingCast submittedGap = ownedOwner;
    submittedGap.ability.beginOrdinaryAnnouncement();
    EXPECT_TRUE(submittedGap.reconcileAbilityCosts(*state, kLocalPlayer));
    EXPECT_EQ(submittedGap.ability.stage(), PendingActivatedAbility::Stage::BeginPending);
    EXPECT_EQ(submittedGap.ability.permanentOid, ownedSourceId);
    EXPECT_TRUE(sameRows(submittedGap.ability.counterGroupContributions, ownedRows));
    EXPECT_TRUE(sameFlexPayments(submittedGap.ability.fixedFlexPayments, ownedFlex));

    for (int offerCase = 0; offerCase < 2; ++offerCase) {
        const bool removeOffers = offerCase == 0;
        SCOPED_TRACE(removeOffers ? "accepted transaction with empty live offers" :
                                    "accepted transaction with changed live offers");
        applyRaw(aggregateCounterTwoGroupOfferBatch());
        RuledPendingCast acceptedOwner;
        const auto acceptedCapture = prepareTwoGroupBegin(acceptedOwner);
        ASSERT_TRUE(acceptedCapture.has_value());
        auto accepted = makeOwnedAggregateCounterPaymentSnapshot(*acceptedCapture, 91);
        accepted.set_locked_total_cost("3");
        ASSERT_EQ(acceptedOwner.acknowledgeCounterGroupBegin(*acceptedCapture, true, &accepted),
                  RuledOwnedCounterGroupBeginUpdate::Accepted);
        ASSERT_EQ(acceptedOwner.ability.stage(), PendingActivatedAbility::Stage::Paying);
        ASSERT_EQ(acceptedOwner.ability.transactionId(), 91u);
        acceptedOwner.ability.remainingCost.insert(QLatin1Char('X'), 3); // Distinguishable local payment staging.
        ASSERT_EQ(acceptedOwner.ability.remainingCost.value(QLatin1Char('X')), 3);
        ASSERT_EQ(acceptedOwner.ability.counterGroupContributions.size(), 3);
        ASSERT_EQ(acceptedOwner.ability.fixedFlexPayments.size(), 2);
        ASSERT_TRUE(acceptedOwner.ability.counterGroupConfirmed);
        ASSERT_TRUE(acceptedOwner.ability.hasInitializedPayment());

        auto refreshedBatch = aggregateCounterTwoGroupOfferBatch();
        refreshedBatch.mutable_events(0)
            ->mutable_zone_view()
            ->mutable_per_player(0)
            ->clear_battlefield_objects();
        auto &refreshedActions = (*refreshedBatch.mutable_legal_by_player())[kLocalPlayer];
        refreshedActions.mutable_pending_ability_activation()->CopyFrom(accepted);
        auto &refreshedCosts = (*refreshedActions.mutable_cost_choices_by_ability())[wireKey];
        if (removeOffers) {
            refreshedActions.mutable_cost_choices_by_ability()->erase(wireKey);
        } else {
            ASSERT_EQ(refreshedCosts.choices_size(), 2);
            refreshedCosts.mutable_choices(1)
                ->mutable_counter_removal_group()
                ->mutable_candidates(0)
                ->mutable_options(0)
                ->set_available_count(1);
        }
        applyRaw(refreshedBatch);

        EXPECT_FALSE(state->activatedAbilityForOid(kUnsupportedCostSourceOid, kUnsupportedCostAbilityIndex).has_value());
        ASSERT_TRUE(state->pendingAbilityActivation.has_value());
        EXPECT_EQ(state->pendingAbilityActivation->transaction_id(), 91u);
        EXPECT_EQ(state->pendingAbilityActivation->actor_player_id(), kLocalPlayer);
        const auto pendingProjection = state->pendingAbilityActivation->SerializeAsString();
        if (removeOffers) {
            EXPECT_TRUE(state->abilityCostChoices(kUnsupportedCostSourceOid, kUnsupportedCostAbilityIndex).isEmpty());
        } else {
            const auto liveChoices = state->abilityCostChoices(kUnsupportedCostSourceOid, kUnsupportedCostAbilityIndex);
            ASSERT_EQ(liveChoices.size(), 2);
            EXPECT_EQ(liveChoices.at(1).counterRemovalGroup->candidates.front().options.front().availableCount, 1u);
        }

        const auto acceptedRows = acceptedOwner.ability.counterGroupContributions;
        const auto acceptedFlex = acceptedOwner.ability.fixedFlexPayments;
        const auto acceptedRemaining = acceptedOwner.ability.remainingCost;
        const auto acceptedLockedCost = acceptedOwner.ability.engineLockedCostLabel;
        const auto acceptedSourceId = acceptedOwner.ability.permanentOid;
        const auto acceptedSourceGeneration = acceptedOwner.ability.expectedZoneChangeGeneration;
        const auto acceptedSourceZone = acceptedOwner.ability.sourceZone;
        const auto acceptedAbilityIndex = acceptedOwner.ability.abilityIndex;
        const auto acceptedDraftId = acceptedOwner.ability.draftId;
        const auto acceptedRevision = acceptedOwner.ability.revision();
        const auto acceptedAbilityText = acceptedOwner.ability.abilityText;
        const auto acceptedNextCostChoice = acceptedOwner.ability.nextCostChoice;
        const auto acceptedWaitingForMana = acceptedOwner.ability.waitingForMana;
        ASSERT_TRUE(acceptedOwner.ability.costChoices.isEmpty());
        ASSERT_TRUE(acceptedOwner.ability.costSelections.isEmpty());

        for (const auto stage : {PendingActivatedAbility::Stage::Paying,
                                 PendingActivatedAbility::Stage::CommitPending,
                                 PendingActivatedAbility::Stage::CancelPending}) {
            auto stagedOwner = acceptedOwner;
            if (stage == PendingActivatedAbility::Stage::CommitPending)
                ASSERT_TRUE(stagedOwner.ability.beginCommit());
            else if (stage == PendingActivatedAbility::Stage::CancelPending)
                ASSERT_TRUE(stagedOwner.ability.beginCancel());
            EXPECT_TRUE(stagedOwner.reconcileAbilityCosts(*state, kLocalPlayer));
            EXPECT_EQ(stagedOwner.ability.stage(), stage);
            EXPECT_EQ(stagedOwner.ability.transactionId(), 91u);
            EXPECT_EQ(stagedOwner.ability.revision(), acceptedRevision);
            EXPECT_EQ(stagedOwner.ability.permanentOid, acceptedSourceId);
            EXPECT_EQ(stagedOwner.ability.expectedZoneChangeGeneration, acceptedSourceGeneration);
            EXPECT_EQ(stagedOwner.ability.sourceZone, acceptedSourceZone);
            EXPECT_EQ(stagedOwner.ability.abilityIndex, acceptedAbilityIndex);
            EXPECT_EQ(stagedOwner.ability.draftId, acceptedDraftId);
            EXPECT_EQ(stagedOwner.ability.abilityText, acceptedAbilityText);
            EXPECT_EQ(stagedOwner.ability.nextCostChoice, acceptedNextCostChoice);
            EXPECT_EQ(stagedOwner.ability.waitingForMana, acceptedWaitingForMana);
            EXPECT_TRUE(sameRows(stagedOwner.ability.counterGroupContributions, acceptedRows));
            EXPECT_TRUE(sameFlexPayments(stagedOwner.ability.fixedFlexPayments, acceptedFlex));
            EXPECT_TRUE(stagedOwner.ability.counterGroupConfirmed);
            EXPECT_TRUE(stagedOwner.ability.hasInitializedPayment());
            EXPECT_EQ(stagedOwner.ability.remainingCost, acceptedRemaining);
            EXPECT_EQ(stagedOwner.ability.engineLockedCostLabel, acceptedLockedCost);
            EXPECT_TRUE(stagedOwner.ability.costChoices.isEmpty());
            EXPECT_TRUE(stagedOwner.ability.costSelections.isEmpty());
            ASSERT_TRUE(state->pendingAbilityActivation.has_value());
            EXPECT_EQ(state->pendingAbilityActivation->SerializeAsString(), pendingProjection);
        }
    }
}

TEST_F(RuledClientTest, UnknownCounterKindsDisablePositiveLegacyCostOffers)
{
    apply(unsupportedCostBatch());
    ASSERT_TRUE(state->activatedAbilityForOid(kUnsupportedCostSourceOid, kUnsupportedCostAbilityIndex));
    EXPECT_TRUE(state->abilityActivatable(kUnsupportedCostSourceOid, kUnsupportedCostAbilityIndex));
    const auto supported = state->abilityCostChoices(kUnsupportedCostSourceOid, kUnsupportedCostAbilityIndex);
    ASSERT_EQ(supported.size(), 1);
    EXPECT_EQ(supported.front().kind, RuledCostChoiceKind::Tap);
    EXPECT_EQ(supported.front().candidateIds, QSet<quint32>{901});
    EXPECT_EQ(supported.front().candidateGenerations.value(901), 13u);

    for (const int wireKind : {777}) {
        apply(unsupportedCostBatch(wireKind));
        ASSERT_TRUE(state->activatedAbilityForOid(kUnsupportedCostSourceOid, kUnsupportedCostAbilityIndex));
        const auto choices = state->abilityCostChoices(kUnsupportedCostSourceOid, kUnsupportedCostAbilityIndex);
        ASSERT_EQ(choices.size(), 1);
        EXPECT_EQ(choices.front().kind, RuledCostChoiceKind::Unspecified);
        EXPECT_EQ(choices.front().min, 1);
        EXPECT_EQ(choices.front().max, 1);
        EXPECT_EQ(choices.front().candidateIds, QSet<quint32>{901});
        EXPECT_EQ(choices.front().candidateGenerations.value(901), 13u);
        const auto key = RuledClientState::abilityTargetKey(kUnsupportedCostSourceOid, kUnsupportedCostAbilityIndex);
        ASSERT_TRUE(state->abilityCostData.contains(key));
        EXPECT_FALSE(state->abilityCostData.value(key).nonManaCostsPayable) << wireKind;
        EXPECT_FALSE(state->abilityActivatable(kUnsupportedCostSourceOid, kUnsupportedCostAbilityIndex)) << wireKind;
    }
}

TEST_F(RuledClientTest, UnsupportedCostRefreshInvalidatesOnlyUnacceptedOrdinaryDrafts)
{
    apply(unsupportedCostBatch(static_cast<int>(ruled::v1::COST_CHOICE_KIND_TAP), true));
    ASSERT_TRUE(state->abilityActivatable(kUnsupportedCostSourceOid, kUnsupportedCostAbilityIndex));
    RuledPendingCast transaction;
    auto &draft = prepareUnsupportedCostDraft(transaction, *state);
    ASSERT_EQ(draft.costChoices.size(), 2);
    ASSERT_EQ(draft.costChoices.at(0).candidateIds, QSet<quint32>{902});
    ASSERT_EQ(draft.costChoices.at(1).kind, RuledCostChoiceKind::Tap);
    draft.nextCostChoice = 1;
    draft.costSelections.append({0, RuledCostChoiceZone::Battlefield, {902}, {14}});
    ASSERT_TRUE(ruledPendingAbilitySourceStillCurrent(*state, draft));
    ASSERT_TRUE(transaction.reconcileAbilityCosts(*state, kLocalPlayer));

    apply(unsupportedCostBatch(static_cast<int>(ruled::v1::COST_CHOICE_KIND_REMOVE_COUNTERS_FROM_PERMANENTS), true));
    const auto current = state->abilityCostChoices(kUnsupportedCostSourceOid, kUnsupportedCostAbilityIndex);
    ASSERT_EQ(current.size(), 2);
    EXPECT_EQ(current.at(0).kind, RuledCostChoiceKind::Tap);
    EXPECT_EQ(current.at(1).kind, RuledCostChoiceKind::RemoveCountersFromPermanents);
    ASSERT_TRUE(draft.valid);
    ASSERT_EQ(draft.costChoices.size(), 2);
    EXPECT_EQ(draft.costChoices.at(1).kind, RuledCostChoiceKind::Tap);
    ASSERT_EQ(draft.costSelections.size(), 1);
    EXPECT_EQ(draft.costSelections.front().selectedIds, QVector<quint32>{902});
    EXPECT_EQ(draft.costSelections.front().selectedGenerations, QVector<quint64>{14});
    EXPECT_FALSE(ruledPendingAbilitySourceStillCurrent(*state, draft));
    EXPECT_FALSE(transaction.reconcileAbilityCosts(*state, kLocalPlayer));
    EXPECT_TRUE(draft.valid) << "reconciliation reports rejection; the UI bridge owns cancellation";
    ASSERT_EQ(draft.costSelections.size(), 1);
    EXPECT_EQ(draft.costSelections.front().selectedIds, QVector<quint32>{902});
}

TEST_F(RuledClientTest, UnsupportedLiveOffersPreserveSubmittedAndMatchingAcceptedActivation)
{
    apply(unsupportedCostBatch(777, false, true));
    const auto cached = state->abilityCostChoices(kUnsupportedCostSourceOid, kUnsupportedCostAbilityIndex);
    ASSERT_EQ(cached.size(), 1);
    ASSERT_EQ(cached.front().kind, RuledCostChoiceKind::Unspecified);
    ASSERT_TRUE(state->permanentActionFor(kUnsupportedCostSourceOid,
                                          kUnsupportedCostSourceGeneration,
                                          ruled::v1::PERMANENT_ACTION_KIND_TURN_FACE_UP,
                                          std::nullopt));

    PendingActivatedAbility draft;
    draft.valid = true;
    draft.permanentOid = kUnsupportedCostSourceOid;
    draft.sourceZone = state->abilitySourceZone(kUnsupportedCostSourceOid);
    draft.expectedZoneChangeGeneration = kUnsupportedCostSourceGeneration;
    draft.abilityIndex = kUnsupportedCostAbilityIndex;
    draft.costChoices = cached;
    PendingActivatedAbility::Target target;
    target.ref.set_kind(ruled::v1::TARGET_REF_KIND_PLAYER);
    target.ref.set_object_id(kOpponent);
    draft.selectedTargets.append(target);
    draft.remainingCost.insert(QChar('G'), 2);
    EXPECT_FALSE(ruledPendingAbilitySourceStillCurrent(*state, draft));

    draft.beginOrdinaryAnnouncement();
    EXPECT_TRUE(ruledPendingAbilitySourceStillCurrent(*state, draft));
    draft.rejectBegin();
    ASSERT_TRUE(draft.beginCommit());
    EXPECT_TRUE(ruledPendingAbilitySourceStillCurrent(*state, draft));

    auto &accepted = state->pendingAbilityActivation.emplace();
    accepted.set_transaction_id(91);
    accepted.set_source_object_id(kUnsupportedCostSourceOid);
    accepted.set_source_zone_change_generation(kUnsupportedCostSourceGeneration);
    accepted.set_ability_index(kUnsupportedCostAbilityIndex);
    accepted.set_stage(ruled::v1::ABILITY_ACTIVATION_STAGE_PAYMENT);
    draft.applyEngineSnapshot(accepted);
    EXPECT_TRUE(ruledPendingAbilitySourceStillCurrent(*state, draft));
    ASSERT_TRUE(draft.beginCommit());
    EXPECT_TRUE(ruledPendingAbilitySourceStillCurrent(*state, draft));

    accepted.set_transaction_id(92);
    EXPECT_FALSE(ruledPendingAbilitySourceStillCurrent(*state, draft));
    accepted.set_transaction_id(91);
    accepted.set_source_object_id(kUnsupportedCostSourceOid + 1);
    EXPECT_FALSE(ruledPendingAbilitySourceStillCurrent(*state, draft));
    accepted.set_source_object_id(kUnsupportedCostSourceOid);
    accepted.set_source_zone_change_generation(kUnsupportedCostSourceGeneration + 1);
    EXPECT_FALSE(ruledPendingAbilitySourceStillCurrent(*state, draft));
    accepted.set_source_zone_change_generation(kUnsupportedCostSourceGeneration);
    accepted.set_ability_index(kUnsupportedCostAbilityIndex + 1);
    EXPECT_FALSE(ruledPendingAbilitySourceStillCurrent(*state, draft));
    accepted.set_ability_index(kUnsupportedCostAbilityIndex);
    EXPECT_TRUE(ruledPendingAbilitySourceStillCurrent(*state, draft));

    draft.resetLifecycle();
    draft.permanentAction = true;
    draft.permanentActionKind = ruled::v1::PERMANENT_ACTION_KIND_TURN_FACE_UP;
    EXPECT_TRUE(ruledPendingAbilitySourceStillCurrent(*state, draft));
    draft.expectedZoneChangeGeneration = kUnsupportedCostSourceGeneration + 1;
    EXPECT_FALSE(ruledPendingAbilitySourceStillCurrent(*state, draft));
    EXPECT_EQ(draft.selectedTargets.size(), 1);
    EXPECT_EQ(draft.selectedTargets.front().ref.object_id(), static_cast<quint32>(kOpponent));
    EXPECT_EQ(draft.remainingCost.value(QChar('G')), 2);
}

TEST_F(RuledClientTest, UnknownCounterCapabilitiesRemainFailClosedAcrossPendingOperations)
{
    const QVector<AbilityCommandProbe> probes = {AbilityCommandProbe::Activate,
                                                  AbilityCommandProbe::Begin,
                                                  AbilityCommandProbe::Preview,
                                                  AbilityCommandProbe::Commit};
    apply(unsupportedCostBatch(static_cast<int>(ruled::v1::COST_CHOICE_KIND_TAP), true));

    for (const auto probe : probes) {
        RuledPendingCast transaction;
        auto &draft = prepareUnsupportedCostDraft(transaction, *state);
        int cancellations = 0;
        int operations = 0;
        if (probe == AbilityCommandProbe::Commit) {
            auto &accepted = state->pendingAbilityActivation.emplace();
            accepted.set_transaction_id(91);
            accepted.set_source_object_id(kUnsupportedCostSourceOid);
            accepted.set_source_zone_change_generation(kUnsupportedCostSourceGeneration);
            accepted.set_ability_index(kUnsupportedCostAbilityIndex);
            accepted.set_stage(ruled::v1::ABILITY_ACTIVATION_STAGE_PAYMENT);
            draft.applyEngineSnapshot(accepted);
        }
        host.sentCommands.clear();
        ruledDispatchPendingAbilityCostOperation(
            *state, transaction.ability, [&]() { ++cancellations; transaction.clearAbility(); }, [&]() {
                ++operations;
                transaction.ability.nextCostChoice = 6;
                sendAbilityCommandProbe(host, transaction.ability, probe);
            });
        ASSERT_EQ(cancellations, 0);
        ASSERT_EQ(operations, 1);
        EXPECT_EQ(transaction.ability.nextCostChoice, 6);
        ASSERT_EQ(host.sentCommands.size(), 1);
        ASSERT_TRUE(isAbilityCommandProbe(host.sentCommands.front(), probe));
        const auto &command = host.sentCommands.front();
        switch (probe) {
            case AbilityCommandProbe::Activate:
                EXPECT_EQ(command.activate_ability().source_object_id(), kUnsupportedCostSourceOid);
                EXPECT_EQ(command.activate_ability().expected_zone_change_generation(),
                          kUnsupportedCostSourceGeneration);
                break;
            case AbilityCommandProbe::Begin:
                EXPECT_EQ(command.begin_ability_activation().source_object_id(), kUnsupportedCostSourceOid);
                EXPECT_EQ(command.begin_ability_activation().expected_zone_change_generation(),
                          kUnsupportedCostSourceGeneration);
                host.answerPendingAck(true);
                break;
            case AbilityCommandProbe::Preview:
                ASSERT_TRUE(command.preview_payment().has_activate_ability());
                EXPECT_EQ(command.preview_payment().activate_ability().source_object_id(), kUnsupportedCostSourceOid);
                EXPECT_EQ(command.preview_payment().activate_ability().expected_zone_change_generation(),
                          kUnsupportedCostSourceGeneration);
                break;
            case AbilityCommandProbe::Commit:
                EXPECT_EQ(command.commit_ability_activation().transaction_id(), 91u);
                EXPECT_EQ(command.commit_ability_activation().expected_revision(), 3u);
                EXPECT_TRUE(command.commit_ability_activation().has_payment());
                break;
        }
        transaction.clearAbility();
        state->pendingAbilityActivation.reset();
        host.sentCommands.clear();
    }

    for (const int wireKind : {static_cast<int>(ruled::v1::COST_CHOICE_KIND_REMOVE_COUNTERS_FROM_PERMANENTS), 777}) {
        apply(unsupportedCostBatch(static_cast<int>(ruled::v1::COST_CHOICE_KIND_TAP), true));
        const auto cachedSupported = state->abilityCostChoices(kUnsupportedCostSourceOid, kUnsupportedCostAbilityIndex);
        ASSERT_EQ(cachedSupported.size(), 2);
        ASSERT_EQ(cachedSupported.at(0).candidateIds, QSet<quint32>{902});
        ASSERT_EQ(cachedSupported.at(1).candidateIds, QSet<quint32>{901});
        apply(unsupportedCostBatch(wireKind, true));
        const auto current = state->abilityCostChoices(kUnsupportedCostSourceOid, kUnsupportedCostAbilityIndex);
        ASSERT_EQ(current.size(), 2);
        EXPECT_EQ(current.at(0).kind, RuledCostChoiceKind::Tap);
        if (wireKind == static_cast<int>(ruled::v1::COST_CHOICE_KIND_REMOVE_COUNTERS_FROM_PERMANENTS)) {
            EXPECT_EQ(current.at(1).kind, RuledCostChoiceKind::RemoveCountersFromPermanents);
            EXPECT_TRUE(current.at(1).counterRemovalGroup.has_value());
        } else {
            EXPECT_EQ(current.at(1).kind, RuledCostChoiceKind::Unspecified);
            EXPECT_FALSE(current.at(1).counterRemovalGroup.has_value());
        }
        const auto costDataKey = RuledClientState::abilityTargetKey(kUnsupportedCostSourceOid,
                                                                     kUnsupportedCostAbilityIndex);
        ASSERT_TRUE(state->abilityCostData.contains(costDataKey));
        EXPECT_FALSE(state->abilityCostData.value(costDataKey).nonManaCostsPayable);
        EXPECT_FALSE(state->abilityActivatable(kUnsupportedCostSourceOid, kUnsupportedCostAbilityIndex));

        for (const auto probe : probes) {
            RuledPendingCast transaction;
            auto &draft = transaction.beginAbility();
            draft.permanentOid = kUnsupportedCostSourceOid;
            draft.sourceZone = state->abilitySourceZone(kUnsupportedCostSourceOid);
            draft.expectedZoneChangeGeneration = kUnsupportedCostSourceGeneration;
            draft.abilityIndex = kUnsupportedCostAbilityIndex;
            draft.costChoices = cachedSupported;
            draft.nextCostChoice = 1;
            draft.waitingForCost = true;
            draft.costSelections.append({0, RuledCostChoiceZone::Battlefield, {902}, {14}});
            ASSERT_TRUE(draft.valid);
            ASSERT_EQ(draft.costSelections.size(), 1);
            ASSERT_EQ(draft.costChoices.at(1).kind, RuledCostChoiceKind::Tap);
            host.sentCommands.clear();
            int cancellations = 0;
            int operations = 0;
            const auto dispatch = [&]() {
                ruledDispatchPendingAbilityCostOperation(
                    *state, transaction.ability, [&]() { ++cancellations; transaction.clearAbility(); }, [&]() {
                        ++operations;
                        if (probe == AbilityCommandProbe::Begin) {
                            transaction.ability.costSelections.append(
                                {1, RuledCostChoiceZone::Battlefield, {901}, {13}});
                            ++transaction.ability.nextCostChoice;
                            transaction.ability.waitingForCost = false;
                        } else {
                            transaction.ability.xValue = 77;
                        }
                        sendAbilityCommandProbe(host, transaction.ability, probe);
                    });
            };
            dispatch();
            if (probe == AbilityCommandProbe::Begin && !host.sentCommands.isEmpty()) {
                ASSERT_TRUE(host.sentCommands.back().has_begin_ability_activation());
                host.answerPendingAck(true);
            }
            EXPECT_EQ(cancellations, 1) << "wire kind " << wireKind;
            EXPECT_EQ(operations, 0) << "wire kind " << wireKind;
            EXPECT_FALSE(transaction.ability.valid);
            EXPECT_TRUE(transaction.ability.costSelections.isEmpty());
            EXPECT_EQ(transaction.ability.nextCostChoice, 0);
            EXPECT_EQ(transaction.ability.xValue, 0u);
            EXPECT_TRUE(host.sentCommands.isEmpty()) << "wire kind " << wireKind;

            // The same production dispatcher is the continuation prefix; a canceled draft is a no-op.
            dispatch();
            EXPECT_EQ(cancellations, 1);
            EXPECT_EQ(operations, 0);
            EXPECT_TRUE(host.sentCommands.isEmpty());
            host.sentCommands.clear();
        }
    }

    apply(unsupportedCostBatch(777, true));
    const auto cachedUnsupported = state->abilityCostChoices(kUnsupportedCostSourceOid, kUnsupportedCostAbilityIndex);
    ASSERT_EQ(cachedUnsupported.size(), 2);
    ASSERT_EQ(cachedUnsupported.at(1).kind, RuledCostChoiceKind::Unspecified);
    EXPECT_FALSE(cachedUnsupported.at(1).counterRemovalGroup.has_value());
    apply(unsupportedCostBatch(static_cast<int>(ruled::v1::COST_CHOICE_KIND_TAP), true));
    const auto refreshedSupported = state->abilityCostChoices(kUnsupportedCostSourceOid, kUnsupportedCostAbilityIndex);
    ASSERT_EQ(refreshedSupported.size(), 2);
    EXPECT_TRUE(state->abilityActivatable(kUnsupportedCostSourceOid, kUnsupportedCostAbilityIndex));
    RuledPendingCast cachedTransaction;
    auto &cachedDraft = cachedTransaction.beginAbility();
    cachedDraft.permanentOid = kUnsupportedCostSourceOid;
    cachedDraft.sourceZone = state->abilitySourceZone(kUnsupportedCostSourceOid);
    cachedDraft.expectedZoneChangeGeneration = kUnsupportedCostSourceGeneration;
    cachedDraft.abilityIndex = kUnsupportedCostAbilityIndex;
    cachedDraft.costChoices = cachedUnsupported;
    cachedDraft.nextCostChoice = 1;
    cachedDraft.waitingForCost = true;
    cachedDraft.costSelections.append({0, RuledCostChoiceZone::Battlefield, {902}, {14}});
    ASSERT_TRUE(cachedDraft.valid);
    ASSERT_EQ(cachedDraft.costSelections.size(), 1);
    EXPECT_EQ(cachedDraft.costChoices.at(1).kind, RuledCostChoiceKind::Unspecified);
    const auto cachedReceipts = cachedDraft.costSelections;
    EXPECT_FALSE(cachedTransaction.reconcileAbilityCosts(*state, kLocalPlayer));
    ASSERT_EQ(cachedDraft.costChoices.size(), 2);
    EXPECT_EQ(cachedDraft.costChoices.at(1).kind, RuledCostChoiceKind::Unspecified);
    EXPECT_EQ(cachedDraft.costSelections.size(), cachedReceipts.size());
    EXPECT_EQ(cachedDraft.costSelections.front().selectedIds, cachedReceipts.front().selectedIds);
    int cachedCancellations = 0;
    int cachedOperations = 0;
    host.sentCommands.clear();
    ruledDispatchPendingAbilityCostOperation(
        *state, cachedTransaction.ability, [&]() { ++cachedCancellations; cachedTransaction.clearAbility(); }, [&]() {
            ++cachedOperations;
            cachedTransaction.ability.nextCostChoice = 6;
            sendAbilityCommandProbe(host, cachedTransaction.ability, AbilityCommandProbe::Activate);
        });
    EXPECT_EQ(cachedCancellations, 1);
    EXPECT_EQ(cachedOperations, 0);
    EXPECT_FALSE(cachedTransaction.ability.valid);
    EXPECT_TRUE(cachedTransaction.ability.costSelections.isEmpty());
    EXPECT_TRUE(host.sentCommands.isEmpty());
    ruledDispatchPendingAbilityCostOperation(
        *state, cachedTransaction.ability, [&]() { ++cachedCancellations; cachedTransaction.clearAbility(); }, [&]() {
            ++cachedOperations;
            sendAbilityCommandProbe(host, cachedTransaction.ability, AbilityCommandProbe::Activate);
        });
    EXPECT_EQ(cachedCancellations, 1);
    EXPECT_EQ(cachedOperations, 0);
    EXPECT_TRUE(host.sentCommands.isEmpty());
}

TEST(RuledPendingCostSelectionTest, GraveyardSelectionMembershipTracksPhysicalObjectIds)
{
    PendingActivatedAbility pending;
    pending.valid = true;
    pending.waitingForCost = true;
    RuledCostChoice choice;
    choice.costIndex = 3;
    choice.zone = RuledCostChoiceZone::Graveyard;
    choice.candidateIds = {501u, 502u};
    choice.candidateGenerations = {{501u, 7u}, {502u, 8u}};
    choice.min = choice.max = 2;
    pending.costChoices.append(choice);
    pending.costSelections.append(
        {3, RuledCostChoiceZone::Graveyard, QVector<quint32>({501u, 502u}), QVector<quint64>({7u, 8u})});

    EXPECT_TRUE(ruledPendingGraveyardCostSelectionContains(pending, 501u));
    EXPECT_TRUE(ruledPendingGraveyardCostSelectionContains(pending, 502u));
    EXPECT_FALSE(ruledPendingGraveyardCostSelectionContains(pending, 503u));
    pending.waitingForCost = false;
    EXPECT_FALSE(ruledPendingGraveyardCostSelectionContains(pending, 501u));
}

TEST(RuledPendingCostSelectionTest, GraveyardProgressCountsOnlyCurrentEngineCandidates)
{
    PendingActivatedAbility pending;
    pending.valid = true;
    pending.waitingForCost = true;
    pending.nextCostChoice = 0;

    RuledCostChoice choice;
    choice.costIndex = 4;
    choice.zone = RuledCostChoiceZone::Graveyard;
    choice.candidateIds = {501u, 502u};
    choice.candidateGenerations = {{501u, 7u}, {502u, 8u}};
    choice.min = 2;
    choice.max = 2;
    pending.costChoices = {choice};

    // 500 is the activated source and is intentionally absent from the engine-authored cohort.
    // It must not make Confirm legal even if a stale local transaction contains it.
    pending.costSelections = {{4, RuledCostChoiceZone::Graveyard, {500u, 501u, 501u}, {0u, 7u, 7u}}};

    const auto progress = ruledPendingGraveyardCostSelectionProgress(pending);
    ASSERT_TRUE(progress.has_value());
    EXPECT_EQ(progress->required, 2);
    EXPECT_EQ(progress->selected, 1);
    EXPECT_FALSE(progress->confirmable);
    EXPECT_FALSE(ruledPendingGraveyardCostSelectionContains(pending, 500u));
    EXPECT_TRUE(ruledPendingGraveyardCostSelectionContains(pending, 501u));
}

TEST(RuledPendingCostSelectionTest, TapPaymentProgressDoesNotRequestAGraveyardView)
{
    PendingActivatedAbility pending;
    pending.valid = true;
    pending.waitingForCost = true;
    RuledCostChoice choice;
    choice.costIndex = 1;
    choice.zone = RuledCostChoiceZone::Battlefield;
    choice.kind = RuledCostChoiceKind::Tap;
    choice.min = choice.max = 1;
    choice.candidateIds = {501};
    pending.costChoices = {choice};
    const auto progress = ruledPendingGraveyardCostSelectionProgress(pending);
    ASSERT_TRUE(progress.has_value());
    EXPECT_EQ(progress->zone, RuledCostChoiceZone::Battlefield);
    EXPECT_EQ(progress->required, 1);
    EXPECT_FALSE(progress->confirmable);
}

TEST_F(RuledClientTest, AggregateObjectPaymentUsesAuthoritativeContributionsAndGenerations)
{
    ruled::v1::RuledEventBatch batch;
    auto &costs = (*(*batch.mutable_legal_by_player())[kLocalPlayer].mutable_cost_choices_by_ability())[178];
    costs.set_non_mana_costs_payable(true);
    auto *choice = costs.add_choices();
    choice->set_cost_index(0);
    choice->set_zone(ruled::v1::COST_CHOICE_ZONE_GRAVEYARD);
    choice->set_kind(ruled::v1::COST_CHOICE_KIND_EXILE);
    choice->set_max(2);
    choice->mutable_aggregate_minimum()->set_minimum(3);
    choice->mutable_aggregate_minimum()->set_contribution_kind(ruled::v1::OBJECT_CONTRIBUTION_KIND_MANA_VALUE);
    for (const auto &[objectId, generation, contribution] :
         std::array<std::tuple<quint32, quint64, qint64>, 2>{{{501, 7, 1}, {502, 9, 2}}}) {
        choice->add_candidate_ids(objectId);
        auto *candidate = choice->add_candidate_objects();
        candidate->mutable_object()->set_object_id(objectId);
        candidate->mutable_object()->set_zone_change_generation(generation);
        candidate->set_contribution(contribution);
    }
    apply(batch);

    const auto parsed = state->abilityCostChoices(0, 178);
    ASSERT_EQ(parsed.size(), 1);
    EXPECT_EQ(parsed[0].aggregateMinimum, 3);
    EXPECT_EQ(parsed[0].contributionKind, RuledObjectContributionKind::ManaValue);
    EXPECT_EQ(parsed[0].candidateContributions.value(502), 2);
    EXPECT_EQ(parsed[0].candidateGenerations.value(502), 9u);

    PendingActivatedAbility pending;
    pending.valid = true;
    pending.waitingForCost = true;
    pending.costChoices = parsed;
    pending.costSelections = {{0, RuledCostChoiceZone::Graveyard, {501}, {7}}};
    auto progress = ruledPendingGraveyardCostSelectionProgress(pending);
    ASSERT_TRUE(progress);
    EXPECT_EQ(progress->selected, 1);
    EXPECT_FALSE(progress->confirmable);
    pending.costSelections[0].selectedIds.append(502);
    pending.costSelections[0].selectedGenerations.append(9);
    progress = ruledPendingGraveyardCostSelectionProgress(pending);
    ASSERT_TRUE(progress);
    EXPECT_EQ(progress->selected, 3);
    EXPECT_TRUE(progress->confirmable);

    ruled::v1::CostSelection command;
    ruledWriteCostObjectRefs(pending.costSelections[0], command);
    ASSERT_EQ(command.graveyard_objects().objects_size(), 2);
    EXPECT_EQ(command.graveyard_objects().objects(1).object_id(), 502u);
    EXPECT_EQ(command.graveyard_objects().objects(1).zone_change_generation(), 9u);
}

TEST_F(RuledClientTest, RequirementSetsSurviveABatchWithoutLegalActions)
{
    ruled::v1::RuledEventBatch withActions;
    auto &actions = (*withActions.mutable_legal_by_player())[kLocalPlayer];
    addHandAction(actions, ruled::v1::HAND_ACTION_CAST_SPELL, 0, "Grizzly Bears");
    actions.add_attack_requirement_ids(100); // CR 508.1d
    actions.set_minimum_attack_requirement_count(1);
    actions.add_required_blocker_ids(200); // CR 509.1c
    actions.add_selectable_attacker_ids(100);
    addLegalPlayerAttack(actions, 100);
    addLegalBlockPair(actions, 200, 300);
    apply(withActions);
    ASSERT_EQ(state->attackRequirementOids.size(), 1);

    // A Servatrice-synthesized preview echo has no legal_by_player entry: legal actions clear,
    // but the engine-authoritative must-attack / must-block sets must survive.
    ruled::v1::RuledEventBatch preview;
    auto *ap = preview.add_events()->mutable_attackers_preview();
    ap->set_declaring_player_id(kOpponent);
    *ap->add_assignments() = playerAttackAssignment(100);
    apply(preview);

    EXPECT_FALSE(state->isHandActionLegal(ruled::v1::HAND_ACTION_CAST_SPELL, 0));
    EXPECT_TRUE(state->attackRequirementOids.contains(100));
    EXPECT_TRUE(state->requiredBlockerOids.contains(200));
    EXPECT_TRUE(state->isSelectableAttacker(100));
    EXPECT_TRUE(state->isSelectableBlocker(200));
    EXPECT_TRUE(state->remoteAttackerPreviewOids.contains(100));
}

TEST_F(RuledClientTest, AuthoritativeOmittedCombatCapabilitiesClear)
{
    auto batch = phaseBatch(ruled::v1::PHASE_ID_DECLARE_ATTACKERS, kLocalPlayer);
    auto &actions = (*batch.mutable_legal_by_player())[kLocalPlayer];
    actions.add_attack_requirement_ids(100);
    actions.set_minimum_attack_requirement_count(1);
    auto *limit = actions.add_attack_declaration_limits();
    limit->set_attacked_player_id(kOpponent);
    limit->set_maximum_attackers(2);
    actions.add_selectable_attacker_ids(100);
    addLegalPlayerAttack(actions, 100);
    apply(batch);
    ASSERT_TRUE(state->attackRequirementOids.contains(100));
    // An engine phase snapshot with no recipient LegalActions retracts combat authority.
    apply(phaseBatch(ruled::v1::PHASE_ID_DECLARE_ATTACKERS, kLocalPlayer));
    EXPECT_TRUE(state->attackRequirementOids.isEmpty());
    EXPECT_EQ(state->minimumAttackRequirementCount, 0u);
    EXPECT_TRUE(state->attackDeclarationLimits.isEmpty());
    EXPECT_TRUE(state->selectableAttackerOids.isEmpty());
    EXPECT_TRUE(state->legalAttackAssignmentsByAttacker.isEmpty());
}

TEST_F(RuledClientTest, AttackCapsAndAchievableRequirementsSurviveRelayCourtesyShapes)
{
    for (const bool blockersPreview : {false, true}) {
        for (const int staged : {1, 2, 3}) {
            state->clearSessionState();
            auto batch = phaseBatch(ruled::v1::PHASE_ID_DECLARE_ATTACKERS, kLocalPlayer);
            auto &actions = (*batch.mutable_legal_by_player())[kLocalPlayer];
            for (const quint32 oid : {100u, 101u, 102u}) {
                actions.add_attack_requirement_ids(oid);
                actions.add_selectable_attacker_ids(oid);
                addLegalPlayerAttack(actions, oid);
            }
            actions.set_minimum_attack_requirement_count(2);
            auto *limit = actions.add_attack_declaration_limits();
            limit->set_attacked_player_id(kOpponent);
            limit->set_maximum_attackers(2);
            apply(batch);
            for (int index = 0; index < staged; ++index)
                state->togglePendingAttacker(100 + index);
            EXPECT_EQ(state->combatDeclarationSatisfied(), staged == 2);
            ruled::v1::RuledEventBatch courtesy;
            if (blockersPreview)
                courtesy.add_events()->mutable_blockers_preview()->set_declaring_player_id(kOpponent);
            else
                courtesy.add_events()->mutable_attackers_preview()->set_declaring_player_id(kLocalPlayer);
            courtesy.add_events()->mutable_log()->set_text("courtesy");
            courtesy.add_events()->mutable_battlefield_object_map();
            courtesy.add_events()->mutable_face_down_object_map();
            courtesy.add_events()->mutable_hand_slot_map();
            courtesy.add_events()->mutable_graveyard_object_map();
            courtesy.add_events()->mutable_exile_object_map();
            apply(courtesy);
            EXPECT_EQ(state->attackRequirementOids.size(), 3);
            EXPECT_EQ(state->minimumAttackRequirementCount, 2u);
            ASSERT_EQ(state->attackDeclarationLimits.size(), 1);
            EXPECT_EQ(state->combatDeclarationSatisfied(), staged == 2);
            EXPECT_TRUE(state->attackDeclarationExplanation().contains("At most 2"));
            EXPECT_TRUE(state->attackDeclarationExplanation([](int) { return QStringLiteral("Opponent"); })
                            .contains("At most 2 creatures may attack Opponent."));
        }
    }
}

TEST_F(RuledClientTest, AttackCapsEmptyMixedAuthoritativeAndResetRetractCapabilities)
{
    for (int mode = 0; mode < 4; ++mode) {
        auto batch = phaseBatch(ruled::v1::PHASE_ID_DECLARE_ATTACKERS, kLocalPlayer);
        auto &actions = (*batch.mutable_legal_by_player())[kLocalPlayer];
        actions.add_attack_requirement_ids(100);
        actions.set_minimum_attack_requirement_count(1);
        actions.add_attack_declaration_limits()->set_maximum_attackers(2);
        actions.add_selectable_attacker_ids(100);
        addLegalPlayerAttack(actions, 100);
        apply(batch);
        if (mode == 3) {
            state->clearSessionState();
        } else {
            ruled::v1::RuledEventBatch replacement;
            replacement.add_events()->mutable_attackers_preview()->set_declaring_player_id(kOpponent);
            if (mode == 0)
                (*replacement.mutable_legal_by_player())[kLocalPlayer];
            else if (mode == 1)
                (*replacement.mutable_legal_by_player())[kOpponent];
            else
                replacement.add_events()->mutable_log(); // followed by an authoritative phase
            if (mode == 2)
                *replacement.add_events() = phaseBatch(ruled::v1::PHASE_ID_DECLARE_ATTACKERS, kLocalPlayer).events(0);
            apply(replacement);
        }
        EXPECT_TRUE(state->attackRequirementOids.isEmpty());
        EXPECT_EQ(state->minimumAttackRequirementCount, 0u);
        EXPECT_TRUE(state->attackDeclarationLimits.isEmpty());
        EXPECT_TRUE(state->selectableAttackerOids.isEmpty());
        EXPECT_TRUE(state->legalAttackAssignmentsByAttacker.isEmpty());
    }
}

TEST_F(RuledClientTest, PlayerAttackCapsExcludePermanentDefendersAndGlobalCapsCountThem)
{
    for (const bool global : {false, true}) {
        state->clearSessionState();
        auto batch = phaseBatch(ruled::v1::PHASE_ID_DECLARE_ATTACKERS, kLocalPlayer);
        auto &actions = (*batch.mutable_legal_by_player())[kLocalPlayer];
        for (const quint32 oid : {100u, 101u, 102u}) {
            actions.add_selectable_attacker_ids(oid);
            addLegalPlayerAttack(actions, oid);
            *actions.add_legal_attack_assignments() = permanentAttackAssignment(oid, 500, 9);
        }
        auto *limit = actions.add_attack_declaration_limits();
        if (!global)
            limit->set_attacked_player_id(kOpponent);
        limit->set_maximum_attackers(2);
        apply(batch);
        for (const quint32 oid : {100u, 101u, 102u}) {
            state->togglePendingAttacker(oid);
            ASSERT_TRUE(state->chooseAttackPermanentDefender(500));
        }
        EXPECT_EQ(state->combatDeclarationSatisfied(), !global);
        state->togglePendingAttacker(102);
        EXPECT_TRUE(state->combatDeclarationSatisfied());
    }
}

TEST_F(RuledClientTest, LegalActionsBatchEmitsUndoableManaCount)
{
    QSignalSpy spy(state, &RuledClientState::undoableManaAbilitiesChanged);
    ruled::v1::RuledEventBatch batch;
    (*batch.mutable_legal_by_player())[kLocalPlayer].set_undoable_mana_abilities(2);
    apply(batch);
    ASSERT_EQ(spy.count(), 1);
    EXPECT_EQ(spy.at(0).at(0).toInt(), 2);

    // No entry for us this batch → the affordance is retracted.
    apply(phaseBatch(ruled::v1::PHASE_ID_MAIN1, kLocalPlayer));
    ASSERT_EQ(spy.count(), 2);
    EXPECT_EQ(spy.at(1).at(0).toInt(), 0);
}

TEST_F(RuledClientTest, PendingAttackReceiptUndoabilityDrivesTheUndoAffordance)
{
    QSignalSpy spy(state, &RuledClientState::undoableManaAbilitiesChanged);
    ruled::v1::RuledEventBatch batch;
    auto &actions = (*batch.mutable_legal_by_player())[kLocalPlayer];
    auto *pending = actions.mutable_pending_attack_declaration();
    pending->set_transaction_id(91);
    auto *undoable = pending->add_mana_ability_undo_options();
    undoable->set_activation_command_index(17);
    undoable->set_reversible(true);
    auto *unavailable = pending->add_mana_ability_undo_options();
    unavailable->set_activation_command_index(18);
    unavailable->set_reversible(false);
    apply(batch);
    ASSERT_EQ(spy.count(), 1);
    EXPECT_EQ(spy.at(0).at(0).toInt(), 1);

    ruled::v1::RuledEventBatch unavailableBatch;
    auto &unavailableActions = (*unavailableBatch.mutable_legal_by_player())[kLocalPlayer];
    auto *unavailablePending = unavailableActions.mutable_pending_attack_declaration();
    unavailablePending->set_transaction_id(91);
    unavailablePending->add_mana_ability_undo_options()->set_reversible(false);
    apply(unavailableBatch);
    ASSERT_EQ(spy.count(), 2);
    EXPECT_EQ(spy.at(1).at(0).toInt(), 0);
}

TEST_F(RuledClientTest, LegalActionsForAnotherPlayerAreIgnored)
{
    ruled::v1::RuledEventBatch batch;
    auto &actions = (*batch.mutable_legal_by_player())[kOpponent];
    addHandAction(actions, ruled::v1::HAND_ACTION_CAST_SPELL, 1, "Lightning Bolt", 0, true);
    apply(batch);
    EXPECT_FALSE(state->isHandActionLegal(ruled::v1::HAND_ACTION_CAST_SPELL, 1));
}

TEST_F(RuledClientTest, RestrictedManaSnapshotsAndPaymentEligibilityStaySeparate)
{
    QSignalSpy manaSpy(state, &RuledClientState::restrictedManaChanged);
    ruled::v1::RuledEventBatch batch;
    auto *pool = batch.add_events()->mutable_mana_pool_updated();
    pool->set_player_id(kLocalPlayer);
    pool->set_r(2); // unrestricted mana remains in the legacy counter snapshot
    auto *restricted = pool->add_restricted_groups();
    restricted->set_restriction_group_id(7);
    restricted->set_r(1);
    restricted->set_display_label("Spend this mana only to cast a creature spell.");

    auto &actions = (*batch.mutable_legal_by_player())[kLocalPlayer];
    auto *cast = addHandAction(actions, ruled::v1::HAND_ACTION_CAST_SPELL, 3, "Fire Elemental", 0, false, "3RR");
    cast->add_eligible_restricted_mana_group_ids(7);
    auto &ability = (*actions.mutable_mana_payment_by_ability())[(quint64(100) << 32) | 2u];
    ability.add_eligible_restricted_mana_group_ids(7);
    apply(batch);

    ASSERT_EQ(manaSpy.count(), 1);
    EXPECT_EQ(manaSpy.at(0).at(0).toInt(), kLocalPlayer);
    const auto groups = state->restrictedManaForPlayer(kLocalPlayer);
    ASSERT_EQ(groups.size(), 1);
    EXPECT_EQ(groups.at(0).groupId, 7u);
    EXPECT_EQ(groups.at(0).r, 1);
    EXPECT_EQ(groups.at(0).displayLabel, QStringLiteral("Spend this mana only to cast a creature spell."));
    EXPECT_EQ(state->eligibleRestrictedManaForCast(3, 0, RuledCastSource::Hand), QSet<quint32>({7}));
    EXPECT_EQ(state->eligibleRestrictedManaForAbility(100, 2), QSet<quint32>({7}));

    ruled::v1::RuledEventBatch cleared;
    cleared.add_events()->mutable_mana_pool_updated()->set_player_id(kLocalPlayer);
    apply(cleared);
    EXPECT_TRUE(state->restrictedManaForPlayer(kLocalPlayer).isEmpty());
    EXPECT_TRUE(state->eligibleRestrictedManaForCast(3, 0, RuledCastSource::Hand).isEmpty());
    EXPECT_TRUE(state->eligibleRestrictedManaForAbility(100, 2).isEmpty());
}

TEST_F(RuledClientTest, RestrictedManaGroupsKeepDistinctEngineSourcePresentations)
{
    ruled::v1::RuledEventBatch batch;
    auto *pool = batch.add_events()->mutable_mana_pool_updated();
    pool->set_player_id(kLocalPlayer);
    for (quint32 groupId = 7; groupId <= 8; ++groupId) {
        const quint32 sourceObject = groupId + 34;
        auto *group = pool->add_restricted_groups();
        group->set_restriction_group_id(groupId);
        group->set_g(1);
        group->set_display_label(
            "Scry 1 when you cast a creature spell that shares a creature type with your commander.");
        auto *presentation = group->mutable_presentation();
        presentation->set_fallback_text(
            QStringLiteral("Path of Ancestry (player %1, source object %2)")
                .arg(kLocalPlayer)
                .arg(sourceObject)
                .toStdString());
    }
    apply(batch);

    const auto groups = state->restrictedManaForPlayer(kLocalPlayer);
    ASSERT_EQ(groups.size(), 2);
    EXPECT_EQ(groups.at(0).groupId, 7u);
    EXPECT_EQ(groups.at(1).groupId, 8u);
    EXPECT_NE(groups.at(0).displayLabel, groups.at(1).displayLabel);
    EXPECT_TRUE(groups.at(0).displayLabel.contains(QStringLiteral("source object 41")));
    EXPECT_TRUE(groups.at(1).displayLabel.contains(QStringLiteral("source object 42")));
}

// ---------------------------------------------------------------------------------------
// Stack tracking
// ---------------------------------------------------------------------------------------

TEST_F(RuledClientTest, StackPushAndResolveKeepLifoOrder)
{
    QSignalSpy orderSpy(state, &RuledClientState::stackOrderChanged);
    ruled::v1::RuledEventBatch batch;
    batch.add_events()->mutable_stack_pushed()->set_object_id(10);
    auto *second = batch.add_events()->mutable_stack_pushed();
    second->set_object_id(11);
    second->mutable_targets()->Add()->set_object_id(100);
    apply(batch);

    // Front of the list resolves first.
    EXPECT_EQ(state->getStackOidOrder(), QList<quint32>({11, 10}));
    EXPECT_TRUE(state->hasStackItems());
    EXPECT_EQ(state->stackTargetsByStackOid.value(11), QVector<quint32>({100}));
    EXPECT_EQ(orderSpy.count(), 1);

    ruled::v1::RuledEventBatch resolve;
    resolve.add_events()->mutable_stack_resolved()->set_object_id(11);
    apply(resolve);
    EXPECT_EQ(state->getStackOidOrder(), QList<quint32>({10}));
    EXPECT_FALSE(state->stackTargetsByStackOid.contains(11));
    EXPECT_EQ(host.removedSyntheticCards, QVector<quint32>({11}));
}

TEST_F(RuledClientTest, PermanentMovedRetiresOnlyTheExactStackObjectAndTargets)
{
    ruled::v1::RuledEventBatch pushed;
    auto *retainedStackObject = pushed.add_events()->mutable_stack_pushed();
    retainedStackObject->set_object_id(10);
    retainedStackObject->mutable_targets()->Add()->set_object_id(101);
    auto *selectedStackObject = pushed.add_events()->mutable_stack_pushed();
    selectedStackObject->set_object_id(11);
    selectedStackObject->mutable_targets()->Add()->set_object_id(100);
    apply(pushed);

    const auto selectedTargetKey = RuledClientState::stackTargetKey(11, 100);
    const auto retainedTargetKey = RuledClientState::stackTargetKey(10, 101);
    state->stackTargetKindByStackAndTargetOid.insert(
        selectedTargetKey, RuledTargetItemKind::Battlefield);
    state->stackTargetKindByStackAndTargetOid.insert(
        retainedTargetKey, RuledTargetItemKind::Battlefield);

    ruled::v1::RuledEventBatch moved;
    auto *untrackedPermanent = moved.add_events()->mutable_permanent_moved();
    untrackedPermanent->set_object_id(100);
    untrackedPermanent->set_destination(ruled::v1::PermanentMoved::DESTINATION_GRAVEYARD);
    auto *selectedCard = moved.add_events()->mutable_permanent_moved();
    selectedCard->set_object_id(11);
    selectedCard->set_destination(ruled::v1::PermanentMoved::DESTINATION_HAND);
    apply(moved);

    EXPECT_EQ(state->getStackOidOrder(), QList<quint32>({10}));
    EXPECT_FALSE(state->stackTargetsByStackOid.contains(11));
    EXPECT_EQ(state->stackTargetsByStackOid.value(10), QVector<quint32>({101}));
    EXPECT_FALSE(state->stackTargetKindByStackAndTargetOid.contains(selectedTargetKey));
    EXPECT_TRUE(state->stackTargetKindByStackAndTargetOid.contains(retainedTargetKey));
    EXPECT_EQ(host.removedSyntheticCards, QVector<quint32>({11}));
}

TEST_F(RuledClientTest, StackObjectRemovedRetiresOnlyExactSyntheticPresentation)
{
    ruled::v1::RuledEventBatch source;
    addPermanent(source.add_events(), kLocalPlayer, 100, 7);
    auto *trigger = source.add_events()->mutable_trigger_needs_target();
    trigger->set_source_permanent_id(100);
    trigger->set_ability_index(0);
    trigger->set_ability_text("Return target creature card from your graveyard to your hand.");
    trigger->set_controller_player_id(kLocalPlayer);
    apply(source);

    ruled::v1::RuledEventBatch pushed;
    auto *departingAbility = pushed.add_events()->mutable_stack_pushed();
    departingAbility->set_object_id(900);
    departingAbility->set_description("Departing triggered ability");
    departingAbility->set_ability_annotation("Return target creature card...");
    departingAbility->set_is_triggered(true);
    departingAbility->set_ability_controller_player_id(kLocalPlayer);
    departingAbility->mutable_ability_source()->set_object_id(100u);
    departingAbility->mutable_ability_source()->set_zone_change_generation(0u);
    state->battlefieldGenerationByOid.insert(100u, 0u);
    departingAbility->mutable_targets()->Add()->set_object_id(200);

    auto *unrelatedAbility = pushed.add_events()->mutable_stack_pushed();
    unrelatedAbility->set_object_id(901);
    unrelatedAbility->set_description("Unrelated ability");
    unrelatedAbility->set_ability_annotation("Unrelated ability text");
    unrelatedAbility->set_is_triggered(true);

    auto *unrelatedPhysicalSpell = pushed.add_events()->mutable_stack_pushed();
    unrelatedPhysicalSpell->set_object_id(902);
    unrelatedPhysicalSpell->set_description("Lightning Bolt");
    unrelatedPhysicalSpell->set_card_id("lightning_bolt");
    apply(pushed);

    ASSERT_EQ(host.createdSyntheticCards.size(), 2);
    EXPECT_EQ(state->stackSourceOidByStackOid.value(900), 100u);
    EXPECT_EQ(state->cardIdForEngineOid(100), 7);
    EXPECT_EQ(state->getStackOidOrder(), QList<quint32>({902, 901, 900}));
    const auto departingTargetKey = RuledClientState::stackTargetKey(900, 200);
    const auto unrelatedTargetKey = RuledClientState::stackTargetKey(901, 201);
    state->stackTargetKindByStackAndTargetOid.insert(departingTargetKey,
                                                     RuledTargetItemKind::Battlefield);
    state->stackTargetsByStackOid[901] = {201};
    state->stackTargetKindByStackAndTargetOid.insert(unrelatedTargetKey,
                                                     RuledTargetItemKind::Battlefield);

    ruled::v1::RuledEventBatch removed;
    removed.add_events()->mutable_stack_object_removed()->set_object_id(900);
    apply(removed);

    EXPECT_EQ(state->getStackOidOrder(), QList<quint32>({902, 901}));
    EXPECT_FALSE(state->stackTargetsByStackOid.contains(900));
    EXPECT_FALSE(state->stackTargetKindByStackAndTargetOid.contains(departingTargetKey));
    EXPECT_EQ(state->stackTargetsByStackOid.value(901), QVector<quint32>({201}));
    EXPECT_TRUE(state->stackTargetKindByStackAndTargetOid.contains(unrelatedTargetKey));
    EXPECT_EQ(state->stackAnnotation(901), QStringLiteral("Unrelated ability text"));
    EXPECT_EQ(state->cardIdForEngineOid(100), 7)
        << "retiring the ability presentation leaves its source permanent mapped";
    EXPECT_EQ(host.removedSyntheticCards, QVector<quint32>({900}));
    EXPECT_EQ(host.createdSyntheticCards.size(), 2)
        << "the unrelated ability and physical spell presentation are not recreated or removed";
}

TEST_F(RuledClientTest, PhysicalStackCardIsPublishedOnlyAfterStackPushed)
{
    constexpr quint32 reservedOid = 20;
    constexpr int physicalCardId = 7;

    ruled::v1::RuledEventBatch begun;
    addPermanent(begun.add_events(), kLocalPlayer, reservedOid, physicalCardId);
    apply(begun);

    EXPECT_FALSE(state->isPublishedStackCard(kLocalPlayer, physicalCardId))
        << "a reserved cast source must stay hidden while its payment transaction is open";

    ruled::v1::RuledEventBatch committed;
    committed.add_events()->mutable_stack_pushed()->set_object_id(reservedOid);
    apply(committed);

    EXPECT_TRUE(state->isPublishedStackCard(kLocalPlayer, physicalCardId));

    ruled::v1::RuledEventBatch resolved;
    resolved.add_events()->mutable_stack_resolved()->set_object_id(reservedOid);
    apply(resolved);

    EXPECT_FALSE(state->isPublishedStackCard(kLocalPlayer, physicalCardId));
}

TEST_F(RuledClientTest, StackBindingRehomeRefreshesPresentationWithoutPublishingPendingCards)
{
    constexpr quint32 firstStackOid = 10;
    constexpr quint32 secondStackOid = 11;
    constexpr quint32 pendingStackOid = 12;
    QSignalSpy hasItemsSpy(state, &RuledClientState::stackHasItemsChanged);
    QSignalSpy orderSpy(state, &RuledClientState::stackOrderChanged);

    const auto addStackBinding = [](ruled::v1::RuledEvent *event, int playerId, quint32 oid, int serverCardId) {
        auto *entry = event->mutable_battlefield_object_map()->add_entries();
        entry->set_player_id(playerId);
        entry->set_engine_object_id(oid);
        entry->set_server_card_id(serverCardId);
    };

    ruled::v1::RuledEventBatch pendingOnly;
    addStackBinding(pendingOnly.add_events(), kLocalPlayer, pendingStackOid, 42);
    apply(pendingOnly);
    EXPECT_FALSE(state->hasStackItems());
    EXPECT_FALSE(state->isPublishedStackCard(kLocalPlayer, 42))
        << "a reserved physical card is not public until its oid is in StackPushed order";
    EXPECT_EQ(hasItemsSpy.count(), 0)
        << "a pending reservation alone must not open the ruled Stack window";
    EXPECT_EQ(orderSpy.count(), 0);

    ruled::v1::RuledEventBatch initialStack;
    auto *initialMap = initialStack.add_events();
    addStackBinding(initialMap, kLocalPlayer, firstStackOid, 40);
    addStackBinding(initialMap, kLocalPlayer, secondStackOid, 41);
    addStackBinding(initialMap, kLocalPlayer, pendingStackOid, 42);
    initialStack.add_events()->mutable_stack_pushed()->set_object_id(firstStackOid);
    initialStack.add_events()->mutable_stack_pushed()->set_object_id(secondStackOid);
    apply(initialStack);

    ASSERT_EQ(state->getStackOidOrder(), QList<quint32>({secondStackOid, firstStackOid}));
    ASSERT_TRUE(state->isPublishedStackCard(kLocalPlayer, 40));
    ASSERT_TRUE(state->isPublishedStackCard(kLocalPlayer, 41));
    EXPECT_FALSE(state->isPublishedStackCard(kLocalPlayer, 42));
    ASSERT_EQ(hasItemsSpy.count(), 1);
    ASSERT_EQ(orderSpy.count(), 1);

    ruled::v1::RuledEventBatch rehomedMap;
    auto *rehomed = rehomedMap.add_events();
    // Stack OIDs are unchanged; only their physical owner and player-scoped card ids moved.
    addStackBinding(rehomed, kOpponent, firstStackOid, 41);
    addStackBinding(rehomed, kOpponent, secondStackOid, 42);
    // The pending reservation also follows its exact physical card but remains unpublished.
    addStackBinding(rehomed, kOpponent, pendingStackOid, 43);

    const auto rehomeIsSettled = [this, firstStackOid, secondStackOid] {
        return state->engineOidOwner.value(firstStackOid, -1) == kOpponent &&
               state->engineOidToCardId.value(firstStackOid, -1) == 41 &&
               state->engineOidOwner.value(secondStackOid, -1) == kOpponent &&
               state->engineOidToCardId.value(secondStackOid, -1) == 42 &&
               state->engineOidOwner.value(pendingStackOid, -1) == kOpponent &&
               state->engineOidToCardId.value(pendingStackOid, -1) == 43 &&
               state->getStackOidOrder() == QList<quint32>({secondStackOid, firstStackOid}) &&
               state->isPublishedStackCard(kOpponent, 41) && state->isPublishedStackCard(kOpponent, 42) &&
               !state->isPublishedStackCard(kOpponent, 43);
    };
    QVector<bool> hasItemsSawSettledRehome;
    QVector<bool> orderSawSettledRehome;
    QObject::connect(state, &RuledClientState::stackHasItemsChanged, state, [&, rehomeIsSettled](bool hasItems) {
        hasItemsSawSettledRehome.append(hasItems && rehomeIsSettled());
    });
    QObject::connect(state, &RuledClientState::stackOrderChanged, state, [&, rehomeIsSettled](const QList<quint32> &) {
        orderSawSettledRehome.append(rehomeIsSettled());
    });
    apply(rehomedMap);

    EXPECT_EQ(state->getStackOidOrder(), QList<quint32>({secondStackOid, firstStackOid}));
    EXPECT_FALSE(state->isPublishedStackCard(kLocalPlayer, 40));
    EXPECT_FALSE(state->isPublishedStackCard(kLocalPlayer, 41));
    EXPECT_TRUE(state->isPublishedStackCard(kOpponent, 41));
    EXPECT_TRUE(state->isPublishedStackCard(kOpponent, 42));
    EXPECT_FALSE(state->isPublishedStackCard(kOpponent, 43))
        << "refreshing the physical binding must not publish an uncommitted cast reservation";
    EXPECT_EQ(hasItemsSpy.count(), 2)
        << "the open Stack window must reselect the survivor's zone after its map is current";
    EXPECT_EQ(orderSpy.count(), 2)
        << "StackZone and its view re-sort only after this signal, so a physical owner/id rebind must invalidate layout even when oid order is unchanged";
    EXPECT_EQ(hasItemsSawSettledRehome, QVector<bool>({true}))
        << "the open Stack window must receive its refresh only after survivor card bindings are current";
    EXPECT_EQ(orderSawSettledRehome, QVector<bool>({true}))
        << "StackZone layout must refresh only after survivor card bindings are current";

    ruled::v1::RuledEventBatch unchangedMap;
    auto *unchanged = unchangedMap.add_events();
    addStackBinding(unchanged, kOpponent, firstStackOid, 41);
    addStackBinding(unchanged, kOpponent, secondStackOid, 42);
    addStackBinding(unchanged, kOpponent, pendingStackOid, 43);
    apply(unchangedMap);
    EXPECT_EQ(hasItemsSpy.count(), 2) << "an unchanged full map must not reopen or rebind the Stack window";
    EXPECT_EQ(orderSpy.count(), 2) << "an unchanged full map must not force another Stack layout pass";
}

TEST_F(RuledClientTest, StackPushPreservesEveryTypedTargetWithoutPlayerIdCollisions)
{
    ruled::v1::RuledEventBatch batch;
    auto *push = batch.add_events()->mutable_stack_pushed();
    push->set_object_id(50);
    auto *first = push->add_targets();
    first->set_object_id(1); // Deliberately collides with kOpponent.
    first->set_group_index(0);
    first->set_kind(ruled::v1::TARGET_REF_KIND_PERMANENT);
    auto *second = push->add_targets();
    second->set_object_id(2);
    second->set_group_index(0);
    second->set_kind(ruled::v1::TARGET_REF_KIND_PERMANENT);

    apply(batch);

    EXPECT_EQ(state->stackTargetsByStackOid.value(50), QVector<quint32>({1, 2}));
    EXPECT_EQ(state->latchedTargetKind(50, 1), RuledTargetItemKind::Battlefield);
    EXPECT_EQ(state->latchedTargetKind(50, 2), RuledTargetItemKind::Battlefield);
}

TEST(RuledTargetRefKindTest, UsesGraveyardCandidateDomainForSelectionPresentation)
{
    RuledTargetGroupData group;
    group.validGraveyardIds.insert(7);

    EXPECT_EQ(ruledTargetRefKind(group, 7, kLocalPlayer), ruled::v1::TARGET_REF_KIND_GRAVEYARD);
}

TEST(RuledTargetGroupConfirmationTest, OptionalSingleTargetNeedsAnExplicitZeroTargetConfirmation)
{
    RuledTargetGroupData optionalCreature;
    optionalCreature.minTargets = 0;
    optionalCreature.maxTargets = 1;

    EXPECT_TRUE(ruledTargetGroupUsesExplicitConfirmation(optionalCreature));

    RuledTargetGroupData requiredSingle;
    requiredSingle.minTargets = 1;
    requiredSingle.maxTargets = 1;
    EXPECT_FALSE(ruledTargetGroupUsesExplicitConfirmation(requiredSingle));

    PendingRuledSpellCast pending;
    pending.valid = true;
    pending.waitingForTarget = true;
    pending.minTargets = 0;
    pending.maxTargets = 1;
    EXPECT_TRUE(ruledPendingTargetSelectionCanConfirm(pending));
}

TEST(RuledActivatedTargetSelectionTest, TeferiOptionalFourStagesZeroThroughFourAndConfirmsOnlyOnce)
{
    RuledTargetGroupData group;
    group.groupIndex = 7;
    group.minTargets = 0;
    group.maxTargets = 4;
    RuledSpellTargetData data;
    data.groups.append(group);

    for (int targetCount = 0; targetCount <= 4; ++targetCount) {
        PendingActivatedAbility ability;
        ability.valid = true;
        ability.waitingForTarget = true;
        for (int index = 0; index < targetCount; ++index) {
            PendingActivatedAbility::Target target;
            target.ref.set_group_index(7);
            target.ref.set_kind(ruled::v1::TARGET_REF_KIND_PERMANENT);
            target.ref.set_object_id(static_cast<quint32>(100 + index));
            target.zoneChangeGeneration = static_cast<quint64>(10 + index);
            EXPECT_EQ(ruledStageActivatedAbilityTarget(ability, group, target),
                      RuledActivatedTargetStageResult::Selected);
        }
        EXPECT_EQ(ruledActivatedTargetSelectionCount(ability, 7), targetCount);
        EXPECT_TRUE(ruledPendingActivatedTargetGroupCanConfirm(ability, data));

        if (targetCount == 4) {
            PendingActivatedAbility::Target fifth;
            fifth.ref.set_group_index(7);
            fifth.ref.set_kind(ruled::v1::TARGET_REF_KIND_PERMANENT);
            fifth.ref.set_object_id(104);
            EXPECT_EQ(ruledStageActivatedAbilityTarget(ability, group, fifth),
                      RuledActivatedTargetStageResult::AtMaximum);
            EXPECT_EQ(ruledActivatedTargetSelectionCount(ability, 7), 4);
        }

        int finalHandoffs = 0;
        if (ruledAdvancePendingActivatedTargetGroup(ability, data, true) ==
            RuledActivatedTargetAdvanceResult::Complete) {
            ++finalHandoffs;
        }
        EXPECT_EQ(finalHandoffs, 1);
        EXPECT_FALSE(ability.waitingForTarget);
        EXPECT_EQ(ruledAdvancePendingActivatedTargetGroup(ability, data, true),
                  RuledActivatedTargetAdvanceResult::Invalid);
        EXPECT_EQ(finalHandoffs, 1);
    }
}

TEST(RuledActivatedTargetSelectionTest, AjaniOptionalSingleTargetAllowsZeroOrOne)
{
    RuledTargetGroupData group;
    group.groupIndex = 0;
    group.minTargets = 0;
    group.maxTargets = 1;
    RuledSpellTargetData data;
    data.groups.append(group);

    PendingActivatedAbility empty;
    empty.valid = true;
    empty.waitingForTarget = true;
    EXPECT_TRUE(ruledPendingActivatedTargetGroupCanConfirm(empty, data));
    EXPECT_EQ(ruledAdvancePendingActivatedTargetGroup(empty, data, true), RuledActivatedTargetAdvanceResult::Complete);

    PendingActivatedAbility one;
    one.valid = true;
    one.waitingForTarget = true;
    PendingActivatedAbility::Target target;
    target.ref.set_group_index(0);
    target.ref.set_kind(ruled::v1::TARGET_REF_KIND_PERMANENT);
    target.ref.set_object_id(501);
    EXPECT_EQ(ruledStageActivatedAbilityTarget(one, group, target), RuledActivatedTargetStageResult::Selected);
    EXPECT_EQ(ruledActivatedTargetSelectionCount(one, 0), 1);
    EXPECT_TRUE(ruledPendingActivatedTargetGroupCanConfirm(one, data));
    EXPECT_EQ(ruledAdvancePendingActivatedTargetGroup(one, data, true), RuledActivatedTargetAdvanceResult::Complete);
}

TEST(RuledActivatedTargetSelectionTest, RequiredSingleTargetStillAdvancesOnClick)
{
    RuledTargetGroupData group;
    group.groupIndex = 3;
    group.minTargets = 1;
    group.maxTargets = 1;
    RuledSpellTargetData data;
    data.groups.append(group);

    PendingActivatedAbility ability;
    ability.valid = true;
    ability.waitingForTarget = true;
    PendingActivatedAbility::Target target;
    target.ref.set_group_index(3);
    target.ref.set_kind(ruled::v1::TARGET_REF_KIND_PERMANENT);
    target.ref.set_object_id(51);
    EXPECT_EQ(ruledStageActivatedAbilityTarget(ability, group, target), RuledActivatedTargetStageResult::Selected);
    EXPECT_FALSE(ruledPendingActivatedTargetGroupCanConfirm(ability, data));
    EXPECT_EQ(ruledAdvancePendingActivatedTargetGroup(ability, data, false), RuledActivatedTargetAdvanceResult::Complete);
    EXPECT_FALSE(ability.waitingForTarget);
}

TEST(RuledActivatedTargetSelectionTest, DuplicateToggleTouchesOnlyTheActiveGroup)
{
    RuledTargetGroupData group;
    group.groupIndex = 4;
    group.minTargets = 0;
    group.maxTargets = 4;
    PendingActivatedAbility ability;
    ability.valid = true;
    ability.waitingForTarget = true;
    ability.selectedTargets = {};

    PendingActivatedAbility::Target active;
    active.ref.set_group_index(4);
    active.ref.set_kind(ruled::v1::TARGET_REF_KIND_PERMANENT);
    active.ref.set_object_id(90);
    active.zoneChangeGeneration = 12;
    PendingActivatedAbility::Target otherGroup = active;
    otherGroup.ref.set_group_index(8);
    ability.selectedTargets = {active, otherGroup};

    EXPECT_EQ(ruledStageActivatedAbilityTarget(ability, group, active), RuledActivatedTargetStageResult::Deselected);
    ASSERT_EQ(ability.selectedTargets.size(), 1);
    EXPECT_EQ(ability.selectedTargets.first().ref.group_index(), 8u);
    EXPECT_EQ(ability.selectedTargets.first().ref.object_id(), 90u);
}

TEST(RuledActivatedTargetSelectionTest, ClickEligibilityPreservesSameObjectSelectedInAnEarlierGroup)
{
    FakeHost host;
    RuledClientState state(&host);
    RuledTargetGroupData prior;
    prior.groupIndex = 0;
    prior.minTargets = 1;
    prior.maxTargets = 1;
    prior.validPermanentIds.insert(90);
    RuledTargetGroupData active;
    active.groupIndex = 1;
    active.minTargets = 0;
    active.maxTargets = 4;
    active.validPermanentIds.insert(90);
    ruled::v1::TargetPairConstraint pairConstraint;
    pairConstraint.set_prior_group_index(0);
    auto *pair = pairConstraint.add_compatible_pairs();
    pair->mutable_prior_target()->set_kind(ruled::v1::TARGET_REF_KIND_PERMANENT);
    pair->mutable_prior_target()->set_object_id(90);
    pair->mutable_candidate()->set_kind(ruled::v1::TARGET_REF_KIND_PERMANENT);
    pair->mutable_candidate()->set_object_id(90);
    active.pairConstraint = pairConstraint;
    RuledSpellTargetData data;
    data.groups = {prior, active};
    state.validTargetsByAbility.insert(RuledClientState::abilityTargetKey(80, 1), data);

    PendingActivatedAbility ability;
    ability.valid = true;
    ability.waitingForTarget = true;
    ability.permanentOid = 80;
    ability.abilityIndex = 1;
    ability.activeTargetGroupPosition = 1;
    PendingActivatedAbility::Target earlier;
    earlier.ref.set_group_index(0);
    earlier.ref.set_kind(ruled::v1::TARGET_REF_KIND_PERMANENT);
    earlier.ref.set_object_id(90);
    ability.selectedTargets.append(earlier);
    PendingActivatedAbility::Target activeDuplicate = earlier;
    activeDuplicate.ref.set_group_index(1);
    ability.selectedTargets.append(activeDuplicate);

    EXPECT_EQ(ruledPendingAbilityTargetClickEligibility({}, ability, state, RuledTargetCandidateKind::Battlefield,
                                                        90, kLocalPlayer),
              RuledTargetClickEligibility::Legal);
    EXPECT_EQ(ruledStageActivatedAbilityTarget(ability, active, activeDuplicate),
              RuledActivatedTargetStageResult::Deselected);
    ASSERT_EQ(ability.selectedTargets.size(), 1);
    EXPECT_EQ(ability.selectedTargets.first().ref.group_index(), 0u);
}

TEST(RuledActivatedTargetSelectionTest, ConfirmingZeroTargetsAdvancesToTheNextGroup)
{
    RuledTargetGroupData first;
    first.groupIndex = 5;
    first.minTargets = 0;
    first.maxTargets = 4;
    RuledTargetGroupData second;
    second.groupIndex = 11;
    second.minTargets = 1;
    second.maxTargets = 1;
    RuledSpellTargetData data;
    data.groups = {first, second};

    PendingActivatedAbility ability;
    ability.valid = true;
    ability.waitingForTarget = true;
    EXPECT_EQ(ruledAdvancePendingActivatedTargetGroup(ability, data, true), RuledActivatedTargetAdvanceResult::NextGroup);
    EXPECT_EQ(ability.activeTargetGroupPosition, 1);
    EXPECT_TRUE(ability.waitingForTarget);

    PendingActivatedAbility::Target target;
    target.ref.set_group_index(11);
    target.ref.set_kind(ruled::v1::TARGET_REF_KIND_PERMANENT);
    target.ref.set_object_id(700);
    EXPECT_EQ(ruledStageActivatedAbilityTarget(ability, second, target), RuledActivatedTargetStageResult::Selected);
    EXPECT_EQ(ruledAdvancePendingActivatedTargetGroup(ability, data, false), RuledActivatedTargetAdvanceResult::Complete);
    EXPECT_EQ(ability.selectedTargets.size(), 1);
    EXPECT_EQ(ability.selectedTargets.first().ref.group_index(), 11u);
}

TEST(RuledActivatedTargetSelectionTest, ReconcilePrunesStaleGenerationAndRefreshesAnEmptyOptionalGroup)
{
    FakeHost host;
    RuledClientState state(&host);
    RuledSpellTargetData data;
    RuledTargetGroupData group;
    group.groupIndex = 2;
    group.minTargets = 0;
    group.maxTargets = 4;
    group.validPermanentIds.insert(44);
    group.hasXTargetChoices = true;
    RuledXTargetChoiceData currentChoice;
    currentChoice.xValue = 1;
    currentChoice.candidateGenerations.insert(44, 9);
    group.xTargetChoices.append(currentChoice);
    data.groups.append(group);
    state.validTargetsByAbility.insert(RuledClientState::abilityTargetKey(80, 1), data);
    state.battlefieldGenerationByOid.insert(44, 9);

    PendingActivatedAbility ability;
    ability.valid = true;
    ability.waitingForTarget = true;
    ability.permanentOid = 80;
    ability.abilityIndex = 1;
    ability.xValue = 1;
    PendingActivatedAbility::Target stale;
    stale.ref.set_group_index(2);
    stale.ref.set_kind(ruled::v1::TARGET_REF_KIND_PERMANENT);
    stale.ref.set_object_id(44);
    stale.ref.set_expected_zone_change_generation(8);
    stale.zoneChangeGeneration = 8;
    ability.selectedTargets.append(stale);

    PendingRuledSpellCast emptySpell;
    EXPECT_TRUE(reconcileRuledPendingTargets(emptySpell, ability, state, kLocalPlayer));
    EXPECT_TRUE(ability.selectedTargets.isEmpty());
    EXPECT_TRUE(ability.waitingForTarget);

    // Empty optional groups also cause a refresh on a new legal-actions snapshot.
    EXPECT_TRUE(reconcileRuledPendingTargets(emptySpell, ability, state, kLocalPlayer));
    EXPECT_TRUE(ruledPendingActivatedTargetGroupCanConfirm(ability, data));
}

TEST(RuledActivatedTargetSelectionTest, PlayerTargetIdentityDoesNotAliasAnObjectWithTheSameNumber)
{
    RuledTargetGroupData group;
    group.groupIndex = 0;
    group.minTargets = 0;
    group.maxTargets = 1;
    group.canTargetOpponent = true;
    PendingActivatedAbility ability;
    ability.valid = true;
    ability.waitingForTarget = true;
    PendingActivatedAbility::Target player;
    player.ref.set_group_index(0);
    player.ref.set_kind(ruled::v1::TARGET_REF_KIND_PLAYER);
    player.ref.set_object_id(1);
    EXPECT_EQ(ruledStageActivatedAbilityTarget(ability, group, player), RuledActivatedTargetStageResult::Selected);

    RuledPendingCast pending;
    pending.ability = ability;
    EXPECT_TRUE(pending.isPlayerSelectedAsPendingAbilityTarget(1));
    EXPECT_FALSE(pending.isTargetSelectedForPendingAbility(1));
}

TEST(RuledPendingTargetProgressionTest, DamageDisplaySurvivesPaymentAndRespectsAllocationCaps)
{
    RuledPendingCast pending;
    auto &spell = pending.spell;
    spell.valid = true;
    spell.isDamageTargets = true;
    spell.xValue = 5;
    spell.maxTargets = 0;
    EXPECT_EQ(pending.effectiveDamageTargetsMax(), 5);
    spell.fixedDamage = 2;
    EXPECT_EQ(pending.effectiveDamageTargetsMax(), 2);
    spell.maxTargets = 1;
    EXPECT_EQ(pending.effectiveDamageTargetsMax(), 1);
    spell.damageDivision = ruled::v1::DAMAGE_DIVISION_EVEN_AT_RESOLUTION;
    spell.maxTargets = 0;
    EXPECT_EQ(pending.effectiveDamageTargetsMax(), 0);

    spell.damageDivision = ruled::v1::DAMAGE_DIVISION_CHOOSE_AT_CAST;
    spell.fixedDamage = 0;
    spell.selectedTargetOids = {101, 205};
    EXPECT_EQ(pending.prepareSpellDamageAllocation(), RuledPendingCast::DamageAllocationStep::Allocating);
    EXPECT_FALSE(pending.spellDamageAllocationIsLegal());
    EXPECT_FALSE(pending.confirmSpellDamageAllocation());
    EXPECT_EQ(pending.bumpSpellDamageAllocation(101, -1), RuledPendingCast::DamageAllocationChange::Unchanged);
    EXPECT_EQ(pending.bumpSpellDamageAllocation(101, 1), RuledPendingCast::DamageAllocationChange::Changed);
    EXPECT_EQ(pending.bumpSpellDamageAllocation(205, 2), RuledPendingCast::DamageAllocationChange::Changed);
    EXPECT_EQ(pending.bumpSpellDamageAllocation(205, 1), RuledPendingCast::DamageAllocationChange::Unchanged);
    EXPECT_EQ(pending.bumpSpellDamageAllocation(999, 1), RuledPendingCast::DamageAllocationChange::Unavailable);
    EXPECT_TRUE(pending.spellDamageAllocationIsLegal());
    EXPECT_EQ(pending.spellDamageAllocationForOid(205), 3);
    EXPECT_EQ(pending.spellDamageAllocationForOid(999), 0);

    // Confirmation leaves the submitted split visible throughout payment.
    EXPECT_TRUE(pending.confirmSpellDamageAllocation());
    EXPECT_FALSE(pending.confirmSpellDamageAllocation());
    spell.waitingForTarget = false;
    spell.remainingCost[QChar('R')] = 1;
    spell.targetDamageAllocations.clear();
    EXPECT_EQ(pending.spellDamageAllocationForOid(101), 2);
    EXPECT_EQ(pending.spellDamageAllocationForOid(205), 3);
    EXPECT_FALSE(pending.spellDamageAllocationIsLegal());
    pending.clearSpell();
    EXPECT_FALSE(pending.isSpellDamageAllocationDisplayActive());
    EXPECT_EQ(pending.spellDamageAllocationForOid(205), 0);
}

TEST(RuledPendingTargetProgressionTest, DamagePreparationPreservesAutomaticAndInvalidPaths)
{
    RuledPendingCast pending;
    auto &spell = pending.spell;
    spell.valid = true;
    spell.isDamageTargets = true;
    spell.fixedDamage = 2;
    spell.selectedTargetOids = {101};
    EXPECT_EQ(pending.prepareSpellDamageAllocation(), RuledPendingCast::DamageAllocationStep::Ready);
    EXPECT_EQ(spell.selectedTargetDamages, QVector<quint32>({2}));
    EXPECT_FALSE(spell.inDamageAllocationMode);

    spell.selectedTargetOids = {101, 205, 309};
    EXPECT_EQ(pending.prepareSpellDamageAllocation(), RuledPendingCast::DamageAllocationStep::Invalid);
    EXPECT_FALSE(spell.inDamageAllocationMode);
    EXPECT_TRUE(spell.valid); // The UI owns cancellation and its notifications.

    spell.damageDivision = ruled::v1::DAMAGE_DIVISION_EVEN_AT_RESOLUTION;
    EXPECT_EQ(pending.prepareSpellDamageAllocation(), RuledPendingCast::DamageAllocationStep::Ready);
    EXPECT_EQ(spell.selectedTargetDamages, QVector<quint32>({0, 0, 0}));
    EXPECT_FALSE(spell.inDamageAllocationMode);
    spell.selectedTargetOids.clear();
    EXPECT_EQ(pending.prepareSpellDamageAllocation(), RuledPendingCast::DamageAllocationStep::Ready);
    EXPECT_TRUE(spell.selectedTargetDamages.isEmpty());
}

TEST(RuledPendingTargetProgressionTest, StoresGroupsAndModesBeforePaymentWithoutLosingSelections)
{
    FakeHost host;
    RuledClientState state(&host);
    RuledPendingCast pending;
    auto &spell = pending.spell;
    spell.valid = true;
    spell.handIndex = 3;
    spell.activeModePosition = 0;
    spell.activeTargetGroupPosition = 0;

    RuledTargetGroupData first;
    first.groupIndex = 2;
    first.minTargets = 1;
    first.maxTargets = 1;
    RuledTargetGroupData second;
    second.groupIndex = 7;
    second.minTargets = 0;
    second.maxTargets = 2;
    RuledSpellTargetData firstMode;
    firstMode.groups = {first, second};
    RuledSpellTargetData lastMode;
    lastMode.groups = {first};
    lastMode.isDamageTargets = true;
    lastMode.fixedDamage = 3;
    lastMode.damageDivision = ruled::v1::DAMAGE_DIVISION_PER_TARGET;
    spell.selectedModes = {{4, QStringLiteral("first"), true, firstMode, {}, {}},
                           {8, QStringLiteral("untargeted"), false, {}, {}, {}},
                           {12, QStringLiteral("last"), true, lastMode, {}, {}}};
    state.handActions[ruled::v1::HAND_ACTION_CAST_SPELL]
        .modalOptionsByCastKey[RuledClientState::handCastActionKey(3, 0, ruled::v1::CAST_METHOD_NORMAL)] = {
        {4, QStringLiteral("first"), true, true, firstMode},
        {8, QStringLiteral("untargeted"), true, false, {}},
        {12, QStringLiteral("last"), true, true, lastMode}};

    pending.loadCurrentTargetGroup(state);
    EXPECT_EQ(spell.minTargets, 1);
    spell.selectedTargetOids = {101};
    spell.selectedTargetDamages = {2};
    ASSERT_TRUE(pending.storeCurrentTargetGroupAndAdvance(state));
    EXPECT_EQ(spell.activeTargetGroupPosition, 1);
    EXPECT_EQ(spell.minTargets, 0);
    EXPECT_EQ(spell.maxTargets, 2);
    EXPECT_TRUE(spell.selectedTargetOids.isEmpty());
    EXPECT_TRUE(pending.isTargetSelectedForPendingSpell(101));
    spell.selectedTargetOids = {205, 309};
    spell.selectedTargetDamages = {1, 1};
    EXPECT_FALSE(pending.storeCurrentTargetGroupAndAdvance(state));
    EXPECT_EQ(spell.selectedTargetOidsByGroup.at(1), QVector<quint32>({205, 309}));

    ASSERT_TRUE(pending.storeCurrentModalTargetsAndAdvance(state));
    EXPECT_EQ(spell.activeModePosition, 2); // Skip the untargeted mode, retain authored indices.
    EXPECT_EQ(spell.selectedModes.at(0).modeIndex, 4);
    EXPECT_EQ(spell.selectedModes.at(0).selectedTargetOidsByGroup.at(0), QVector<quint32>({101}));
    EXPECT_EQ(spell.selectedModes.at(0).selectedTargetDamagesByGroup.at(0), QVector<quint32>({2}));
    EXPECT_TRUE(spell.waitingForTarget);
    EXPECT_TRUE(spell.isDamageTargets);
    EXPECT_EQ(spell.fixedDamage, 3);
    EXPECT_EQ(spell.damageDivision, ruled::v1::DAMAGE_DIVISION_PER_TARGET);
    EXPECT_TRUE(spell.selectedTargetOids.isEmpty());

    spell.selectedTargetOids = {401};
    spell.selectedTargetDamages = {3};
    EXPECT_FALSE(pending.storeCurrentTargetGroupAndAdvance(state));
    EXPECT_FALSE(pending.storeCurrentModalTargetsAndAdvance(state));
    EXPECT_EQ(spell.activeModePosition, -1);
    EXPECT_EQ(spell.selectedModes.at(2).selectedTargetOids, QVector<quint32>({401}));
    EXPECT_EQ(spell.selectedModes.at(2).selectedTargetDamagesByGroup.at(0), QVector<quint32>({3}));
}

TEST(RuledPendingTargetProgressionTest, MissingOrRemovedGroupDoesNotAdvanceOrOverwriteStaging)
{
    FakeHost host;
    RuledClientState state(&host);
    RuledPendingCast pending;
    pending.spell.valid = true;
    pending.spell.handIndex = 3;
    pending.spell.activeTargetGroupPosition = 4;
    pending.spell.selectedTargetOids = {101};
    pending.spell.selectedTargetDamages = {2};
    pending.loadCurrentTargetGroup(state);
    EXPECT_FALSE(pending.storeCurrentTargetGroupAndAdvance(state));
    EXPECT_FALSE(pending.storeCurrentModalTargetsAndAdvance(state));
    EXPECT_EQ(pending.spell.activeTargetGroupPosition, 4);
    EXPECT_EQ(pending.spell.selectedTargetOids, QVector<quint32>({101}));
    EXPECT_EQ(pending.spell.selectedTargetDamages, QVector<quint32>({2}));
    EXPECT_TRUE(pending.spell.selectedTargetOidsByGroup.isEmpty());
}

TEST(RuledRestrictedManaModelTest, FullyStagedGroupConsumesNoLayoutColumn)
{
    RuledRestrictedManaGroup group;
    group.groupId = 7;
    group.r = 1;
    RuledRestrictedManaSelections staged;
    staged[7][QChar('R')] = 1;

    EXPECT_EQ(ruledVisibleRestrictedManaColumnCount({group}, staged), 0);
}

TEST(RuledRestrictedManaModelTest, ReportsOnlyNewlyProducedContributions)
{
    RuledRestrictedManaTracker tracker;
    EXPECT_TRUE(tracker.observe({}).isEmpty());

    RuledRestrictedManaGroup group;
    group.groupId = 7;
    group.r = 1;
    const auto firstProduction = tracker.observe({group});
    ASSERT_EQ(firstProduction.size(), 1);
    EXPECT_EQ(firstProduction.at(0).groupId, 7u);
    EXPECT_EQ(firstProduction.at(0).symbol, QChar('R'));
    EXPECT_EQ(firstProduction.at(0).amount, 1);

    EXPECT_TRUE(tracker.observe({group}).isEmpty());
    group.r = 2;
    const auto secondProduction = tracker.observe({group});
    ASSERT_EQ(secondProduction.size(), 1);
    EXPECT_EQ(secondProduction.at(0).amount, 1);

    group.r = 0;
    EXPECT_TRUE(tracker.observe({group}).isEmpty());
}

TEST(RuledTargetingCostTest, DeduplicatesOneApplicationAcrossGroupsAndTypedIdCollisions)
{
    RuledSpellTargetData data;
    RuledTargetGroupData first;
    first.validPermanentIds.insert(1); // Deliberately collides with kOpponent.
    first.validPermanentIds.insert(10);
    RuledTargetGroupData second;
    second.validPermanentIds.insert(11);
    data.groups = {first, second};

    RuledTargetingCostApplication kopala;
    kopala.applicationId = 99;
    kopala.genericMana = 2;
    kopala.affectedTargets = {
        {ruled::v1::TARGET_REF_KIND_PLAYER, 1},
        {ruled::v1::TARGET_REF_KIND_PERMANENT, 10},
        {ruled::v1::TARGET_REF_KIND_PERMANENT, 11},
    };
    data.targetingCostApplications = {kopala};

    EXPECT_EQ(ruledTargetingCostForSelection(data, {{1, 10}, {11}}, {}, kLocalPlayer), 2);
    EXPECT_EQ(ruledTargetingCostForSelection(data, {{1}, {}}, {}, kLocalPlayer), 0);
}

TEST(RuledTargetingCostTest, SumsDistinctApplicationsButDeduplicatesAcrossModes)
{
    RuledSpellTargetData data;
    RuledTargetGroupData group;
    group.validPermanentIds.insert(20);
    data.groups = {group};
    data.targetingCostApplications = {
        {100, 2, {{ruled::v1::TARGET_REF_KIND_PERMANENT, 20}}},
        {101, 2, {{ruled::v1::TARGET_REF_KIND_PERMANENT, 20}}},
    };
    EXPECT_EQ(ruledTargetingCostForSelection(data, {{20}}, {}, kLocalPlayer), 4);

    PendingRuledSpellCast spell;
    spell.valid = true;
    PendingRuledSpellCast::SelectedMode first;
    first.targets = data;
    first.selectedTargetOids = {20};
    first.selectedTargetOidsByGroup = {{20}};
    PendingRuledSpellCast::SelectedMode second = first;
    spell.selectedModes = {first, second};
    EXPECT_EQ(ruledModalSpellTargetingCost(spell, kLocalPlayer), 4);
}

TEST(RuledTargetedCostReductionTest, DeduplicatesOneApplicationAcrossGroupsAndUsesTypedCandidates)
{
    RuledSpellTargetData data;
    RuledTargetGroupData first;
    first.validPermanentIds.insert(1); // Deliberately collides with kOpponent.
    first.validPermanentIds.insert(20);
    RuledTargetGroupData second;
    second.validPermanentIds.insert(21);
    data.groups = {first, second};

    data.targetedCostReductionApplications = {
        {200,
         3,
         {
             {ruled::v1::TARGET_REF_KIND_PLAYER, 1},
             {ruled::v1::TARGET_REF_KIND_PERMANENT, 20},
             {ruled::v1::TARGET_REF_KIND_PERMANENT, 21},
         }},
    };

    EXPECT_EQ(ruledTargetedCostReductionForSelection(data, {{1, 20}, {21}}, {}, kLocalPlayer), 3);
    EXPECT_EQ(ruledTargetedCostReductionForSelection(data, {{1}, {}}, {}, kLocalPlayer), 0);
}

TEST(RuledTargetedCostReductionTest, SumsDistinctApplicationsButDeduplicatesAcrossModes)
{
    RuledSpellTargetData data;
    RuledTargetGroupData group;
    group.validPermanentIds.insert(30);
    data.groups = {group};
    data.targetedCostReductionApplications = {
        {300, 3, {{ruled::v1::TARGET_REF_KIND_PERMANENT, 30}}},
        {301, 1, {{ruled::v1::TARGET_REF_KIND_PERMANENT, 30}}},
    };
    EXPECT_EQ(ruledTargetedCostReductionForSelection(data, {{30}}, {}, kLocalPlayer), 4);

    PendingRuledSpellCast spell;
    spell.valid = true;
    PendingRuledSpellCast::SelectedMode first;
    first.targets = data;
    first.selectedTargetOids = {30};
    first.selectedTargetOidsByGroup = {{30}};
    PendingRuledSpellCast::SelectedMode second = first;
    spell.selectedModes = {first, second};
    EXPECT_EQ(ruledModalSpellTargetedCostReduction(spell, kLocalPlayer), 4);
}

TEST(RuledTargetingCostTest, AppliesReductionsAfterXAndAllIncreases)
{
    // Base generic includes the chosen X (2); Fireball and targeting taxes add 3; reduction is 4.
    EXPECT_EQ(ruledFinalGenericCost(2, 3, 4), 1);
    EXPECT_EQ(ruledFinalGenericCost(0, 2, 5), 0);
}

TEST_F(RuledClientTest, ParsesRawCostReductionAndTargetApplications)
{
    ruled::v1::RuledEventBatch batch;
    auto &actions = (*batch.mutable_legal_by_player())[kLocalPlayer];
    auto *cast = addHandAction(actions, ruled::v1::HAND_ACTION_CAST_SPELL, 4, "Test Spell", 0, true, "{3}{U}");
    cast->set_generic_cost_reduction(1);
    auto &targets = (*actions.mutable_valid_targets_by_hand_slot())[quint32(4) << 8];
    auto *group = targets.add_groups();
    group->set_group_index(0);
    group->add_valid_permanent_ids(30);
    auto *application = targets.add_targeting_cost_applications();
    application->set_application_id(700);
    application->set_generic_mana(2);
    auto *candidate = application->add_affected_targets();
    candidate->set_kind(ruled::v1::TARGET_REF_KIND_PERMANENT);
    candidate->set_object_id(30);
    auto *reduction = targets.add_targeted_cost_reduction_applications();
    reduction->set_application_id(701);
    reduction->set_generic_mana(3);
    auto *qualifying = reduction->add_qualifying_targets();
    qualifying->set_kind(ruled::v1::TARGET_REF_KIND_PERMANENT);
    qualifying->set_object_id(30);

    apply(batch);

    const auto faces = state->handActionFaceOptions(ruled::v1::HAND_ACTION_CAST_SPELL, 4);
    ASSERT_EQ(faces.size(), 1);
    EXPECT_EQ(faces.first().manaCost, QStringLiteral("{3}{U}"));
    EXPECT_EQ(faces.first().genericCostReduction, 1);
    const auto parsed = state->spellTargetData(4, 0, RuledCastSource::Hand);
    ASSERT_EQ(parsed.targetingCostApplications.size(), 1);
    EXPECT_EQ(parsed.targetingCostApplications.first().applicationId, 700u);
    EXPECT_EQ(parsed.targetingCostApplications.first().genericMana, 2);
    ASSERT_EQ(parsed.targetedCostReductionApplications.size(), 1);
    EXPECT_EQ(parsed.targetedCostReductionApplications.first().applicationId, 701u);
    EXPECT_EQ(parsed.targetedCostReductionApplications.first().genericMana, 3);
    ASSERT_EQ(parsed.targetedCostReductionApplications.first().qualifyingTargets.size(), 1);
    EXPECT_EQ(parsed.targetedCostReductionApplications.first().qualifyingTargets.first().oid, 30u);
}

// CR 608.2b: a target that changes zones becomes a new object, so the arrow endpoint recorded when
// the target was chosen is never revised. Without the write-once latch, an oid that enters the
// graveyard map after its permanent dies re-resolves to the graveyard pile and the arrow points
// there instead of disappearing.
TEST_F(RuledClientTest, TargetKindIsLatchedOnceAndNotRevisedWhenTheTargetDies)
{
    ruled::v1::RuledEventBatch push;
    auto *ability = push.add_events()->mutable_stack_pushed();
    ability->set_object_id(11);
    ability->mutable_targets()->Add()->set_object_id(100);
    apply(push);

    EXPECT_EQ(state->latchedTargetKind(11, 100), RuledTargetItemKind::Unknown);
    state->latchTargetKind(11, 100, RuledTargetItemKind::Battlefield);
    EXPECT_EQ(state->latchedTargetKind(11, 100), RuledTargetItemKind::Battlefield);

    // The permanent dies and its oid now also appears in the graveyard map. A re-classification
    // must not overwrite the original answer.
    state->latchTargetKind(11, 100, RuledTargetItemKind::Graveyard);
    EXPECT_EQ(state->latchedTargetKind(11, 100), RuledTargetItemKind::Battlefield);
}

// An unresolvable target (its CardItem does not exist yet) leaves the entry unlatched so a later
// sync can classify it, rather than freezing Unknown and never drawing the arrow.
TEST_F(RuledClientTest, UnknownTargetKindIsNotLatched)
{
    state->latchTargetKind(11, 100, RuledTargetItemKind::Unknown);
    EXPECT_TRUE(state->stackTargetKindByStackAndTargetOid.isEmpty());

    state->latchTargetKind(11, 100, RuledTargetItemKind::Graveyard);
    EXPECT_EQ(state->latchedTargetKind(11, 100), RuledTargetItemKind::Graveyard);
}

// The latch is keyed per (stack object, target), so two spells aimed at the same object are
// independent — and resolving one must not strip the other's endpoint.
TEST_F(RuledClientTest, TargetKindLatchIsClearedWhenItsStackObjectResolves)
{
    ruled::v1::RuledEventBatch push;
    auto *first = push.add_events()->mutable_stack_pushed();
    first->set_object_id(11);
    first->mutable_targets()->Add()->set_object_id(100);
    auto *second = push.add_events()->mutable_stack_pushed();
    second->set_object_id(12);
    second->mutable_targets()->Add()->set_object_id(100);
    apply(push);

    state->latchTargetKind(11, 100, RuledTargetItemKind::Battlefield);
    state->latchTargetKind(12, 100, RuledTargetItemKind::Battlefield);

    ruled::v1::RuledEventBatch resolve;
    resolve.add_events()->mutable_stack_resolved()->set_object_id(11);
    apply(resolve);

    EXPECT_EQ(state->latchedTargetKind(11, 100), RuledTargetItemKind::Unknown);
    EXPECT_EQ(state->latchedTargetKind(12, 100), RuledTargetItemKind::Battlefield);
}

TEST_F(RuledClientTest, CounteredSpellLeavesTheStackWithItsCounterspell)
{
    ruled::v1::RuledEventBatch push;
    push.add_events()->mutable_stack_pushed()->set_object_id(10); // the spell
    auto *counter = push.add_events()->mutable_stack_pushed();
    counter->set_object_id(11); // Counterspell targeting it
    counter->mutable_targets()->Add()->set_object_id(10);
    apply(push);
    ASSERT_EQ(state->getStackOidOrder().size(), 2);

    // The engine explicitly names the countered object; resolving Counterspell retires only
    // Counterspell itself.
    ruled::v1::RuledEventBatch resolve;
    resolve.add_events()->mutable_stack_object_countered()->set_object_id(10);
    resolve.add_events()->mutable_stack_resolved()->set_object_id(11);
    apply(resolve);
    EXPECT_TRUE(state->getStackOidOrder().isEmpty());
    EXPECT_FALSE(state->hasStackItems());
}

TEST_F(RuledClientTest, AbilityOnStackGetsASyntheticCardAndClearsThePendingTrigger)
{
    ruled::v1::RuledEventBatch needsTarget;
    auto *tnt = needsTarget.add_events()->mutable_trigger_needs_target();
    tnt->set_source_permanent_id(100);
    tnt->set_ability_index(0);
    tnt->set_ability_text("Return target creature card from your graveyard to your hand.");
    tnt->set_controller_player_id(kLocalPlayer);
    apply(needsTarget);
    ASSERT_TRUE(state->hasPendingTriggerTarget());
    EXPECT_EQ(state->pendingTriggerSource(), 100u);

    ruled::v1::RuledEventBatch push;
    auto *sp = push.add_events()->mutable_stack_pushed();
    sp->set_object_id(900);
    sp->set_description("Gravedigger ETB");
    sp->set_ability_annotation("Return target creature card...");
    // No card_id => an ability, which has no physical CardItem. is_triggered distinguishes a
    // *triggered* ability (this one) from an activated one; only the former retires the prompt.
    sp->set_is_triggered(true);
    sp->set_ability_controller_player_id(kLocalPlayer);
    sp->mutable_ability_source()->set_object_id(100u);
    sp->mutable_ability_source()->set_zone_change_generation(0u);
    state->battlefieldGenerationByOid.insert(100u, 0u);
    apply(push);

    EXPECT_FALSE(state->hasPendingTriggerTarget());
    EXPECT_EQ(state->stackSourceOidByStackOid.value(900), 100u);
    ASSERT_EQ(host.createdSyntheticCards.size(), 1);
    EXPECT_EQ(host.createdSyntheticCards[0].oid, 900u);
    EXPECT_EQ(host.createdSyntheticCards[0].name, QStringLiteral("Gravedigger ETB"));
    EXPECT_EQ(host.createdSyntheticCards[0].controllerPlayerId, kLocalPlayer);
    EXPECT_EQ(state->stackAnnotation(900), QStringLiteral("Return target creature card..."));
}

TEST_F(RuledClientTest, SyntheticStackCardIdentityIsBidirectionalAndSurvivesBattlefieldSync)
{
    constexpr quint32 abilityOid = 900;
    constexpr int fakeCardId = 0x70000384;
    state->registerSyntheticStackCard(abilityOid, fakeCardId, kLocalPlayer);

    EXPECT_EQ(state->engineOidForCardId(kLocalPlayer, fakeCardId), abilityOid);
    EXPECT_EQ(state->cardIdForEngineOid(abilityOid), fakeCardId)
        << "stack target resolution needs the reverse identity to prefer the visible window copy";

    ruled::v1::RuledEventBatch sync;
    sync.add_events()->mutable_battlefield_object_map();
    apply(sync);
    EXPECT_EQ(state->engineOidForCardId(kLocalPlayer, fakeCardId), abilityOid);
    EXPECT_EQ(state->cardIdForEngineOid(abilityOid), fakeCardId);

    // GameEventHandler uses -1 when Qt deleted the CardItem before normal teardown; the state
    // still owns the registered fake ID and must remove the exact forward mapping.
    state->unregisterSyntheticStackCard(abilityOid, -1);
    EXPECT_EQ(state->engineOidForCardId(kLocalPlayer, fakeCardId), 0u);
    EXPECT_EQ(state->cardIdForEngineOid(abilityOid), -1);
}

// Paying a sacrifice cost (Bottle Gnomes) queues a dies trigger, and the activated ability that
// consumed the cost reaches the stack in the same batch. Both an activated and a triggered ability
// are card_id-less, so without is_triggered the activated one wiped the prompt for the trigger it
// had just caused — the player was left with a choice the engine was still blocking on.
TEST_F(RuledClientTest, ActivatedAbilityOnStackKeepsThePendingTriggerPrompt)
{
    ruled::v1::RuledEventBatch batch;
    auto *sp = batch.add_events()->mutable_stack_pushed();
    sp->set_object_id(900);
    sp->set_description("Bottle Gnomes");
    sp->set_ability_annotation("Sacrifice this creature: You gain 3 life.");
    sp->set_is_triggered(false); // an *activated* ability
    auto *tnt = batch.add_events()->mutable_trigger_needs_target();
    tnt->set_source_permanent_id(100);
    tnt->set_ability_index(0);
    tnt->set_ability_text("Whenever this creature or another creature dies, target player loses 1 life...");
    tnt->set_controller_player_id(kLocalPlayer);
    apply(batch);

    EXPECT_TRUE(state->hasPendingTriggerTarget())
        << "an activated ability must not retire the trigger prompt it caused";
    EXPECT_EQ(state->pendingTriggerSource(), 100u);
}

TEST_F(RuledClientTest, TokenActivatedAbilityForwardsItsDisplayIdentityToTheSyntheticStackCard)
{
    ruled::v1::RuledEventBatch batch;
    auto *sp = batch.add_events()->mutable_stack_pushed();
    sp->set_object_id(901);
    sp->set_description("Clue");
    sp->set_ability_annotation("{2}, Sacrifice this token: Draw a card.");
    sp->set_is_triggered(false);
    auto *identity = sp->mutable_source_token_identity();
    identity->set_name("Clue");
    identity->set_mana_cost("{3}{R}{R}");
    identity->set_mana_value(5);
    identity->add_types("Artifact");
    identity->add_ability_texts("{2}, Sacrifice this token: Draw a card.");

    apply(batch);

    ASSERT_EQ(host.createdSyntheticCards.size(), 1);
    ASSERT_TRUE(host.createdSyntheticCards[0].sourceTokenIdentity.has_value());
    const auto &forwarded = *host.createdSyntheticCards[0].sourceTokenIdentity;
    EXPECT_EQ(forwarded.name, QStringLiteral("Clue"));
    EXPECT_TRUE(forwarded.basePt.isEmpty());
    EXPECT_TRUE(forwarded.color.isEmpty());
    EXPECT_EQ(forwarded.manaCost, QStringLiteral("{3}{R}{R}"));
    EXPECT_EQ(forwarded.manaValue, 5u);
    EXPECT_TRUE(forwarded.keywords.isEmpty());
    EXPECT_EQ(forwarded.abilityTexts, QStringList({QStringLiteral("{2}, Sacrifice this token: Draw a card.")}));
}

// Gravedigger ETB: a pending trigger whose only legal targets sit in a graveyard makes the tab
// auto-open the local graveyard view, so the player can click the target without hunting for it.
//
// NB the engine currently only fills LegalActions.valid_targets_by_ability from a permanent's
// *activated* abilities (engine/legal_actions.rs), so a triggered ability never gets an entry and
// this never fires in a real game. This pins the client half of the contract; the engine half is
// the open half.
TEST_F(RuledClientTest, PendingTriggerWithGraveyardTargetsAsksForTheGraveyardView)
{
    QSignalSpy spy(state, &RuledClientState::graveyardTargetsNeeded);

    ruled::v1::RuledEventBatch batch;
    auto *tnt = batch.add_events()->mutable_trigger_needs_target();
    tnt->set_source_permanent_id(100);
    tnt->set_ability_index(0);
    tnt->set_ability_text("Return target creature card from your graveyard to your hand.");
    tnt->set_controller_player_id(kLocalPlayer);
    // The signal names players, not a bool, so the oid has to be mapped to a graveyard first.
    auto *ge = batch.add_events()->mutable_graveyard_object_map()->add_entries();
    ge->set_player_id(kLocalPlayer);
    ge->set_engine_object_id(500);
    ge->set_server_card_id(11);
    auto &actions = (*batch.mutable_legal_by_player())[kLocalPlayer];
    auto &abilityTargets = (*actions.mutable_valid_targets_by_ability())[(quint64(100) << 32) | 0u];
    auto *triggerGroup = abilityTargets.add_groups();
    triggerGroup->set_group_index(0);
    triggerGroup->set_prompt_text("Choose target creature card");
    triggerGroup->set_min(1);
    triggerGroup->set_max(1);
    triggerGroup->add_valid_graveyard_ids(500);
    apply(batch);

    ASSERT_TRUE(state->hasPendingTriggerTarget());
    ASSERT_GE(spy.count(), 1);
    EXPECT_EQ(spy.last().at(0).value<QList<int>>(), QList<int>{kLocalPlayer});

    // Once the ability itself is on the stack the target has been chosen, so the view may close.
    // A *triggered* ability push (card_id-less, annotated, is_triggered) retires the prompt.
    const int beforePush = spy.count();
    ruled::v1::RuledEventBatch push;
    auto *sp = push.add_events()->mutable_stack_pushed();
    sp->set_object_id(900);
    sp->set_ability_annotation("Return target creature card...");
    sp->set_is_triggered(true);
    apply(push);
    EXPECT_FALSE(state->hasPendingTriggerTarget());
    ASSERT_GT(spy.count(), beforePush);
    EXPECT_TRUE(spy.last().at(0).value<QList<int>>().isEmpty());
}

// Reanimate reads *a* graveyard, so a pending cast must be able to ask for the opponent's view.
TEST_F(RuledClientTest, PendingCastGraveyardTargetsNameTheOwningPlayer)
{
    QSignalSpy spy(state, &RuledClientState::graveyardTargetsNeeded);

    ruled::v1::RuledEventBatch batch;
    auto *gy = batch.add_events()->mutable_graveyard_object_map();
    auto *mine = gy->add_entries();
    mine->set_player_id(kLocalPlayer);
    mine->set_engine_object_id(500);
    mine->set_server_card_id(11);
    auto *theirs = gy->add_entries();
    theirs->set_player_id(kOpponent);
    theirs->set_engine_object_id(501);
    theirs->set_server_card_id(11); // same card id, different owner — must not collide
    apply(batch);

    // A cast whose only legal target sits in the opponent's graveyard.
    state->setPendingCastGraveyardTargets({501u});
    ASSERT_GE(spy.count(), 1);
    EXPECT_EQ(spy.last().at(0).value<QList<int>>(), QList<int>{kOpponent});

    // Targeting both graveyards asks for both views.
    state->setPendingCastGraveyardTargets({500u, 501u});
    QList<int> both = spy.last().at(0).value<QList<int>>();
    std::sort(both.begin(), both.end());
    EXPECT_EQ(both, (QList<int>{kLocalPlayer, kOpponent}));

    // Cancelling the cast retracts the request.
    state->setPendingCastGraveyardTargets({});
    EXPECT_TRUE(spy.last().at(0).value<QList<int>>().isEmpty());
}

// A cast spell keeps its target's graveyard open until it leaves the stack, so the targeting
// arrow stays anchored to the card instead of to a pile the player can no longer see.
TEST_F(RuledClientTest, GraveyardStaysRequestedWhileTheSpellIsOnTheStack)
{
    QSignalSpy spy(state, &RuledClientState::graveyardTargetsNeeded);

    // A creature in the opponent's graveyard, and a spell on the stack targeting it.
    ruled::v1::RuledEventBatch batch;
    auto *ge = batch.add_events()->mutable_graveyard_object_map()->add_entries();
    ge->set_player_id(kOpponent);
    ge->set_engine_object_id(700);
    ge->set_server_card_id(21);
    auto *sp = batch.add_events()->mutable_stack_pushed();
    sp->set_object_id(800);
    sp->add_targets()->set_object_id(700);
    apply(batch);

    // No pending cast any more — the spell has been cast — but the graveyard is still wanted.
    ASSERT_GE(spy.count(), 1);
    EXPECT_EQ(spy.last().at(0).value<QList<int>>(), QList<int>{kOpponent})
        << "the spell is on the stack, so its target's graveyard must stay open";

    // It resolves: nothing wants the graveyard any more.
    ruled::v1::RuledEventBatch resolved;
    auto *sr = resolved.add_events()->mutable_stack_resolved();
    sr->set_object_id(800);
    apply(resolved);
    EXPECT_TRUE(spy.last().at(0).value<QList<int>>().isEmpty()) << "once the spell leaves the stack the view may close";
}

// A target chosen in a graveyard is latched as such the moment it is put on the stack, without
// waiting for the (deferred) arrow sync — `emitGraveyardTargetsNeeded` runs first and needs the
// answer already.
TEST_F(RuledClientTest, GraveyardTargetIsLatchedWhenTheSpellIsPushed)
{
    ruled::v1::RuledEventBatch batch;
    auto *ge = batch.add_events()->mutable_graveyard_object_map()->add_entries();
    ge->set_player_id(kOpponent);
    ge->set_engine_object_id(700);
    ge->set_server_card_id(21);
    auto *sp = batch.add_events()->mutable_stack_pushed();
    sp->set_object_id(800);
    sp->add_targets()->set_object_id(700);
    apply(batch);

    EXPECT_EQ(state->latchedTargetKind(800, 700), RuledTargetItemKind::Graveyard);
}

// The regression this pairs with: an ability targeting a *permanent* must never open a graveyard
// view, even when that permanent dies while the ability is still on the stack and its oid joins the
// graveyard map. Only targets chosen in a graveyard count.
TEST_F(RuledClientTest, ADyingTargetDoesNotRequestAGraveyardView)
{
    // The ability goes on the stack while its target is a permanent — nothing in any graveyard yet.
    ruled::v1::RuledEventBatch push;
    auto *sp = push.add_events()->mutable_stack_pushed();
    sp->set_object_id(800);
    sp->add_targets()->set_object_id(100);
    apply(push);
    EXPECT_EQ(state->latchedTargetKind(800, 100), RuledTargetItemKind::Unknown);

    QSignalSpy spy(state, &RuledClientState::graveyardTargetsNeeded);

    // The target is destroyed in response: its oid now appears in a graveyard map.
    ruled::v1::RuledEventBatch died;
    auto *ge = died.add_events()->mutable_graveyard_object_map()->add_entries();
    ge->set_player_id(kOpponent);
    ge->set_engine_object_id(100);
    ge->set_server_card_id(21);
    apply(died);

    ASSERT_GE(spy.count(), 1);
    EXPECT_TRUE(spy.last().at(0).value<QList<int>>().isEmpty())
        << "an ability that targeted a permanent must not pop graveyard views when it dies";
}

TEST_F(RuledClientTest, TriggerNeedsTargetOnlyPendsForItsController)
{
    ruled::v1::RuledEventBatch batch;
    auto *tnt = batch.add_events()->mutable_trigger_needs_target();
    tnt->set_source_permanent_id(100);
    tnt->set_ability_text("Draw a card.");
    tnt->set_controller_player_id(kOpponent);
    apply(batch);
    // We see the trigger, but only its controller may answer it.
    EXPECT_FALSE(state->hasPendingTriggerTarget());
    EXPECT_EQ(state->pendingTriggerController(), kOpponent);
    EXPECT_TRUE(state->isWaitingForChoice());
    EXPECT_EQ(state->choiceWaitingPlayer(), kOpponent);

    // A redacted observer event has no target candidates. Repeated publication must keep waiting.
    apply(batch);
    EXPECT_TRUE(state->isWaitingForChoice());

    ruled::v1::RuledEventBatch completed;
    completed.add_events()->mutable_stack_pushed()->set_object_id(501);
    apply(completed);
    EXPECT_FALSE(state->isWaitingForChoice());
}

TEST_F(RuledClientTest, TriggerModeWaitTransfersToTheNextDecidingPlayer)
{
    ruled::v1::RuledEventBatch batch;
    auto *trigger = batch.add_events()->mutable_trigger_needs_target();
    trigger->set_controller_player_id(kOpponent);
    trigger->set_min_modes(1);
    trigger->set_max_modes(1);
    // Modes are private, so the observer learns only who is deciding.
    apply(batch);
    EXPECT_TRUE(state->isWaitingForChoice());
    EXPECT_FALSE(state->hasPendingChoiceOptions());

    trigger->set_controller_player_id(kLocalPlayer);
    trigger->add_modes()->set_label("Draw a card.");
    apply(batch);
    EXPECT_FALSE(state->isWaitingForChoice());
    EXPECT_TRUE(state->hasPendingChoiceOfKind(RuledClientState::ChoiceKind::TriggerMode));
}

TEST_F(RuledClientTest, SpellCopyInheritsTheOriginalPrinting)
{
    host.providerIds.insert(10, QStringLiteral("lea"));
    ruled::v1::RuledEventBatch push;
    push.add_events()->mutable_stack_pushed()->set_object_id(10);
    auto *copy = push.add_events()->mutable_stack_pushed();
    copy->set_object_id(11);
    copy->set_card_id("lightning_bolt");
    copy->set_description("Lightning Bolt (copy)");
    copy->set_ability_annotation("copy");
    copy->set_is_copy(true);
    copy->set_copy_source_object_id(10);
    apply(push);

    ASSERT_EQ(host.createdSyntheticCards.size(), 1);
    EXPECT_EQ(host.createdSyntheticCards[0].oid, 11u);
    EXPECT_EQ(host.createdSyntheticCards[0].setName, QStringLiteral("lea"));
}

TEST_F(RuledClientTest, AbilityCopyClearsRetargetChoiceWithoutBorrowingAnUnrelatedTriggerSource)
{
    RuledClientState::RuledPendingChoice pending;
    pending.kind = RuledClientState::ChoiceKind::CopyTarget;
    pending.candidateOids = {123u};
    stageChoiceForTest(pending);
    state->lastTriggerSourceOid = 777u;
    state->lastTriggerControllerPlayerId = kOpponent;

    ruled::v1::RuledEventBatch batch;
    auto *copy = batch.add_events()->mutable_stack_pushed();
    copy->set_object_id(901u);
    copy->set_description("Voltaic Key");
    copy->set_ability_annotation("Untap target artifact. (copy)");
    copy->set_is_copy(true);
    copy->set_is_triggered(false);
    copy->set_copy_source_object_id(900u);
    apply(batch);

    EXPECT_FALSE(state->hasPendingChoiceOfKind(RuledClientState::ChoiceKind::CopyTarget));
    EXPECT_FALSE(state->stackSourceOidByStackOid.contains(901u));
    ASSERT_EQ(host.createdSyntheticCards.size(), 1);
    EXPECT_EQ(host.createdSyntheticCards[0].controllerPlayerId, -1)
        << "without published metadata the stale trigger controller is not the copy's controller";
}

TEST_F(RuledClientTest, AbilityCopyPushDoesNotEraseTheNextCopyTargetPromptInTheSameBatch)
{
    RuledClientState::RuledPendingChoice old;
    old.kind = RuledClientState::ChoiceKind::CopyTarget;
    old.candidateOids = {123u};
    stageChoiceForTest(old);

    ruled::v1::RuledEventBatch batch;
    auto *copy = batch.add_events()->mutable_stack_pushed();
    copy->set_object_id(901u);
    copy->set_description("Goblin Welder");
    copy->set_ability_annotation("Exchange artifacts. (copy)");
    copy->set_is_copy(true);
    auto *next = batch.add_events()->mutable_resolution_choice_required();
    next->set_deciding_player_id(kLocalPlayer);
    next->set_choice_kind(ruled::v1::CHOICE_KIND_TARGET_OBJECTS);
    next->set_prompt_text("Choose the next copy's target or keep the original.");
    next->set_min(0);
    next->set_max(1);
    next->add_candidate_object_ids(456u);
    next->add_candidate_card_ids("sol_ring");
    next->add_candidate_names("Sol Ring");
    apply(batch);

    ASSERT_TRUE(state->hasPendingChoiceOfKind(RuledClientState::ChoiceKind::CopyTarget));
    EXPECT_TRUE(state->isPendingChoiceCandidate(RuledClientState::ChoiceKind::CopyTarget, 456u));
    EXPECT_FALSE(state->isPendingChoiceCandidate(RuledClientState::ChoiceKind::CopyTarget, 123u));
}

TEST_F(RuledClientTest, AbilityCopyUsesPublishedControllerAndExactPublicSourceIncarnation)
{
    state->lastTriggerSourceOid = 777u;
    state->lastTriggerControllerPlayerId = kOpponent;
    ruled::v1::RuledEventBatch batch;
    auto *view = batch.add_events()->mutable_zone_view();
    auto *player = view->add_per_player();
    player->set_player_id(kLocalPlayer);
    auto *source = player->add_battlefield_objects();
    source->set_object_id(100u);
    source->set_zone_change_generation(4u);
    auto *copy = batch.add_events()->mutable_stack_pushed();
    copy->set_object_id(901u);
    copy->set_description("Clue");
    copy->set_ability_annotation("Draw a card. (copy)");
    copy->set_is_copy(true);
    copy->set_ability_controller_player_id(kLocalPlayer);
    copy->mutable_ability_source()->set_object_id(100u);
    copy->mutable_ability_source()->set_zone_change_generation(4u);
    copy->mutable_source_token_identity()->set_name("Clue");
    apply(batch);
    ASSERT_EQ(host.createdSyntheticCards.size(), 1);
    EXPECT_EQ(host.createdSyntheticCards[0].controllerPlayerId, kLocalPlayer);
    ASSERT_TRUE(host.createdSyntheticCards[0].sourceTokenIdentity.has_value());
    EXPECT_EQ(host.createdSyntheticCards[0].sourceTokenIdentity->name, QStringLiteral("Clue"));
    EXPECT_EQ(state->stackSourceOidByStackOid.value(901u), 100u);

    // The same OID returning with a new generation is a different source occurrence.
    ruled::v1::RuledEventBatch returned;
    auto *returnedView = returned.add_events()->mutable_zone_view();
    auto *returnedPlayer = returnedView->add_per_player();
    returnedPlayer->set_player_id(kLocalPlayer);
    auto *newSource = returnedPlayer->add_battlefield_objects();
    newSource->set_object_id(100u);
    newSource->set_zone_change_generation(6u);
    apply(returned);
    EXPECT_FALSE(state->stackSourceOidByStackOid.contains(901u));
    EXPECT_TRUE(state->stackSourceCapturedOidByStackOid.contains(901u));
}

TEST_F(RuledClientTest, AbilityCopyWithoutPublicSourceNeverUsesStaleTriggerOrHiddenIdentity)
{
    state->lastTriggerSourceOid = 777u;
    state->lastTriggerControllerPlayerId = kOpponent;
    ruled::v1::RuledEventBatch batch;
    auto *copy = batch.add_events()->mutable_stack_pushed();
    copy->set_object_id(901u);
    copy->set_description("Face-down permanent");
    copy->set_ability_annotation("Ability (copy)");
    copy->set_is_copy(true);
    copy->set_ability_controller_player_id(kLocalPlayer);
    apply(batch);
    ASSERT_EQ(host.createdSyntheticCards.size(), 1);
    EXPECT_EQ(host.createdSyntheticCards[0].name, QStringLiteral("Face-down permanent"));
    EXPECT_EQ(host.createdSyntheticCards[0].controllerPlayerId, kLocalPlayer);
    EXPECT_FALSE(state->stackSourceOidByStackOid.contains(901u));
}

TEST_F(RuledClientTest, ReconnectStackPresentationReplacesStaleAndRepeatedCopiesBeforeChoice)
{
    ruled::v1::RuledEventBatch old;
    for (quint32 oid : {900u, 999u}) {
        auto *ability = old.add_events()->mutable_stack_pushed();
        ability->set_object_id(oid);
        ability->set_description("Voltaic Key");
        ability->set_ability_annotation("Untap target artifact.");
        ability->set_ability_controller_player_id(kLocalPlayer);
    }
    auto *physical = old.add_events()->mutable_stack_pushed();
    physical->set_object_id(902u);
    physical->set_card_id("lightning_bolt");
    physical->set_description("Lightning Bolt");
    apply(old);
    state->registerSyntheticStackCard(900u, 100900, kLocalPlayer);
    state->registerSyntheticStackCard(999u, 100999, kLocalPlayer);
    state->engineOidToCardId.insert(902u, 42);
    state->currentPriorityPlayerId = kLocalPlayer;
    host.priorityPlayer = kLocalPlayer;
    ASSERT_EQ(state->getStackOidOrder(), QList<quint32>({902u, 999u, 900u}));

    ruled::v1::RuledEventBatch snapshot;
    snapshot.set_replace_stack_presentation(true);
    snapshot.add_events()->mutable_priority_changed()->set_player_id(kOpponent);
    snapshot.add_events()->mutable_stack_pushed()->CopyFrom(*physical);
    for (quint32 oid : {900u, 901u}) {
        auto *ability = snapshot.add_events()->mutable_stack_pushed();
        ability->set_object_id(oid);
        ability->set_description("Voltaic Key");
        ability->set_ability_annotation("Untap target artifact.");
        ability->set_ability_controller_player_id(kLocalPlayer);
        ability->set_is_copy(oid == 901u);
    }
    auto *choice = snapshot.add_events()->mutable_resolution_choice_required();
    choice->set_deciding_player_id(kLocalPlayer);
    choice->set_choice_kind(ruled::v1::CHOICE_KIND_TARGET_OBJECTS);
    choice->set_prompt_text("Keep or change the target.");
    choice->set_min(0);
    choice->set_max(1);
    choice->add_candidate_object_ids(456u);
    choice->add_candidate_card_ids("sol_ring");
    choice->add_candidate_names("Sol Ring");
    apply(snapshot);
    EXPECT_EQ(state->getStackOidOrder(), QList<quint32>({901u, 900u, 902u}));
    EXPECT_TRUE(host.removedSyntheticCards.contains(999u));
    EXPECT_FALSE(host.removedSyntheticCards.contains(902u));
    EXPECT_EQ(state->cardIdForEngineOid(902u), 42);
    EXPECT_EQ(state->currentPriorityPlayerId, kOpponent);
    EXPECT_EQ(host.priorityPlayer, kOpponent);
    EXPECT_TRUE(state->isPendingChoiceCandidate(RuledClientState::ChoiceKind::CopyTarget, 456u));

    state->registerSyntheticStackCard(900u, 100900, kLocalPlayer);
    state->registerSyntheticStackCard(901u, 100901, kLocalPlayer);
    apply(snapshot);
    EXPECT_EQ(state->getStackOidOrder(), QList<quint32>({901u, 900u, 902u}));
    EXPECT_EQ(state->cardIdForEngineOid(902u), 42);
    EXPECT_TRUE(state->isPendingChoiceCandidate(RuledClientState::ChoiceKind::CopyTarget, 456u));

    state->registerSyntheticStackCard(900u, 100900, kLocalPlayer);
    state->registerSyntheticStackCard(901u, 100901, kLocalPlayer);
    ruled::v1::RuledEventBatch empty;
    empty.set_replace_stack_presentation(true);
    apply(empty);
    EXPECT_TRUE(state->getStackOidOrder().isEmpty());
    EXPECT_TRUE(state->stackTargetsByStackOid.isEmpty());
}

TEST_F(RuledClientTest, PendingSpellCastIsAuthoritativePerBatchState)
{
    ruled::v1::RuledEventBatch begun;
    auto *pending = (*begun.mutable_legal_by_player())[kLocalPlayer].mutable_pending_spell_cast();
    pending->set_transaction_id(81u);
    pending->set_reserved_object_id(901u);
    pending->set_locked_total_cost("{2}{R}");
    pending->add_eligible_restricted_mana_group_ids(17u);
    pending->mutable_announcement()->mutable_source()->set_hand_index(3u);
    pending->mutable_announcement()->set_cast_method(ruled::v1::CAST_METHOD_WITHOUT_MANA_COST);
    apply(begun);
    ASSERT_TRUE(state->pendingSpellCast.has_value());
    EXPECT_EQ(state->pendingSpellCast->transaction_id(), 81u);
    EXPECT_EQ(state->pendingSpellCast->announcement().cast_method(), ruled::v1::CAST_METHOD_WITHOUT_MANA_COST);
    EXPECT_EQ(state->pendingSpellCast->locked_total_cost(), "{2}{R}");
    EXPECT_EQ(state->eligibleRestrictedManaForPendingCast(), QSet<quint32>({17u}));

    ruled::v1::RuledEventBatch previewOnly;
    previewOnly.add_events()->mutable_attackers_preview();
    apply(previewOnly);
    ASSERT_TRUE(state->pendingSpellCast.has_value())
        << "a Servatrice-only preview without LegalActions must not retire engine state";

    ruled::v1::RuledEventBatch completed;
    (*completed.mutable_legal_by_player())[kLocalPlayer];
    apply(completed);
    EXPECT_FALSE(state->pendingSpellCast.has_value());
}

TEST(RuledPaymentTest, CommitSpellCastPreviewAndSubmissionCarryOnlyLockedTransactionPayment)
{
    RuledPayment payment;
    payment.begin();
    ruled::v1::RuledCommand commit;
    commit.mutable_commit_spell_cast()->set_transaction_id(7001u);
    auto request = payment.requestAction(commit);
    ASSERT_TRUE(request.has_commit_spell_cast());
    EXPECT_EQ(request.commit_spell_cast().transaction_id(), 7001u);

    ruled::v1::PaymentPreview response;
    response.set_transaction_id(request.transaction_id());
    response.set_revision(request.revision());
    response.set_valid(true);
    ASSERT_TRUE(payment.apply(response));
    ASSERT_TRUE(payment.payMana('R'));
    payment.writePayment(commit);
    EXPECT_EQ(commit.commit_spell_cast().transaction_id(), 7001u);
    EXPECT_EQ(commit.commit_spell_cast().payment().mana().r(), 1u);
    EXPECT_FALSE(commit.has_cast_spell());
}

TEST_F(RuledClientTest, PreparationStackUsesDatabaseIdentityInsteadOfInsetName)
{
    host.presentationCards = readPresentationCards(R"(<cockatrice_carddatabase version="4"><cards>
      <card><name>Infirmary Healer // Stream of Life</name><text>Prepared creature and inset.</text></card>
      <card><name>Stream of Life</name><text>Standalone spell.</text></card>
    </cards></cockatrice_carddatabase>)");
    ruled::v1::RuledEventBatch batch;
    auto *copy = batch.add_events()->mutable_stack_pushed();
    copy->set_object_id(900);
    copy->set_card_id("infirmary_healer_stream_of_life");
    copy->set_description("Stream of Life");
    copy->set_card_display_name("Infirmary Healer // Stream of Life");
    copy->set_ability_annotation("Stream of Life; X = 3");
    copy->set_is_copy(true);
    copy->set_is_prepare_spell(true);
    apply(batch);
    ASSERT_EQ(host.createdSyntheticCards.size(), 1);
    const auto &name = host.createdSyntheticCards[0].name;
    EXPECT_EQ(name, QStringLiteral("Infirmary Healer // Stream of Life"));
    ASSERT_TRUE(host.presentationCards.contains(name));
    EXPECT_EQ(host.presentationCards.value(name)->getText(), QStringLiteral("Prepared creature and inset."));
    EXPECT_EQ(state->stackAnnotationByOid.value(900), QStringLiteral("Stream of Life; X = 3"));
}

TEST_F(RuledClientTest, PreparationExileDisplayUsesSettledBindingsAndFullSnapshotsWithoutLocalActions)
{
    ruled::v1::RuledEventBatch batch;
    auto *view = batch.add_events()->mutable_zone_view();
    view->set_battlefields_unchanged(true);
    auto *player = view->add_per_player();
    player->set_player_id(kOpponent);
    player->add_exile_object_ids(50);
    player->add_exile_object_ids(77);
    auto *copy = player->add_prepare_spell_copies();
    copy->set_object_id(77);
    copy->set_display_name("Infirmary Healer // Stream of Life");
    // Relay mappings can follow the engine snapshot in the same batch.
    auto *entry = batch.add_events()->mutable_exile_object_map()->add_entries();
    entry->set_player_id(kOpponent);
    entry->set_engine_object_id(77);
    entry->set_server_card_id(123);
    apply(batch);
    ASSERT_EQ(host.preparationCopies.size(), 1);
    EXPECT_EQ(host.preparationCopies[0].objectId, 77u);
    EXPECT_EQ(host.preparationCopies[0].playerId, kOpponent);
    EXPECT_EQ(host.preparationCopies[0].serverCardId, 123);
    EXPECT_EQ(host.preparationCopies[0].zoneIndex, 1);
    EXPECT_EQ(host.preparationCopies[0].displayName, QStringLiteral("Infirmary Healer // Stream of Life"));
    apply(batch);
    ASSERT_EQ(host.preparationCopies.size(), 1);
    batch.mutable_events()->SwapElements(0, 1);
    apply(batch);
    ASSERT_EQ(host.preparationCopies.size(), 1); // Mapping-before-snapshot is equivalent.
    batch.mutable_events(0)->mutable_exile_object_map()->mutable_entries(0)->set_player_id(kLocalPlayer);
    apply(batch);
    EXPECT_TRUE(host.preparationCopies.isEmpty()); // Never bind an entry to another seat's card.
    batch.mutable_events(0)->mutable_exile_object_map()->mutable_entries(0)->set_player_id(kOpponent);
    apply(batch);
    ASSERT_EQ(host.preparationCopies.size(), 1);
    const int syncs = host.preparationSyncs;
    apply(ruled::v1::RuledEventBatch{});
    EXPECT_EQ(host.preparationSyncs, syncs); // No snapshot is not an empty snapshot.
    batch.mutable_events(1)->mutable_zone_view()->mutable_per_player(0)->clear_prepare_spell_copies();
    apply(batch);
    EXPECT_TRUE(host.preparationCopies.isEmpty());
}

TEST_F(RuledClientTest, PhaseChangeEmptiesTheStack)
{
    ruled::v1::RuledEventBatch push;
    push.add_events()->mutable_stack_pushed()->set_object_id(10);
    apply(push);
    ASSERT_TRUE(state->hasStackItems());

    apply(phaseBatch(ruled::v1::PHASE_ID_MAIN2, kLocalPlayer));
    EXPECT_FALSE(state->hasStackItems());
    EXPECT_TRUE(state->stackTargetsByStackOid.isEmpty());
}

// ---------------------------------------------------------------------------------------
// Phase / combat state machine
// ---------------------------------------------------------------------------------------

TEST_F(RuledClientTest, PhaseChangeMapsToToolbarSlotAndCombatPhase)
{
    apply(phaseBatch(ruled::v1::PHASE_ID_DECLARE_ATTACKERS, kLocalPlayer));
    EXPECT_EQ(host.toolbarPhase, 5);
    EXPECT_EQ(host.activePlayer, kLocalPlayer);
    EXPECT_EQ(state->getCombatPhase(), RuledClientState::RuledCombatPhase::DeclareAttackers);
    EXPECT_TRUE(state->localPlayerIsActive());
    EXPECT_FALSE(state->localPlayerIsDefender());

    // CR 510.4: assign-combat-damage shares the declare-blockers toolbar slot.
    apply(phaseBatch(ruled::v1::PHASE_ID_ASSIGN_COMBAT_DAMAGE, kLocalPlayer));
    EXPECT_EQ(host.toolbarPhase, 6);
    EXPECT_EQ(state->getCombatPhase(), RuledClientState::RuledCombatPhase::AssignCombatDamage);
}

TEST_F(RuledClientTest, FirstStrikeStepTransitionsAreAnnounced)
{
    QSignalSpy spy(state, &RuledClientState::firstStrikeDamageStepActiveChanged);
    apply(phaseBatch(ruled::v1::PHASE_ID_FIRST_STRIKE_DAMAGE, kLocalPlayer));
    EXPECT_TRUE(state->inFirstStrikeDamageStep());
    ASSERT_EQ(spy.count(), 1);
    EXPECT_TRUE(spy.at(0).at(0).toBool());

    apply(phaseBatch(ruled::v1::PHASE_ID_COMBAT_DAMAGE, kLocalPlayer));
    EXPECT_FALSE(state->inFirstStrikeDamageStep());
    ASSERT_EQ(spy.count(), 2);
    EXPECT_FALSE(spy.at(1).at(0).toBool());
}

TEST_F(RuledClientTest, OpeningPhaseSlugIsRecognised)
{
    apply(phaseBatch(ruled::v1::PHASE_ID_OPENING_MULLIGAN, kLocalPlayer));
    EXPECT_TRUE(state->engineOpeningPhaseActive());
    apply(phaseBatch(ruled::v1::PHASE_ID_UNTAP, kLocalPlayer));
    EXPECT_FALSE(state->engineOpeningPhaseActive());
}

TEST_F(RuledClientTest, AttackerStagingSyncsAPreviewAndClearsOnDeclaration)
{
    auto batch = phaseBatch(ruled::v1::PHASE_ID_DECLARE_ATTACKERS, kLocalPlayer);
    auto &actions = (*batch.mutable_legal_by_player())[kLocalPlayer];
    actions.add_selectable_attacker_ids(100);
    addLegalPlayerAttack(actions, 100);
    apply(batch);
    ASSERT_TRUE(state->localPlayerIsActive());
    host.sentCommands.clear();

    state->togglePendingAttacker(100);
    EXPECT_TRUE(state->isPendingAttacker(100));
    ASSERT_EQ(host.sentCommands.size(), 1);
    ASSERT_TRUE(host.sentCommands[0].has_preview_declare_attackers());
    EXPECT_EQ(host.sentCommands[0].preview_declare_attackers().assignments_size(), 1);

    state->togglePendingAttacker(100);
    EXPECT_FALSE(state->isPendingAttacker(100));
    ASSERT_EQ(host.sentCommands.size(), 2);
    EXPECT_EQ(host.sentCommands[1].preview_declare_attackers().assignments_size(), 0);

    ruled::v1::RuledEventBatch declared;
    auto *ad = declared.add_events()->mutable_attackers_declared();
    ad->set_attacking_player_id(kLocalPlayer);
    *ad->add_assignments() = playerAttackAssignment(100);
    apply(declared);
    EXPECT_TRUE(state->isCurrentAttacker(100));
    EXPECT_TRUE(state->getPendingAttackerOids().isEmpty());
    EXPECT_TRUE(state->hasAttackersSubmittedThisStep());
    // The declare step is over for us: no longer "active" for combat-control purposes.
    EXPECT_FALSE(state->localPlayerIsActive());
}

TEST_F(RuledClientTest, ConfirmAttackersIsGatedOnMustAttackRequirements)
{
    ruled::v1::RuledEventBatch batch = phaseBatch(ruled::v1::PHASE_ID_DECLARE_ATTACKERS, kLocalPlayer);
    auto &actions = (*batch.mutable_legal_by_player())[kLocalPlayer];
    actions.add_attack_requirement_ids(100); // CR 508.1d "attacks if able"
    actions.set_minimum_attack_requirement_count(1);
    actions.add_selectable_attacker_ids(100);
    addLegalPlayerAttack(actions, 100);
    apply(batch);

    EXPECT_FALSE(state->combatDeclarationSatisfied());
    state->togglePendingAttacker(100);
    EXPECT_TRUE(state->combatDeclarationSatisfied());
}

TEST_F(RuledClientTest, MultipleAttackDefendersRequireAnAuthoritativeDestinationBeforeConfirm)
{
    auto batch = phaseBatch(ruled::v1::PHASE_ID_DECLARE_ATTACKERS, kLocalPlayer);
    auto &actions = (*batch.mutable_legal_by_player())[kLocalPlayer];
    actions.add_selectable_attacker_ids(100);
    *actions.add_legal_attack_assignments() = playerAttackAssignment(100);
    *actions.add_legal_attack_assignments() = permanentAttackAssignment(100, 500, 9);
    apply(batch);
    host.sentCommands.clear();

    state->togglePendingAttacker(100);
    EXPECT_TRUE(state->isPendingAttacker(100));
    EXPECT_TRUE(state->isChoosingAttackDefender());
    EXPECT_FALSE(state->combatDeclarationSatisfied());
    EXPECT_TRUE(state->isLegalAttackPlayerDefender(kOpponent));
    EXPECT_TRUE(state->isLegalAttackPermanentDefender(500));

    ASSERT_TRUE(state->chooseAttackPermanentDefender(500));
    EXPECT_FALSE(state->isChoosingAttackDefender());
    EXPECT_TRUE(state->combatDeclarationSatisfied());
    ASSERT_FALSE(host.sentCommands.isEmpty());
    const auto &preview = host.sentCommands.constLast().preview_declare_attackers();
    ASSERT_EQ(preview.assignments_size(), 1);
    EXPECT_EQ(preview.assignments(0).defender().object_id(), 500u);
    EXPECT_EQ(preview.assignments(0).defender_zone_change_generation(), 9u);

    host.sentCommands.clear();
    state->confirmAttackers();
    ASSERT_EQ(host.sentCommands.size(), 1);
    ASSERT_EQ(host.sentCommands[0].declare_attackers().assignments_size(), 1);
    EXPECT_EQ(host.sentCommands[0].declare_attackers().assignments(0).defender().object_id(), 500u);
    EXPECT_TRUE(state->hasAttackersSubmittedThisStep());
    EXPECT_TRUE(state->isPendingAttacker(100));

    host.answerPendingAck(false);
    EXPECT_FALSE(state->hasAttackersSubmittedThisStep());
    EXPECT_TRUE(state->isPendingAttacker(100));

    state->confirmAttackers();
    host.answerPendingAck(true);
    EXPECT_TRUE(state->hasAttackersSubmittedThisStep());
    EXPECT_TRUE(state->isPendingAttacker(100)); // The engine's committed event clears the draft.

    ruled::v1::RuledEventBatch committed;
    *committed.add_events()->mutable_attackers_declared()->add_assignments() =
        permanentAttackAssignment(100, 500, 9);
    apply(committed);
    EXPECT_FALSE(state->isPendingAttacker(100));
}

TEST_F(RuledClientTest, PendingAttackPaymentComesFromLocalLegalActionsAndCancellationKeepsDraft)
{
    auto batch = phaseBatch(ruled::v1::PHASE_ID_DECLARE_ATTACKERS, kLocalPlayer);
    auto &actions = (*batch.mutable_legal_by_player())[kLocalPlayer];
    actions.add_selectable_attacker_ids(100);
    addLegalPlayerAttack(actions, 100);
    apply(batch);
    state->togglePendingAttacker(100);

    batch = phaseBatch(ruled::v1::PHASE_ID_DECLARE_ATTACKERS, kLocalPlayer);
    auto &pendingActions = (*batch.mutable_legal_by_player())[kLocalPlayer];
    pendingActions.add_selectable_attacker_ids(100);
    addLegalPlayerAttack(pendingActions, 100);
    auto *pending = pendingActions.mutable_pending_attack_declaration();
    pending->set_transaction_id(81);
    pending->set_revision(4);
    pending->set_attacking_player_id(kLocalPlayer);
    *pending->add_assignments() = playerAttackAssignment(100);
    apply(batch);
    ASSERT_TRUE(state->pendingAttackDeclaration.has_value());
    EXPECT_EQ(state->pendingAttackDeclaration->transaction_id(), 81u);

    state->finishAttackDeclarationCancellation(81);
    EXPECT_FALSE(state->pendingAttackDeclaration.has_value());
    EXPECT_TRUE(state->isPendingAttacker(100));

    auto opponentOnly = phaseBatch(ruled::v1::PHASE_ID_DECLARE_ATTACKERS, kLocalPlayer);
    auto *opponent = (*opponentOnly.mutable_legal_by_player())[kOpponent].mutable_pending_attack_declaration();
    opponent->set_transaction_id(82);
    apply(opponentOnly);
    EXPECT_FALSE(state->pendingAttackDeclaration.has_value());
}

TEST_F(RuledClientTest, MobilizeDefenderChoiceUsesStructuredEngineOptionsAndRestoresAfterRejection)
{
    ruled::v1::RuledEventBatch batch;
    auto *choice = batch.add_events()->mutable_resolution_choice_required();
    choice->set_deciding_player_id(kLocalPlayer);
    choice->set_choice_kind(ruled::v1::CHOICE_KIND_ATTACKING_TOKEN_DEFENDER);
    choice->set_min(1);
    choice->set_max(1);
    choice->set_prompt_text("Choose what the Warrior token attacks.");

    auto *player = choice->add_combat_defender_options();
    player->mutable_defender()->set_kind(ruled::v1::TARGET_REF_KIND_PLAYER);
    player->mutable_defender()->set_object_id(kOpponent);
    player->set_defending_player_id(kOpponent);

    auto *planeswalker = choice->add_combat_defender_options();
    planeswalker->mutable_defender()->set_kind(ruled::v1::TARGET_REF_KIND_PERMANENT);
    planeswalker->mutable_defender()->set_object_id(500);
    planeswalker->set_defender_zone_change_generation(9);
    planeswalker->set_defending_player_id(kOpponent);

    apply(batch);
    ASSERT_TRUE(state->hasPendingChoiceOfKind(RuledClientState::ChoiceKind::AttackingTokenDefender));
    EXPECT_TRUE(state->isLegalAttackPlayerDefender(kOpponent));
    EXPECT_TRUE(state->isLegalAttackPermanentDefender(500));
    EXPECT_FALSE(state->isLegalAttackPermanentDefender(501));

    host.sentCommands.clear();
    ASSERT_TRUE(state->chooseAttackPermanentDefender(500));
    ASSERT_EQ(host.sentCommands.size(), 1);
    const auto &submission = host.sentCommands.constFirst().submit_resolution_choice();
    ASSERT_TRUE(submission.has_chosen_combat_defender());
    EXPECT_EQ(submission.chosen_combat_defender().defender().kind(), ruled::v1::TARGET_REF_KIND_PERMANENT);
    EXPECT_EQ(submission.chosen_combat_defender().defender().object_id(), 500u);
    EXPECT_EQ(submission.chosen_combat_defender().defender_zone_change_generation(), 9u);
    EXPECT_FALSE(state->hasPendingChoiceOfKind(RuledClientState::ChoiceKind::AttackingTokenDefender));

    host.answerPendingAck(false);
    EXPECT_TRUE(state->hasPendingChoiceOfKind(RuledClientState::ChoiceKind::AttackingTokenDefender));
    EXPECT_TRUE(state->isLegalAttackPlayerDefender(kOpponent));
}

TEST_F(RuledClientTest, MobilizeAttackersAppendToTheDeclaredCombatCohort)
{
    ruled::v1::RuledEventBatch declared;
    auto *attackers = declared.add_events()->mutable_attackers_declared();
    attackers->set_attacking_player_id(kLocalPlayer);
    *attackers->add_assignments() = playerAttackAssignment(100);
    apply(declared);

    ruled::v1::RuledEventBatch mobilized;
    auto *added = mobilized.add_events()->mutable_attackers_added();
    *added->add_assignments() = permanentAttackAssignment(200, 500, 9);
    apply(mobilized);

    EXPECT_EQ(state->getCurrentAttackerOids(), QSet<quint32>({100u, 200u}));
    ASSERT_TRUE(state->getCurrentAttackAssignments().contains(200));
    EXPECT_EQ(state->getCurrentAttackAssignments().value(200).defender().object_id(), 500u);
    EXPECT_EQ(state->getCurrentAttackAssignments().value(200).defender_zone_change_generation(), 9u);
}

TEST_F(RuledClientTest, BlockerStagingPairsToAnAttackerAndSyncsAPreview)
{
    ruled::v1::RuledEventBatch declared;
    auto *ad = declared.add_events()->mutable_attackers_declared();
    *ad->add_assignments() = playerAttackAssignment(100, kLocalPlayer);
    *ad->add_assignments() = playerAttackAssignment(101, kLocalPlayer); // Only blocker 200 can block this attacker.
    *ad->add_assignments() =
        playerAttackAssignment(102, kLocalPlayer); // Unblockable: no legal pair targets this attacker.
    apply(declared);
    // The opponent is the active player during our declare-blockers step.
    auto batch = phaseBatch(ruled::v1::PHASE_ID_DECLARE_BLOCKERS, kOpponent);
    batch.add_events()->mutable_priority_changed()->set_player_id(kLocalPlayer);
    auto &actions = (*batch.mutable_legal_by_player())[kLocalPlayer];
    auto *firstPair = actions.add_legal_block_pairs();
    firstPair->set_blocker_id(200);
    firstPair->set_attacker_id(100);
    auto *secondPair = actions.add_legal_block_pairs();
    secondPair->set_blocker_id(201);
    secondPair->set_attacker_id(100);
    auto *thirdPair = actions.add_legal_block_pairs();
    thirdPair->set_blocker_id(200);
    thirdPair->set_attacker_id(101);
    apply(batch);
    ASSERT_TRUE(state->localPlayerIsDefender());
    host.sentCommands.clear();

    state->toggleStagedBlocker(200);
    EXPECT_TRUE(state->hasStagedBlocker());
    EXPECT_TRUE(state->isStagedBlocker(200));

    // No staged blocker has an edge to the unblockable attacker.
    state->pairStagedBlockerToAttacker(102);
    EXPECT_TRUE(state->hasStagedBlocker());
    EXPECT_EQ(state->pendingBlockTargetForBlocker(200), 0u);
    EXPECT_TRUE(host.sentCommands.isEmpty());

    // Pairing multiple staged blockers is all-or-nothing: blocker 200 can block attacker 101,
    // but blocker 201 cannot, so neither blocker is moved and no preview is emitted.
    state->toggleStagedBlocker(201);
    state->pairStagedBlockerToAttacker(101);
    EXPECT_TRUE(state->isStagedBlocker(200));
    EXPECT_TRUE(state->isStagedBlocker(201));
    EXPECT_EQ(state->pendingBlockTargetForBlocker(200), 0u);
    EXPECT_EQ(state->pendingBlockTargetForBlocker(201), 0u);
    EXPECT_TRUE(host.sentCommands.isEmpty());

    state->pairStagedBlockerToAttacker(100);
    EXPECT_FALSE(state->hasStagedBlocker());
    EXPECT_EQ(state->pendingBlockTargetForBlocker(200), 100u);
    EXPECT_EQ(state->pendingBlockTargetForBlocker(201), 100u);
    ASSERT_EQ(host.sentCommands.size(), 1);
    ASSERT_TRUE(host.sentCommands[0].has_preview_declare_blockers());
    ASSERT_EQ(host.sentCommands[0].preview_declare_blockers().block_pairs_size(), 2);

    // Pairing to a creature that is not a declared attacker is a no-op.
    state->toggleStagedBlocker(200);
    state->pairStagedBlockerToAttacker(999);
    EXPECT_TRUE(state->isStagedBlocker(200));
}

TEST_F(RuledClientTest, RejectedBlockDeclarationRollsBackTheLocalGuard)
{
    ruled::v1::RuledEventBatch declared;
    *declared.add_events()->mutable_attackers_declared()->add_assignments() = playerAttackAssignment(100, kLocalPlayer);
    apply(declared);
    auto batch = phaseBatch(ruled::v1::PHASE_ID_DECLARE_BLOCKERS, kOpponent);
    batch.add_events()->mutable_priority_changed()->set_player_id(kLocalPlayer);
    addLegalBlockPair((*batch.mutable_legal_by_player())[kLocalPlayer], 200, 100);
    apply(batch);
    state->toggleStagedBlocker(200);
    state->pairStagedBlockerToAttacker(100);
    host.sentCommands.clear();

    QSignalSpy rejectedSpy(state, &RuledClientState::blockerRejected);
    state->confirmBlockers();
    ASSERT_EQ(host.sentCommands.size(), 1);
    EXPECT_TRUE(host.sentCommands[0].has_declare_blockers());
    // Optimistically committed while the round-trip is in flight.
    EXPECT_TRUE(state->hasBlockersSubmittedThisStep());
    EXPECT_EQ(state->getCommittedBlocks().value(200), 100u);

    host.answerPendingAck(false);
    EXPECT_EQ(rejectedSpy.count(), 1);
    EXPECT_FALSE(state->hasBlockersSubmittedThisStep());
    // The staged pairs come back so the defender can fix and resubmit.
    EXPECT_EQ(state->getPendingBlocks().value(200), 100u);
    EXPECT_TRUE(state->getCommittedBlocks().isEmpty());
}

TEST_F(RuledClientTest, AppliesAndClearsObjectIdKeyedExileCastActions)
{
    constexpr quint32 objectId = 0x01020304u;
    ruled::v1::RuledEventBatch batch;
    auto &actions = (*batch.mutable_legal_by_player())[kLocalPlayer];
    auto *cast = actions.add_zone_cast_actions();
    cast->set_source_zone(ruled::v1::CAST_SOURCE_ZONE_EXILE);
    cast->set_cast_method(ruled::v1::CAST_METHOD_NORMAL);
    cast->set_object_id(objectId);
    cast->set_face_index(0);
    cast->set_card_name("Bonecrusher Giant");
    cast->set_cost("{2}{R}");
    cast->set_needs_target(true);
    auto &targets = (*actions.mutable_valid_targets_by_zone_object())[(quint64(objectId) << 8) | 0u];
    auto *zoneGroup = targets.add_groups();
    zoneGroup->set_group_index(0);
    zoneGroup->set_prompt_text("Choose target");
    zoneGroup->set_min(1);
    zoneGroup->set_max(1);
    zoneGroup->add_valid_permanent_ids(99);
    apply(batch);

    ASSERT_TRUE(state->isZoneActionLegal(objectId, RuledCastSource::Exile));
    ASSERT_EQ(state->zoneActionSource(objectId), RuledCastSource::Exile);
    ASSERT_EQ(state->zoneActionCost(objectId, 0, RuledCastSource::Exile), QStringLiteral("{2}{R}"));
    ASSERT_EQ(state->zoneActionFaceOptions(objectId, RuledCastSource::Exile).size(), 1);
    EXPECT_EQ(state->zoneActionFaceOptions(objectId, RuledCastSource::Exile).first().faceName,
              QStringLiteral("Bonecrusher Giant"));
    EXPECT_TRUE(state->isValidSpellTarget(static_cast<int>(objectId), 0, 99, RuledCastSource::Exile));

    apply(phaseBatch(ruled::v1::PHASE_ID_MAIN1, kLocalPlayer));
    EXPECT_FALSE(state->isZoneActionLegal(objectId, RuledCastSource::Exile));
    EXPECT_FALSE(state->isValidSpellTarget(static_cast<int>(objectId), 0, 99, RuledCastSource::Exile));
}

TEST_F(RuledClientTest, CombatStagingIgnoresCreaturesOutsideEngineSelectableSets)
{
    auto attackers = phaseBatch(ruled::v1::PHASE_ID_DECLARE_ATTACKERS, kLocalPlayer);
    auto &attackerActions = (*attackers.mutable_legal_by_player())[kLocalPlayer];
    attackerActions.add_selectable_attacker_ids(100);
    addLegalPlayerAttack(attackerActions, 100);
    apply(attackers);
    host.sentCommands.clear();

    state->togglePendingAttacker(200);
    EXPECT_FALSE(state->isPendingAttacker(200));
    EXPECT_TRUE(host.sentCommands.isEmpty());
    state->togglePendingAttacker(100);
    EXPECT_TRUE(state->isPendingAttacker(100));

    ruled::v1::RuledEventBatch declared;
    *declared.add_events()->mutable_attackers_declared()->add_assignments() = playerAttackAssignment(300, kLocalPlayer);
    apply(declared);
    auto blockers = phaseBatch(ruled::v1::PHASE_ID_DECLARE_BLOCKERS, kOpponent);
    blockers.add_events()->mutable_priority_changed()->set_player_id(kLocalPlayer);
    addLegalBlockPair((*blockers.mutable_legal_by_player())[kLocalPlayer], 400, 300);
    apply(blockers);

    state->toggleStagedBlocker(500);
    EXPECT_FALSE(state->isStagedBlocker(500));
    state->toggleStagedBlocker(400);
    EXPECT_TRUE(state->isStagedBlocker(400));
}

TEST_F(RuledClientTest, RemovedFromCombatPrunesAttackersAndBlockPairs)
{
    ruled::v1::RuledEventBatch setup;
    auto *ad = setup.add_events()->mutable_attackers_declared();
    *ad->add_assignments() = playerAttackAssignment(100);
    *ad->add_assignments() = playerAttackAssignment(101);
    auto *bd = setup.add_events()->mutable_blockers_declared();
    auto *pair = bd->add_block_pairs();
    pair->set_attacker_id(100);
    pair->set_blocker_id(200);
    apply(setup);
    ASSERT_TRUE(state->isCurrentAttacker(100));
    ASSERT_TRUE(state->getCurrentAttackAssignments().contains(101));
    ASSERT_EQ(state->getCommittedBlocks().value(200), 100u);

    // CR 701.19a: regeneration removes the blocker from combat.
    ruled::v1::RuledEventBatch removed;
    removed.add_events()->mutable_removed_from_combat()->add_object_ids(200);
    apply(removed);
    EXPECT_TRUE(state->getCommittedBlocks().isEmpty());
    EXPECT_FALSE(state->committedBlockerGroups.contains(100));
    EXPECT_TRUE(state->isCurrentAttacker(100));

    ruled::v1::RuledEventBatch removedAttacker;
    removedAttacker.add_events()->mutable_removed_from_combat()->add_object_ids(101);
    apply(removedAttacker);
    EXPECT_FALSE(state->isCurrentAttacker(101));
    // Arrow rendering consumes assignments, so removal must prune both mirrors.
    EXPECT_FALSE(state->getCurrentAttackAssignments().contains(101));
    EXPECT_TRUE(state->getCurrentAttackAssignments().contains(100));
}

TEST(RuledPaymentTest, AttackDeclarationPreviewAndCommitCarryExactLockedPayment)
{
    RuledPayment payment;
    payment.begin(true);
    ruled::v1::RuledCommand command;
    auto *commit = command.mutable_commit_attack_declaration();
    commit->set_transaction_id(91);
    commit->set_expected_revision(7);

    auto request = payment.requestAction(command);
    ASSERT_TRUE(request.has_commit_attack_declaration());
    EXPECT_EQ(request.commit_attack_declaration().transaction_id(), 91u);
    EXPECT_EQ(request.commit_attack_declaration().expected_revision(), 7u);

    ASSERT_TRUE(payment.payMana('G'));
    request = payment.requestAction(command);
    ASSERT_TRUE(request.has_commit_attack_declaration());
    EXPECT_EQ(request.commit_attack_declaration().payment().mana().g(), 1u);

    ruled::v1::PaymentPreview response;
    response.set_transaction_id(request.transaction_id());
    response.set_revision(request.revision());
    response.set_valid(true);
    response.set_complete(true);
    *response.mutable_selection() = request.commit_attack_declaration().payment();
    ASSERT_TRUE(payment.apply(response));
    ASSERT_TRUE(payment.beginSubmission());

    payment.writePayment(command);
    EXPECT_EQ(command.commit_attack_declaration().transaction_id(), 91u);
    EXPECT_EQ(command.commit_attack_declaration().expected_revision(), 7u);
    EXPECT_EQ(command.commit_attack_declaration().payment().mana().g(), 1u);
}

TEST(RuledPaymentTest, AttackManaUndoCommandUsesTheSelectedReversibleReceipt)
{
    ruled::v1::PendingAttackDeclaration pending;
    pending.set_transaction_id(92);
    auto *older = pending.add_mana_ability_undo_options();
    older->set_activation_command_index(17);
    older->set_reversible(true);
    auto *blocked = pending.add_mana_ability_undo_options();
    blocked->set_activation_command_index(18);
    blocked->set_reversible(false);
    auto *newer = pending.add_mana_ability_undo_options();
    newer->set_activation_command_index(19);
    newer->set_reversible(true);

    const auto command = RuledPayment::undoManaAbilityCommand(&pending, 17);
    ASSERT_TRUE(command.has_value());
    ASSERT_TRUE(command->has_undo_mana_ability());
    EXPECT_EQ(command->undo_mana_ability().attack_transaction_id(), 92u);
    EXPECT_EQ(command->undo_mana_ability().activation_command_index(), 17u);
    EXPECT_TRUE(RuledPayment::undoManaAbilityCommand(&pending, 17, 92).has_value());
    EXPECT_FALSE(RuledPayment::undoManaAbilityCommand(&pending, 17, 93).has_value());
    EXPECT_FALSE(RuledPayment::undoManaAbilityCommand(nullptr, 17, 92).has_value());
    EXPECT_FALSE(RuledPayment::undoManaAbilityCommand(&pending, 18).has_value());
    const auto newest = RuledPayment::undoManaAbilityCommand(&pending);
    ASSERT_TRUE(newest.has_value());
    EXPECT_EQ(newest->undo_mana_ability().activation_command_index(), 19u);

    pending.clear_mana_ability_undo_options();
    pending.add_mana_ability_undo_options()->set_reversible(false);
    EXPECT_FALSE(RuledPayment::undoManaAbilityCommand(&pending).has_value());

    const auto ordinary = RuledPayment::undoManaAbilityCommand(nullptr);
    ASSERT_TRUE(ordinary.has_value());
    EXPECT_TRUE(ordinary->has_undo_mana_ability());
    EXPECT_EQ(ordinary->undo_mana_ability().attack_transaction_id(), 0u);
}

TEST(RuledPendingTargetTest, PairedAbilityTargetsReconcileChangedEdgesWithoutObjectRemoval)
{
    FakeHost host;
    RuledClientState state(&host);
    RuledSpellTargetData data;
    data.groups.resize(2);
    data.groups[0].validPermanentIds = {40, 41};
    auto &second = data.groups[1];
    second.groupIndex = 1;
    second.validGraveyardIds = {50, 51};
    auto &constraint = second.pairConstraint.emplace();
    constraint.set_prior_group_index(0);
    auto *pair = constraint.add_compatible_pairs();
    pair->mutable_prior_target()->set_kind(ruled::v1::TARGET_REF_KIND_PERMANENT);
    pair->mutable_prior_target()->set_object_id(40);
    pair->mutable_candidate()->set_kind(ruled::v1::TARGET_REF_KIND_GRAVEYARD);
    pair->mutable_candidate()->set_object_id(50);
    state.validTargetsByAbility.insert(RuledClientState::abilityTargetKey(30, 1), data);
    PendingActivatedAbility ability;
    ability.valid = true;
    ability.waitingForTarget = true;
    ability.permanentOid = 30;
    ability.abilityIndex = 1;
    ability.activeTargetGroupPosition = 1;
    auto &prior = ability.selectedTargets.emplaceBack();
    prior.ref.set_kind(ruled::v1::TARGET_REF_KIND_PERMANENT);
    prior.ref.set_object_id(40);
    EXPECT_EQ(ruledTargetClickEligibility({}, ability, state, RuledTargetCandidateKind::Graveyard, 50, kLocalPlayer),
              RuledTargetClickEligibility::Legal);
    EXPECT_EQ(ruledTargetClickEligibility({}, ability, state, RuledTargetCandidateKind::Graveyard, 51, kLocalPlayer),
              RuledTargetClickEligibility::Illegal);
    auto &candidate = ability.selectedTargets.emplaceBack();
    candidate.ref.set_kind(ruled::v1::TARGET_REF_KIND_GRAVEYARD);
    candidate.ref.set_object_id(50);
    candidate.ref.set_group_index(1);
    ability.waitingForTarget = false;
    ability.waitingForMana = true;
    // Both objects remain candidates; the authoritative relation changed.
    data.groups[1].pairConstraint->mutable_compatible_pairs(0)->mutable_prior_target()->set_object_id(41);
    state.validTargetsByAbility.insert(RuledClientState::abilityTargetKey(30, 1), data);
    PendingRuledSpellCast spell;
    EXPECT_TRUE(reconcileRuledPendingTargets(spell, ability, state, kLocalPlayer));
    ASSERT_EQ(ability.selectedTargets.size(), 1);
    EXPECT_EQ(ability.selectedTargets.first().ref.object_id(), 40u);
    EXPECT_EQ(ability.activeTargetGroupPosition, 1);
    EXPECT_TRUE(ability.waitingForTarget);
    EXPECT_FALSE(ability.waitingForMana);
    EXPECT_EQ(ruledTargetClickEligibility({}, ability, state, RuledTargetCandidateKind::Graveyard, 50, kLocalPlayer),
              RuledTargetClickEligibility::Illegal);
}

TEST(RuledPendingTargetTest, PairCompatibilityRequiresOneEarlierTypedSelection)
{
    RuledTargetGroupData group;
    group.groupIndex = 7;
    auto &constraint = group.pairConstraint.emplace();
    constraint.set_prior_group_index(3);
    auto *pair = constraint.add_compatible_pairs();
    pair->mutable_prior_target()->set_kind(ruled::v1::TARGET_REF_KIND_PERMANENT);
    pair->mutable_prior_target()->set_object_id(40);
    pair->mutable_candidate()->set_kind(ruled::v1::TARGET_REF_KIND_GRAVEYARD);
    pair->mutable_candidate()->set_object_id(50);
    QVector<ruled::v1::TargetRef> selected;
    EXPECT_FALSE(ruledTargetPairCompatible(group, ruled::v1::TARGET_REF_KIND_GRAVEYARD, 50, selected));
    auto &target = selected.emplaceBack();
    target.set_group_index(3);
    target.set_object_id(40);
    target.set_kind(ruled::v1::TARGET_REF_KIND_PERMANENT);
    EXPECT_TRUE(ruledTargetPairCompatible(group, ruled::v1::TARGET_REF_KIND_GRAVEYARD, 50, selected));
    EXPECT_FALSE(ruledTargetPairCompatible(group, ruled::v1::TARGET_REF_KIND_PERMANENT, 50, selected));
    target.set_kind(ruled::v1::TARGET_REF_KIND_STACK);
    EXPECT_FALSE(ruledTargetPairCompatible(group, ruled::v1::TARGET_REF_KIND_GRAVEYARD, 50, selected));
    target.set_kind(ruled::v1::TARGET_REF_KIND_PERMANENT);
    selected.append(target);
    EXPECT_FALSE(ruledTargetPairCompatible(group, ruled::v1::TARGET_REF_KIND_GRAVEYARD, 50, selected));
    selected.removeLast();
    constraint.set_prior_group_index(7);
    EXPECT_FALSE(ruledTargetPairCompatible(group, ruled::v1::TARGET_REF_KIND_GRAVEYARD, 50, selected));
}

TEST(RuledPendingTargetTest, PairedSpellTargetsPruneChangedEdgesAndReopenTheDependentGroup)
{
    FakeHost host;
    RuledClientState state(&host);
    RuledSpellTargetData data;
    data.groups.resize(2);
    data.groups[0].validPermanentIds = {40};
    auto &second = data.groups[1];
    second.groupIndex = 1;
    second.validGraveyardIds = {50, 51};
    auto &constraint = second.pairConstraint.emplace();
    auto *pair = constraint.add_compatible_pairs();
    pair->mutable_prior_target()->set_kind(ruled::v1::TARGET_REF_KIND_PERMANENT);
    pair->mutable_prior_target()->set_object_id(40);
    pair->mutable_candidate()->set_kind(ruled::v1::TARGET_REF_KIND_GRAVEYARD);
    pair->mutable_candidate()->set_object_id(50);
    state.validTargetsByHandSlot.insert(RuledClientState::spellTargetKey(6, 0), data);
    PendingRuledSpellCast spell;
    spell.valid = true;
    spell.waitingForTarget = true;
    spell.handIndex = 6;
    spell.activeTargetGroupPosition = 1;
    spell.selectedTargetOidsByGroup = {{40}, {}};
    EXPECT_EQ(ruledTargetClickEligibility(spell, {}, state, RuledTargetCandidateKind::Graveyard, 50, kLocalPlayer),
              RuledTargetClickEligibility::Legal);
    EXPECT_EQ(ruledTargetClickEligibility(spell, {}, state, RuledTargetCandidateKind::Graveyard, 51, kLocalPlayer),
              RuledTargetClickEligibility::Illegal);
    spell.selectedTargetOids = {50};
    spell.selectedTargetOidsByGroup[1] = {50};
    spell.selectedTargetDamages = {3};
    spell.selectedTargetDamagesByGroup = {{0}, {3}};
    spell.targetDamageAllocations = {3};
    data.groups[1].pairConstraint->clear_compatible_pairs();
    state.validTargetsByHandSlot.insert(RuledClientState::spellTargetKey(6, 0), data);
    PendingActivatedAbility ability;
    EXPECT_TRUE(reconcileRuledPendingTargets(spell, ability, state, kLocalPlayer));
    EXPECT_EQ(spell.selectedTargetOidsByGroup[0], QVector<quint32>({40}));
    EXPECT_TRUE(spell.selectedTargetOidsByGroup[1].isEmpty());
    EXPECT_TRUE(spell.selectedTargetOids.isEmpty());
    EXPECT_TRUE(spell.selectedTargetDamages.isEmpty());
    EXPECT_TRUE(spell.targetDamageAllocations.isEmpty());
    EXPECT_EQ(spell.activeTargetGroupPosition, 1);
    EXPECT_TRUE(spell.waitingForTarget);
}

TEST_F(RuledClientTest, StalePairsArePrunedWhenPermanentsLeaveTheBattlefield)
{
    ruled::v1::RuledEventBatch setup;
    auto *bd = setup.add_events()->mutable_blockers_declared();
    auto *pair = bd->add_block_pairs();
    pair->set_attacker_id(100);
    pair->set_blocker_id(200);
    apply(setup);
    ASSERT_EQ(state->getCommittedBlocks().value(200), 100u);

    // A battlefield map that no longer lists the blocker prunes the pair.
    ruled::v1::RuledEventBatch mapOnly;
    addPermanent(mapOnly.add_events(), kLocalPlayer, 100, 7);
    apply(mapOnly);
    EXPECT_TRUE(state->getCommittedBlocks().isEmpty());
}

// ---------------------------------------------------------------------------------------
// Combat damage assignment (CR 510.1a-d, 702.19)
// ---------------------------------------------------------------------------------------

TEST_F(RuledClientTest, MultipleBlockersQueueAnAssignmentSeededLethalFirst)
{
    apply(phaseBatch(ruled::v1::PHASE_ID_DECLARE_BLOCKERS, kLocalPlayer));
    ruled::v1::RuledEventBatch batch;
    // A 5/5 attacker blocked by a 2/2 and a 3/3.
    auto *ev = batch.add_events();
    addPermanent(ev, kLocalPlayer, 100, 1);
    addPermanent(ev, kOpponent, 200, 2);
    addPermanent(ev, kOpponent, 201, 3);
    auto *view = batch.add_events()->mutable_zone_view()->add_per_player();
    view->set_player_id(kLocalPlayer);
    for (const auto &[oid, power, toughness] :
         std::initializer_list<std::tuple<quint32, quint32, quint32>>{{100, 5, 5}, {200, 2, 2}, {201, 3, 3}}) {
        auto *object = view->add_battlefield_objects();
        object->set_object_id(oid);
        object->set_power(power);
        object->set_toughness(toughness);
    }
    auto *bd = batch.add_events()->mutable_blockers_declared();
    for (const quint32 blocker : {200u, 201u}) {
        auto *pair = bd->add_block_pairs();
        pair->set_attacker_id(100);
        pair->set_blocker_id(blocker);
    }
    apply(batch);

    EXPECT_EQ(state->currentCombatDamageAttackerOid(), 100u);
    EXPECT_EQ(state->currentCombatDamageAttackerPower(), 5);
    // Greedy lethal-first: 2 to the 2/2, the remaining 3 to the last blocker.
    EXPECT_EQ(state->assignedCombatDamageForBlocker(200), 2u);
    EXPECT_EQ(state->assignedCombatDamageForBlocker(201), 3u);
    EXPECT_EQ(state->localCombatDamageAssignedTotal(), 5);
    EXPECT_TRUE(state->localCombatDamageAssignmentLegal());

    // Reducing below the attacker's power makes it illegal again (no trample here).
    state->bumpBlockerCombatDamage(201, -1);
    EXPECT_EQ(state->localCombatDamageAssignedTotal(), 4);
    EXPECT_FALSE(state->localCombatDamageAssignmentLegal());
    // Bumping never exceeds the attacker's power.
    state->bumpBlockerCombatDamage(201, +99);
    EXPECT_EQ(state->assignedCombatDamageForBlocker(201), 5u);
}

TEST_F(RuledClientTest, TrampleAssignsLethalToBlockerAndTheRemainderToThePlayer)
{
    apply(phaseBatch(ruled::v1::PHASE_ID_DECLARE_BLOCKERS, kLocalPlayer));
    ruled::v1::RuledEventBatch batch;
    auto *ev = batch.add_events();
    addPermanent(ev, kLocalPlayer, 100, 1)->add_keywords("Trample"); // CR 702.19
    addPermanent(ev, kOpponent, 200, 2);
    auto *view = batch.add_events()->mutable_zone_view()->add_per_player();
    view->set_player_id(kLocalPlayer);
    auto *attackerView = view->add_battlefield_objects();
    attackerView->set_object_id(100);
    attackerView->set_power(6);
    attackerView->set_toughness(6);
    auto *blockerView = view->add_battlefield_objects();
    blockerView->set_object_id(200);
    blockerView->set_power(1);
    blockerView->set_toughness(2);
    auto *pair = batch.add_events()->mutable_blockers_declared()->add_block_pairs();
    pair->set_attacker_id(100);
    pair->set_blocker_id(200);
    apply(batch);

    // A single blocker still needs an explicit assignment when the attacker tramples.
    EXPECT_EQ(state->currentCombatDamageAttackerOid(), 100u);
    EXPECT_EQ(state->assignedCombatDamageForBlocker(200), 2u);
    EXPECT_EQ(state->localCombatDamagePlayerDamage(), 4);
    EXPECT_TRUE(state->localCombatDamageAssignmentLegal());

    host.sentCommands.clear();
    state->confirmCombatDamageForCurrentAttacker();
    ASSERT_EQ(host.sentCommands.size(), 1);
    const auto &acd = host.sentCommands[0].assign_combat_damage();
    EXPECT_EQ(acd.attacker_id(), 100u);
    ASSERT_EQ(acd.assignments_size(), 1);
    EXPECT_EQ(acd.assignments(0).blocker_id(), 200u);
    EXPECT_EQ(acd.assignments(0).damage(), 2u);
    EXPECT_EQ(acd.defending_player_damage(), 4u);

    // Assigning less than lethal to the blocker is illegal under CR 702.19.
    state->bumpBlockerCombatDamage(200, -1);
    EXPECT_FALSE(state->localCombatDamageAssignmentLegal());
}

TEST_F(RuledClientTest, CombatPowerFallsBackToTheHostWhenZoneViewIsAbsent)
{
    host.fallbackPt.insert(100, {4, 4});
    EXPECT_EQ(state->combatPowerForCreatureOid(100), 4);
    EXPECT_EQ(state->combatToughnessForCreatureOid(100), 4);
    // Nothing known at all: power 0, toughness 1 (the documented floor).
    EXPECT_EQ(state->combatPowerForCreatureOid(999), 0);
    EXPECT_EQ(state->combatToughnessForCreatureOid(999), 1);
}

// ---------------------------------------------------------------------------------------
// Zone view: marked damage, activated abilities, first-strike flag
// ---------------------------------------------------------------------------------------

TEST_F(RuledClientTest, ZoneViewProjectsLoyaltyDefenseAndBattleProtector)
{
    ruled::v1::RuledEventBatch batch;
    auto *view = batch.add_events()->mutable_zone_view()->add_per_player();
    view->set_player_id(kLocalPlayer);
    auto *planeswalker = view->add_battlefield_objects();
    planeswalker->set_object_id(100);
    planeswalker->set_is_planeswalker(true);
    planeswalker->set_loyalty(4);
    auto *battle = view->add_battlefield_objects();
    battle->set_object_id(200);
    battle->set_is_battle(true);
    battle->set_defense(5);
    battle->set_battle_protector_player_id(kOpponent);
    apply(batch);

    EXPECT_EQ(state->loyaltyForPermanentOid(100), 4);
    EXPECT_EQ(state->defenseForPermanentOid(200), 5);
    EXPECT_EQ(state->battleProtectorForPermanentOid(200), kOpponent);
    EXPECT_EQ(state->defenseForPermanentOid(100), -1);

    ruled::v1::RuledEventBatch replacement;
    replacement.add_events()->mutable_zone_view()->add_per_player()->set_player_id(kLocalPlayer);
    apply(replacement);
    EXPECT_EQ(state->loyaltyForPermanentOid(100), -1);
    EXPECT_EQ(state->defenseForPermanentOid(200), -1);
    EXPECT_EQ(state->battleProtectorForPermanentOid(200), -1);
}

TEST_F(RuledClientTest, ZoneViewParsesDamageAndPipeDelimitedAbilities)
{
    QSignalSpy fsSpy(state, &RuledClientState::firstStrikeStepPendingChanged);
    ruled::v1::RuledEventBatch batch;
    auto *view = batch.add_events()->mutable_zone_view()->add_per_player();
    view->set_player_id(kLocalPlayer);
    view->set_first_strike_step_pending(true);
    auto *object = view->add_battlefield_objects();
    object->set_object_id(100);
    object->set_damage(3);
    auto *manaAbility = object->add_activated_abilities();
    manaAbility->set_ability_index(0);
    manaAbility->set_text("Add {G}.");
    manaAbility->set_mana_produced("G");
    manaAbility->set_is_mana_ability(true);
    manaAbility->set_cost_label("{T}");
    auto *drawAbility = object->add_activated_abilities();
    drawAbility->set_ability_index(1);
    drawAbility->set_text("Sacrifice this: draw a card.");
    drawAbility->set_mana_cost("1");
    drawAbility->set_cost_label("Sacrifice this");
    apply(batch);

    EXPECT_EQ(state->markedDamageForEngineOid(100), 3);
    const auto abilities = state->activatedAbilitiesForOid(100);
    ASSERT_EQ(abilities.size(), 2);
    ASSERT_TRUE(abilities[0]);
    ASSERT_TRUE(abilities[1]);
    EXPECT_EQ(abilities[0]->text, QStringLiteral("Add {G}."));
    EXPECT_EQ(abilities[1]->text, QStringLiteral("Sacrifice this: draw a card."));
    EXPECT_TRUE(abilities[0]->manaCost.isEmpty());
    EXPECT_EQ(abilities[1]->manaCost, QStringLiteral("1"));
    // Classification comes from the engine; output text remains a separate presentation field.
    EXPECT_TRUE(abilities[0]->isManaAbility());
    EXPECT_FALSE(abilities[1]->isManaAbility());
    EXPECT_EQ(abilities[0]->manaProduced, QStringLiteral("G"));
    EXPECT_TRUE(abilities[1]->manaProduced.isEmpty());
    EXPECT_EQ(abilities[0]->costLabel, QStringLiteral("{T}"));
    EXPECT_EQ(abilities[1]->costLabel, QStringLiteral("Sacrifice this"));

    EXPECT_TRUE(state->isFirstStrikeStepPending());
    ASSERT_EQ(fsSpy.count(), 1);
    EXPECT_TRUE(fsSpy.at(0).at(0).toBool());

    // The flag only re-announces on a change.
    apply(batch);
    EXPECT_EQ(fsSpy.count(), 1);
}

TEST_F(RuledClientTest, BattlefieldAbilityIndicesPreserveAuthoredZoneGaps)
{
    ruled::v1::RuledEventBatch batch;
    auto *view = batch.add_events()->mutable_zone_view()->add_per_player();
    view->set_player_id(kLocalPlayer);
    auto *object = view->add_battlefield_objects();
    object->set_object_id(203);
    for (const auto &[abilityIndex, label] : std::initializer_list<std::pair<quint32, const char *>>{
             {1, "+1: Create an emblem."}, {2, "0: Surveil 2."}, {3, "-2: Tap target creature."}}) {
        auto *ability = object->add_activated_abilities();
        ability->set_ability_index(abilityIndex);
        ability->set_text(label);
        ability->set_activatable(true);
    }
    apply(batch);

    EXPECT_EQ(state->activatedAbilityIndicesForOid(203), QList<int>({1, 2, 3}));
    EXPECT_EQ(state->activatedAbilityMenuLabel(203, 1), QStringLiteral("+1: Create an emblem."));
    EXPECT_EQ(state->activatedAbilityMenuLabel(203, 2), QStringLiteral("0: Surveil 2."));
    EXPECT_EQ(state->activatedAbilityMenuLabel(203, 3), QStringLiteral("-2: Tap target creature."));
    EXPECT_TRUE(state->activatedAbilityMenuLabel(203, 0).isEmpty());
}

TEST_F(RuledClientTest, DatabaseOracleTextReachesAbilityMenusAndTracksDatabaseReload)
{
    host.presentationCards = readPresentationCards(R"(<cockatrice_carddatabase version="4"><cards><card>
      <name>Prodigal Sorcerer</name><text>Ordinary card display</text><ruled-oracle>
      <face card-name="Prodigal Sorcerer" face-name="Prodigal Sorcerer">
      <text>{T}: Prodigal Sorcerer deals 1 damage to any target.</text></face>
      </ruled-oracle></card></cards></cockatrice_carddatabase>)");
    const QString text = QStringLiteral("{T}: Prodigal Sorcerer deals 1 damage to any target.");
    ruled::v1::RuledEventBatch batch;
    auto *view = batch.add_events()->mutable_zone_view()->add_per_player();
    view->set_player_id(kLocalPlayer);
    auto *object = view->add_battlefield_objects();
    object->set_object_id(203);
    auto *ability = object->add_activated_abilities();
    ability->set_ability_index(0);
    ability->set_text("Legacy fallback");
    ability->set_activatable(true);
    auto *presentation = ability->mutable_presentation();
    presentation->set_external_card_name("Prodigal Sorcerer");
    presentation->set_external_face_name("Prodigal Sorcerer");
    presentation->set_oracle_text_sha256(RuledOracleText::textSha256(text).toStdString());
    presentation->add_oracle_line_indices(1);
    presentation->set_fallback_text("Prodigal Sorcerer — activated ability (activated_01)");
    apply(batch);
    EXPECT_EQ(state->activatedAbilityMenuLabel(203, 0), text);
    EXPECT_TRUE(state->abilityActivatable(203, 0));
    host.presentationCards.clear();
    apply(batch);
    EXPECT_EQ(state->activatedAbilityMenuLabel(203, 0), QString::fromStdString(presentation->fallback_text()));
    EXPECT_TRUE(state->abilityActivatable(203, 0));
}

TEST_F(RuledClientTest, AbilityDecodingPreservesSparseSlotsAndPresentationAcrossSourceZones)
{
    ruled::v1::RuledEventBatch batch;
    auto *view = batch.add_events()->mutable_zone_view()->add_per_player();
    view->set_player_id(kLocalPlayer);
    auto *object = view->add_battlefield_objects();
    object->set_object_id(203);
    ruled::v1::AbilityInfo ability;
    ability.set_ability_index(2);
    ability.set_text("Unused label");
    ability.mutable_presentation()->set_fallback_text("Presented ability");
    ability.set_mana_cost("{1}");
    ability.set_mana_produced("G");
    ability.set_is_mana_ability(true);
    ability.set_cost_label("{1}, {T}");
    ability.set_activatable(true);
    ability.set_has_only_tap_cost(true);
    *object->add_activated_abilities() = ability;
    auto *zone = (*batch.mutable_legal_by_player())[kLocalPlayer].add_zone_ability_actions();
    zone->set_object_id(204);
    zone->set_source_zone(ruled::v1::ABILITY_SOURCE_ZONE_HAND);
    zone->set_hand_index(5);
    zone->set_zone_change_generation(7);
    zone->set_ability_index(2);
    *zone->mutable_ability() = ability;
    // The zone action's outer index is authoritative, not the embedded AbilityInfo index.
    zone->mutable_ability()->set_ability_index(9);
    apply(batch);

    for (quint32 oid : {203u, 204u}) {
        const auto entries = state->activatedAbilitiesForOid(oid);
        ASSERT_EQ(entries.size(), 3);
        EXPECT_FALSE(entries[0]);
        EXPECT_FALSE(entries[1]);
        ASSERT_TRUE(entries[2]);
        EXPECT_EQ(entries[2]->text, QStringLiteral("Presented ability"));
        EXPECT_EQ(entries[2]->manaCost, QStringLiteral("{1}"));
        EXPECT_EQ(entries[2]->manaProduced, QStringLiteral("G"));
        EXPECT_EQ(entries[2]->costLabel, QStringLiteral("{1}, {T}"));
        EXPECT_TRUE(entries[2]->hasOnlyTapCost);
        EXPECT_TRUE(entries[2]->usesDirectManaActivation());
        EXPECT_FALSE(state->abilityActivatable(oid, 0));
        EXPECT_TRUE(state->abilityActivatable(oid, 2));
    }
    EXPECT_EQ(state->zoneAbilityOidForHandSlot(5), 204u);
    EXPECT_EQ(state->abilitySourceGeneration(204), 7u);

    object->mutable_activated_abilities(0)->clear_presentation();
    object->mutable_activated_abilities(0)->set_text("Updated ability");
    object->mutable_activated_abilities(0)->set_activatable(false);
    *zone->mutable_ability() = object->activated_abilities(0);
    apply(batch);
    for (quint32 oid : {203u, 204u}) {
        EXPECT_EQ(state->activatedAbilityMenuLabel(oid, 2), QStringLiteral("Updated ability"));
        EXPECT_FALSE(state->abilityActivatable(oid, 2));
    }
}

TEST_F(RuledClientTest, LoyaltyXAbilityInfoPreservesPresenceAndRejectsMalformedRanges)
{
    ruled::v1::RuledEventBatch batch;
    auto *view = batch.add_events()->mutable_zone_view()->add_per_player();
    view->set_player_id(kLocalPlayer);

    const auto addAbility = [view](quint32 oid, quint32 minX, quint32 maxX, bool addStorageChoice,
                                   bool addManaXCost) {
        auto *object = view->add_battlefield_objects();
        object->set_object_id(oid);
        object->set_zone_change_generation(23);
        auto *ability = object->add_activated_abilities();
        ability->set_ability_index(0);
        ability->set_text("Sorin -X");
        ability->set_activatable(true);
        if (addManaXCost) {
            ability->set_mana_cost("X");
        }
        ability->mutable_loyalty_x_choice()->set_min_x(minX);
        ability->mutable_loyalty_x_choice()->set_max_x(maxX);
        if (addStorageChoice) {
            auto *storage = ability->mutable_x_counter_mana_choice();
            storage->set_counter_label("charge");
            storage->set_max_x(3);
            storage->set_first_color("R");
            storage->set_second_color("G");
        }
    };

    addAbility(810u, 0u, 0u, false, false);
    addAbility(811u, 2u, 7u, false, false);
    addAbility(812u, 7u, 2u, false, false);
    addAbility(813u, 0u, 0x80000000u, false, false);
    addAbility(814u, 0u, 0u, true, false);
    addAbility(815u, 0u, 7u, false, true);
    auto *plainObject = view->add_battlefield_objects();
    plainObject->set_object_id(816u);
    plainObject->set_zone_change_generation(23);
    auto *plainAbility = plainObject->add_activated_abilities();
    plainAbility->set_ability_index(0);
    plainAbility->set_text("Ordinary ability");
    plainAbility->set_activatable(true);
    apply(batch);

    const auto zero = state->activatedAbilityForOid(810u, 0);
    ASSERT_TRUE(zero);
    ASSERT_TRUE(zero->loyaltyXChoice);
    EXPECT_EQ(zero->loyaltyXChoice->minX, 0u);
    EXPECT_EQ(zero->loyaltyXChoice->maxX, 0u);
    EXPECT_TRUE(state->abilityActivatable(810u, 0));

    const auto nonzero = state->activatedAbilityForOid(811u, 0);
    ASSERT_TRUE(nonzero);
    ASSERT_TRUE(nonzero->loyaltyXChoice);
    EXPECT_EQ(nonzero->loyaltyXChoice->minX, 2u);
    EXPECT_EQ(nonzero->loyaltyXChoice->maxX, 7u);
    EXPECT_TRUE(state->abilityActivatable(811u, 0));

    EXPECT_FALSE(state->abilityActivatable(812u, 0));
    EXPECT_FALSE(state->abilityActivatable(813u, 0));
    EXPECT_FALSE(state->abilityActivatable(814u, 0));
    EXPECT_FALSE(state->abilityActivatable(815u, 0));
    const auto ordinary = state->activatedAbilityForOid(816u, 0);
    ASSERT_TRUE(ordinary);
    EXPECT_FALSE(ordinary->loyaltyXChoice);
    EXPECT_TRUE(state->abilityActivatable(816u, 0));
}

TEST_F(RuledClientTest, RemovingEarlierGrantRetainsSurvivingSparseSlotAndRecipientGeneration)
{
    ruled::v1::RuledEventBatch batch;
    auto *object = batch.add_events()->mutable_zone_view()->add_per_player()->add_battlefield_objects();
    object->set_object_id(203);
    object->set_zone_change_generation(4);
    for (quint32 slot : {0u, 1u, 2u}) {
        auto *ability = object->add_activated_abilities();
        ability->set_ability_index(slot);
        ability->set_text(slot == 0 ? "Native mana" : "{T}: Add one mana of any color.");
        ability->set_mana_produced(slot == 0 ? "G" : "W|U|B|R|G");
        ability->set_is_mana_ability(true);
        ability->set_has_only_tap_cost(true);
        ability->set_activatable(true);
    }
    apply(batch);
    EXPECT_EQ(state->activatedAbilityIndicesForOid(203), QList<int>({0, 1, 2}));
    const auto survivor = state->activatedAbilitiesForOid(203)[2];

    object->mutable_activated_abilities()->DeleteSubrange(1, 1);
    apply(batch);
    EXPECT_EQ(state->activatedAbilityIndicesForOid(203), QList<int>({0, 2}));
    const auto current = state->activatedAbilitiesForOid(203);
    ASSERT_EQ(current.size(), 3);
    EXPECT_FALSE(current[1]);
    ASSERT_TRUE(current[2]);
    EXPECT_EQ(current[2]->manaProduced, survivor->manaProduced);
    EXPECT_EQ(state->activatedAbilityMenuLabel(203, 2), QStringLiteral("{T}: Add one mana of any color."));
    EXPECT_FALSE(state->abilityActivatable(203, 1));
    EXPECT_TRUE(state->abilityActivatable(203, 2));
    EXPECT_EQ(state->abilitySourceGeneration(203), 4u);

    ruled::v1::RuledEventBatch unchanged;
    unchanged.add_events()->mutable_zone_view()->set_battlefields_unchanged(true);
    apply(unchanged);
    EXPECT_EQ(state->activatedAbilityIndicesForOid(203), QList<int>({0, 2}));

    // A new recipient incarnation replaces the catalog; old tombstones do not survive it.
    object->set_zone_change_generation(5);
    object->clear_activated_abilities();
    auto *newAbility = object->add_activated_abilities();
    newAbility->set_ability_index(0);
    newAbility->set_text("New incarnation");
    newAbility->set_activatable(true);
    apply(batch);
    EXPECT_EQ(state->activatedAbilityIndicesForOid(203), QList<int>({0}));
    EXPECT_EQ(state->abilitySourceGeneration(203), 5u);
    EXPECT_TRUE(state->activatedAbilityMenuLabel(203, 2).isEmpty());
}

TEST_F(RuledClientTest, AbilitySnapshotsRetainBattlefieldButExpireZoneOffersAndResetTogether)
{
    ruled::v1::RuledEventBatch batch;
    auto *object = batch.add_events()->mutable_zone_view()->add_per_player()->add_battlefield_objects();
    object->set_object_id(203);
    object->set_zone_change_generation(4);
    auto *ability = object->add_activated_abilities();
    ability->set_ability_index(2);
    ability->set_text("{T}: Add {G}.");
    ability->set_mana_produced("G");
    ability->set_is_mana_ability(true);
    ability->set_cost_label("{T}");
    ability->set_activatable(true);
    auto *zone = (*batch.mutable_legal_by_player())[kLocalPlayer].add_zone_ability_actions();
    zone->set_object_id(204);
    zone->set_source_zone(ruled::v1::ABILITY_SOURCE_ZONE_HAND);
    zone->set_hand_index(5);
    zone->set_zone_change_generation(7);
    zone->set_ability_index(2);
    *zone->mutable_ability() = *ability;
    apply(batch);

    EXPECT_FALSE(state->abilityActivatable(203, 0)); // a sparse gap is disabled
    EXPECT_TRUE(state->abilityActivatable(203, 3));  // preserve the unknown-index fallback
    EXPECT_TRUE(state->abilityActivatable(999, 0));
    EXPECT_TRUE(state->abilityActivatable(203, -1));
    state->abilityCostData[RuledClientState::abilityTargetKey(203, 2)].nonManaCostsPayable = false;
    EXPECT_FALSE(state->abilityActivatable(203, 2));

    apply(ruled::v1::RuledEventBatch{});
    EXPECT_EQ(state->activatedAbilityIndicesForOid(203), QList<int>({2}));
    EXPECT_EQ(state->abilitySourceGeneration(203), 4u);
    EXPECT_TRUE(state->abilityActivatable(203, 2)); // per-batch cost data expired
    EXPECT_TRUE(state->activatedAbilityIndicesForOid(204).isEmpty());
    EXPECT_EQ(state->zoneAbilityOidForHandSlot(5), 0u);

    ruled::v1::RuledEventBatch unchanged;
    unchanged.add_events()->mutable_zone_view()->set_battlefields_unchanged(true);
    apply(unchanged);
    EXPECT_EQ(state->activatedAbilityMenuLabel(203, 2), QStringLiteral("{T}: Add {G}."));

    ruled::v1::RuledEventBatch empty;
    empty.add_events()->mutable_zone_view();
    apply(empty);
    EXPECT_TRUE(state->activatedAbilityIndicesForOid(203).isEmpty());

    apply(batch);
    state->clearSessionState(RuledSessionResetScope::KeepCurrentBatch);
    EXPECT_EQ(state->activatedAbilityIndicesForOid(203), QList<int>({2}));
    EXPECT_EQ(state->activatedAbilityIndicesForOid(204), QList<int>({2}));
    EXPECT_EQ(state->zoneAbilityOidForHandSlot(5), 204u);
    EXPECT_EQ(state->abilitySourceGeneration(204), 7u);
    state->clearSessionState();
    EXPECT_TRUE(state->activatedAbilityIndicesForOid(203).isEmpty());
    EXPECT_TRUE(state->activatedAbilityIndicesForOid(204).isEmpty());
    EXPECT_EQ(state->zoneAbilityOidForHandSlot(5), 0u);
}

TEST_F(RuledClientTest, AbilityZoneTransitionReplacesPresentationAndSourceIdentity)
{
    ruled::v1::RuledEventBatch batch;
    auto *zone = (*batch.mutable_legal_by_player())[kLocalPlayer].add_zone_ability_actions();
    zone->set_object_id(204);
    zone->set_source_zone(ruled::v1::ABILITY_SOURCE_ZONE_HAND);
    zone->set_hand_index(5);
    zone->set_zone_change_generation(7);
    zone->set_ability_index(2);
    zone->mutable_ability()->set_text("Hand action");
    apply(batch);

    ruled::v1::RuledEventBatch battlefield;
    auto *object = battlefield.add_events()->mutable_zone_view()->add_per_player()->add_battlefield_objects();
    object->set_object_id(204);
    object->set_zone_change_generation(8);
    auto *ability = object->add_activated_abilities();
    ability->set_ability_index(0);
    ability->set_text("Battlefield action");
    ability->set_activatable(true);
    apply(battlefield);
    EXPECT_EQ(state->activatedAbilityIndicesForOid(204), QList<int>({0}));
    EXPECT_EQ(state->activatedAbilityMenuLabel(204, 0), QStringLiteral("Battlefield action"));
    EXPECT_EQ(state->abilitySourceZone(204), ruled::v1::ABILITY_SOURCE_ZONE_BATTLEFIELD);
    EXPECT_EQ(state->abilitySourceGeneration(204), 8u);
    EXPECT_EQ(state->zoneAbilityOidForHandSlot(5), 0u);

    batch.add_events()->mutable_zone_view(); // full empty battlefield snapshot
    zone->set_source_zone(ruled::v1::ABILITY_SOURCE_ZONE_GRAVEYARD);
    zone->clear_hand_index();
    zone->set_zone_change_generation(9);
    zone->mutable_ability()->set_text("Graveyard action");
    apply(batch);
    EXPECT_EQ(state->activatedAbilityIndicesForOid(204), QList<int>({2}));
    EXPECT_TRUE(state->activatedAbilityMenuLabel(204, 0).isEmpty());
    EXPECT_EQ(state->activatedAbilityMenuLabel(204, 2), QStringLiteral("Graveyard action"));
    EXPECT_EQ(state->abilitySourceZone(204), ruled::v1::ABILITY_SOURCE_ZONE_GRAVEYARD);
    EXPECT_EQ(state->abilitySourceGeneration(204), 9u);
}

TEST_F(RuledClientTest, AbilityDiagnosticsDescribeEntriesAndDropExpiredPrivateOffers)
{
    ruled::v1::RuledEventBatch batch;
    auto *object = batch.add_events()->mutable_zone_view()->add_per_player()->add_battlefield_objects();
    object->set_object_id(203);
    auto *ability = object->add_activated_abilities();
    ability->set_ability_index(2);
    ability->set_text("{1}, {T}: Add {G}.");
    ability->set_mana_cost("{1}");
    ability->set_mana_produced("G");
    ability->set_is_mana_ability(true);
    ability->set_cost_label("{1}, {T}");
    ability->set_activatable(true);
    auto *zone = (*batch.mutable_legal_by_player())[kLocalPlayer].add_zone_ability_actions();
    zone->set_object_id(204);
    zone->set_source_zone(ruled::v1::ABILITY_SOURCE_ZONE_HAND);
    zone->set_hand_index(5);
    zone->set_zone_change_generation(7);
    zone->set_ability_index(2);
    *zone->mutable_ability() = *ability;
    zone->mutable_ability()->set_text("Private hand action");
    apply(batch);

    const auto snapshot = state->diagnosticSnapshot();
    const auto abilities = snapshot.value("activatedAbilitiesByOid").toArray();
    ASSERT_EQ(abilities.size(), 2);
    EXPECT_EQ(abilities[0].toObject().value("key").toInt(), 203);
    const auto entries = abilities[0].toObject().value("value").toArray();
    ASSERT_EQ(entries.size(), 3);
    EXPECT_TRUE(entries[0].isNull());
    EXPECT_TRUE(entries[1].isNull());
    const QJsonObject expected{{"text", "{1}, {T}: Add {G}."},
                               {"manaCost", "{1}"},
                               {"manaProduced", "G"},
                               {"manaOptionLabels", QJsonArray{}},
                               {"xCounterManaChoice", QJsonValue(QJsonValue::Null)},
                               {"costLabel", "{1}, {T}"},
                               {"activatable", true},
                               {"hasOnlyTapCost", false}};
    EXPECT_EQ(entries[2].toObject(), expected);
    EXPECT_EQ(snapshot, state->diagnosticSnapshot());

    apply(ruled::v1::RuledEventBatch{});
    const auto expired = state->diagnosticSnapshot().value("activatedAbilitiesByOid").toArray();
    ASSERT_EQ(expired.size(), 1);
    EXPECT_EQ(expired[0].toObject().value("key").toInt(), 203);
    state->clearSessionState();
    EXPECT_TRUE(state->diagnosticSnapshot().value("activatedAbilitiesByOid").toArray().isEmpty());
}

TEST_F(RuledClientTest, ParsesSpellCostChoicesForHandAndPublicZoneCasts)
{
    ruled::v1::RuledEventBatch batch;
    auto &actions = (*batch.mutable_legal_by_player())[kLocalPlayer];
    auto *hand = actions.add_hand_actions();
    hand->set_kind(ruled::v1::HAND_ACTION_CAST_SPELL);
    hand->set_cast_method(ruled::v1::CAST_METHOD_NORMAL);
    hand->set_hand_index(3);
    hand->set_face_index(0);
    auto *discard = hand->mutable_cost_choices()->add_choices();
    discard->set_cost_index(0);
    discard->set_zone(ruled::v1::COST_CHOICE_ZONE_HAND);
    discard->add_candidate_ids(1);

    auto *zone = actions.add_zone_cast_actions();
    zone->set_cast_method(ruled::v1::CAST_METHOD_NORMAL);
    zone->set_object_id(77);
    zone->set_face_index(1);
    auto *sacrifice = zone->mutable_cost_choices()->add_choices();
    sacrifice->set_cost_index(0);
    sacrifice->set_zone(ruled::v1::COST_CHOICE_ZONE_BATTLEFIELD);
    sacrifice->add_candidate_ids(900);
    apply(batch);

    const auto handCosts = state->spellCostData(3, 0, RuledCastSource::Hand);
    ASSERT_EQ(handCosts.choices.size(), 1);
    EXPECT_EQ(handCosts.choices.first().zone, RuledCostChoiceZone::Hand);
    EXPECT_EQ(handCosts.choices.first().candidateIds, QSet<quint32>({1}));
    const auto zoneCosts = state->spellCostData(77, 1, RuledCastSource::Graveyard);
    ASSERT_EQ(zoneCosts.choices.size(), 1);
    EXPECT_EQ(zoneCosts.choices.first().zone, RuledCostChoiceZone::Battlefield);
    EXPECT_EQ(zoneCosts.choices.first().candidateIds, QSet<quint32>({900}));
}

TEST_F(RuledClientTest, ActivatedAbilityMenuLabelsDoNotDuplicateStructuredCosts)
{
    ruled::v1::RuledEventBatch batch;
    auto *view = batch.add_events()->mutable_zone_view()->add_per_player();
    view->set_player_id(kLocalPlayer);
    auto *object = view->add_battlefield_objects();
    object->set_object_id(100);

    auto *manaAbility = object->add_activated_abilities();
    manaAbility->set_ability_index(0);
    manaAbility->set_text("{2}{R}: Ability text.");
    manaAbility->set_mana_cost("2R");
    manaAbility->set_cost_label("{2}{R}");

    auto *compositeAbility = object->add_activated_abilities();
    compositeAbility->set_ability_index(1);
    compositeAbility->set_text("{2}, {T}, Sacrifice a creature: Draw a card.");
    compositeAbility->set_mana_cost("2");
    compositeAbility->set_cost_label("{2}, {T}, Sacrifice a creature");
    apply(batch);

    EXPECT_EQ(state->activatedAbilityMenuLabel(100, 0), QStringLiteral("{2}{R}: Ability text."));
    EXPECT_EQ(state->activatedAbilityMenuLabel(100, 1), QStringLiteral("{2}, {T}, Sacrifice a creature: Draw a card."));
}

TEST_F(RuledClientTest, WarRoomNumericEngineCostsReachBothMenusWithoutChangingUnsupportedPrefixes)
{
    host.presentationCards = readPresentationCards(R"(<cockatrice_carddatabase version="4"><cards><card>
      <name>War Room</name><ruled-oracle><face card-name="War Room" face-name="War Room">
      <text>{3}, {T}, Pay life equal to the number of colors in your commanders' color identity: Draw a card.</text>
      </face></ruled-oracle></card><card><name>Fountainport</name><ruled-oracle>
      <face card-name="Fountainport" face-name="Fountainport">
      <text>{2}, {T}, Sacrifice a token: Draw a card.</text></face>
      </ruled-oracle></card></cards></cockatrice_carddatabase>)");
    const QString oracle = QStringLiteral(
        "{3}, {T}, Pay life equal to the number of colors in your commanders' color identity: Draw a card.");
    const QString fountain = QStringLiteral("{2}, {T}, Sacrifice a token: Draw a card.");
    for (int life : {0, 1, 5}) {
        ruled::v1::RuledEventBatch batch;
        auto *object = batch.add_events()->mutable_zone_view()->add_per_player()->add_battlefield_objects();
        object->set_object_id(100);
        auto *ability = object->add_activated_abilities();
        ability->set_ability_index(0);
        const QString cost = QStringLiteral("{3}, {T}, Pay %1 life").arg(life);
        const QString expected = cost + QStringLiteral(": Draw a card.");
        ability->set_text(expected.toStdString());
        ability->set_cost_label(cost.toStdString());
        ability->set_activatable(true);
        auto *presentation = ability->mutable_presentation();
        presentation->set_external_card_name("War Room");
        presentation->set_external_face_name("War Room");
        presentation->set_oracle_text_sha256(RuledOracleText::textSha256(oracle).toStdString());
        presentation->add_oracle_line_indices(1);
        presentation->set_fallback_text(expected.toStdString());
        apply(batch);
        EXPECT_EQ(state->activatedAbilityMenuLabel(100, 0), expected);
        const auto options = RuledPendingCast::cardActionMenuOptions({}, *state, 100);
        ASSERT_EQ(options.size(), 1);
        EXPECT_EQ(options[0].label, expected);
        EXPECT_TRUE(options[0].enabled);
    }

    const QList<QPair<QString, QString>> unsupported{
        {QStringLiteral("Fountainport — activated ability (activated_02)"),
         QStringLiteral("{2}, {T}, Sacrifice a permanent")},
        {QStringLiteral("{2}, {T}, Sacrifice a permanent: Draw a card."),
         QStringLiteral("{2}, {T}, Sacrifice a permanent")},
        {QStringLiteral("{2}, Discard a card: Draw a card."), QStringLiteral("{2}, Discard a card")},
        {QStringLiteral("{X}, {T}: Draw a card."), QStringLiteral("{X}, {T}")},
        {QStringLiteral("Unstructured fallback"), QStringLiteral("{3}, {T}, Pay 1 life")}};
    for (const auto &entry : unsupported) {
        ruled::v1::RuledEventBatch batch;
        auto *object = batch.add_events()->mutable_zone_view()->add_per_player()->add_battlefield_objects();
        object->set_object_id(100);
        auto *ability = object->add_activated_abilities();
        ability->set_ability_index(0);
        ability->set_text(entry.first.toStdString());
        ability->set_cost_label(entry.second.toStdString());
        auto *presentation = ability->mutable_presentation();
        presentation->set_external_card_name("Fountainport");
        presentation->set_external_face_name("Fountainport");
        presentation->set_oracle_text_sha256(RuledOracleText::textSha256(fountain).toStdString());
        presentation->add_oracle_line_indices(1);
        presentation->set_fallback_text(entry.first.toStdString());
        apply(batch);
        EXPECT_EQ(state->activatedAbilityMenuLabel(100, 0), fountain);
        const auto options = RuledPendingCast::cardActionMenuOptions({}, *state, 100);
        ASSERT_EQ(options.size(), 1);
        EXPECT_EQ(options[0].label, fountain);
        EXPECT_FALSE(options[0].enabled);
    }
}

TEST_F(RuledClientTest, EffectiveLoyaltyCostsReachBothMenusOnlyWithCompleteMatchingPrefixes)
{
    // Synthetic Oracle text isolates the same emitted presentation contract for each cost form.
    const QString oracle = QStringLiteral("+1: Scry 1, then draw a card.\n"
                                          "0: Create two Wolf creature tokens.\n"
                                          "-1: Target player draws a card.\n"
                                          "-3: Destroy target creature. Draw a card.\n"
                                          "{3}, {T}, +1: Draw a card.\n"
                                          "{2}, {T}, Sacrifice a token, +1: Draw a card.");
    host.presentationCards = readPresentationCards(
        QByteArray("<cockatrice_carddatabase version=\"4\"><cards><card><name>Loyalty Fixture</name>"
                   "<ruled-oracle><face card-name=\"Loyalty Fixture\" face-name=\"Loyalty Fixture\"><text>") +
        oracle.toUtf8() + QByteArray("</text></face></ruled-oracle></card></cards></cockatrice_carddatabase>"));
    struct CostCase
    {
        const char *fallback;
        const char *cost;
        int line;
        const char *expected;
    };
    const CostCase cases[] = {
        {"+2: Generic ability", "+2", 1, "+2: Scry 1, then draw a card."},
        {"+1: Generic ability", "+1", 2, "+1: Create two Wolf creature tokens."},
        {"+0: Generic ability", "+0", 3, "+0: Target player draws a card."},
        {"-2: Generic ability", "-2", 4, "-2: Destroy target creature. Draw a card."},
        {"{3}, {T}, +2: Generic ability", "{3}, {T}, +2", 5, "{3}, {T}, +2: Draw a card."},
        {"+2: Stale fallback", "+3", 1, "+1: Scry 1, then draw a card."},
        {"{2}, {T}, Sacrifice a permanent, +2: Generic ability", "{2}, {T}, Sacrifice a permanent, +2", 6,
         "{2}, {T}, Sacrifice a token, +1: Draw a card."},
    };
    for (const auto &entry : cases) {
        ruled::v1::RuledEventBatch batch;
        auto *view = batch.add_events()->mutable_zone_view()->add_per_player();
        view->set_player_id(kLocalPlayer);
        auto *object = view->add_battlefield_objects();
        object->set_object_id(100);
        auto *ability = object->add_activated_abilities();
        ability->set_ability_index(0);
        ability->set_text(entry.fallback);
        ability->set_cost_label(entry.cost);
        ability->set_activatable(true);
        auto *presentation = ability->mutable_presentation();
        presentation->set_external_card_name("Loyalty Fixture");
        presentation->set_external_face_name("Loyalty Fixture");
        presentation->set_oracle_text_sha256(RuledOracleText::textSha256(oracle).toStdString());
        presentation->add_oracle_line_indices(entry.line);
        presentation->set_fallback_text(entry.fallback);
        apply(batch);
        const auto expected = QString::fromUtf8(entry.expected);
        EXPECT_EQ(state->activatedAbilityMenuLabel(100, 0), expected);
        const auto options = RuledPendingCast::cardActionMenuOptions({}, *state, 100);
        ASSERT_EQ(options.size(), 1);
        EXPECT_EQ(options[0].label, expected);
        EXPECT_TRUE(options[0].enabled);
    }
}

TEST_F(RuledClientTest, WarRoomAbsentCommanderRetainsOracleLabelAndEngineDisabledAction)
{
    ruled::v1::RuledEventBatch batch;
    auto *object = batch.add_events()->mutable_zone_view()->add_per_player()->add_battlefield_objects();
    object->set_object_id(100);
    auto *ability = object->add_activated_abilities();
    ability->set_ability_index(1);
    ability->set_text("{3}, {T}, Pay commander identity life: Draw a card.");
    ability->set_cost_label("{3}, {T}, Pay life (no commander)");
    ability->set_activatable(false);
    apply(batch);
    EXPECT_FALSE(state->abilityActivatable(100, 1));
    const auto options = RuledPendingCast::cardActionMenuOptions({}, *state, 100);
    ASSERT_EQ(options.size(), 1);
    EXPECT_FALSE(options[0].enabled);
    EXPECT_EQ(options[0].label, QString::fromStdString(ability->text()));
}

TEST_F(RuledClientTest, ActivatedAbilityAvailabilityTracksTheEngineAcrossFullZoneViews)
{
    auto availabilityBatch = [](bool activatable) {
        ruled::v1::RuledEventBatch batch;
        auto *view = batch.add_events()->mutable_zone_view()->add_per_player();
        view->set_player_id(kLocalPlayer);
        auto *object = view->add_battlefield_objects();
        object->set_object_id(100);
        auto *ability = object->add_activated_abilities();
        ability->set_ability_index(0);
        ability->set_text("{1}{W}, {T}: Tap target creature. Activate only if you control a creature with flying.");
        ability->set_activatable(activatable);
        return batch;
    };

    apply(availabilityBatch(false));
    EXPECT_FALSE(state->abilityActivatable(100, 0));
    EXPECT_EQ(state->activatedAbilityMenuLabel(100, 0),
              QStringLiteral("{1}{W}, {T}: Tap target creature. Activate only if you control a creature with flying."));

    apply(availabilityBatch(true));
    EXPECT_TRUE(state->abilityActivatable(100, 0));
}

TEST_F(RuledClientTest, BattlefieldOmissionRetainsStateWhileOtherZoneViewFieldsUpdate)
{
    ruled::v1::RuledEventBatch full;
    auto *fullView = full.add_events()->mutable_zone_view()->add_per_player();
    fullView->set_player_id(kLocalPlayer);
    auto *object = fullView->add_battlefield_objects();
    object->set_object_id(100);
    object->set_power(4);
    object->set_toughness(5);
    object->set_damage(2);
    auto *ability = object->add_activated_abilities();
    ability->set_ability_index(0);
    ability->set_text("Draw a card.");
    ability->set_cost_label("{T}");
    ability->set_activatable(true);
    apply(full);

    ruled::v1::RuledEventBatch omitted;
    auto *omittedZone = omitted.add_events()->mutable_zone_view();
    omittedZone->set_battlefields_unchanged(true);
    auto *omittedView = omittedZone->add_per_player();
    omittedView->set_player_id(kLocalPlayer);
    omittedView->set_first_strike_step_pending(true);
    apply(omitted);

    EXPECT_EQ(state->markedDamageForEngineOid(100), 2);
    EXPECT_EQ(state->combatPowerForCreatureOid(100), 4);
    EXPECT_EQ(state->combatToughnessForCreatureOid(100), 5);
    const auto parsedAbility = state->activatedAbilityForOid(100, 0);
    ASSERT_TRUE(parsedAbility);
    EXPECT_EQ(parsedAbility->text, QStringLiteral("Draw a card."));
    EXPECT_EQ(parsedAbility->costLabel, QStringLiteral("{T}"));
    EXPECT_TRUE(state->abilityActivatable(100, 0));
    EXPECT_TRUE(state->isFirstStrikeStepPending()) << "non-battlefield fields in an omitted view still apply";

    ruled::v1::RuledEventBatch explicitEmpty;
    explicitEmpty.add_events()->mutable_zone_view()->add_per_player()->set_player_id(kLocalPlayer);
    apply(explicitEmpty);
    EXPECT_EQ(state->markedDamageForEngineOid(100), 0);
    EXPECT_TRUE(state->activatedAbilitiesForOid(100).isEmpty());
}

TEST_F(RuledClientTest, SerializedZoneViewThenIdentityMapPreservesDamageForCombatAllocation)
{
    apply(phaseBatch(ruled::v1::PHASE_ID_DECLARE_BLOCKERS, kLocalPlayer));

    // This matches the public ruled-batch order: the engine's full battlefield projection is
    // serialized before Servatrice's physical-card identity map.
    ruled::v1::RuledEventBatch fullSnapshot;
    auto *zoneView = fullSnapshot.add_events()->mutable_zone_view();
    auto *local = zoneView->add_per_player();
    local->set_player_id(kLocalPlayer);
    auto *attackerView = local->add_battlefield_objects();
    attackerView->set_object_id(100);
    attackerView->set_zone_change_generation(8);
    attackerView->set_power(5);
    attackerView->set_toughness(5);
    auto *opponent = zoneView->add_per_player();
    opponent->set_player_id(kOpponent);
    auto *blockerView = opponent->add_battlefield_objects();
    blockerView->set_object_id(200);
    blockerView->set_zone_change_generation(4);
    blockerView->set_power(2);
    blockerView->set_toughness(5);
    blockerView->set_damage(3);

    auto *objectMap = fullSnapshot.add_events();
    addPermanent(objectMap, kLocalPlayer, 100, 1)->add_keywords("Trample");
    addPermanent(objectMap, kOpponent, 200, 2);
    auto *blockers = fullSnapshot.add_events()->mutable_blockers_declared();
    auto *pair = blockers->add_block_pairs();
    pair->set_attacker_id(100);
    pair->set_blocker_id(200);
    apply(fullSnapshot);

    EXPECT_EQ(state->markedDamageForEngineOid(200), 3);
    EXPECT_EQ(state->battlefieldGenerationByOid.value(200), 4u);
    EXPECT_EQ(state->combatPowerForCreatureOid(100), 5);
    EXPECT_EQ(state->combatToughnessForCreatureOid(200), 5);
    // Damage already marked on the blocker reduces the lethal assignment from five to two;
    // the remaining three trample damage goes to the defending player (CR 702.19).
    EXPECT_EQ(state->assignedCombatDamageForBlocker(200), 2u);
    EXPECT_EQ(state->localCombatDamagePlayerDamage(), 3);

    // An unchanged battlefield view omits the object projection but may still be followed by a
    // complete identity-map refresh. That map must preserve the same (ObjectId, generation) data.
    ruled::v1::RuledEventBatch unchangedSnapshot;
    auto *unchanged = unchangedSnapshot.add_events()->mutable_zone_view();
    unchanged->set_battlefields_unchanged(true);
    unchanged->add_per_player()->set_player_id(kLocalPlayer);
    unchanged->add_per_player()->set_player_id(kOpponent);
    auto *unchangedMap = unchangedSnapshot.add_events();
    addPermanent(unchangedMap, kLocalPlayer, 100, 1)->add_keywords("Trample");
    addPermanent(unchangedMap, kOpponent, 200, 2);
    apply(unchangedSnapshot);

    EXPECT_EQ(state->markedDamageForEngineOid(200), 3);
    EXPECT_EQ(state->battlefieldGenerationByOid.value(200), 4u);
    EXPECT_EQ(state->combatPowerForCreatureOid(100), 5);
    EXPECT_EQ(state->combatToughnessForCreatureOid(200), 5);
    EXPECT_EQ(state->assignedCombatDamageForBlocker(200), 2u);
    EXPECT_EQ(state->localCombatDamagePlayerDamage(), 3);
}

TEST_F(RuledClientTest, IdentityMapRebindPreservesCombatProjectionForTheSameObjectGeneration)
{
    ruled::v1::RuledEventBatch fullSnapshot;
    auto *zoneView = fullSnapshot.add_events()->mutable_zone_view();
    auto *opponent = zoneView->add_per_player();
    opponent->set_player_id(kOpponent);
    auto *object = opponent->add_battlefield_objects();
    object->set_object_id(200);
    object->set_zone_change_generation(4);
    object->set_power(2);
    object->set_toughness(5);
    object->set_damage(3);
    addPermanent(fullSnapshot.add_events(), kOpponent, 200, 2);
    apply(fullSnapshot);

    // The server-card binding can be refreshed independently of the engine projection. This
    // map-only rebind intentionally keeps ObjectId and generation fixed while changing both
    // physical owner and Server_Card.id.
    ruled::v1::RuledEventBatch rebind;
    auto *map = rebind.add_events();
    addPermanent(map, kLocalPlayer, 200, 22);
    apply(rebind);

    EXPECT_EQ(state->playerIdForEngineOid(200), kLocalPlayer);
    EXPECT_EQ(state->engineOidForCardId(kLocalPlayer, 22), 200u);
    EXPECT_EQ(state->engineOidForCardId(kOpponent, 2), 0u);
    EXPECT_EQ(state->cardIdForEngineOid(200), 22);
    EXPECT_EQ(state->battlefieldGenerationByOid.value(200), 4u);
    EXPECT_EQ(state->markedDamageForEngineOid(200), 3);
    EXPECT_EQ(state->combatPowerForCreatureOid(200), 2);
    EXPECT_EQ(state->combatToughnessForCreatureOid(200), 5);
}

TEST_F(RuledClientTest, CombatProjectionDoesNotSurviveObjectLeaveOrZoneChangeGeneration)
{
    auto publishFullBattlefield = [&](bool includeObject,
                                      quint64 generation,
                                      quint32 power,
                                      quint32 toughness,
                                      quint32 damage) {
        ruled::v1::RuledEventBatch batch;
        auto *view = batch.add_events()->mutable_zone_view();
        view->add_per_player()->set_player_id(kLocalPlayer);
        auto *opponent = view->add_per_player();
        opponent->set_player_id(kOpponent);
        if (includeObject) {
            auto *object = opponent->add_battlefield_objects();
            object->set_object_id(200);
            object->set_zone_change_generation(generation);
            object->set_power(power);
            object->set_toughness(toughness);
            object->set_damage(damage);
            addPermanent(batch.add_events(), kOpponent, 200, 2);
        } else {
            // A full empty projection and an empty identity map are the serialized leave event.
            batch.add_events()->mutable_battlefield_object_map();
        }
        apply(batch);
    };

    publishFullBattlefield(true, 4, 2, 5, 3);
    EXPECT_EQ(state->markedDamageForEngineOid(200), 3);
    EXPECT_EQ(state->combatToughnessForCreatureOid(200), 5);

    // The same ObjectId can be reintroduced as a new battlefield object. Its new generation and
    // fresh projection replace every characteristic from the prior object incarnation.
    publishFullBattlefield(true, 9, 4, 6, 1);
    EXPECT_EQ(state->markedDamageForEngineOid(200), 1);
    EXPECT_EQ(state->battlefieldGenerationByOid.value(200), 9u);
    EXPECT_EQ(state->combatPowerForCreatureOid(200), 4);
    EXPECT_EQ(state->combatToughnessForCreatureOid(200), 6);

    publishFullBattlefield(false, 0, 0, 0, 0);
    EXPECT_EQ(state->markedDamageForEngineOid(200), 0);
    EXPECT_FALSE(state->battlefieldGenerationByOid.contains(200));
    EXPECT_EQ(state->combatPowerForCreatureOid(200), 0);
    EXPECT_EQ(state->combatToughnessForCreatureOid(200), 1);
}

// ---------------------------------------------------------------------------------------
// Simultaneous trigger ordering (CR 603.3b)
// ---------------------------------------------------------------------------------------

namespace
{
/// A two-candidate ordering prompt addressed to `decidingPlayer`.
ruled::v1::RuledEventBatch triggerOrderBatch(int decidingPlayer)
{
    ruled::v1::RuledEventBatch batch;
    auto *tor = batch.add_events()->mutable_trigger_order_required();
    tor->set_deciding_player_id(decidingPlayer);
    auto *first = tor->add_candidates();
    first->set_trigger_object_id(501);
    first->set_source_permanent_id(41);
    first->set_ability_index(0);
    first->set_source_card_name("Blood Artist");
    first->set_ability_text("Target player loses 1 life and you gain 1 life.");
    auto *second = tor->add_candidates();
    second->set_trigger_object_id(502);
    second->set_source_permanent_id(42);
    second->set_ability_index(0);
    second->set_source_card_name("Blood Artist");
    second->set_ability_text("Target player loses 1 life and you gain 1 life.");
    return batch;
}
} // namespace

TEST_F(RuledClientTest, TriggerOrderRequiredOpensTheOrderingChoiceForTheDecider)
{
    apply(triggerOrderBatch(kLocalPlayer));

    ASSERT_TRUE(state->hasPendingTriggerOrder());
    const auto candidates = state->triggerOrderCandidates();
    ASSERT_EQ(candidates.size(), 2);
    EXPECT_EQ(candidates[0].oid, 501u);
    EXPECT_EQ(candidates[0].sourceOid, 41u);
    EXPECT_EQ(candidates[0].cardName, QStringLiteral("Blood Artist"));
    // The ability text is what the popup annotates each card image with.
    EXPECT_FALSE(candidates[0].abilityText.isEmpty());
}

TEST_F(RuledClientTest, SessionResetClosesAnOpenTriggerOrderingChoice)
{
    apply(triggerOrderBatch(kLocalPlayer));
    ASSERT_TRUE(state->hasPendingTriggerOrder());

    int uiChangeCount = 0;
    bool active = true;
    QVector<RuledTriggerOrderCandidate> candidates;
    QObject::connect(state, &RuledClientState::triggerOrderUiChanged, state,
                     [&](bool nextActive, QVector<RuledTriggerOrderCandidate> nextCandidates) {
                         ++uiChangeCount;
                         active = nextActive;
                         candidates = std::move(nextCandidates);
                     });

    // Concession ends the current ruled session; starting the next game reuses this TabGame.
    // The ordering popup is non-closeable, so teardown must explicitly tell its UI owner to
    // destroy it rather than merely clearing the view-model choice.
    state->clearSessionState();

    EXPECT_EQ(uiChangeCount, 1);
    EXPECT_FALSE(active);
    EXPECT_TRUE(candidates.isEmpty());
    EXPECT_FALSE(state->hasPendingTriggerOrder());
}

TEST_F(RuledClientTest, FullSessionResetExpiresPromptAckAndRequiresFreshRcrWhenIdIsReused)
{
    constexpr quint64 promptId = 9301;
    ruled::v1::RuledEventBatch paymentPrompt;
    auto *snapshot = paymentPrompt.mutable_resolution_prompt_state();
    snapshot->set_active(true);
    snapshot->set_deciding_player_id(kLocalPlayer);
    snapshot->set_resolution_prompt_id(promptId);
    auto *paymentChoice = paymentPrompt.add_events()->mutable_resolution_choice_required();
    paymentChoice->set_deciding_player_id(kLocalPlayer);
    paymentChoice->set_choice_kind(ruled::v1::CHOICE_KIND_MANA_PAYMENT);
    paymentChoice->set_resolution_prompt_id(promptId);
    paymentChoice->set_source_object_id(445);
    paymentChoice->set_generic_mana_cost(1);
    paymentChoice->set_payment_currently_legal(true);
    applyRaw(paymentPrompt);
    ASSERT_TRUE(state->isCurrentResolutionPrompt(promptId));
    ASSERT_TRUE(state->isResolutionPaymentActive());

    state->payment.begin(true);
    state->payment.selection.mutable_mana()->set_c(1);
    state->declineResolutionMana();
    ASSERT_FALSE(state->pendingChoice.has_value());
    ASSERT_EQ(host.sentCommands.size(), 1);

    state->clearSessionState(RuledSessionResetScope::All);
    EXPECT_FALSE(state->resolutionPromptSnapshotValid);
    EXPECT_FALSE(state->resolutionPromptActive);
    EXPECT_EQ(state->resolutionPromptDeciderPlayerId, -1);
    EXPECT_EQ(state->resolutionPromptId, 0u);
    EXPECT_TRUE(state->resolutionPromptSemantics.empty());
    EXPECT_FALSE(state->isCurrentResolutionPrompt(promptId));

    host.answerPendingAck(false);
    EXPECT_FALSE(state->pendingChoice.has_value());
    EXPECT_FALSE(state->isResolutionPaymentActive());
    EXPECT_FALSE(state->payment.active);

    // New sessions may restart their prompt counter. Reusing the number without a matching new
    // RCR must stay unactionable; only the exact current offer may establish that identity again.
    ruled::v1::RuledEventBatch reusedIdWithoutOffer;
    auto *reusedSnapshot = reusedIdWithoutOffer.mutable_resolution_prompt_state();
    reusedSnapshot->set_active(true);
    reusedSnapshot->set_deciding_player_id(kLocalPlayer);
    reusedSnapshot->set_resolution_prompt_id(promptId);
    applyRaw(reusedIdWithoutOffer);
    EXPECT_FALSE(state->isCurrentResolutionPrompt(promptId));
    EXPECT_FALSE(state->pendingChoice.has_value());

    ruled::v1::RuledEventBatch newOffer;
    auto *newSnapshot = newOffer.mutable_resolution_prompt_state();
    newSnapshot->set_active(true);
    newSnapshot->set_deciding_player_id(kLocalPlayer);
    newSnapshot->set_resolution_prompt_id(promptId);
    auto *newChoice = newOffer.add_events()->mutable_resolution_choice_required();
    newChoice->set_deciding_player_id(kLocalPlayer);
    newChoice->set_choice_kind(ruled::v1::CHOICE_KIND_SPECIAL_CAST);
    newChoice->set_resolution_prompt_id(promptId);
    newChoice->add_candidate_object_ids(702);
    newChoice->add_candidate_selectable(true);
    applyRaw(newOffer);
    EXPECT_TRUE(state->isCurrentResolutionPrompt(promptId));
    EXPECT_TRUE(state->isPendingSpecialCastCandidate(702));
}

TEST_F(RuledClientTest, TriggerOrderRequiredIsNotPromptedForTheOpponent)
{
    apply(triggerOrderBatch(kLocalPlayer + 1));

    EXPECT_FALSE(state->hasPendingTriggerOrder());
    EXPECT_TRUE(state->triggerOrderCandidates().isEmpty());
    EXPECT_TRUE(state->isWaitingForChoice());
    EXPECT_EQ(state->choiceWaitingPlayer(), kLocalPlayer + 1);

    apply(triggerOrderBatch(kLocalPlayer));
    EXPECT_FALSE(state->isWaitingForChoice());
    EXPECT_TRUE(state->hasPendingTriggerOrder());
}

TEST_F(RuledClientTest, TriggerOrderPopupCardsMapToTheirTriggers)
{
    apply(triggerOrderBatch(kLocalPlayer));

    // The popup identifies cards by index; those are the only ids that count as candidates.
    EXPECT_TRUE(state->isTriggerOrderPickCard(0));
    EXPECT_TRUE(state->isTriggerOrderPickCard(1));
    EXPECT_FALSE(state->isTriggerOrderPickCard(2));
    EXPECT_FALSE(state->isTriggerOrderPickCard(501));
}

TEST_F(RuledClientTest, ClickingAnOrderingCardPlacesThatTriggerImmediately)
{
    apply(triggerOrderBatch(kLocalPlayer));

    host.sentCommands.clear();
    state->pickTriggerOrderCard(1); // the second candidate

    // One click is one placement: no confirm step, and the oid is the clicked card's trigger.
    ASSERT_EQ(host.sentCommands.size(), 1);
    EXPECT_EQ(host.sentCommands[0].submit_trigger_order().trigger_object_id(), 502u);
    // Cleared straight away — the engine replies with either a target prompt or a shorter
    // ordering prompt, and a lingering popup would invite a click it is about to refuse.
    EXPECT_FALSE(state->hasPendingTriggerOrder());
}

TEST_F(RuledClientTest, ClickingANonCandidateCardSendsNothing)
{
    apply(triggerOrderBatch(kLocalPlayer));

    host.sentCommands.clear();
    state->pickTriggerOrderCard(7);

    EXPECT_TRUE(host.sentCommands.empty());
    EXPECT_TRUE(state->hasPendingTriggerOrder());
}

TEST_F(RuledClientTest, PlacingOneTriggerAndReofferingTheRestStaysActiveInOneBatch)
{
    apply(triggerOrderBatch(kLocalPlayer));
    ASSERT_TRUE(state->hasPendingTriggerOrder());

    // The engine's reply to a pick is one batch that both puts the chosen trigger on the stack and
    // re-offers the rest. It must net out to "still ordering" — if the StackPushed clear won, the
    // popup would be torn down and rebuilt, losing its position mid-choice.
    ruled::v1::RuledEventBatch reply;
    auto *sp = reply.add_events()->mutable_stack_pushed();
    sp->set_object_id(501);
    sp->set_description("Blood Artist");
    sp->set_is_triggered(true);
    auto *tor = reply.add_events()->mutable_trigger_order_required();
    tor->set_deciding_player_id(kLocalPlayer);
    auto *remaining = tor->add_candidates();
    remaining->set_trigger_object_id(502);
    remaining->set_source_card_name("Blood Artist");
    remaining->set_ability_text("Target player loses 1 life and you gain 1 life.");
    apply(reply);

    ASSERT_TRUE(state->hasPendingTriggerOrder());
    ASSERT_EQ(state->triggerOrderCandidates().size(), 1);
    EXPECT_EQ(state->triggerOrderCandidates()[0].oid, 502u);
}

TEST_F(RuledClientTest, ARepeatedPromptReplacesTheCandidatesWithWhatIsLeft)
{
    // The engine re-sends the prompt after each pick with one fewer candidate; the popup is
    // rebuilt from whatever the latest prompt carries.
    apply(triggerOrderBatch(kLocalPlayer));
    ASSERT_EQ(state->triggerOrderCandidates().size(), 2);

    ruled::v1::RuledEventBatch second;
    auto *tor = second.add_events()->mutable_trigger_order_required();
    tor->set_deciding_player_id(kLocalPlayer);
    auto *only = tor->add_candidates();
    only->set_trigger_object_id(502);
    only->set_source_card_name("Blood Artist");
    only->set_ability_text("Target player loses 1 life and you gain 1 life.");
    apply(second);

    ASSERT_EQ(state->triggerOrderCandidates().size(), 1);
    EXPECT_EQ(state->triggerOrderCandidates()[0].oid, 502u);
    EXPECT_TRUE(state->isTriggerOrderPickCard(0));
    EXPECT_FALSE(state->isTriggerOrderPickCard(1));
}

TEST_F(RuledClientTest, StackPushedForACandidateClearsTheTriggerOrderState)
{
    apply(triggerOrderBatch(kLocalPlayer));
    ASSERT_TRUE(state->hasPendingTriggerOrder());

    // The engine reserved 501 for this trigger, so seeing it on the stack proves the prompt was
    // answered — covers reconnects and resynced batches, where no local submit happened.
    ruled::v1::RuledEventBatch pushed;
    auto *sp = pushed.add_events()->mutable_stack_pushed();
    sp->set_object_id(501);
    sp->set_description("Blood Artist");
    sp->set_is_triggered(true);
    apply(pushed);

    EXPECT_FALSE(state->hasPendingTriggerOrder());
}

TEST_F(RuledClientTest, TriggerOrderReplacesAnyStalePendingChoice)
{
    // One pending choice at a time: an ordering prompt must tear down whatever was parked before.
    ruled::v1::RuledEventBatch tnt;
    auto *needs = tnt.add_events()->mutable_trigger_needs_target();
    needs->set_controller_player_id(kLocalPlayer);
    needs->set_source_permanent_id(41);
    needs->set_ability_text("Target player loses 1 life.");
    apply(tnt);
    ASSERT_TRUE(state->hasPendingTriggerTarget());

    apply(triggerOrderBatch(kLocalPlayer));

    EXPECT_TRUE(state->hasPendingTriggerOrder());
    EXPECT_FALSE(state->hasPendingTriggerTarget());
}

// ---------------------------------------------------------------------------------------
// Tier-3 resolution choices (CR 608)
// ---------------------------------------------------------------------------------------

TEST_F(RuledClientTest, ManaPaymentChoiceCreatesRefreshesAndSerializesDecisions)
{
    QSignalSpy paymentUiSpy(state, &RuledClientState::resolutionPaymentUiChanged);
    auto paymentBatch = [](bool legal, quint64 promptId) {
        ruled::v1::RuledEventBatch batch;
        auto *snapshot = batch.mutable_resolution_prompt_state();
        snapshot->set_active(true);
        snapshot->set_deciding_player_id(kLocalPlayer);
        snapshot->set_resolution_prompt_id(promptId);
        auto *rcr = batch.add_events()->mutable_resolution_choice_required();
        rcr->set_deciding_player_id(kLocalPlayer);
        rcr->set_resolution_prompt_id(promptId);
        rcr->set_choice_kind(ruled::v1::CHOICE_KIND_MANA_PAYMENT);
        rcr->set_prompt_text("Pay {4} or decline.");
        rcr->set_generic_mana_cost(4);
        rcr->set_payment_currently_legal(legal);
        return batch;
    };

    apply(paymentBatch(false, 9401));
    ASSERT_EQ(paymentUiSpy.count(), 1);
    EXPECT_TRUE(paymentUiSpy.at(0).at(0).toBool());
    ASSERT_TRUE(state->isResolutionPaymentActive());
    EXPECT_EQ(state->resolutionPaymentGenericCost(), 4);
    EXPECT_EQ(state->resolutionPaymentRemainingCost(), QStringLiteral("{4}"));
    EXPECT_FALSE(state->resolutionPaymentCurrentlyLegal());
    state->payResolutionMana();
    EXPECT_TRUE(host.sentCommands.isEmpty());

    state->payment.begin(true);
    ruled::v1::RuledCommand paymentCommand;
    paymentCommand.mutable_submit_resolution_choice()->set_decision(ruled::v1::RESOLUTION_CHOICE_DECISION_PAY_MANA);
    const auto previewRequest = state->payment.requestAction(paymentCommand);
    ruled::v1::RuledEventBatch previewBatch;
    auto *preview = previewBatch.mutable_payment_preview();
    preview->set_transaction_id(previewRequest.transaction_id());
    preview->set_revision(previewRequest.revision());
    preview->set_valid(true);
    preview->set_remaining_cost("{3}");
    apply(previewBatch);
    EXPECT_EQ(state->resolutionPaymentRemainingCost(), QStringLiteral("{3}"));

    // This is a new logical payment occurrence: its fresh ID retires the previous prompt's
    // staging even though source and cost match. Readiness-only changes for the same occurrence
    // keep the ID and payment staging, as covered by the same-ID refresh regression.
    apply(paymentBatch(true, 9402));
    ASSERT_GE(paymentUiSpy.count(), 2);
    EXPECT_TRUE(paymentUiSpy.last().at(0).toBool());
    ASSERT_TRUE(state->resolutionPaymentCurrentlyLegal());
    state->payResolutionMana();
    ASSERT_EQ(host.sentCommands.size(), 1);
    EXPECT_EQ(host.sentCommands.last().submit_resolution_choice().resolution_prompt_id(), 9402u);
    EXPECT_EQ(host.sentCommands.last().submit_resolution_choice().decision(),
              ruled::v1::RESOLUTION_CHOICE_DECISION_PAY_MANA);
    EXPECT_EQ(host.sentCommands.last().submit_resolution_choice().chosen_object_ids_size(), 0);
    EXPECT_FALSE(state->isResolutionPaymentActive());

    apply(paymentBatch(false, 9403));
    ASSERT_TRUE(state->isResolutionPaymentActive());
    ASSERT_TRUE(state->pendingChoice.has_value());
    EXPECT_EQ(state->resolutionPromptId, 9403u);
    EXPECT_EQ(state->pendingChoice->resolutionPromptId, 9403u);
    host.answerPendingAck(false);
    EXPECT_TRUE(state->isResolutionPaymentActive());
    EXPECT_EQ(state->resolutionPromptId, 9403u)
        << "a rejected ACK from the prior affordability offer must not restore its retired choice";
    ASSERT_TRUE(state->pendingChoice.has_value());
    EXPECT_EQ(state->pendingChoice->resolutionPromptId, 9403u);
    EXPECT_FALSE(state->resolutionPaymentCurrentlyLegal());
}

TEST_F(RuledClientTest, SameManaPaymentReadinessRefreshRetainsQueuedManaAndPreviewProgression)
{
    constexpr quint64 promptId = 9410;
    constexpr quint32 sourceObjectId = 442;
    auto paymentBatch = [=](bool legal) {
        ruled::v1::RuledEventBatch batch;
        auto *snapshot = batch.mutable_resolution_prompt_state();
        snapshot->set_active(true);
        snapshot->set_deciding_player_id(kLocalPlayer);
        snapshot->set_resolution_prompt_id(promptId);
        auto *rcr = batch.add_events()->mutable_resolution_choice_required();
        rcr->set_deciding_player_id(kLocalPlayer);
        rcr->set_resolution_prompt_id(promptId);
        rcr->set_source_object_id(sourceObjectId);
        rcr->set_choice_kind(ruled::v1::CHOICE_KIND_MANA_PAYMENT);
        rcr->set_prompt_text("Pay {3} or decline.");
        rcr->set_generic_mana_cost(3);
        rcr->set_payment_currently_legal(legal);
        return batch;
    };
    apply(paymentBatch(false));
    ASSERT_TRUE(state->isResolutionPaymentActive());
    EXPECT_FALSE(state->resolutionPaymentCurrentlyLegal());

    state->payment.begin(true);
    ASSERT_TRUE(state->payment.stageMana('R', 0, 17));
    ruled::v1::RuledCommand payCommand;
    auto *pay = payCommand.mutable_submit_resolution_choice();
    pay->set_resolution_prompt_id(promptId);
    pay->set_decision(ruled::v1::RESOLUTION_CHOICE_DECISION_PAY_MANA);
    const auto firstRequest = state->payment.requestAction(payCommand);
    ASSERT_EQ(firstRequest.resolution_choice().payment().mana().r(), 1u);
    ASSERT_TRUE(state->payment.stageMana('R', 0, 18));
    ASSERT_EQ(state->payment.queuedMana.size(), 1);
    const auto transaction = state->payment.transaction();

    const auto previewBatchFor = [sourceObjectId](const ruled::v1::PreviewPayment &request,
                                                  quint32 selectedRed, bool complete,
                                                  const std::string &remainingCost) {
        ruled::v1::RuledEventBatch batch;
        auto *preview = batch.mutable_payment_preview();
        preview->set_transaction_id(request.transaction_id());
        preview->set_revision(request.revision());
        preview->set_valid(true);
        preview->set_complete(complete);
        preview->set_total_cost("{3}");
        preview->set_remaining_cost(remainingCost);
        *preview->mutable_selection() = request.resolution_choice().payment();
        preview->mutable_selection()->set_expected_state_revision(request.revision());
        preview->mutable_selection()->mutable_source()->set_object_id(sourceObjectId);
        preview->mutable_selection()->mutable_source()->set_zone_change_generation(2);
        preview->mutable_selection()->mutable_mana()->set_r(selectedRed);
        return batch;
    };

    apply(previewBatchFor(firstRequest, 1, false, "{2}"));
    ASSERT_EQ(state->payment.selection.mana().r(), 1u);
    ASSERT_EQ(state->payment.nextPreviewAction(false), RuledPayment::PreviewAction::DrainQueuedMana);
    const auto secondMana = state->payment.queuedMana.takeFirst();
    ASSERT_TRUE(state->payment.payMana(secondMana.symbol, secondMana.groupId, secondMana.counterId));
    const auto secondRequest = state->payment.requestAction(payCommand);
    ASSERT_EQ(secondRequest.transaction_id(), transaction);
    ASSERT_EQ(secondRequest.resolution_choice().payment().mana().r(), 2u);

    // The third Mountain activation arrives while the second preview is still in flight.
    ASSERT_TRUE(state->payment.stageMana('R', 0, 19));
    ASSERT_EQ(state->payment.queuedMana.size(), 1);
    apply(paymentBatch(true));
    EXPECT_TRUE(state->resolutionPaymentCurrentlyLegal());
    ASSERT_TRUE(state->pendingChoice.has_value());
    EXPECT_TRUE(state->pendingChoice->paymentCurrentlyLegal);
    EXPECT_EQ(state->resolutionPromptId, promptId);
    EXPECT_TRUE(state->payment.active);
    EXPECT_TRUE(state->payment.pending);
    EXPECT_EQ(state->payment.transaction(), transaction);
    EXPECT_EQ(state->payment.selection.mana().r(), 2u);
    ASSERT_EQ(state->payment.queuedMana.size(), 1);

    apply(previewBatchFor(secondRequest, 2, false, "{1}"));
    ASSERT_EQ(state->payment.selection.mana().r(), 2u);
    ASSERT_EQ(state->payment.nextPreviewAction(false), RuledPayment::PreviewAction::DrainQueuedMana);
    const auto thirdMana = state->payment.queuedMana.takeFirst();
    ASSERT_TRUE(state->payment.payMana(thirdMana.symbol, thirdMana.groupId, thirdMana.counterId));
    const auto thirdRequest = state->payment.requestAction(payCommand);
    ASSERT_EQ(thirdRequest.transaction_id(), transaction);
    ASSERT_EQ(thirdRequest.resolution_choice().payment().mana().r(), 3u);

    apply(previewBatchFor(thirdRequest, 3, true, "{0}"));
    EXPECT_EQ(state->payment.nextPreviewAction(false), RuledPayment::PreviewAction::None);
    state->payResolutionMana();
    ASSERT_EQ(host.sentCommands.size(), 1);
    const auto &submitted = host.sentCommands.last().submit_resolution_choice();
    EXPECT_EQ(submitted.resolution_prompt_id(), promptId);
    EXPECT_EQ(submitted.decision(), ruled::v1::RESOLUTION_CHOICE_DECISION_PAY_MANA);
    EXPECT_EQ(submitted.payment().source().object_id(), sourceObjectId);
    EXPECT_EQ(submitted.payment().mana().r(), 3u);
    EXPECT_EQ(state->payment.transaction(), transaction);
}

TEST_F(RuledClientTest, VariableManaContributionWaitsForPreviewAndCanExplicitlyPayZero)
{
    ruled::v1::RuledEventBatch batch;
    auto *rcr = batch.add_events()->mutable_resolution_choice_required();
    rcr->set_deciding_player_id(kLocalPlayer);
    rcr->set_choice_kind(ruled::v1::CHOICE_KIND_MANA_PAYMENT);
    rcr->set_prompt_text("Choose any amount of mana to contribute.");
    rcr->set_payment_currently_legal(true);
    rcr->set_variable_mana_contribution(true);
    apply(batch);

    ASSERT_TRUE(state->isResolutionPaymentActive());
    EXPECT_TRUE(state->isVariableManaContribution());
    EXPECT_FALSE(state->resolutionPaymentCurrentlyLegal());
    state->declineResolutionMana();
    state->payResolutionMana();
    EXPECT_TRUE(host.sentCommands.isEmpty());

    state->payment.begin(true);
    ruled::v1::RuledCommand requestCommand;
    requestCommand.mutable_submit_resolution_choice()->set_decision(ruled::v1::RESOLUTION_CHOICE_DECISION_PAY_MANA);
    const auto request = state->payment.requestAction(requestCommand);
    ruled::v1::RuledEventBatch previewBatch;
    auto *preview = previewBatch.mutable_payment_preview();
    preview->set_transaction_id(request.transaction_id());
    preview->set_revision(request.revision());
    preview->set_valid(true);
    preview->set_complete(true);
    preview->set_total_cost("{0}");
    preview->set_remaining_cost("{0}");
    apply(previewBatch);

    ASSERT_TRUE(state->resolutionPaymentCurrentlyLegal());
    state->payResolutionMana();
    ASSERT_EQ(host.sentCommands.size(), 1);
    const auto &submitted = host.sentCommands.last().submit_resolution_choice();
    EXPECT_EQ(submitted.decision(), ruled::v1::RESOLUTION_CHOICE_DECISION_PAY_MANA);
    EXPECT_EQ(submitted.payment().mana().w() + submitted.payment().mana().u() + submitted.payment().mana().b() +
                  submitted.payment().mana().r() + submitted.payment().mana().g() + submitted.payment().mana().c(),
              0u);
    EXPECT_TRUE(state->payment.submitting);

    host.answerPendingAck(false);
    EXPECT_TRUE(state->isResolutionPaymentActive());
    EXPECT_TRUE(state->isVariableManaContribution());
    EXPECT_FALSE(state->payment.submitting);
    EXPECT_TRUE(state->payment.pending);
}

TEST_F(RuledClientTest, RejectedManaPaymentRestoresThePrompt)
{
    ruled::v1::RuledEventBatch batch;
    auto *rcr = batch.add_events()->mutable_resolution_choice_required();
    rcr->set_deciding_player_id(kLocalPlayer);
    rcr->set_choice_kind(ruled::v1::CHOICE_KIND_MANA_PAYMENT);
    rcr->set_prompt_text("Pay {4} or decline.");
    rcr->set_generic_mana_cost(4);
    rcr->set_payment_currently_legal(true);
    apply(batch);

    state->declineResolutionMana();
    ASSERT_FALSE(state->isResolutionPaymentActive());
    ASSERT_EQ(host.sentCommands.size(), 1);
    EXPECT_EQ(host.sentCommands.last().submit_resolution_choice().decision(),
              ruled::v1::RESOLUTION_CHOICE_DECISION_DECLINE);
    host.answerPendingAck(false);
    EXPECT_TRUE(state->isResolutionPaymentActive());
    EXPECT_TRUE(state->resolutionPaymentCurrentlyLegal());
}

TEST_F(RuledClientTest, RetiringPromptAfterSubmissionExpiresRejectedPaymentAck)
{
    constexpr quint64 promptId = 9201;
    ruled::v1::RuledEventBatch active;
    auto *snapshot = active.mutable_resolution_prompt_state();
    snapshot->set_active(true);
    snapshot->set_deciding_player_id(kLocalPlayer);
    snapshot->set_resolution_prompt_id(promptId);
    auto *choice = active.add_events()->mutable_resolution_choice_required();
    choice->set_deciding_player_id(kLocalPlayer);
    choice->set_choice_kind(ruled::v1::CHOICE_KIND_MANA_PAYMENT);
    choice->set_resolution_prompt_id(promptId);
    choice->set_source_object_id(440);
    choice->set_generic_mana_cost(1);
    choice->set_payment_currently_legal(true);
    applyRaw(active);
    ASSERT_TRUE(state->isResolutionPaymentActive());

    state->payment.begin(true);
    state->payment.selection.mutable_mana()->set_c(1);
    QSignalSpy finished(state, &RuledClientState::resolutionPaymentSubmissionFinished);
    state->declineResolutionMana();
    ASSERT_FALSE(state->pendingChoice.has_value());
    ASSERT_EQ(host.sentCommands.size(), 1);
    const quint64 submittedRevision = state->pendingChoiceRevision;

    ruled::v1::RuledEventBatch inactive;
    inactive.mutable_resolution_prompt_state()->set_active(false);
    inactive.mutable_resolution_prompt_state()->set_deciding_player_id(0);
    applyRaw(inactive);
    EXPECT_GT(state->pendingChoiceRevision, submittedRevision);
    EXPECT_FALSE(state->pendingChoice.has_value());
    EXPECT_FALSE(state->payment.active);

    host.answerPendingAck(false);
    EXPECT_EQ(finished.count(), 0) << "the retired prompt's rejection callback must be ignored";
    EXPECT_FALSE(state->pendingChoice.has_value());
    EXPECT_FALSE(state->isResolutionPaymentActive());
    EXPECT_FALSE(state->payment.active);
}

TEST_F(RuledClientTest, SamePromptRefreshDoesNotExpireSubmittedPaymentAck)
{
    constexpr quint64 promptId = 9202;
    ruled::v1::RuledEventBatch active;
    auto *snapshot = active.mutable_resolution_prompt_state();
    snapshot->set_active(true);
    snapshot->set_deciding_player_id(kLocalPlayer);
    snapshot->set_resolution_prompt_id(promptId);
    auto *choice = active.add_events()->mutable_resolution_choice_required();
    choice->set_deciding_player_id(kLocalPlayer);
    choice->set_choice_kind(ruled::v1::CHOICE_KIND_MANA_PAYMENT);
    choice->set_resolution_prompt_id(promptId);
    choice->set_source_object_id(441);
    choice->set_generic_mana_cost(1);
    choice->set_payment_currently_legal(true);
    applyRaw(active);
    ASSERT_TRUE(state->isResolutionPaymentActive());

    state->payment.begin(true);
    state->payment.selection.mutable_mana()->set_c(1);
    QSignalSpy finished(state, &RuledClientState::resolutionPaymentSubmissionFinished);
    state->declineResolutionMana();
    ASSERT_FALSE(state->pendingChoice.has_value());
    ASSERT_EQ(host.sentCommands.size(), 1);
    const quint64 submittedRevision = state->pendingChoiceRevision;

    ruled::v1::RuledEventBatch refresh;
    refresh.mutable_resolution_prompt_state()->CopyFrom(active.resolution_prompt_state());
    applyRaw(refresh);
    EXPECT_EQ(state->pendingChoiceRevision, submittedRevision);
    EXPECT_EQ(state->resolutionPromptId, promptId);
    EXPECT_FALSE(state->pendingChoice.has_value());

    host.answerPendingAck(false);
    EXPECT_EQ(finished.count(), 1);
    ASSERT_TRUE(state->isResolutionPaymentActive());
    EXPECT_EQ(state->pendingChoice->resolutionPromptId, promptId);
    EXPECT_TRUE(state->payment.active);
    EXPECT_FALSE(state->payment.submitting);
}

enum class ChoiceAckVariant
{
    Payment,
    BranchDecline,
    SpecialCast,
    Replacement,
    BranchSelect,
    Defender
};

ruled::v1::RuledEventBatch authoritySnapshot(bool localAuthority = false)
{
    ruled::v1::RuledEventBatch batch;
    batch.add_events()->mutable_active_public_reveal_snapshot();
    auto *view = batch.add_events()->mutable_zone_view();
    view->set_battlefields_unchanged(true);
    view->add_per_player()->set_player_id(kLocalPlayer);
    if (localAuthority)
        (*batch.mutable_legal_by_player())[kLocalPlayer];
    return batch;
}

RuledClientState::RuledPendingChoice authorityChoice(ChoiceAckVariant variant)
{
    using Kind = RuledClientState::ChoiceKind;
    RuledClientState::RuledPendingChoice choice;
    choice.promptText = QStringLiteral("Original choice");
    RuledChoiceOption option;
    option.enabled = true;
    option.label = QStringLiteral("Published option");
    option.index = 4;
    switch (variant) {
        case ChoiceAckVariant::Payment:
            choice.kind = Kind::ResolutionPayment;
            choice.paymentSourceOid = 900;
            choice.genericManaCost = 4;
            choice.paymentCurrentlyLegal = true;
            break;
        case ChoiceAckVariant::BranchDecline:
        case ChoiceAckVariant::BranchSelect:
            choice.kind = Kind::ResolutionBranch;
            choice.mayDecline = true;
            break;
        case ChoiceAckVariant::SpecialCast:
            choice.kind = Kind::SpecialCast;
            choice.candidateOids = {700};
            option.index = 0;
            break;
        case ChoiceAckVariant::Replacement:
            choice.kind = Kind::ReplacementOption;
            option.index = 7001;
            break;
        case ChoiceAckVariant::Defender: {
            choice.kind = Kind::AttackingTokenDefender;
            ruled::v1::CombatDefenderOption defender;
            defender.mutable_defender()->set_kind(ruled::v1::TARGET_REF_KIND_PERMANENT);
            defender.mutable_defender()->set_object_id(500);
            defender.set_defender_zone_change_generation(9);
            defender.set_defending_player_id(kOpponent);
            choice.combatDefenderOptions.append(defender);
            break;
        }
    }
    choice.choiceOptions.append(option);
    return choice;
}

void submitAuthorityChoice(RuledClientState *state, ChoiceAckVariant variant)
{
    switch (variant) {
        case ChoiceAckVariant::Payment:
            state->declineResolutionMana();
            break;
        case ChoiceAckVariant::BranchDecline:
            state->declinePendingClickChoice();
            break;
        case ChoiceAckVariant::SpecialCast:
            state->submitPendingChoiceOption(0);
            break;
        case ChoiceAckVariant::Replacement:
            state->submitPendingChoiceOption(7001);
            break;
        case ChoiceAckVariant::BranchSelect:
            state->submitPendingChoiceOption(4);
            break;
        case ChoiceAckVariant::Defender:
            EXPECT_TRUE(state->chooseAttackPermanentDefender(500));
            break;
    }
}

TEST_F(RuledClientTest, AuthoritativeRevocationClearsLocalActionsAndPreservesPhysicalStack)
{
    auto choice = authorityChoice(ChoiceAckVariant::Payment);
    choice.kind = RuledClientState::ChoiceKind::ResolutionPick;
    choice.pickZone = RuledClientState::PickZone::Hand;
    state->setPendingChoice(choice);
    state->pendingSpellCast.emplace();
    state->pendingSpellCast->set_transaction_id(23);
    state->stackOidOrder = {900};
    state->engineOidToCardId.insert(900, 42);
    state->payment.begin();
    ASSERT_TRUE(state->payment.stageMana(QChar('U'), 0, 51));
    state->payment.clear();
    state->payment.begin();
    ASSERT_TRUE(state->payment.stageMana(QChar('U'), 0, 52));
    const auto revision = state->pendingChoiceRevision;
    QSignalSpy picker(state, &RuledClientState::resolutionHandPickUiChanged);
    apply(authoritySnapshot());
    EXPECT_FALSE(state->pendingChoice.has_value());
    EXPECT_FALSE(state->pendingSpellCast.has_value());
    EXPECT_FALSE(state->payment.active);
    EXPECT_NE(state->pendingChoiceRevision, revision);
    EXPECT_TRUE(state->payment.takeAllOptimisticManaCounterIds().isEmpty());
    EXPECT_EQ(state->stackOidOrder, QVector<quint32>({900}));
    EXPECT_EQ(state->engineOidToCardId.value(900), 42);
    EXPECT_TRUE(host.removedSyntheticCards.isEmpty());
    EXPECT_TRUE(host.sentCommands.isEmpty());
    ASSERT_FALSE(picker.isEmpty());
    EXPECT_EQ(picker.last().at(0).toInt(), -1);
}

TEST_F(RuledClientTest, EmptyAuthoritySnapshotRetiresPriorityOnBothRecipients)
{
    for (int recipient : {kLocalPlayer, kOpponent}) {
        host.local = recipient;
        state->currentPriorityPlayerId = recipient;
        host.priorityPlayer = recipient;
        apply(authoritySnapshot());
        EXPECT_EQ(state->currentPriorityPlayerId, -1);
        EXPECT_EQ(host.priorityPlayer, -1);
    }
}

TEST_F(RuledClientTest, EmptyLocalAuthorityPreservesRetainedAndFreshRemotePriority)
{
    for (bool freshPriority : {false, true}) {
        host.local = kOpponent;
        state->currentPriorityPlayerId = kLocalPlayer;
        host.priorityPlayer = kLocalPlayer;
        auto batch = authoritySnapshot();
        if (freshPriority)
            batch.add_events()->mutable_priority_changed()->set_player_id(kLocalPlayer);
        apply(batch);
        EXPECT_EQ(state->currentPriorityPlayerId, kLocalPlayer);
        EXPECT_EQ(host.priorityPlayer, kLocalPlayer);
    }
}

TEST_F(RuledClientTest, AuthorityMarkerPreservesLivePlayersAndBothCombatPreviews)
{
    for (int variant : {0, 1, 2}) {
        state->setPendingChoice(authorityChoice(ChoiceAckVariant::Payment));
        state->payment.begin();
        const auto revision = state->pendingChoiceRevision;
        const auto transaction = state->payment.transaction();
        auto batch = authoritySnapshot(variant == 0);
        if (variant == 1)
            batch.add_events()->mutable_attackers_preview()->set_declaring_player_id(kLocalPlayer);
        if (variant == 2)
            batch.add_events()->mutable_blockers_preview()->set_declaring_player_id(kLocalPlayer);
        apply(batch);
        EXPECT_TRUE(state->isResolutionPaymentActive());
        EXPECT_TRUE(state->payment.active);
        EXPECT_EQ(state->pendingChoiceRevision, revision);
        EXPECT_EQ(state->payment.transaction(), transaction);
    }
}

TEST_F(RuledClientTest, AuthorityRevocationPreservesFreshOpponentWaiting)
{
    state->setPendingChoice(authorityChoice(ChoiceAckVariant::Payment));
    state->payment.begin();
    auto batch = authoritySnapshot();
    auto *choice = batch.add_events()->mutable_resolution_choice_required();
    choice->set_deciding_player_id(kOpponent);
    choice->set_choice_kind(ruled::v1::CHOICE_KIND_MANA_PAYMENT);
    choice->set_generic_mana_cost(2);
    apply(batch);
    EXPECT_FALSE(state->pendingChoice.has_value());
    EXPECT_FALSE(state->payment.active);
    EXPECT_TRUE(state->isWaitingForChoice());
    EXPECT_EQ(state->choiceWaitingPlayer(), kOpponent);
}

TEST_F(RuledClientTest, AuthorityRevocationSupersedesAllSixLateChoiceAcknowledgements)
{
    for (const auto variant :
         {ChoiceAckVariant::Payment, ChoiceAckVariant::BranchDecline, ChoiceAckVariant::SpecialCast,
          ChoiceAckVariant::Replacement, ChoiceAckVariant::BranchSelect, ChoiceAckVariant::Defender}) {
        for (const bool revoked : {false, true}) {
            for (const bool accepted : {false, true}) {
                SCOPED_TRACE(::testing::Message()
                             << static_cast<int>(variant) << ", revoked=" << revoked << ", accepted=" << accepted);
                host.sentCommands.clear();
                const auto original = authorityChoice(variant);
                stageChoiceForTest(original);
                state->payment.begin();
                submitAuthorityChoice(state, variant);
                ASSERT_EQ(host.sentCommands.size(), 1);
                ASSERT_FALSE(state->pendingChoice.has_value());
                if (revoked)
                    apply(authoritySnapshot());
                const auto revision = state->pendingChoiceRevision;
                const auto transaction = state->payment.transaction();
                QSignalSpy finished(state, &RuledClientState::resolutionPaymentSubmissionFinished);
                host.answerPendingAck(accepted);
                EXPECT_EQ(state->pendingChoice.has_value(), !revoked && !accepted);
                if (revoked) {
                    EXPECT_FALSE(state->payment.active);
                    EXPECT_EQ(state->pendingChoiceRevision, revision);
                    EXPECT_EQ(state->payment.transaction(), transaction);
                    EXPECT_EQ(finished.count(), 0);
                } else if (!accepted) {
                    EXPECT_EQ(state->pendingChoice->kind, original.kind);
                    EXPECT_EQ(state->pendingChoice->promptText, original.promptText);
                }
            }
        }
    }
}

TEST_F(RuledClientTest, AuthorityChoiceRevisionProtectsNewPaymentFromSameSource)
{
    stageChoiceForTest(authorityChoice(ChoiceAckVariant::Payment));
    state->payment.begin();
    state->declineResolutionMana();
    auto next = authoritySnapshot(true);
    auto *choice = next.add_events()->mutable_resolution_choice_required();
    choice->set_deciding_player_id(kLocalPlayer);
    choice->set_choice_kind(ruled::v1::CHOICE_KIND_MANA_PAYMENT);
    choice->set_source_object_id(900);
    choice->set_generic_mana_cost(7);
    choice->set_payment_currently_legal(true);
    choice->set_prompt_text("New payment");
    apply(next);
    state->payment.begin();
    const auto transaction = state->payment.transaction();
    QSignalSpy finished(state, &RuledClientState::resolutionPaymentSubmissionFinished);
    host.answerPendingAck(false);
    ASSERT_TRUE(state->isResolutionPaymentActive());
    EXPECT_EQ(state->resolutionPaymentGenericCost(), 7);
    EXPECT_EQ(state->pendingChoice->promptText, QStringLiteral("New payment"));
    EXPECT_EQ(state->payment.transaction(), transaction);
    EXPECT_EQ(finished.count(), 0);
}

TEST_F(RuledClientTest, ManaPaymentPromptsOnlyTheDecidingPlayer)
{
    ruled::v1::RuledEventBatch batch;
    auto *rcr = batch.add_events()->mutable_resolution_choice_required();
    rcr->set_deciding_player_id(kLocalPlayer + 1);
    rcr->set_choice_kind(ruled::v1::CHOICE_KIND_MANA_PAYMENT);
    rcr->set_generic_mana_cost(4);
    rcr->set_payment_currently_legal(true);
    apply(batch);
    EXPECT_FALSE(state->isResolutionPaymentActive());
    EXPECT_TRUE(state->isWaitingForChoice());
    EXPECT_EQ(state->choiceWaitingPlayer(), kOpponent);

    ruled::v1::RuledEventBatch completed;
    apply(completed);
    EXPECT_FALSE(state->isWaitingForChoice());
}

TEST(RuledCardPopupIdentityTest, ExistingCardRefAndDismissalArePreserved)
{
    RuledCardPopupIdentity identity;
    const CardRef known{QStringLiteral("Island"), QStringLiteral("printing")};
    const CardRef shown = identity.open(known, false, {});
    EXPECT_EQ(shown.name, known.name);
    EXPECT_EQ(shown.providerId, known.providerId);
    EXPECT_EQ(identity.close(known.name), known.name);

    const CardRef empty{};
    const CardRef emptyShown = identity.open(empty, false, {});
    EXPECT_TRUE(emptyShown.isEmpty());
    EXPECT_TRUE(identity.close(QString{}).isEmpty());
    identity.open(empty, true, QStringLiteral("Island"));
    EXPECT_EQ(identity.close(QStringLiteral("_")), QStringLiteral("_"));
}

TEST_F(RuledClientTest, FaceDownObjectMapProvidesPrivateIdentityAndPrunesOnReplacement)
{
    ruled::v1::RuledEventBatch batch;
    auto *entry = batch.add_events()->mutable_face_down_object_map()->add_entries();
    entry->set_viewer_player_id(kLocalPlayer);
    entry->set_engine_object_id(9801u);
    entry->set_zone_change_generation(4u);
    entry->set_server_card_id(17);
    entry->set_card_name("Grizzly Bears");
    entry->set_zone_player_id(kLocalPlayer);
    auto *sameIdFromAnotherZone =
        batch.mutable_events(0)->mutable_face_down_object_map()->add_entries();
    sameIdFromAnotherZone->set_viewer_player_id(kLocalPlayer);
    sameIdFromAnotherZone->set_engine_object_id(9803u);
    sameIdFromAnotherZone->set_zone_change_generation(5u);
    sameIdFromAnotherZone->set_server_card_id(17);
    sameIdFromAnotherZone->set_card_name("Island");
    sameIdFromAnotherZone->set_zone_player_id(kOpponent);
    apply(batch);

    EXPECT_EQ(state->privateFaceDownNameForCard(kLocalPlayer, 17), QString("Grizzly Bears"));
    EXPECT_EQ(state->privateFaceDownNameForCard(kOpponent, 17), QString("Island"));
    EXPECT_EQ(state->privateFaceDownGenerationByOid.value(9801u), 4u);
    EXPECT_EQ(state->privateFaceDownGenerationByOid.value(9803u), 5u);
    EXPECT_TRUE(state->privateFaceDownNameForCard(-1, 17).isEmpty());

    ruled::v1::RuledEventBatch cleared;
    cleared.add_events()->mutable_face_down_object_map();
    apply(cleared);
    EXPECT_TRUE(state->privateFaceDownNameForCard(kLocalPlayer, 17).isEmpty());
    EXPECT_TRUE(state->privateFaceDownNameForCard(kOpponent, 17).isEmpty());
    EXPECT_FALSE(state->privateFaceDownGenerationByOid.contains(9801u));
}

// Exercises the same headless identity adapter used by the popup signal hook; no widget is rendered.
TEST_F(RuledClientTest, FaceDownExilePopupUsesPrivateMapAndDismissesTheShownName)
{
    ruled::v1::RuledEventBatch reconnect;
    auto *entry = reconnect.add_events()->mutable_face_down_object_map()->add_entries();
    entry->set_viewer_player_id(kLocalPlayer);
    entry->set_engine_object_id(9803u);
    entry->set_zone_change_generation(5u);
    entry->set_server_card_id(17);
    entry->set_card_name("Island");
    entry->set_zone_player_id(kOpponent);
    apply(reconnect);

    RuledCardPopupIdentity jar;
    const CardRef blank{};
    const CardRef unauthorized = jar.open(blank, true, state->privateFaceDownNameForCard(kLocalPlayer, 18));
    EXPECT_TRUE(unauthorized.isEmpty());
    EXPECT_TRUE(jar.close(QString{}).isEmpty());
    const CardRef staleCommon{QStringLiteral("Grizzly Bears"), QStringLiteral("old printing")};
    const CardRef staleUnauthorized = jar.open(staleCommon, true, state->privateFaceDownNameForCard(kLocalPlayer, 18));
    EXPECT_TRUE(staleUnauthorized.isEmpty());
    EXPECT_TRUE(jar.close(staleCommon.name).isEmpty());

    RuledCardPopupIdentity hideaway;
    ASSERT_EQ(state->privateFaceDownNameForCard(kOpponent, 17), QStringLiteral("Island"));
    const CardRef authorized = hideaway.open(blank, true, state->privateFaceDownNameForCard(kOpponent, 17));
    EXPECT_EQ(authorized.name, QStringLiteral("Island"));
    EXPECT_TRUE(authorized.providerId.isEmpty());
    ruled::v1::RuledEventBatch clear;
    clear.add_events()->mutable_face_down_object_map();
    apply(clear);
    EXPECT_TRUE(state->privateFaceDownNameForCard(kOpponent, 17).isEmpty());
    EXPECT_EQ(hideaway.close(QString{}), QStringLiteral("Island"));

    RuledCardPopupIdentity ordinary;
    const CardRef known{QStringLiteral("Forest"), QStringLiteral("printing")};
    const CardRef shown = ordinary.open(known, false, QStringLiteral("Island"));
    EXPECT_EQ(shown.name, known.name);
    EXPECT_EQ(shown.providerId, known.providerId);
    EXPECT_EQ(ordinary.close(known.name), known.name);
}

TEST_F(RuledClientTest, MultiplePermanentActionsAreEngineAuthoredAndClearWhenNoLongerPublished)
{
    ruled::v1::RuledEventBatch batch;
    auto *action = (*batch.mutable_legal_by_player())[kLocalPlayer].add_permanent_actions();
    action->set_kind(ruled::v1::PERMANENT_ACTION_KIND_TURN_FACE_UP);
    action->set_object_id(9802u);
    action->set_zone_change_generation(9u);
    action->set_label("Turn face up — {1}{G}");
    action->set_mana_cost("{1}{G}");
    action->add_eligible_restricted_mana_group_ids(7u);
    auto *unlock = (*batch.mutable_legal_by_player())[kLocalPlayer].add_permanent_actions();
    unlock->set_kind(ruled::v1::PERMANENT_ACTION_KIND_UNLOCK_ROOM_DOOR);
    unlock->set_object_id(9802u);
    unlock->set_zone_change_generation(9u);
    unlock->set_label("Unlock Tunnel of Hate — {4}{R}{R}");
    unlock->set_mana_cost("{4}{R}{R}");
    unlock->set_face_index(1u);
    unlock->add_eligible_restricted_mana_group_ids(8u);
    apply(batch);

    const auto parsed = state->permanentActionsForOid(9802u);
    ASSERT_EQ(parsed.size(), 2);
    EXPECT_EQ(parsed[0].kind, ruled::v1::PERMANENT_ACTION_KIND_TURN_FACE_UP);
    EXPECT_EQ(parsed[0].zoneChangeGeneration, 9u);
    EXPECT_EQ(parsed[0].label, QString("Turn face up — {1}{G}"));
    EXPECT_EQ(parsed[0].manaCost, QString("{1}{G}"));
    EXPECT_EQ(parsed[0].eligibleRestrictedManaGroupIds, QSet<quint32>({7u}));
    EXPECT_FALSE(parsed[0].faceIndex.has_value());
    EXPECT_EQ(parsed[1].kind, ruled::v1::PERMANENT_ACTION_KIND_UNLOCK_ROOM_DOOR);
    ASSERT_TRUE(parsed[1].faceIndex.has_value());
    EXPECT_EQ(*parsed[1].faceIndex, 1u);
    EXPECT_EQ(parsed[1].eligibleRestrictedManaGroupIds, QSet<quint32>({8u}));
    const auto current = state->permanentActionFor(9802u, 9u, ruled::v1::PERMANENT_ACTION_KIND_UNLOCK_ROOM_DOOR,
                                                   std::optional<quint32>{1u});
    ASSERT_TRUE(current.has_value());
    EXPECT_EQ(current->label, QString("Unlock Tunnel of Hate — {4}{R}{R}"));
    EXPECT_FALSE(state->permanentActionFor(9802u, 10u, ruled::v1::PERMANENT_ACTION_KIND_UNLOCK_ROOM_DOOR,
                                           std::optional<quint32>{1u}));
    PendingActivatedAbility pending;
    pending.valid = true;
    pending.permanentAction = true;
    pending.permanentActionKind = ruled::v1::PERMANENT_ACTION_KIND_UNLOCK_ROOM_DOOR;
    pending.permanentActionFaceIndex = 1u;
    pending.permanentOid = 9802u;
    pending.expectedZoneChangeGeneration = 9u;
    pending.abilityIndex = -1;
    EXPECT_TRUE(ruledPendingAbilitySourceStillCurrent(*state, pending));
    pending.expectedZoneChangeGeneration = 10u;
    EXPECT_FALSE(ruledPendingAbilitySourceStillCurrent(*state, pending));

    action->clear_eligible_restricted_mana_group_ids();
    action->add_eligible_restricted_mana_group_ids(9u);
    unlock->clear_eligible_restricted_mana_group_ids();
    apply(batch);
    const auto refreshed = state->permanentActionsForOid(9802u);
    ASSERT_EQ(refreshed.size(), 2);
    EXPECT_EQ(refreshed[0].eligibleRestrictedManaGroupIds, QSet<quint32>({9u}));
    EXPECT_TRUE(refreshed[1].eligibleRestrictedManaGroupIds.isEmpty());

    apply(ruled::v1::RuledEventBatch{});
    EXPECT_TRUE(state->permanentActionsForOid(9802u).isEmpty());
}

TEST_F(RuledClientTest, ZoneAbilityActionsBindHandSlotsAndGraveyardObjectsWithoutLeakingAcrossBatches)
{
    ruled::v1::RuledEventBatch batch;
    auto &actions = (*batch.mutable_legal_by_player())[kLocalPlayer];
    auto *hand = actions.add_zone_ability_actions();
    hand->set_source_zone(ruled::v1::ABILITY_SOURCE_ZONE_HAND);
    hand->set_object_id(7101u);
    hand->set_zone_change_generation(4u);
    hand->set_hand_index(3u);
    hand->set_ability_index(0u);
    hand->set_card_name("Shepherding Spirits");
    hand->mutable_ability()->set_text("Plainscycling {2}");
    hand->mutable_ability()->set_mana_cost("{2}");
    hand->mutable_ability()->set_cost_label("{2}, Discard this card");
    hand->mutable_ability()->set_activatable(true);

    auto *graveyard = actions.add_zone_ability_actions();
    graveyard->set_source_zone(ruled::v1::ABILITY_SOURCE_ZONE_GRAVEYARD);
    graveyard->set_object_id(7201u);
    graveyard->set_zone_change_generation(8u);
    graveyard->set_ability_index(2u);
    graveyard->set_card_name("Sagu Pummeler");
    graveyard->mutable_ability()->set_text("Renew {4}{G}");
    graveyard->mutable_ability()->set_mana_cost("{4}{G}");
    graveyard->mutable_ability()->set_activatable(false);
    apply(batch);

    EXPECT_EQ(state->zoneAbilityOidForHandSlot(3), 7101u);
    EXPECT_EQ(state->abilitySourceZone(7101u), ruled::v1::ABILITY_SOURCE_ZONE_HAND);
    EXPECT_EQ(state->abilitySourceGeneration(7101u), 4u);
    const auto handAbility = state->activatedAbilityForOid(7101u, 0);
    ASSERT_TRUE(handAbility);
    EXPECT_EQ(handAbility->text, QString("Plainscycling {2}"));
    EXPECT_EQ(handAbility->manaCost, QString("{2}"));
    EXPECT_TRUE(state->abilityActivatable(7101u, 0));
    EXPECT_EQ(state->abilitySourceZone(7201u), ruled::v1::ABILITY_SOURCE_ZONE_GRAVEYARD);
    EXPECT_EQ(state->activatedAbilityIndicesForOid(7201u), QList<int>({2}));
    EXPECT_FALSE(state->abilityActivatable(7201u, 2));

    apply(ruled::v1::RuledEventBatch{});
    EXPECT_EQ(state->zoneAbilityOidForHandSlot(3), 0u);
    EXPECT_TRUE(state->activatedAbilitiesForOid(7101u).isEmpty());
    EXPECT_TRUE(state->activatedAbilityIndicesForOid(7201u).isEmpty());
}

TEST_F(RuledClientTest, MerchantGraveyardActionSubmitsItsExactPublicZoneIdentity)
{
    ruled::v1::RuledEventBatch batch;
    auto &localActions = (*batch.mutable_legal_by_player())[kLocalPlayer];
    auto *merchant = localActions.add_zone_ability_actions();
    merchant->set_source_zone(ruled::v1::ABILITY_SOURCE_ZONE_GRAVEYARD);
    merchant->set_object_id(7301u);
    merchant->set_zone_change_generation(12u);
    merchant->set_ability_index(0u);
    merchant->set_card_name("Merchant of Many Hats");
    merchant->mutable_ability()->set_text("{2}{B}: Return this card from your graveyard to your hand.");
    merchant->mutable_ability()->set_mana_cost("{2}{B}");
    merchant->mutable_ability()->set_activatable(true);
    auto *opponentOnly = (*batch.mutable_legal_by_player())[kLocalPlayer + 1].add_zone_ability_actions();
    opponentOnly->set_source_zone(ruled::v1::ABILITY_SOURCE_ZONE_GRAVEYARD);
    opponentOnly->set_object_id(9999u);
    opponentOnly->set_zone_change_generation(1u);
    opponentOnly->set_ability_index(0u);
    opponentOnly->mutable_ability()->set_activatable(true);
    apply(batch);

    EXPECT_EQ(state->abilitySourceZone(7301u), ruled::v1::ABILITY_SOURCE_ZONE_GRAVEYARD);
    EXPECT_EQ(state->abilitySourceGeneration(7301u), 12u);
    EXPECT_TRUE(state->abilityActivatable(7301u, 0));
    EXPECT_TRUE(state->activatedAbilityIndicesForOid(9999u).isEmpty());

    PendingActivatedAbility pending;
    pending.permanentOid = 7301u;
    pending.abilityIndex = 0;
    pending.sourceZone = state->abilitySourceZone(7301u);
    pending.expectedZoneChangeGeneration = state->abilitySourceGeneration(7301u);
    ruled::v1::ActivateAbility command;
    pending.writeActivationHeader(command);
    EXPECT_EQ(command.source_object_id(), 7301u);
    EXPECT_EQ(command.source_zone(), ruled::v1::ABILITY_SOURCE_ZONE_GRAVEYARD);
    EXPECT_EQ(command.expected_zone_change_generation(), 12u);
}

TEST_F(RuledClientTest, CastAndCycleAreCombinedIntoOneCardActionMenuModel)
{
    RuledFaceOption face;
    face.faceIndex = 0;
    face.faceName = QStringLiteral("Shepherding Spirits");
    state->activatedAbilitiesByOid[203] = {
        RuledAbilityEntry{QStringLiteral("{2}, Discard this card: Plainscycling {2}"), "{2}", {}, {}, true}};
    const auto options = RuledPendingCast::cardActionMenuOptions({face}, *state, 203);

    ASSERT_EQ(options.size(), 2);
    EXPECT_EQ(options.at(0).kind, RuledCardActionMenuOption::Kind::CastFace);
    EXPECT_EQ(options.at(0).index, 0);
    EXPECT_EQ(options.at(0).label, QStringLiteral("Cast Shepherding Spirits"));
    EXPECT_TRUE(options.at(0).enabled);
    EXPECT_EQ(options.at(1).kind, RuledCardActionMenuOption::Kind::ActivateAbility);
    EXPECT_EQ(options.at(1).index, 0);
    EXPECT_EQ(options.at(1).label, QStringLiteral("{2}, Discard this card: Plainscycling {2}"));
    EXPECT_TRUE(options.at(1).enabled);
}

TEST_F(RuledClientTest, ManaPaymentMenuRetainsEveryEngineOptionAndItsIndex)
{
    RuledAbilityEntry anyColor{"Tap another permanent: Add one mana of any color.", {}, "W/U/B/R/G", {}, true};
    anyColor.manaAbility = true;
    RuledAbilityEntry blueOrGreen{"Add blue or green.", {}, "U/G", {}, false};
    blueOrGreen.manaAbility = true;
    state->activatedAbilitiesByOid[203] = {anyColor, std::nullopt, blueOrGreen};
    const auto options = RuledPendingCast::cardActionMenuOptions({}, *state, 203);
    ASSERT_EQ(options.size(), 7);
    for (int i = 0; i < 5; ++i) {
        EXPECT_EQ(options.at(i).index, 0);
        EXPECT_EQ(options.at(i).manaOptionIndex, i);
        EXPECT_TRUE(options.at(i).enabled);
    }
    EXPECT_TRUE(options.at(3).label.contains("{R}"));
    EXPECT_EQ(options.at(6).index, 2);
    EXPECT_EQ(options.at(6).manaOptionIndex, 1);
    EXPECT_FALSE(options.at(6).enabled);

    state->abilityCostData[RuledClientState::abilityTargetKey(203, 0)].nonManaCostsPayable = false;
    const auto blocked = RuledPendingCast::cardActionMenuOptions({}, *state, 203);
    ASSERT_EQ(blocked.size(), options.size());
    for (const auto &option : blocked)
        EXPECT_FALSE(option.enabled);
    EXPECT_FALSE(state->activatedAbilityForOid(203, 1));
    EXPECT_FALSE(state->activatedAbilityForOid(203, -1));
    EXPECT_FALSE(state->activatedAbilityForOid(203, 3));
    EXPECT_FALSE(state->activatedAbilityForOid(999, 0));
}

TEST_F(RuledClientTest, NestedPaymentMenuOffersOnlyEnginePublishedManaAbilities)
{
    RuledFaceOption face;
    face.faceName = QStringLiteral("Test spell");
    RuledAbilityEntry green{"{T}: Add {G}.", {}, "G", {}, true};
    green.manaAbility = true;
    RuledAbilityEntry blueOrGreen{"{T}: Add {U} or {G}.", {}, "U/G", {}, false};
    blueOrGreen.manaAbility = true;
    state->activatedAbilitiesByOid[203] = {RuledAbilityEntry{"Waterbend {5}: Put a counter.", {}, {}, {}, true}, green,
                                           blueOrGreen};
    const auto options = RuledPendingCast::cardActionMenuOptions({face}, *state, 203, true);
    ASSERT_EQ(options.size(), 3);
    EXPECT_EQ(options.at(0).kind, RuledCardActionMenuOption::Kind::ActivateAbility);
    EXPECT_EQ(options.at(0).index, 1);
    EXPECT_TRUE(options.at(0).enabled);
    EXPECT_EQ(options.at(2).index, 2);
    EXPECT_EQ(options.at(2).manaOptionIndex, 1);
    EXPECT_FALSE(options.at(2).enabled);
}

TEST_F(RuledClientTest, ZeroOutputManaAbilityRetainsItsSelectableOptionsDuringPayment)
{
    ruled::v1::RuledEventBatch batch;
    auto *object = batch.add_events()->mutable_zone_view()->add_per_player()->add_battlefield_objects();
    object->set_object_id(203);
    auto *ability = object->add_activated_abilities();
    ability->set_ability_index(0);
    ability->set_text("{T}: Choose a color. Add one mana of that color for each charge counter on this artifact.");
    ability->set_has_only_tap_cost(true);
    ability->set_activatable(true);
    ability->set_is_mana_ability(true);
    for (const auto *label : {"W", "U", "B", "R", "G"})
        ability->add_mana_option_labels(label);
    apply(batch);

    const auto decoded = state->activatedAbilityForOid(203, 0);
    ASSERT_TRUE(decoded);
    EXPECT_TRUE(decoded->manaProduced.isEmpty());
    EXPECT_EQ(decoded->manaOptionLabels, QStringList({"W", "U", "B", "R", "G"}));
    EXPECT_TRUE(decoded->usesDirectManaActivation());

    const auto options = RuledPendingCast::cardActionMenuOptions({}, *state, 203, true);
    ASSERT_EQ(options.size(), 5);
    for (int i = 0; i < 5; ++i) {
        EXPECT_EQ(options.at(i).kind, RuledCardActionMenuOption::Kind::ActivateAbility);
        EXPECT_EQ(options.at(i).index, 0);
        EXPECT_EQ(options.at(i).manaOptionIndex, i);
        EXPECT_TRUE(options.at(i).enabled);
    }
    EXPECT_TRUE(options.first().label.contains("Choose {W}"));

    const auto ordinaryOptions = RuledPendingCast::cardActionMenuOptions({}, *state, 203, false);
    ASSERT_EQ(ordinaryOptions.size(), 5);
    for (int i = 0; i < 5; ++i)
        EXPECT_EQ(ordinaryOptions.at(i).manaOptionIndex, i);
}

TEST(RuledPendingCastTest, ActivatedVariableManaCostScalesEveryXPipAndPreservesCancellation)
{
    PendingActivatedAbility pending;
    pending.permanentOid = 501;
    pending.abilityIndex = 2;
    pending.remainingCost = RuledPendingCast::parseSimpleManaCost("{X}{X}{U}");
    EXPECT_EQ(ruledActivatedManaXPipCount("{X}{X}{U}", false), 2);
    EXPECT_GT(ruledActivatedXChoiceMaximum(pending, 2), 99u);
    EXPECT_EQ(pending.remainingCost.value(QChar('X')), 2);
    EXPECT_FALSE(ruledApplyActivatedXChoice(pending, 2, std::nullopt));
    EXPECT_EQ(pending.xValue, 0u);
    EXPECT_EQ(pending.remainingCost.value(QChar('X')), 2);
    EXPECT_FALSE(ruledApplyActivatedXChoice(pending, 2, ruledActivatedXChoiceMaximum(pending, 2) + 1));
    EXPECT_EQ(pending.remainingCost.value(QChar('X')), 2);

    EXPECT_TRUE(ruledApplyActivatedXChoice(pending, 2, 2u));
    EXPECT_EQ(pending.xValue, 2u);
    EXPECT_EQ(pending.remainingCost.value(QChar('X')), 4);
    EXPECT_EQ(pending.remainingCost.value(QChar('U')), 1);
    ruled::v1::ActivateAbility command;
    pending.writeActivationHeader(command);
    EXPECT_EQ(command.x_value(), 2u);

    PendingActivatedAbility zero;
    zero.remainingCost = RuledPendingCast::parseSimpleManaCost("{2}{X}{X}");
    EXPECT_TRUE(ruledApplyActivatedXChoice(zero, 2, 0u));
    EXPECT_EQ(zero.remainingCost.value(QChar('X')), 2);
    EXPECT_EQ(zero.xValue, 0u);

    // Storage-counter X is chosen by its bounded split prompt, not by the mana-cost prompt.
    EXPECT_EQ(ruledActivatedManaXPipCount("{X}", true), 0);
    const auto split =
        ruledPromptXCounterManaSplit(4, [](RuledXCounterManaPromptStep step, quint32) -> std::optional<quint32> {
            return step == RuledXCounterManaPromptStep::ChooseX ? 3u : 2u;
        });
    ASSERT_TRUE(split);
    EXPECT_EQ(split->xValue, 3u);
    EXPECT_EQ(split->firstColorCount, 2u);

    RuledPendingCast transaction;
    auto &staged = transaction.beginAbility();
    staged.remainingCost = RuledPendingCast::parseSimpleManaCost("{X}{X}");
    EXPECT_TRUE(ruledApplyActivatedXChoice(staged, 2, 2u));
    transaction.clearAbility(); // same cleanup path after a rejected or cancelled activation
    auto &retry = transaction.beginAbility();
    EXPECT_EQ(retry.xValue, 0u);
    EXPECT_TRUE(retry.remainingCost.isEmpty());
}

TEST_F(RuledClientTest, EmptyCommanderIdentityStillUsesEnginePublishedManaAbilityClassification)
{
    ruled::v1::RuledEventBatch batch;
    auto *object = batch.add_events()->mutable_zone_view()->add_per_player()->add_battlefield_objects();
    object->set_object_id(203);
    auto *ability = object->add_activated_abilities();
    ability->set_ability_index(0);
    ability->set_text("{T}: Add one mana of any color in your commander's color identity.");
    ability->set_has_only_tap_cost(true);
    ability->set_activatable(true);
    ability->set_is_mana_ability(true);
    apply(batch);

    const auto decoded = state->activatedAbilityForOid(203, 0);
    ASSERT_TRUE(decoded);
    EXPECT_TRUE(decoded->manaProduced.isEmpty());
    EXPECT_TRUE(decoded->manaOptionLabels.isEmpty());
    EXPECT_TRUE(decoded->isManaAbility());
    EXPECT_TRUE(decoded->usesDirectManaActivation());
    EXPECT_EQ(decoded->manaOptionsForSelection(), QStringList({QString{}}));

    const auto options = RuledPendingCast::cardActionMenuOptions({}, *state, 203, true);
    ASSERT_EQ(options.size(), 1);
    EXPECT_EQ(options.first().kind, RuledCardActionMenuOption::Kind::ActivateAbility);
    EXPECT_EQ(options.first().index, 0);
    EXPECT_EQ(options.first().manaOptionIndex, 0);
    EXPECT_TRUE(options.first().enabled);
}

TEST_F(RuledClientTest, XCounterManaChoiceIsParsedAsAnEnginePublishedManaAbility)
{
    ruled::v1::RuledEventBatch batch;
    auto *object = batch.add_events()->mutable_zone_view()->add_per_player()->add_battlefield_objects();
    object->set_object_id(203);
    auto *ability = object->add_activated_abilities();
    ability->set_ability_index(2);
    ability->set_text("{1}, Remove X storage counters: Add X mana in any combination of {U} and {B}.");
    ability->set_mana_cost("{1}");
    ability->set_activatable(true);
    ability->set_is_mana_ability(true);
    auto *choice = ability->mutable_x_counter_mana_choice();
    choice->set_counter_label("storage");
    choice->set_max_x(4);
    choice->set_first_color("U");
    choice->set_second_color("B");
    apply(batch);

    const auto decoded = state->activatedAbilityForOid(203, 2);
    ASSERT_TRUE(decoded);
    ASSERT_TRUE(decoded->xCounterManaChoice);
    EXPECT_EQ(decoded->xCounterManaChoice->counterLabel, QStringLiteral("storage"));
    EXPECT_EQ(decoded->xCounterManaChoice->maxX, 4u);
    EXPECT_EQ(decoded->xCounterManaChoice->firstColor, QStringLiteral("U"));
    EXPECT_EQ(decoded->xCounterManaChoice->secondColor, QStringLiteral("B"));
    EXPECT_TRUE(decoded->isManaAbility());
    EXPECT_FALSE(decoded->usesDirectManaActivation());

    const auto nestedOptions = RuledPendingCast::cardActionMenuOptions({}, *state, 203, true);
    ASSERT_EQ(nestedOptions.size(), 1);
    EXPECT_EQ(nestedOptions.first().kind, RuledCardActionMenuOption::Kind::ActivateAbility);
    EXPECT_EQ(nestedOptions.first().index, 2);
    EXPECT_EQ(nestedOptions.first().manaOptionIndex, 0);
    EXPECT_TRUE(nestedOptions.first().enabled);
}

TEST_F(RuledClientTest, PaymentCandidateAndManaAbilityShareOneCardMenu)
{
    RuledAbilityEntry green{"{T}: Add {G}.", {}, "G", {}, true};
    green.manaAbility = true;
    state->activatedAbilitiesByOid[203] = {green};
    const QVector<QPair<int, QString>> waterbend = {
        {ruled::v1::OBJECT_PAYMENT_KIND_WATERBEND, QStringLiteral("Waterbend — pay {1}")}};
    const auto waterbendOptions = RuledPendingCast::cardActionMenuOptions({}, *state, 203, true, waterbend);
    ASSERT_EQ(waterbendOptions.size(), 2);
    EXPECT_EQ(waterbendOptions.at(0).kind, RuledCardActionMenuOption::Kind::PaymentContribution);
    EXPECT_EQ(waterbendOptions.at(0).index, ruled::v1::OBJECT_PAYMENT_KIND_WATERBEND);
    EXPECT_EQ(waterbendOptions.at(1).kind, RuledCardActionMenuOption::Kind::ActivateAbility);

    const QVector<QPair<int, QString>> convoke = {
        {ruled::v1::OBJECT_PAYMENT_KIND_GREEN, QStringLiteral("Convoke — pay {G}")},
        {ruled::v1::OBJECT_PAYMENT_KIND_GENERIC, QStringLiteral("Convoke — pay {1}")}};
    const auto convokeOptions = RuledPendingCast::cardActionMenuOptions({}, *state, 203, true, convoke);
    ASSERT_EQ(convokeOptions.size(), 3);
    EXPECT_EQ(convokeOptions.at(0).kind, RuledCardActionMenuOption::Kind::PaymentContribution);
    EXPECT_EQ(convokeOptions.at(1).kind, RuledCardActionMenuOption::Kind::PaymentContribution);
    EXPECT_EQ(convokeOptions.at(2).kind, RuledCardActionMenuOption::Kind::ActivateAbility);
}

TEST(RuledPendingCastTest, ActivationHeaderPreservesSelectedManaOptionAndSourceIdentity)
{
    PendingActivatedAbility pending;
    pending.permanentOid = 501;
    pending.abilityIndex = 2;
    pending.manaOptionIndex = 3;
    pending.expectedZoneChangeGeneration = 7;
    pending.xValue = 4;
    pending.manaSplitFirstColorCount = 1;
    ruled::v1::ActivateAbility command;
    pending.writeActivationHeader(command);
    EXPECT_EQ(command.source_object_id(), 501u);
    EXPECT_EQ(command.ability_index(), 2u);
    EXPECT_EQ(command.expected_zone_change_generation(), 7u);
    EXPECT_EQ(command.mana_option_index(), 3u);
    EXPECT_EQ(command.x_value(), 4u);
    EXPECT_EQ(command.mana_split_first_color_count(), 1u);
}

TEST_F(RuledClientTest, TriggerModesBecomePromptOptionsAndSubmitTheChosenMode)
{
    ruled::v1::RuledEventBatch batch;
    auto *tnt = batch.add_events()->mutable_trigger_needs_target();
    tnt->set_source_permanent_id(100);
    tnt->set_ability_index(2);
    tnt->set_ability_text("Choose one.");
    tnt->set_controller_player_id(kLocalPlayer);
    auto *life = tnt->add_modes();
    life->set_mode_index(0);
    life->set_label("Gain 4 life");
    life->set_selectable(true);
    auto *counter = tnt->add_modes();
    counter->set_mode_index(1);
    counter->set_label("Put a counter on it");
    counter->set_selectable(true);
    apply(batch);

    ASSERT_TRUE(state->hasPendingChoiceOfKind(RuledClientState::ChoiceKind::TriggerMode));
    ASSERT_EQ(state->pendingChoiceOptions().size(), 2);
    state->submitPendingChoiceOption(1);
    ASSERT_EQ(host.sentCommands.size(), 1);
    ASSERT_TRUE(host.sentCommands[0].has_choose_trigger_target());
    ASSERT_EQ(host.sentCommands[0].choose_trigger_target().selected_modes_size(), 1);
    EXPECT_EQ(host.sentCommands[0].choose_trigger_target().selected_modes(0).mode_index(), 1u);
}

TEST_F(RuledClientTest, TargetedTriggerModeCarriesItsModeIntoTheTargetCommand)
{
    ruled::v1::RuledEventBatch batch;
    auto *tnt = batch.add_events()->mutable_trigger_needs_target();
    tnt->set_source_permanent_id(100);
    tnt->set_ability_index(2);
    tnt->set_ability_text("Choose one.");
    tnt->set_controller_player_id(kLocalPlayer);
    auto *mode = tnt->add_modes();
    mode->set_mode_index(3);
    mode->set_label("Target creature gets +2/+2");
    mode->set_selectable(true);
    mode->set_needs_target(true);
    mode->mutable_targets()->add_groups()->add_valid_permanent_ids(101);
    apply(batch);

    QSignalSpy selectionChanged(state, &RuledClientState::triggerTargetSelectionChanged);
    state->submitPendingChoiceOption(3);
    EXPECT_EQ(selectionChanged.count(), 1);
    ASSERT_TRUE(state->hasPendingTriggerTarget());
    EXPECT_FALSE(state->hasPendingChoiceOptions());
    EXPECT_TRUE(host.sentCommands.isEmpty());
    EXPECT_TRUE(state->abilityTargetData(100, 2).validPermanentIds.contains(101));
    ruled::v1::ChooseTriggerTarget command;
    state->appendPendingTriggerMode(&command);
    ASSERT_EQ(command.selected_modes_size(), 1);
    EXPECT_EQ(command.selected_modes(0).mode_index(), 3u);
}

TEST_F(RuledClientTest, NonModalTriggerPublishesItsClickTargets)
{
    ruled::v1::RuledEventBatch batch;
    auto *tnt = batch.add_events()->mutable_trigger_needs_target();
    tnt->set_source_permanent_id(100);
    tnt->set_ability_index(2);
    tnt->set_ability_text("Deal 3 damage to any target.");
    tnt->set_controller_player_id(kLocalPlayer);
    auto *group = tnt->mutable_targets()->add_groups();
    group->set_group_index(0);
    group->set_min(1);
    group->set_max(1);
    group->set_prompt_text("Choose a target");
    group->add_valid_permanent_ids(101);
    group->set_can_target_opponent(true);
    apply(batch);

    ASSERT_TRUE(state->hasPendingTriggerTarget());
    EXPECT_EQ(state->pendingTriggerTargetPrompt(),
              QString::fromUtf8("Choose a target for “Deal 3 damage to any target.”"));
    EXPECT_EQ(state->pendingTriggerTargetDisplayText(),
              QString::fromUtf8("Choose a target for “Deal 3 damage to any target.”"));
    const auto targets = state->abilityTargetData(100, 2);
    EXPECT_TRUE(targets.validPermanentIds.contains(101));
    EXPECT_TRUE(targets.canTargetOpponent);
}

TEST_F(RuledClientTest, TriggerTargetPromptPreservesSpecificGuidance)
{
    ruled::v1::RuledEventBatch batch;
    auto *tnt = batch.add_events()->mutable_trigger_needs_target();
    tnt->set_source_permanent_id(100);
    tnt->set_ability_index(0);
    tnt->set_ability_text("Exile up to two target cards from a single graveyard.");
    tnt->set_controller_player_id(kLocalPlayer);
    auto *group = tnt->mutable_targets()->add_groups();
    group->set_group_index(0);
    group->set_prompt_text("Choose up to two target cards from a single graveyard.");
    group->set_min(0);
    group->set_max(2);
    apply(batch);

    EXPECT_EQ(state->pendingTriggerTargetPrompt(),
              QString::fromUtf8("Choose targets for “Exile up to two target cards from a single graveyard.”\n"
                                "Choose up to two target cards from a single graveyard."));
    EXPECT_EQ(state->pendingTriggerTargetDisplayText(),
              QString::fromUtf8("Choose targets for “Exile up to two target cards from a single graveyard.”\n"
                                "Choose up to two target cards from a single graveyard.\n"
                                "Selected: 0 (0–2)."));
}

TEST_F(RuledClientTest, TriggerTargetPromptAdvancesSequentialGroupGuidance)
{
    ruled::v1::RuledEventBatch batch;
    auto *tnt = batch.add_events()->mutable_trigger_needs_target();
    tnt->set_source_permanent_id(100);
    tnt->set_ability_index(0);
    tnt->set_ability_text("Choose one artifact and one creature.");
    tnt->set_controller_player_id(kLocalPlayer);
    auto *artifact = tnt->mutable_targets()->add_groups();
    artifact->set_group_index(3);
    artifact->set_prompt_text("Choose target artifact.");
    artifact->set_min(1);
    artifact->set_max(1);
    artifact->add_valid_permanent_ids(701);
    auto *creature = tnt->mutable_targets()->add_groups();
    creature->set_group_index(7);
    creature->set_prompt_text("Choose target creature.");
    creature->set_min(1);
    creature->set_max(1);
    creature->add_valid_permanent_ids(702);
    apply(batch);

    EXPECT_EQ(state->pendingTriggerTargetPrompt(),
              QString::fromUtf8("Choose targets for “Choose one artifact and one creature.”\n"
                                "Target 1 of 2: Choose target artifact."));
    ASSERT_TRUE(state->stagePendingTriggerTarget(ruled::v1::TARGET_REF_KIND_PERMANENT, 701, kLocalPlayer));
    EXPECT_EQ(state->pendingTriggerTargetPrompt(),
              QString::fromUtf8("Choose targets for “Choose one artifact and one creature.”\n"
                                "Target 2 of 2: Choose target creature."));
}

TEST_F(RuledClientTest, TargetPairsUseEngineCompatibilityAndConstrainedEmptyFailsClosed)
{
    ruled::v1::RuledEventBatch batch;
    auto *trigger = batch.add_events()->mutable_trigger_needs_target();
    trigger->set_source_permanent_id(100);
    trigger->set_controller_player_id(kLocalPlayer);
    auto *first = trigger->mutable_targets()->add_groups();
    first->set_group_index(0);
    first->set_min(1);
    first->set_max(1);
    first->add_valid_permanent_ids(701);
    first->add_valid_permanent_ids(702);
    auto *second = trigger->mutable_targets()->add_groups();
    second->set_group_index(1);
    second->set_min(1);
    second->set_max(1);
    second->add_valid_graveyard_ids(801);
    second->add_valid_graveyard_ids(802);
    auto *constraint = second->mutable_pair_constraint();
    constraint->set_prior_group_index(0);
    for (const auto &[prior, candidate] : {std::pair{701u, 801u}, std::pair{702u, 802u}}) {
        auto *pair = constraint->add_compatible_pairs();
        pair->mutable_prior_target()->set_kind(ruled::v1::TARGET_REF_KIND_PERMANENT);
        pair->mutable_prior_target()->set_object_id(prior);
        pair->mutable_candidate()->set_kind(ruled::v1::TARGET_REF_KIND_GRAVEYARD);
        pair->mutable_candidate()->set_object_id(candidate);
    }
    apply(batch);
    ASSERT_TRUE(state->stagePendingTriggerTarget(ruled::v1::TARGET_REF_KIND_PERMANENT, 701, kLocalPlayer));
    EXPECT_FALSE(state->isPendingTriggerTargetCandidate(ruled::v1::TARGET_REF_KIND_GRAVEYARD, 802, kLocalPlayer));
    EXPECT_TRUE(state->isPendingTriggerTargetCandidate(ruled::v1::TARGET_REF_KIND_GRAVEYARD, 801, kLocalPlayer));
    EXPECT_TRUE(host.sentCommands.empty());
    constraint->clear_compatible_pairs();
    apply(batch);
    ASSERT_TRUE(state->stagePendingTriggerTarget(ruled::v1::TARGET_REF_KIND_PERMANENT, 701, kLocalPlayer));
    EXPECT_FALSE(state->isPendingTriggerTargetCandidate(ruled::v1::TARGET_REF_KIND_GRAVEYARD, 801, kLocalPlayer));
    second->clear_pair_constraint();
    apply(batch);
    ASSERT_TRUE(state->stagePendingTriggerTarget(ruled::v1::TARGET_REF_KIND_PERMANENT, 701, kLocalPlayer));
    EXPECT_TRUE(state->isPendingTriggerTargetCandidate(ruled::v1::TARGET_REF_KIND_GRAVEYARD, 802, kLocalPlayer));
}

TEST_F(RuledClientTest, ResolutionBranchesSubmitOpaqueIndexWithoutOpeningADialog)
{
    ruled::v1::RuledEventBatch batch;
    auto *choice = batch.add_events()->mutable_resolution_choice_required();
    choice->set_deciding_player_id(kLocalPlayer);
    choice->set_choice_kind(ruled::v1::CHOICE_KIND_RESOLUTION_BRANCH);
    choice->set_prompt_text("Choose a payment.");
    choice->set_min(0);
    auto *sacrifice = choice->add_resolution_branches();
    sacrifice->set_branch_index(0);
    sacrifice->set_label("Sacrifice a creature");
    sacrifice->set_selectable(true);
    apply(batch);

    ASSERT_TRUE(state->hasPendingChoiceOfKind(RuledClientState::ChoiceKind::ResolutionBranch));
    EXPECT_EQ(host.dialogRequests, 0);
    state->submitPendingChoiceOption(0);
    ASSERT_EQ(host.sentCommands.size(), 1);
    const auto &submission = host.sentCommands[0].submit_resolution_choice();
    EXPECT_EQ(submission.decision(), ruled::v1::RESOLUTION_CHOICE_DECISION_SELECT_BRANCH);
    EXPECT_EQ(submission.selected_branch_index(), 0u);
}

TEST_F(RuledClientTest, ResolutionBranchesKeepDistinctEngineLabelsWhenOracleTextIsShared)
{
    const QString oracleText = QStringLiteral(
        "Counter target spell. Its controller may draw up to two cards at the beginning of the next turn's upkeep.");
    auto card = CardInfo::newInstance("Arcane Denial");
    card->ruled().addFace("Arcane Denial", "Arcane Denial", oracleText);
    host.presentationCards.insert("Arcane Denial", card);

    ruled::v1::RuledEventBatch batch;
    auto *choice = batch.add_events()->mutable_resolution_choice_required();
    choice->set_deciding_player_id(kLocalPlayer);
    choice->set_choice_kind(ruled::v1::CHOICE_KIND_RESOLUTION_BRANCH);
    choice->set_prompt_text("Choose how many cards the affected player draws.");
    choice->set_min(1);
    choice->set_max(1);
    const QStringList labels = {QStringLiteral("The affected player draws no cards."),
                                QStringLiteral("The affected player draws a card."),
                                QStringLiteral("The affected player draws 2 cards.")};
    for (int index = 0; index < labels.size(); ++index) {
        auto *branch = choice->add_resolution_branches();
        branch->set_branch_index(static_cast<quint32>(index));
        branch->set_label(labels[index].toStdString());
        branch->set_selectable(true);
        auto *presentation = branch->mutable_presentation();
        presentation->set_external_card_name("Arcane Denial");
        presentation->set_external_face_name("Arcane Denial");
        presentation->set_oracle_text_sha256(RuledOracleText::textSha256(oracleText).toStdString());
        presentation->add_oracle_line_indices(1);
        presentation->set_fallback_text(labels[index].toStdString());
    }
    auto *unresolvedBranch = choice->add_resolution_branches();
    unresolvedBranch->set_branch_index(static_cast<quint32>(labels.size()));
    unresolvedBranch->set_label("Choice (unresolved_branch)");
    unresolvedBranch->set_selectable(true);
    auto *unresolvedPresentation = unresolvedBranch->mutable_presentation();
    unresolvedPresentation->set_external_card_name("Arcane Denial");
    unresolvedPresentation->set_external_face_name("Arcane Denial");
    unresolvedPresentation->set_oracle_text_sha256(RuledOracleText::textSha256(oracleText).toStdString());
    unresolvedPresentation->add_oracle_line_indices(1);
    unresolvedPresentation->set_fallback_text("Choice (unresolved_branch)");

    apply(batch);

    const auto options = state->pendingChoiceOptions();
    ASSERT_EQ(options.size(), labels.size() + 1);
    for (int index = 0; index < labels.size(); ++index) {
        EXPECT_EQ(options[index].label, labels[index]);
    }
    EXPECT_EQ(options.last().label, oracleText);
}

TEST_F(RuledClientTest, PrivateDiscardDestinationUsesPromptOptionsWithOpaqueIds)
{
    ruled::v1::RuledEventBatch batch;
    auto *choice = batch.add_events()->mutable_resolution_choice_required();
    choice->set_deciding_player_id(kLocalPlayer);
    choice->set_choice_kind(ruled::v1::CHOICE_KIND_PRIVATE_REPLACEMENT);
    choice->set_prompt_text("Choose the discard destination.");
    choice->set_min(1);
    choice->set_max(1);
    choice->add_candidate_object_ids(2);
    choice->add_candidate_object_ids(1);
    choice->add_candidate_names("Exile Fiery Temper (madness)");
    choice->add_candidate_names("Discard Fiery Temper to library (Library of Leng)");
    apply(batch);
    EXPECT_EQ(host.dialogRequests, 0);
    ASSERT_TRUE(state->hasPendingChoiceOptions());
    ASSERT_EQ(state->pendingChoiceOptions().size(), 2);
    EXPECT_EQ(state->pendingChoiceOptions()[0].index, 2);
    state->submitPendingChoiceOption(2);
    ASSERT_EQ(host.sentCommands.size(), 1);
    const auto &answer = host.sentCommands[0].submit_resolution_choice();
    EXPECT_EQ(answer.decision(), ruled::v1::RESOLUTION_CHOICE_DECISION_UNSPECIFIED);
    ASSERT_EQ(answer.chosen_object_ids_size(), 1);
    EXPECT_EQ(answer.chosen_object_ids(0), 2u);
}

TEST_F(RuledClientTest, PrivateDiscardOrderingUsesCardImagesAndClickOrder)
{
    QSignalSpy started(state, &RuledClientState::librarySearchPickStarted);
    ruled::v1::RuledEventBatch batch;
    auto *choice = batch.add_events()->mutable_resolution_choice_required();
    choice->set_deciding_player_id(kLocalPlayer);
    choice->set_choice_kind(ruled::v1::CHOICE_KIND_PRIVATE_REPLACEMENT);
    choice->set_prompt_text("Order discarded cards, top of library first.");
    choice->set_min(2);
    choice->set_max(2);
    choice->set_ordered(true);
    for (int i = 0; i < 2; ++i) {
        choice->add_candidate_object_ids(700 + i);
        choice->add_candidate_server_card_ids(i);
        choice->add_candidate_names("Forest"); // Duplicate names must remain distinct cards.
    }
    apply(batch);
    EXPECT_EQ(host.dialogRequests, 0);
    ASSERT_TRUE(state->isResolutionHandPickActive());
    EXPECT_EQ(started.count(), 1);
    EXPECT_EQ(state->resolutionHandPickViewTitle(), QStringLiteral("Order discarded cards"));
    EXPECT_FALSE(state->resolutionHandPickShowViewControls());
    state->toggleResolutionHandPickCard(1);
    state->toggleResolutionHandPickCard(0);
    state->submitResolutionHandPick();
    ASSERT_EQ(host.sentCommands.size(), 1);
    const auto &answer = host.sentCommands[0].submit_resolution_choice();
    ASSERT_EQ(answer.chosen_object_ids_size(), 2);
    EXPECT_EQ(answer.chosen_object_ids(0), 700u);
    EXPECT_EQ(answer.chosen_object_ids(1), 701u);
}

TEST_F(RuledClientTest, MadnessPaymentDoesNotTreatItsOwnOfferAsABlockingChoice)
{
    for (const auto method : {ruled::v1::CAST_METHOD_MADNESS, ruled::v1::CAST_METHOD_SIEGE_DEFEAT,
                              ruled::v1::CAST_METHOD_PERMISSION}) {
        ruled::v1::RuledEventBatch batch;
        auto *choice = batch.add_events()->mutable_resolution_choice_required();
        choice->set_deciding_player_id(kLocalPlayer);
        choice->set_choice_kind(ruled::v1::CHOICE_KIND_SPECIAL_CAST);
        choice->add_candidate_object_ids(700);
        choice->add_candidate_selectable(true);
        auto *offer = (*batch.mutable_legal_by_player())[kLocalPlayer].add_zone_cast_actions();
        offer->set_object_id(700);
        offer->set_source_zone(ruled::v1::CAST_SOURCE_ZONE_EXILE);
        offer->set_zone_change_generation(3);
        offer->set_cast_method(method);
        offer->set_casting_permission_id(901);
        offer->set_cost("{R}");
        apply(batch);
        RuledPendingCast pending;
        auto &spell = pending.beginSpell();
        spell.handIndex = 700;
        spell.source = RuledCastSource::Exile;
        spell.sourceZoneChangeGeneration = 3;
        spell.castMethod = method;
        spell.castingPermissionId = 901;
        spell.resolutionPromptId = state->resolutionPromptId;
        EXPECT_FALSE(pending.resolutionChoiceBlocksSpell(*state));
        EXPECT_TRUE(RuledPendingCast::matchesSpecialCastOffer(spell, *state));
        spell.waitingForTarget = true;
        EXPECT_FALSE(pending.resolutionChoiceBlocksSpell(*state));
        spell.waitingForTarget = false;
        // The same ownership query lets the prompt yield to targeting or mana payment.
        auto suspended = spell;
        pending.clearSpell();
        EXPECT_FALSE(RuledPendingCast::matchesSpecialCastOffer(pending.spell, *state));
        EXPECT_TRUE(RuledPendingCast::matchesSpecialCastOffer(suspended, *state));
        pending.spell = suspended;
        spell.sourceZoneChangeGeneration = 2;
        EXPECT_TRUE(pending.resolutionChoiceBlocksSpell(*state));
        spell.sourceZoneChangeGeneration = 3;
        spell.castingPermissionId = 902;
        EXPECT_TRUE(pending.resolutionChoiceBlocksSpell(*state));
        spell.castingPermissionId = 901;
        state->pendingChoice->candidateOids = {701};
        EXPECT_TRUE(pending.resolutionChoiceBlocksSpell(*state));
        state->pendingChoice->kind = RuledClientState::ChoiceKind::ResolutionBranch;
        EXPECT_TRUE(pending.resolutionChoiceBlocksSpell(*state));
    }
}

TEST_F(RuledClientTest, SpecialPlayOfferUsesPlayLabel)
{
    ruled::v1::RuledEventBatch batch;
    auto *choice = batch.add_events()->mutable_resolution_choice_required();
    choice->set_deciding_player_id(kLocalPlayer);
    choice->set_choice_kind(ruled::v1::CHOICE_KIND_SPECIAL_CAST);
    choice->set_prompt_text("P4: play linked Forest without paying its mana cost?");
    choice->add_candidate_object_ids(700);
    choice->add_candidate_selectable(true);
    apply(batch);

    ASSERT_TRUE(state->hasPendingChoiceOfKind(RuledClientState::ChoiceKind::SpecialCast));
    const auto options = state->pendingChoiceOptions();
    ASSERT_EQ(options.size(), 2);
    EXPECT_EQ(options.at(1).label, QStringLiteral("Play"));
}

TEST_F(RuledClientTest, SpecialCastKeepsOfferWhileStagingAndOnlyDeclinesExplicitly)
{
    ruled::v1::RuledEventBatch observerReturn;
    auto *observerChoice = observerReturn.add_events()->mutable_resolution_choice_required();
    observerChoice->set_deciding_player_id(kLocalPlayer);
    observerChoice->set_choice_kind(ruled::v1::CHOICE_KIND_AURA_PERMANENT);
    observerChoice->add_candidate_object_ids(601);
    apply(observerReturn);
    ASSERT_TRUE(state->hasPendingChoiceOfKind(RuledClientState::ChoiceKind::AuraPermanent));

    ruled::v1::RuledEventBatch batch;
    auto *choice = batch.add_events()->mutable_resolution_choice_required();
    choice->set_deciding_player_id(kLocalPlayer);
    choice->set_choice_kind(ruled::v1::CHOICE_KIND_SPECIAL_CAST);
    choice->set_prompt_text("Cast Grandmother Ravi Sengir transformed?");
    choice->add_candidate_object_ids(700);
    choice->add_candidate_selectable(true);
    apply(batch);

    ASSERT_TRUE(state->hasPendingChoiceOfKind(RuledClientState::ChoiceKind::SpecialCast));
    EXPECT_FALSE(state->hasPendingChoiceOfKind(RuledClientState::ChoiceKind::AuraPermanent));
    ASSERT_EQ(state->pendingChoiceOptions().size(), 2);
    quint32 requested = 0;
    QObject::connect(state, &RuledClientState::specialCastRequested, state,
                     [&requested](quint32 oid) { requested = oid; });
    host.sentCommands.clear();
    state->submitPendingChoiceOption(1);
    EXPECT_EQ(requested, 700u);
    EXPECT_TRUE(host.sentCommands.empty());
    EXPECT_TRUE(state->hasPendingChoiceOfKind(RuledClientState::ChoiceKind::SpecialCast));

    apply(batch);
    host.sentCommands.clear();
    state->submitPendingChoiceOption(0);
    ASSERT_EQ(host.sentCommands.size(), 1);
    EXPECT_EQ(host.sentCommands[0].submit_resolution_choice().decision(),
              ruled::v1::RESOLUTION_CHOICE_DECISION_DECLINE);
    EXPECT_FALSE(host.sentCommands[0].submit_resolution_choice().has_cast_spell());
}

TEST_F(RuledClientTest, SpecialCastUsesEngineSelectabilityAndAlwaysAllowsDecline)
{
    for (const int flagCount : {0, 1, 2}) {
        ruled::v1::RuledEventBatch batch;
        auto *choice = batch.add_events()->mutable_resolution_choice_required();
        choice->set_deciding_player_id(kLocalPlayer);
        choice->set_choice_kind(ruled::v1::CHOICE_KIND_SPECIAL_CAST);
        choice->add_candidate_object_ids(700);
        for (int i = 0; i < flagCount; ++i)
            choice->add_candidate_selectable(false);
        apply(batch);
        ASSERT_TRUE(state->hasPendingChoiceOfKind(RuledClientState::ChoiceKind::SpecialCast));
        ASSERT_EQ(state->pendingChoiceOptions().size(), 2);
        EXPECT_TRUE(state->pendingChoiceOptions()[0].enabled);
        EXPECT_FALSE(state->pendingChoiceOptions()[1].enabled);
        QSignalSpy castRequested(state, &RuledClientState::specialCastRequested);
        host.sentCommands.clear();
        state->submitPendingChoiceOption(1);
        EXPECT_EQ(castRequested.count(), 0);
        EXPECT_TRUE(host.sentCommands.empty());
        EXPECT_TRUE(state->hasPendingChoiceOfKind(RuledClientState::ChoiceKind::SpecialCast));
        state->submitPendingChoiceOption(0);
        ASSERT_EQ(host.sentCommands.size(), 1);
        EXPECT_EQ(host.sentCommands[0].submit_resolution_choice().decision(),
                  ruled::v1::RESOLUTION_CHOICE_DECISION_DECLINE);
    }
}

TEST_F(RuledClientTest, VariableTriggerTargetsStageOneGraveyardCohortAndRestoreOnRejection)
{
    QSignalSpy selectionChanged(state, &RuledClientState::triggerTargetSelectionChanged);
    ruled::v1::RuledEventBatch batch;
    auto *mapping = batch.add_events()->mutable_graveyard_object_map();
    auto *mineOne = mapping->add_entries();
    mineOne->set_player_id(kLocalPlayer);
    mineOne->set_engine_object_id(501);
    mineOne->set_server_card_id(11);
    auto *mineTwo = mapping->add_entries();
    mineTwo->set_player_id(kLocalPlayer);
    mineTwo->set_engine_object_id(502);
    mineTwo->set_server_card_id(12);
    auto *theirs = mapping->add_entries();
    theirs->set_player_id(kOpponent);
    theirs->set_engine_object_id(601);
    theirs->set_server_card_id(21);
    auto *tnt = batch.add_events()->mutable_trigger_needs_target();
    tnt->set_source_permanent_id(100);
    tnt->set_ability_index(0);
    tnt->set_ability_text("Exile up to two target cards from a single graveyard.");
    tnt->set_controller_player_id(kLocalPlayer);
    auto *group = tnt->mutable_targets()->add_groups();
    group->set_group_index(0);
    group->set_prompt_text("Choose up to two target cards from a single graveyard");
    group->set_min(0);
    group->set_max(2);
    group->set_same_graveyard(true);
    group->add_valid_graveyard_ids(501);
    group->add_valid_graveyard_ids(502);
    group->add_valid_graveyard_ids(601);
    apply(batch);

    EXPECT_TRUE(state->stagePendingTriggerTarget(ruled::v1::TARGET_REF_KIND_GRAVEYARD, 501, kLocalPlayer));
    EXPECT_EQ(selectionChanged.count(), 1);
    EXPECT_FALSE(state->stagePendingTriggerTarget(ruled::v1::TARGET_REF_KIND_GRAVEYARD, 601, kLocalPlayer));
    EXPECT_EQ(selectionChanged.count(), 1);
    EXPECT_TRUE(state->stagePendingTriggerTarget(ruled::v1::TARGET_REF_KIND_GRAVEYARD, 502, kLocalPlayer));
    EXPECT_EQ(selectionChanged.count(), 2);
    EXPECT_EQ(state->pendingTriggerSelectedCount(), 2);
    EXPECT_TRUE(state->isPendingTriggerTargetSelected(501));
    EXPECT_TRUE(state->isPendingTriggerTargetSelected(502));

    state->confirmPendingTriggerTargets();
    ASSERT_EQ(host.sentCommands.size(), 1);
    ASSERT_EQ(host.sentCommands[0].choose_trigger_target().targets_size(), 2);
    EXPECT_TRUE(state->hasPendingTriggerTarget());
    host.answerPendingAck(false);
    EXPECT_TRUE(state->hasPendingTriggerTarget());
    EXPECT_EQ(state->pendingTriggerSelectedCount(), 2);
}

TEST_F(RuledClientTest, AnyNumberTriggerTargetsAcceptMultipleDeselectAndExplicitConfirmation)
{
    ruled::v1::RuledEventBatch batch;
    auto *trigger = batch.add_events()->mutable_trigger_needs_target();
    trigger->set_source_permanent_id(100);
    trigger->set_controller_player_id(kLocalPlayer);
    auto *group = trigger->mutable_targets()->add_groups();
    group->set_min(0);
    group->set_max(4294967295u);
    group->add_valid_permanent_ids(701);
    group->add_valid_permanent_ids(702);
    apply(batch);
    EXPECT_FALSE(state->pendingTriggerTargetDisplayText().contains(QStringLiteral("2147483647")));
    EXPECT_TRUE(state->stagePendingTriggerTarget(ruled::v1::TARGET_REF_KIND_PERMANENT, 701, kLocalPlayer));
    EXPECT_TRUE(state->stagePendingTriggerTarget(ruled::v1::TARGET_REF_KIND_PERMANENT, 702, kOpponent));
    EXPECT_TRUE(host.sentCommands.isEmpty());
    EXPECT_EQ(state->pendingTriggerSelectedCount(), 2);
    EXPECT_TRUE(state->stagePendingTriggerTarget(ruled::v1::TARGET_REF_KIND_PERMANENT, 701, kLocalPlayer));
    EXPECT_EQ(state->pendingTriggerSelectedCount(), 1);
    EXPECT_FALSE(state->stagePendingTriggerTarget(ruled::v1::TARGET_REF_KIND_PLAYER, 0, kOpponent));
    EXPECT_FALSE(state->stagePendingTriggerTarget(ruled::v1::TARGET_REF_KIND_PERMANENT, 999, kLocalPlayer));
    state->confirmPendingTriggerTargets();
    ASSERT_EQ(host.sentCommands.size(), 1);
    ASSERT_EQ(host.sentCommands[0].choose_trigger_target().targets_size(), 1);
    EXPECT_EQ(host.sentCommands[0].choose_trigger_target().targets(0).object_id(), 702u);
}

TEST_F(RuledClientTest, AnyNumberTriggerTargetsCanExplicitlyConfirmZero)
{
    ruled::v1::RuledEventBatch batch;
    auto *trigger = batch.add_events()->mutable_trigger_needs_target();
    trigger->set_source_permanent_id(100);
    trigger->set_controller_player_id(kLocalPlayer);
    auto *group = trigger->mutable_targets()->add_groups();
    group->set_min(0);
    group->set_max(4294967295u);
    group->add_valid_permanent_ids(701);
    apply(batch);
    EXPECT_TRUE(host.sentCommands.isEmpty());
    state->confirmPendingTriggerTargets();
    ASSERT_EQ(host.sentCommands.size(), 1);
    EXPECT_EQ(host.sentCommands[0].choose_trigger_target().targets_size(), 0);
}

TEST_F(RuledClientTest, UnrepresentableTargetMinimumRemainsUnreachable)
{
    ruled::v1::RuledEventBatch batch;
    auto *trigger = batch.add_events()->mutable_trigger_needs_target();
    trigger->set_source_permanent_id(100);
    trigger->set_controller_player_id(kLocalPlayer);
    auto *group = trigger->mutable_targets()->add_groups();
    group->set_min(4294967295u);
    group->set_max(4294967295u);
    group->add_valid_permanent_ids(701);
    apply(batch);
    EXPECT_FALSE(state->stagePendingTriggerTarget(ruled::v1::TARGET_REF_KIND_PERMANENT, 701, kLocalPlayer));
    state->confirmPendingTriggerTargets();
    EXPECT_TRUE(host.sentCommands.isEmpty());
}

TEST_F(RuledClientTest, ExactOneTriggerTargetSubmitsImmediately)
{
    ruled::v1::RuledEventBatch batch;
    auto *tnt = batch.add_events()->mutable_trigger_needs_target();
    tnt->set_source_permanent_id(100);
    tnt->set_ability_index(0);
    tnt->set_ability_text("Choose target creature.");
    tnt->set_controller_player_id(kLocalPlayer);
    auto *group = tnt->mutable_targets()->add_groups();
    group->set_group_index(0);
    group->set_min(1);
    group->set_max(1);
    group->add_valid_permanent_ids(701);
    apply(batch);

    EXPECT_TRUE(state->stagePendingTriggerTarget(ruled::v1::TARGET_REF_KIND_PERMANENT, 701, kLocalPlayer));
    ASSERT_EQ(host.sentCommands.size(), 1);
    EXPECT_EQ(host.sentCommands[0].choose_trigger_target().targets(0).object_id(), 701u);
}

TEST_F(RuledClientTest, TriggerTargetGroupsAdvanceInOrderAndPreserveAuthoredIndices)
{
    ruled::v1::RuledEventBatch batch;
    auto *tnt = batch.add_events()->mutable_trigger_needs_target();
    tnt->set_source_permanent_id(100);
    tnt->set_ability_index(0);
    tnt->set_ability_text("Choose two targets.");
    tnt->set_controller_player_id(kLocalPlayer);
    auto *first = tnt->mutable_targets()->add_groups();
    first->set_group_index(3);
    first->set_min(1);
    first->set_max(1);
    first->add_valid_permanent_ids(701);
    auto *second = tnt->mutable_targets()->add_groups();
    second->set_group_index(7);
    second->set_min(1);
    second->set_max(1);
    second->add_valid_permanent_ids(702);
    apply(batch);

    EXPECT_TRUE(state->stagePendingTriggerTarget(ruled::v1::TARGET_REF_KIND_PERMANENT, 701, kLocalPlayer));
    EXPECT_TRUE(host.sentCommands.isEmpty());
    EXPECT_TRUE(state->stagePendingTriggerTarget(ruled::v1::TARGET_REF_KIND_PERMANENT, 702, kLocalPlayer));
    ASSERT_EQ(host.sentCommands.size(), 1);
    const auto &targets = host.sentCommands[0].choose_trigger_target().targets();
    ASSERT_EQ(targets.size(), 2);
    EXPECT_EQ(targets.Get(0).group_index(), 3u);
    EXPECT_EQ(targets.Get(1).group_index(), 7u);
}

TEST_F(RuledClientTest, ParsesMethodAwareHarmonizeActionAndAuthoritativeCreatureReductions)
{
    ruled::v1::RuledEventBatch batch;
    auto &actions = (*batch.mutable_legal_by_player())[kLocalPlayer];
    auto *flashback = actions.add_zone_cast_actions();
    flashback->set_source_zone(ruled::v1::CAST_SOURCE_ZONE_GRAVEYARD);
    flashback->set_object_id(77);
    flashback->set_face_index(0);
    flashback->set_card_name("Unending Whisper");
    flashback->set_cost("{6}{U}");
    flashback->set_cast_method(ruled::v1::CAST_METHOD_FLASHBACK);

    auto *harmonize = actions.add_zone_cast_actions();
    harmonize->set_source_zone(ruled::v1::CAST_SOURCE_ZONE_GRAVEYARD);
    harmonize->set_object_id(77);
    harmonize->set_face_index(0);
    harmonize->set_card_name("Unending Whisper");
    harmonize->set_cost("{5}{U}");
    harmonize->set_cast_method(ruled::v1::CAST_METHOD_HARMONIZE);
    auto *group = harmonize->mutable_cost_choices()->add_cast_cost_groups();
    group->set_group_index(0);
    group->set_min(0);
    group->set_max(1);
    group->set_prompt("Choose a creature for Harmonize.");
    group->set_skip_label("Pay full Harmonize cost");
    auto *option = group->add_options();
    option->set_option_index(0);
    option->set_label("Tap a creature");
    option->set_kind(ruled::v1::CAST_COST_OPTION_KIND_TAP_PERMANENT_FOR_GENERIC_REDUCTION);
    option->set_selectable(true);
    option->add_valid_permanent_ids(900);
    option->add_valid_permanent_generations(12);
    option->add_valid_permanent_generic_reductions(4);
    apply(batch);

    const auto faces = state->zoneActionFaceOptions(77, RuledCastSource::Graveyard);
    ASSERT_EQ(faces.size(), 2);
    EXPECT_EQ(faces.at(0).castMethod, ruled::v1::CAST_METHOD_FLASHBACK);
    EXPECT_EQ(faces.at(1).castMethod, ruled::v1::CAST_METHOD_HARMONIZE);
    EXPECT_EQ(state->zoneActionCost(77, 0, RuledCastSource::Graveyard, ruled::v1::CAST_METHOD_FLASHBACK),
              QStringLiteral("{6}{U}"));
    EXPECT_EQ(state->zoneActionCost(77, 0, RuledCastSource::Graveyard, ruled::v1::CAST_METHOD_HARMONIZE),
              QStringLiteral("{5}{U}"));
    const auto costs = state->spellCostData(77, 0, RuledCastSource::Graveyard, ruled::v1::CAST_METHOD_HARMONIZE);
    ASSERT_EQ(costs.castCostGroups.size(), 1);
    EXPECT_EQ(costs.castCostGroups[0].skipLabel, QStringLiteral("Pay full Harmonize cost"));
    ASSERT_EQ(costs.castCostGroups[0].options.size(), 1);
    EXPECT_EQ(costs.castCostGroups[0].options[0].kind, RuledCastCostOptionKind::TapPermanentForGenericReduction);
    EXPECT_EQ(costs.castCostGroups[0].options[0].validPermanentGenerations.value(900), 12u);
    EXPECT_EQ(costs.castCostGroups[0].options[0].validPermanentGenericReductions.value(900), 4);
}

TEST_F(RuledClientTest, NormalWarpAndSneakHandOffersKeepDistinctMethodsAndCostChoices)
{
    ruled::v1::RuledEventBatch batch;
    auto &actions = (*batch.mutable_legal_by_player())[kLocalPlayer];
    for (const auto method :
         {ruled::v1::CAST_METHOD_NORMAL, ruled::v1::CAST_METHOD_WARP, ruled::v1::CAST_METHOD_SNEAK}) {
        auto *action = actions.add_hand_actions();
        action->set_kind(ruled::v1::HAND_ACTION_CAST_SPELL);
        action->set_hand_index(3);
        action->set_card_name("Knight Luminary");
        action->set_cast_method(method);
        action->set_cost(method == ruled::v1::CAST_METHOD_WARP    ? "{1}{W}"
                         : method == ruled::v1::CAST_METHOD_SNEAK ? "{2}{W}"
                                                                  : "{3}{W}");
        action->add_eligible_restricted_mana_group_ids(static_cast<quint32>(method));
        if (method == ruled::v1::CAST_METHOD_SNEAK) {
            auto *choice = action->mutable_cost_choices()->add_choices();
            choice->set_cost_index(0);
            choice->set_zone(ruled::v1::COST_CHOICE_ZONE_BATTLEFIELD);
            choice->set_kind(ruled::v1::COST_CHOICE_KIND_RETURN_UNBLOCKED_ATTACKER);
            auto *candidate = choice->add_candidate_objects()->mutable_object();
            candidate->set_object_id(900);
            candidate->set_zone_change_generation(12);
        }
    }
    apply(batch);
    const auto options = state->handActionFaceOptions(ruled::v1::HAND_ACTION_CAST_SPELL, 3);
    ASSERT_EQ(options.size(), 3);
    EXPECT_EQ(options[0].castMethod, ruled::v1::CAST_METHOD_NORMAL);
    EXPECT_EQ(options[1].castMethod, ruled::v1::CAST_METHOD_WARP);
    EXPECT_EQ(options[2].castMethod, ruled::v1::CAST_METHOD_SNEAK);
    EXPECT_EQ(options[1].manaCost, QStringLiteral("{1}{W}"));
    const auto menu = RuledPendingCast::cardActionMenuOptions(options, *state, 0);
    EXPECT_EQ(menu[1].label, QStringLiteral("Warp Knight Luminary ({1}{W})"));
    EXPECT_EQ(menu[1].castMethod, ruled::v1::CAST_METHOD_WARP);
    EXPECT_EQ(menu[2].label, QStringLiteral("Sneak Knight Luminary ({2}{W})"));
    EXPECT_EQ(menu[2].castMethod, ruled::v1::CAST_METHOD_SNEAK);
    const auto sneakCosts = state->spellCostData(3, 0, RuledCastSource::Hand, ruled::v1::CAST_METHOD_SNEAK);
    ASSERT_EQ(sneakCosts.choices.size(), 1);
    EXPECT_EQ(sneakCosts.choices[0].kind, RuledCostChoiceKind::ReturnUnblockedAttacker);
    EXPECT_EQ(sneakCosts.choices[0].candidateGenerations.value(900), 12u);
    EXPECT_EQ(state->eligibleRestrictedManaForCast(3, 0, RuledCastSource::Hand, ruled::v1::CAST_METHOD_NORMAL),
              QSet<quint32>{static_cast<quint32>(ruled::v1::CAST_METHOD_NORMAL)});
    EXPECT_EQ(state->eligibleRestrictedManaForCast(3, 0, RuledCastSource::Hand, ruled::v1::CAST_METHOD_WARP),
              QSet<quint32>{static_cast<quint32>(ruled::v1::CAST_METHOD_WARP)});
    EXPECT_NE(RuledClientState::handCastActionKey(3, 0, ruled::v1::CAST_METHOD_NORMAL),
              RuledClientState::handCastActionKey(4, 0, ruled::v1::CAST_METHOD_NORMAL));
}

TEST_F(RuledClientTest, EmblemHandCastKeepsPaidAndWithoutManaCostChoicesDistinct)
{
    ruled::v1::RuledEventBatch batch;
    auto &actions = (*batch.mutable_legal_by_player())[kLocalPlayer];
    for (const auto method : {ruled::v1::CAST_METHOD_NORMAL, ruled::v1::CAST_METHOD_WITHOUT_MANA_COST}) {
        auto *action = addHandAction(actions, ruled::v1::HAND_ACTION_CAST_SPELL, 3,
                                     "Expensive Spell", 0, false,
                                     method == ruled::v1::CAST_METHOD_NORMAL ? "{7}{U}{U}{U}" : "{0}");
        action->set_cast_method(method);
        action->add_eligible_restricted_mana_group_ids(static_cast<quint32>(method));
        auto *choice = action->mutable_cost_choices()->add_choices();
        choice->set_cost_index(0);
        choice->set_zone(ruled::v1::COST_CHOICE_ZONE_BATTLEFIELD);
        choice->set_kind(ruled::v1::COST_CHOICE_KIND_SACRIFICE);
        auto *candidate = choice->add_candidate_objects()->mutable_object();
        candidate->set_object_id(900);
        candidate->set_zone_change_generation(12);
    }
    apply(batch);
    const auto options = state->handActionFaceOptions(ruled::v1::HAND_ACTION_CAST_SPELL, 3);
    ASSERT_EQ(options.size(), 2);
    EXPECT_EQ(options[0].castMethod, ruled::v1::CAST_METHOD_NORMAL);
    EXPECT_EQ(options[1].castMethod, ruled::v1::CAST_METHOD_WITHOUT_MANA_COST);
    const auto menu = RuledPendingCast::cardActionMenuOptions(options, *state, 0);
    ASSERT_EQ(menu.size(), 2);
    EXPECT_EQ(menu[0].label, QStringLiteral("Cast Expensive Spell ({7}{U}{U}{U})"));
    EXPECT_EQ(menu[1].label, QStringLiteral("Cast Expensive Spell without paying its mana cost ({0})"));
    EXPECT_EQ(menu[1].castMethod, ruled::v1::CAST_METHOD_WITHOUT_MANA_COST);
    const auto costs = state->spellCostData(3, 0, RuledCastSource::Hand,
                                          ruled::v1::CAST_METHOD_WITHOUT_MANA_COST);
    ASSERT_EQ(costs.choices.size(), 1);
    EXPECT_EQ(costs.choices[0].candidateGenerations.value(900), 12u);
    EXPECT_NE(RuledClientState::handCastActionKey(3, 0, ruled::v1::CAST_METHOD_NORMAL),
              RuledClientState::handCastActionKey(3, 0, ruled::v1::CAST_METHOD_WITHOUT_MANA_COST));
}

TEST_F(RuledClientTest, PrototypeHandOfferRemainsDistinctFromNormalCast)
{
    ruled::v1::RuledEventBatch batch;
    auto &actions = (*batch.mutable_legal_by_player())[kLocalPlayer];
    addHandAction(actions, ruled::v1::HAND_ACTION_CAST_SPELL, 12, "Skitterbeam Battalion", 0, false, "{9}");
    auto *prototype = addHandAction(actions, ruled::v1::HAND_ACTION_CAST_SPELL, 12, "Skitterbeam Battalion", 0,
                                    false, "{3}{R}{R}");
    prototype->set_prototyped(true);
    apply(batch);

    const auto options = state->handActionFaceOptions(ruled::v1::HAND_ACTION_CAST_SPELL, 12);
    ASSERT_EQ(options.size(), 2);
    EXPECT_FALSE(options[0].prototyped);
    EXPECT_TRUE(options[1].prototyped);
    EXPECT_EQ(options[0].manaCost, QStringLiteral("{9}"));
    EXPECT_EQ(options[1].manaCost, QStringLiteral("{3}{R}{R}"));
    EXPECT_TRUE(state->isHandCastActionLegal(12, 0, ruled::v1::CAST_METHOD_NORMAL, false));
    EXPECT_TRUE(state->isHandCastActionLegal(12, 0, ruled::v1::CAST_METHOD_NORMAL, true));

    const auto menu = RuledPendingCast::cardActionMenuOptions(options, *state, 0);
    ASSERT_EQ(menu.size(), 2);
    EXPECT_EQ(menu[0].label, QStringLiteral("Cast Skitterbeam Battalion ({9})"));
    EXPECT_FALSE(menu[0].prototyped);
    EXPECT_EQ(menu[1].label, QStringLiteral("Prototype Cast Skitterbeam Battalion ({3}{R}{R})"));
    EXPECT_TRUE(menu[1].prototyped);
}

TEST(RuledPendingCostSelectionTest, SneakReturnIsGenerationBoundAndExclusive)
{
    RuledCostChoice returned;
    returned.costIndex = 0;
    returned.kind = RuledCostChoiceKind::ReturnUnblockedAttacker;
    returned.zone = RuledCostChoiceZone::Battlefield;
    EXPECT_TRUE(ruledCostUsesObjectRefs(returned));
    EXPECT_EQ(ruledCostSelectionPrompt(returned, QStringLiteral("Foot Ninjas")),
              QStringLiteral("Choose an unblocked attacker to return for Foot Ninjas."));

    RuledCostChoice sacrifice;
    sacrifice.costIndex = 1;
    sacrifice.kind = RuledCostChoiceKind::Sacrifice;
    const QVector<RuledCostChoice> choices{returned, sacrifice};
    EXPECT_TRUE(
        ruledCostSelectionConflicts(sacrifice, choices, {0, RuledCostChoiceZone::Battlefield, {900}, {12}}, 900));
}

TEST(RuledPendingCostSelectionTest, ReturnTappedCreatureUsesExactRefsAndIsExclusive)
{
    RuledCostChoice returned;
    returned.costIndex = 1;
    returned.kind = RuledCostChoiceKind::ReturnTappedCreature;
    returned.zone = RuledCostChoiceZone::Battlefield;
    returned.candidateIds.insert(900);
    returned.candidateGenerations.insert(900, 12);
    EXPECT_TRUE(ruledCostUsesObjectRefs(returned));
    EXPECT_TRUE(ruledCostNeedsConfirmation(returned));
    EXPECT_EQ(ruledCostSelectionPrompt(returned, QStringLiteral("Urban Retreat")),
              QStringLiteral("Choose a tapped creature you control to return to its owner's hand for Urban Retreat."));

    RuledCostChoice sacrifice;
    sacrifice.costIndex = 2;
    sacrifice.kind = RuledCostChoiceKind::Sacrifice;
    const QVector<RuledCostChoice> choices{returned, sacrifice};
    EXPECT_TRUE(
        ruledCostSelectionConflicts(sacrifice, choices, {1, RuledCostChoiceZone::Battlefield, {900}, {12}}, 900));

    PendingActivatedAbility local;
    local.deferredReturnTappedCreature = true;
    local.costChoices = {returned};
    local.costSelections = {{1, RuledCostChoiceZone::Battlefield, {900}, {12}}};
    const auto ref = ruledReturnTappedCreatureSelection(local);
    ASSERT_TRUE(ref.has_value());
    EXPECT_EQ(ref->object_id(), 900u);
    EXPECT_EQ(ref->zone_change_generation(), 12u);
    local.costChoices[0].candidateGenerations[900] = 13;
    EXPECT_FALSE(ruledReturnTappedCreatureSelection(local).has_value());
}

TEST(RuledPendingCostSelectionTest, EmptyReturnTappedCreatureCandidatesPromptForManaAbility)
{
    RuledPendingCast pending;
    auto &ability = pending.beginAbility();
    ability.valid = true;
    ability.deferredReturnTappedCreature = true;
    ability.waitingForReturnTappedCreatureCandidate = true;
    ability.cardName = QStringLiteral("Urban Retreat");

    EXPECT_EQ(pending.pendingRuledAbilityCostPromptText(),
              QStringLiteral("Activate a mana ability that taps a creature you control, then return it to its "
                             "owner's hand to activate Urban Retreat."));
}

TEST_F(RuledClientTest, ExilePermissionOffersKeepOpaqueIdentityCostAndSourceLabel)
{
    ruled::v1::RuledEventBatch batch;
    auto &actions = (*batch.mutable_legal_by_player())[kLocalPlayer];
    for (const auto [id, cost, source] :
         {std::tuple<quint64, const char *, const char *>{41, "{2}", "Airbending Lesson"},
          std::tuple<quint64, const char *, const char *>{42, "", "Release to the Wind"}}) {
        auto *group = actions.add_exile_play_permission_groups();
        group->set_group_id(id);
        group->set_source_label(source);
        group->add_object_ids(77);
        auto *action = actions.add_zone_cast_actions();
        action->set_source_zone(ruled::v1::CAST_SOURCE_ZONE_EXILE);
        action->set_object_id(77);
        action->set_card_name("Grizzly Bears");
        action->set_cost(cost);
        action->set_cast_method(ruled::v1::CAST_METHOD_PERMISSION);
        action->set_zone_change_generation(9);
        action->set_casting_permission_id(id);
    }
    apply(batch);

    const auto faces = state->zoneActionFaceOptions(77, RuledCastSource::Exile);
    ASSERT_EQ(faces.size(), 2);
    EXPECT_EQ(faces[0].castingPermissionId, 41u);
    EXPECT_EQ(faces[0].permissionSourceLabel, QStringLiteral("Airbending Lesson"));
    EXPECT_EQ(faces[1].castingPermissionId, 42u);
    EXPECT_EQ(state->zoneActionCost(77, 0, RuledCastSource::Exile, ruled::v1::CAST_METHOD_PERMISSION, 41),
              QStringLiteral("{2}"));
    EXPECT_TRUE(state->zoneActionCost(77, 0, RuledCastSource::Exile, ruled::v1::CAST_METHOD_PERMISSION, 42).isEmpty());
    const auto menu = RuledPendingCast::cardActionMenuOptions(faces, *state, 0);
    ASSERT_EQ(menu.size(), 2);
    EXPECT_EQ(menu[0].label, QStringLiteral("Cast Grizzly Bears — Airbending Lesson ({2})"));
    EXPECT_EQ(menu[1].label, QStringLiteral("Cast Grizzly Bears — Release to the Wind"));
}

TEST_F(RuledClientTest, PreparationViewReplacementRetiresOldLinkAndAcceptsFreshCopy)
{
    const auto makeView = [](quint32 copyOid, quint64 copyGeneration, bool hasActiveCopy) {
        ruled::v1::RuledEventBatch batch;
        auto *view = batch.add_events()->mutable_zone_view();
        view->set_battlefields_unchanged(false);
        auto *player = view->add_per_player();
        player->set_player_id(kLocalPlayer);
        auto *source = player->add_battlefield_objects();
        source->set_object_id(42u);
        source->set_zone_change_generation(7u);
        auto *preparation = source->mutable_preparation();
        if (hasActiveCopy) {
            preparation->set_copy_object_id(copyOid);
            preparation->set_copy_zone_change_generation(copyGeneration);
            player->add_exile_object_ids(copyOid);
            auto *copy = player->add_prepare_spell_copies();
            copy->set_object_id(copyOid);
            copy->set_display_name("Infirmary Healer // Stream of Life");
            copy->mutable_source()->set_object_id(42u);
            copy->mutable_source()->set_zone_change_generation(7u);
        }
        return batch;
    };
    const auto addCastOffer = [](ruled::v1::RuledEventBatch &batch, quint32 copyOid, quint64 copyGeneration) {
        auto *action = (*batch.mutable_legal_by_player())[kLocalPlayer].add_zone_cast_actions();
        action->set_source_zone(ruled::v1::CAST_SOURCE_ZONE_EXILE);
        action->set_object_id(copyOid);
        action->set_zone_change_generation(copyGeneration);
        action->set_face_index(1u);
        action->set_card_name("Stream of Life");
        action->set_cost("{G}");
        action->set_casting_permission_id(91u);
        action->mutable_preparation_source()->set_object_id(42u);
        action->mutable_preparation_source()->set_zone_change_generation(7u);
    };

    // A present generation of zero is a complete active-copy identity.
    auto active = makeView(77u, 0u, true);
    addCastOffer(active, 77u, 0u);
    apply(active);
    ASSERT_TRUE(state->preparedCopyBySourceOid.contains(42u));
    EXPECT_EQ(state->preparedCopyBySourceOid.value(42u), 77u);
    ASSERT_TRUE(state->preparedCopyGenerationBySourceOid.contains(42u));
    EXPECT_EQ(state->preparedCopyGenerationBySourceOid.value(42u), 0u);
    const auto activeCopy = state->preparationCastCopy(42u);
    ASSERT_TRUE(activeCopy.has_value());
    EXPECT_EQ(*activeCopy, 77u);

    // A full replacement keeps the PreparationState designation but removes both active-copy fields.
    auto out = makeView(0u, 0u, false);
    apply(out);
    EXPECT_FALSE(state->preparedCopyBySourceOid.contains(42u))
        << "a prepared designation without a complete active-copy pair has no source-to-copy link";
    EXPECT_FALSE(state->preparedCopyGenerationBySourceOid.contains(42u));
    EXPECT_FALSE(state->preparationCastCopy(42u).has_value());

    // Phase-in restores a distinct active copy and generation without changing the source identity.
    auto fresh = makeView(78u, 9u, true);
    addCastOffer(fresh, 78u, 9u);
    apply(fresh);
    ASSERT_TRUE(state->preparedCopyBySourceOid.contains(42u));
    EXPECT_EQ(state->preparedCopyBySourceOid.value(42u), 78u);
    ASSERT_TRUE(state->preparedCopyGenerationBySourceOid.contains(42u));
    EXPECT_EQ(state->preparedCopyGenerationBySourceOid.value(42u), 9u);
    const auto freshCopy = state->preparationCastCopy(42u);
    ASSERT_TRUE(freshCopy.has_value());
    EXPECT_EQ(*freshCopy, 78u);
}

TEST_F(RuledClientTest, PreparationCopyOidWithoutGenerationCannotBindOrOfferCast)
{
    ruled::v1::RuledEventBatch batch;
    auto *view = batch.add_events()->mutable_zone_view();
    auto *player = view->add_per_player();
    player->set_player_id(kLocalPlayer);
    player->add_exile_object_ids(77u);
    auto *source = player->add_battlefield_objects();
    source->set_object_id(42u);
    source->set_zone_change_generation(7u);
    source->mutable_preparation()->set_copy_object_id(77u);
    auto *copy = player->add_prepare_spell_copies();
    copy->set_object_id(77u);
    copy->set_display_name("Infirmary Healer // Stream of Life");
    copy->mutable_source()->set_object_id(42u);
    copy->mutable_source()->set_zone_change_generation(7u);
    auto *action = (*batch.mutable_legal_by_player())[kLocalPlayer].add_zone_cast_actions();
    action->set_source_zone(ruled::v1::CAST_SOURCE_ZONE_EXILE);
    action->set_object_id(77u);
    action->set_zone_change_generation(5u);
    action->set_face_index(1u);
    action->set_card_name("Stream of Life");
    action->set_cost("{G}");
    action->set_casting_permission_id(91u);
    action->mutable_preparation_source()->set_object_id(42u);
    action->mutable_preparation_source()->set_zone_change_generation(7u);

    apply(batch);
    EXPECT_FALSE(state->preparedCopyBySourceOid.contains(42u));
    EXPECT_FALSE(state->preparedCopyGenerationBySourceOid.contains(42u));
    EXPECT_FALSE(state->preparationCastCopy(42u).has_value());
}

TEST_F(RuledClientTest, PreparationCopyGenerationWithoutOidCannotBindOrOfferCast)
{
    ruled::v1::RuledEventBatch batch;
    auto *view = batch.add_events()->mutable_zone_view();
    auto *player = view->add_per_player();
    player->set_player_id(kLocalPlayer);
    player->add_exile_object_ids(77u);
    auto *source = player->add_battlefield_objects();
    source->set_object_id(42u);
    source->set_zone_change_generation(7u);
    source->mutable_preparation()->set_copy_zone_change_generation(0u);
    auto *copy = player->add_prepare_spell_copies();
    copy->set_object_id(77u);
    copy->set_display_name("Infirmary Healer // Stream of Life");
    copy->mutable_source()->set_object_id(42u);
    copy->mutable_source()->set_zone_change_generation(7u);
    auto *action = (*batch.mutable_legal_by_player())[kLocalPlayer].add_zone_cast_actions();
    action->set_source_zone(ruled::v1::CAST_SOURCE_ZONE_EXILE);
    action->set_object_id(77u);
    action->set_zone_change_generation(0u);
    action->set_face_index(1u);
    action->set_card_name("Stream of Life");
    action->set_cost("{G}");
    action->set_casting_permission_id(91u);
    action->mutable_preparation_source()->set_object_id(42u);
    action->mutable_preparation_source()->set_zone_change_generation(7u);

    apply(batch);
    EXPECT_FALSE(state->preparedCopyBySourceOid.contains(42u));
    EXPECT_FALSE(state->preparedCopyGenerationBySourceOid.contains(42u));
    EXPECT_FALSE(state->preparationCastCopy(42u).has_value());
}

TEST_F(RuledClientTest, PreparationCastOfferMustMatchActiveCopyGeneration)
{
    ruled::v1::RuledEventBatch batch;
    auto *view = batch.add_events()->mutable_zone_view();
    auto *player = view->add_per_player();
    player->set_player_id(kLocalPlayer);
    player->add_exile_object_ids(77u);
    auto *source = player->add_battlefield_objects();
    source->set_object_id(42u);
    source->set_zone_change_generation(7u);
    source->mutable_preparation()->set_copy_object_id(77u);
    source->mutable_preparation()->set_copy_zone_change_generation(0u);
    auto *copy = player->add_prepare_spell_copies();
    copy->set_object_id(77u);
    copy->set_display_name("Infirmary Healer // Stream of Life");
    copy->mutable_source()->set_object_id(42u);
    copy->mutable_source()->set_zone_change_generation(7u);
    auto *action = (*batch.mutable_legal_by_player())[kLocalPlayer].add_zone_cast_actions();
    action->set_source_zone(ruled::v1::CAST_SOURCE_ZONE_EXILE);
    action->set_object_id(77u);
    action->set_zone_change_generation(9u);
    action->set_face_index(1u);
    action->set_card_name("Stream of Life");
    action->set_cost("{G}");
    action->set_casting_permission_id(91u);
    action->mutable_preparation_source()->set_object_id(42u);
    action->mutable_preparation_source()->set_zone_change_generation(7u);

    apply(batch);
    EXPECT_FALSE(state->preparationCastCopy(42u).has_value())
        << "a legal action for the same ObjectId but a different generation is stale";
}

TEST_F(RuledClientTest, PreparationMenuUsesTheSameGenerationBoundExileAction)
{
    ruled::v1::RuledEventBatch batch;
    auto *view = batch.add_events()->mutable_zone_view();
    auto *player = view->add_per_player();
    player->set_player_id(kLocalPlayer);
    auto *permanent = player->add_battlefield_objects();
    permanent->set_object_id(42);
    permanent->set_zone_change_generation(7);
    permanent->mutable_preparation()->set_copy_object_id(77);
    permanent->mutable_preparation()->set_copy_zone_change_generation(0);
    auto *action = (*batch.mutable_legal_by_player())[kLocalPlayer].add_zone_cast_actions();
    action->set_source_zone(ruled::v1::CAST_SOURCE_ZONE_EXILE);
    action->set_object_id(77);
    action->set_face_index(1);
    action->set_zone_change_generation(0);
    action->set_card_name("Stream of Life");
    action->set_cost("{X}{G}");
    action->set_casting_permission_id(91);
    action->mutable_preparation_source()->set_object_id(42);
    action->mutable_preparation_source()->set_zone_change_generation(7);
    apply(batch);
    auto initialCopy = state->preparationCastCopy(42);
    ASSERT_TRUE(initialCopy.has_value());
    EXPECT_EQ(*initialCopy, 77u);
    const auto faces = state->zoneActionFaceOptions(77, RuledCastSource::Exile);
    ASSERT_EQ(faces.size(), 1);
    const auto menu = RuledPendingCast::cardActionMenuOptions(faces, *state, 42, false, {});
    ASSERT_EQ(menu.size(), 1);
    EXPECT_EQ(menu.first().label, QStringLiteral("Cast Stream of Life ({X}{G})"));
    EXPECT_EQ(menu.first().castingPermissionId, 91u);
    EXPECT_EQ(faces.first().castingPermissionId, 91u);
    permanent->set_zone_change_generation(8);
    apply(batch);
    EXPECT_FALSE(state->preparationCastCopy(42).has_value());
    permanent->set_zone_change_generation(7);
    apply(batch);
    auto restoredCopy = state->preparationCastCopy(42);
    ASSERT_TRUE(restoredCopy.has_value());
    EXPECT_EQ(*restoredCopy, 77u);
    batch.clear_legal_by_player();
    view->set_battlefields_unchanged(true);
    apply(batch);
    EXPECT_FALSE(state->preparationCastCopy(42).has_value());
}

TEST_F(RuledClientTest, PublicCastGenerationRejectsAnOldSelectionAfterReexile)
{
    ruled::v1::RuledEventBatch batch;
    auto *action = (*batch.mutable_legal_by_player())[kLocalPlayer].add_zone_cast_actions();
    action->set_source_zone(ruled::v1::CAST_SOURCE_ZONE_EXILE);
    action->set_object_id(77);
    action->set_card_name("Knight Luminary");
    action->set_cast_method(ruled::v1::CAST_METHOD_NORMAL);
    action->set_zone_change_generation(4);
    apply(batch);
    EXPECT_TRUE(state->isZoneCastActionLegal(77, 0, RuledCastSource::Exile, ruled::v1::CAST_METHOD_NORMAL, 0, 4));
    action->set_zone_change_generation(6);
    apply(batch);
    EXPECT_FALSE(state->isZoneCastActionLegal(77, 0, RuledCastSource::Exile, ruled::v1::CAST_METHOD_NORMAL, 0, 4));
    EXPECT_TRUE(state->isZoneCastActionLegal(77, 0, RuledCastSource::Exile, ruled::v1::CAST_METHOD_NORMAL, 0, 6));
}

TEST_F(RuledClientTest, PublicZoneCastActionsRequireTheClickedSourceZone)
{
    ruled::v1::RuledEventBatch batch;
    auto &actions = (*batch.mutable_legal_by_player())[kLocalPlayer];
    auto *harmonize = actions.add_zone_cast_actions();
    harmonize->set_source_zone(ruled::v1::CAST_SOURCE_ZONE_GRAVEYARD);
    harmonize->set_object_id(77);
    harmonize->set_face_index(0);
    harmonize->set_card_name("Unending Whisper");
    harmonize->set_cost("{5}{U}");
    harmonize->set_cast_method(ruled::v1::CAST_METHOD_HARMONIZE);
    apply(batch);

    EXPECT_TRUE(state->isZoneActionLegal(77, RuledCastSource::Graveyard));
    EXPECT_FALSE(state->isZoneActionLegal(77, RuledCastSource::Exile));
    EXPECT_EQ(state->zoneActionFaceOptions(77, RuledCastSource::Graveyard).size(), 1);
    EXPECT_TRUE(state->zoneActionFaceOptions(77, RuledCastSource::Exile).isEmpty());
    EXPECT_EQ(state->zoneActionCost(77, 0, RuledCastSource::Graveyard, ruled::v1::CAST_METHOD_HARMONIZE),
              QStringLiteral("{5}{U}"));
    EXPECT_TRUE(state->zoneActionCost(77, 0, RuledCastSource::Exile, ruled::v1::CAST_METHOD_HARMONIZE).isEmpty());
}

TEST_F(RuledClientTest, AppliesExileLandActionsAndStablePermissionGroupSnapshots)
{
    constexpr quint32 objectId = 700;
    ruled::v1::RuledEventBatch batch;
    auto &actions = (*batch.mutable_legal_by_player())[kLocalPlayer];
    auto *land = actions.add_zone_land_actions();
    land->set_source_zone(ruled::v1::CAST_SOURCE_ZONE_EXILE);
    land->set_object_id(objectId);
    land->set_face_index(1);
    land->set_card_name("Timbercrown Pathway");
    land->set_zone_change_generation(17);
    auto *group = actions.add_exile_play_permission_groups();
    group->set_group_id(42);
    group->set_source_label("Clockwork Percussionist");
    group->add_object_ids(objectId);
    auto *overlap = actions.add_exile_play_permission_groups();
    overlap->set_group_id(43);
    overlap->set_source_label("Second permission");
    overlap->add_object_ids(objectId);

    QSignalSpy changed(state, &RuledClientState::exilePlayPermissionGroupsChanged);
    apply(batch);
    ASSERT_TRUE(state->isZoneLandActionLegal(objectId, RuledCastSource::Exile));
    EXPECT_FALSE(state->isZoneLandActionLegal(objectId, RuledCastSource::Graveyard));
    ASSERT_EQ(state->zoneLandFaceOptions(objectId, RuledCastSource::Exile).size(), 1);
    const auto landFace = state->zoneLandFaceOptions(objectId, RuledCastSource::Exile).first();
    EXPECT_EQ(landFace.faceName, QStringLiteral("Timbercrown Pathway"));
    EXPECT_EQ(landFace.zoneChangeGeneration, 17u);
    ASSERT_EQ(state->exilePlayPermissionGroups.size(), 2);
    EXPECT_EQ(state->exilePlayPermissionGroups.value(42).sourceLabel, QStringLiteral("Clockwork Percussionist"));
    EXPECT_EQ(state->exilePlayPermissionGroups.value(42).objectIds, QVector<quint32>{objectId});
    EXPECT_EQ(state->exilePlayPermissionGroups.value(43).objectIds, QVector<quint32>{objectId});
    EXPECT_EQ(changed.count(), 1);

    apply(batch);
    EXPECT_EQ(changed.count(), 1) << "an identical snapshot must not reopen a dismissed popup";

    auto cleared = phaseBatch(ruled::v1::PHASE_ID_MAIN1, kLocalPlayer);
    (*cleared.mutable_legal_by_player())[kLocalPlayer];
    apply(cleared);
    EXPECT_FALSE(state->isZoneLandActionLegal(objectId, RuledCastSource::Exile));
    EXPECT_TRUE(state->exilePlayPermissionGroups.isEmpty());
    EXPECT_EQ(changed.count(), 2);
}

TEST_F(RuledClientTest, AppliesGenerationBoundGraveyardLandActionsBySource)
{
    constexpr quint32 objectId = 214;
    ruled::v1::RuledEventBatch batch;
    auto *land = (*batch.mutable_legal_by_player())[kLocalPlayer].add_zone_land_actions();
    land->set_source_zone(ruled::v1::CAST_SOURCE_ZONE_GRAVEYARD);
    land->set_object_id(objectId);
    land->set_face_index(0);
    land->set_card_name("Forest");
    land->set_zone_change_generation(23);

    apply(batch);

    EXPECT_TRUE(state->isZoneLandActionLegal(objectId, RuledCastSource::Graveyard));
    EXPECT_FALSE(state->isZoneLandActionLegal(objectId, RuledCastSource::Exile));
    const auto faces = state->zoneLandFaceOptions(objectId, RuledCastSource::Graveyard);
    ASSERT_EQ(faces.size(), 1);
    EXPECT_EQ(faces.first().faceIndex, 0);
    EXPECT_EQ(faces.first().zoneChangeGeneration, 23u);
}

TEST_F(RuledClientTest, SpecialCastPromptCaptureRejectsStalePromptIds)
{
    constexpr quint32 objectId = 214;
    constexpr quint64 promptId = 9101;
    ruled::v1::RuledEventBatch batch;
    auto *snapshot = batch.mutable_resolution_prompt_state();
    snapshot->set_active(true);
    snapshot->set_deciding_player_id(kLocalPlayer);
    snapshot->set_resolution_prompt_id(promptId);
    auto *choice = batch.add_events()->mutable_resolution_choice_required();
    choice->set_deciding_player_id(kLocalPlayer);
    choice->set_choice_kind(ruled::v1::CHOICE_KIND_SPECIAL_CAST);
    choice->set_resolution_prompt_id(promptId);
    choice->add_candidate_object_ids(objectId);
    choice->add_candidate_selectable(true);
    applyRaw(batch);

    ASSERT_TRUE(state->isPendingSpecialCastCandidate(objectId));
    EXPECT_EQ(state->specialCastPromptIdForCandidate(objectId), promptId);
    EXPECT_TRUE(state->isCurrentSpecialCastPrompt(promptId, objectId));
    EXPECT_FALSE(state->isCurrentSpecialCastPrompt(promptId - 1, objectId));
    EXPECT_FALSE(state->isCurrentSpecialCastPrompt(0, objectId));
    EXPECT_EQ(state->specialCastPromptIdForCandidate(objectId + 1), 0u);
}

TEST_F(RuledClientTest, ZoneLandPlayOfferRequiresExactGenerationAndSpecialPromptId)
{
    constexpr quint32 objectId = 215;
    constexpr quint64 promptId = 9102;
    ruled::v1::RuledEventBatch batch;
    auto *snapshot = batch.mutable_resolution_prompt_state();
    snapshot->set_active(true);
    snapshot->set_deciding_player_id(kLocalPlayer);
    snapshot->set_resolution_prompt_id(promptId);
    auto *choice = batch.add_events()->mutable_resolution_choice_required();
    choice->set_deciding_player_id(kLocalPlayer);
    choice->set_choice_kind(ruled::v1::CHOICE_KIND_SPECIAL_CAST);
    choice->set_resolution_prompt_id(promptId);
    choice->add_candidate_object_ids(objectId);
    choice->add_candidate_selectable(true);
    auto *land = (*batch.mutable_legal_by_player())[kLocalPlayer].add_zone_land_actions();
    land->set_source_zone(ruled::v1::CAST_SOURCE_ZONE_EXILE);
    land->set_object_id(objectId);
    land->set_face_index(1);
    land->set_card_name("Timbercrown Pathway");
    land->set_zone_change_generation(17);
    applyRaw(batch);

    EXPECT_TRUE(state->isCurrentZoneLandPlayOffer(objectId, RuledCastSource::Exile, 1, 17, promptId));
    EXPECT_FALSE(state->isCurrentZoneLandPlayOffer(objectId, RuledCastSource::Exile, 1, 17, promptId - 1));
    EXPECT_FALSE(state->isCurrentZoneLandPlayOffer(objectId, RuledCastSource::Exile, 1, 17, 0))
        << "an active PlayCard candidate must not submit without its exact prompt ID";
    EXPECT_FALSE(state->isCurrentZoneLandPlayOffer(objectId, RuledCastSource::Exile, 1, 18, promptId))
        << "the same engine OID with a new zone generation is a different physical offer";
}

TEST_F(RuledClientTest, OrdinaryZoneLandOfferUsesNoResolutionPromptId)
{
    constexpr quint32 objectId = 216;
    ruled::v1::RuledEventBatch batch;
    auto *land = (*batch.mutable_legal_by_player())[kLocalPlayer].add_zone_land_actions();
    land->set_source_zone(ruled::v1::CAST_SOURCE_ZONE_GRAVEYARD);
    land->set_object_id(objectId);
    land->set_face_index(0);
    land->set_card_name("Forest");
    land->set_zone_change_generation(23);
    applyRaw(batch);

    EXPECT_TRUE(state->isCurrentZoneLandPlayOffer(objectId, RuledCastSource::Graveyard, 0, 23, 0));
    EXPECT_FALSE(state->isCurrentZoneLandPlayOffer(objectId, RuledCastSource::Graveyard, 0, 23, 1));
}

TEST_F(RuledClientTest, UnmatchedActivePromptBlocksUnboundZoneLandOffer)
{
    constexpr quint32 objectId = 217;
    constexpr quint64 promptId = 9304;
    const auto makeLandOffer = [this, objectId](ruled::v1::RuledEventBatch &batch) {
        auto *land = (*batch.mutable_legal_by_player())[kLocalPlayer].add_zone_land_actions();
        land->set_source_zone(ruled::v1::CAST_SOURCE_ZONE_EXILE);
        land->set_object_id(objectId);
        land->set_face_index(0);
        land->set_card_name("Forest");
        land->set_zone_change_generation(24);
    };

    ruled::v1::RuledEventBatch active;
    active.mutable_resolution_prompt_state()->set_active(true);
    active.mutable_resolution_prompt_state()->set_deciding_player_id(kLocalPlayer);
    active.mutable_resolution_prompt_state()->set_resolution_prompt_id(promptId);
    makeLandOffer(active);
    applyRaw(active);

    ASSERT_TRUE(state->resolutionPromptSnapshotValid);
    ASSERT_TRUE(state->resolutionPromptActive);
    EXPECT_EQ(state->resolutionPromptId, 0u)
        << "the final owner snapshot without its exact RCR is deliberately unactionable";
    EXPECT_FALSE(state->isCurrentZoneLandPlayOffer(objectId, RuledCastSource::Exile, 0, 24, 0));

    // A same-ID offer whose answer semantics change is unusable, but its valid final snapshot
    // still identifies an active local prompt and must keep the ordinary-ID fallback closed.
    ruled::v1::RuledEventBatch validSpecialOffer;
    auto *validSnapshot = validSpecialOffer.mutable_resolution_prompt_state();
    validSnapshot->set_active(true);
    validSnapshot->set_deciding_player_id(kLocalPlayer);
    validSnapshot->set_resolution_prompt_id(promptId);
    auto *validRcr = validSpecialOffer.add_events()->mutable_resolution_choice_required();
    validRcr->set_deciding_player_id(kLocalPlayer);
    validRcr->set_choice_kind(ruled::v1::CHOICE_KIND_SPECIAL_CAST);
    validRcr->set_resolution_prompt_id(promptId);
    validRcr->add_candidate_object_ids(objectId);
    validRcr->add_candidate_selectable(true);
    makeLandOffer(validSpecialOffer);
    applyRaw(validSpecialOffer);
    ASSERT_TRUE(state->isCurrentZoneLandPlayOffer(objectId, RuledCastSource::Exile, 0, 24, promptId));

    ruled::v1::RuledEventBatch changedSpecialOffer;
    auto *changedSnapshot = changedSpecialOffer.mutable_resolution_prompt_state();
    changedSnapshot->CopyFrom(validSpecialOffer.resolution_prompt_state());
    auto *changedRcr = changedSpecialOffer.add_events()->mutable_resolution_choice_required();
    changedRcr->CopyFrom(*validRcr);
    changedRcr->set_candidate_object_ids(0, objectId + 1);
    makeLandOffer(changedSpecialOffer);
    applyRaw(changedSpecialOffer);
    EXPECT_TRUE(state->resolutionPromptSnapshotValid);
    EXPECT_TRUE(state->resolutionPromptActive);
    EXPECT_EQ(state->resolutionPromptDeciderPlayerId, kLocalPlayer);
    EXPECT_EQ(state->resolutionPromptId, 0u);
    EXPECT_TRUE(state->hasActiveLocalResolutionPrompt());
    EXPECT_FALSE(state->isCurrentZoneLandPlayOffer(objectId, RuledCastSource::Exile, 0, 24, 0));

    ruled::v1::RuledEventBatch inactive;
    inactive.mutable_resolution_prompt_state()->set_active(false);
    inactive.mutable_resolution_prompt_state()->set_deciding_player_id(0);
    makeLandOffer(inactive);
    applyRaw(inactive);
    EXPECT_FALSE(state->resolutionPromptActive);
    EXPECT_TRUE(state->isCurrentZoneLandPlayOffer(objectId, RuledCastSource::Exile, 0, 24, 0));
}

TEST_F(RuledClientTest, CounterRemovalCostsAreRecognized)
{
    ruled::v1::RuledEventBatch batch;
    auto *hand = (*batch.mutable_legal_by_player())[kLocalPlayer].add_hand_actions();
    hand->set_kind(ruled::v1::HAND_ACTION_CAST_SPELL);
    hand->set_cast_method(ruled::v1::CAST_METHOD_NORMAL);
    hand->set_hand_index(3);
    auto *choice = hand->mutable_cost_choices()->add_choices();
    choice->set_kind(ruled::v1::COST_CHOICE_KIND_REMOVE_COUNTERS);
    choice->set_cost_index(1);
    choice->set_min(1);
    choice->set_max(1);
    auto *removal = choice->mutable_counter_removal();
    removal->mutable_source()->set_object_id(900);
    removal->mutable_source()->set_zone_change_generation(12);
    removal->set_count(1);
    auto *option = removal->add_options();
    option->set_option_id(3);
    option->set_label("stun");
    option->set_available_count(2);
    apply(batch);
    const auto costs = state->spellCostData(3, 0, RuledCastSource::Hand);
    ASSERT_EQ(costs.choices.size(), 1);
    EXPECT_NE(costs.choices[0].kind, RuledCostChoiceKind::Unspecified);
    const auto &parsed = costs.choices.front();
    ASSERT_EQ(parsed.counterOptions.size(), 1);
    EXPECT_EQ(parsed.counterOptions.front().label, QStringLiteral("stun"));
    PendingActivatedAbility pending;
    pending.valid = true;
    pending.permanentOid = 900;
    pending.expectedZoneChangeGeneration = 12;
    pending.costChoices = costs.choices;
    EXPECT_TRUE(RuledPendingCast::chooseCounterCosts(nullptr, pending));
    ASSERT_EQ(pending.costSelections.size(), 1);
    EXPECT_EQ(pending.nextCostChoice, 1);
    EXPECT_TRUE(ruledCounterSelectionStillLegal(pending.costSelections.front(), parsed));
    ruled::v1::CostSelection command;
    ruledWriteCounterRemoval(pending.costSelections.front(), command);
    EXPECT_EQ(command.counter_removal().option_id(), 3u);
    EXPECT_EQ(command.counter_removal().source().object_id(), 900u);
    EXPECT_EQ(command.counter_removal().source().zone_change_generation(), 12u);
    auto stale = parsed;
    ++stale.counterSourceGeneration;
    EXPECT_FALSE(ruledCounterSelectionStillLegal(pending.costSelections.front(), stale));
    stale = parsed;
    stale.counterOptions.clear();
    EXPECT_FALSE(ruledCounterSelectionStillLegal(pending.costSelections.front(), stale));
}

TEST_F(RuledClientTest, SelectedCounterRemovalUsesAnAuthoritativePermanentPicker)
{
    ruled::v1::RuledEventBatch batch;
    auto &costs = (*(*batch.mutable_legal_by_player())[kLocalPlayer].mutable_cost_choices_by_ability())[193];
    costs.set_non_mana_costs_payable(true);
    auto *choice = costs.add_choices();
    choice->set_kind(ruled::v1::COST_CHOICE_KIND_REMOVE_COUNTERS);
    choice->set_zone(ruled::v1::COST_CHOICE_ZONE_BATTLEFIELD);
    choice->set_cost_index(1);
    choice->set_min(1);
    choice->set_max(1);
    choice->add_candidate_ids(901);
    auto *candidate = choice->add_candidate_objects();
    candidate->mutable_object()->set_object_id(901);
    candidate->mutable_object()->set_zone_change_generation(13);
    candidate->set_contribution(2);
    auto *removal = choice->mutable_counter_removal();
    removal->set_count(1);
    auto *option = removal->add_options();
    option->set_option_id(1);
    option->set_label("+1/+1");
    option->set_available_count(1);
    apply(batch);

    const auto parsed = state->abilityCostChoices(0, 193);
    ASSERT_EQ(parsed.size(), 1);
    const auto &selected = parsed.front();
    EXPECT_EQ(selected.kind, RuledCostChoiceKind::RemoveCounters);
    EXPECT_EQ(selected.counterSourceId, 0u);
    EXPECT_EQ(selected.counterCount, 1u);
    ASSERT_EQ(selected.counterOptions.size(), 1);
    EXPECT_EQ(selected.candidateIds, QSet<quint32>{901});
    EXPECT_EQ(selected.candidateGenerations.value(901), 13u);
    EXPECT_EQ(selected.candidateContributions.value(901), 2);
    EXPECT_TRUE(ruledCostUsesObjectRefs(selected));
    EXPECT_TRUE(ruledCostSelectionPrompt(selected, QStringLiteral("Ray Fillet, Man Ray"))
                    .contains(QStringLiteral("counter"), Qt::CaseInsensitive));

    PendingActivatedAbility pending;
    pending.valid = true;
    pending.permanentOid = 900;
    pending.expectedZoneChangeGeneration = 12;
    pending.costChoices = parsed;
    EXPECT_TRUE(RuledPendingCast::chooseCounterCosts(nullptr, pending));
    EXPECT_TRUE(pending.costSelections.isEmpty());
    EXPECT_EQ(pending.nextCostChoice, 0);

    RuledPendingCostSelection payment{1, RuledCostChoiceZone::Battlefield, {901}, {13}, 1};
    EXPECT_TRUE(ruledCounterSelectionStillLegal(payment, selected));
    ruled::v1::CostSelection command;
    ruledWriteCounterRemoval(payment, command);
    EXPECT_EQ(command.counter_removal().option_id(), 1u);
    EXPECT_EQ(command.counter_removal().source().object_id(), 901u);
    EXPECT_EQ(command.counter_removal().source().zone_change_generation(), 13u);

    auto stale = selected;
    stale.candidateGenerations[901] = 14;
    EXPECT_FALSE(ruledCounterSelectionStillLegal(payment, stale));
    stale = selected;
    stale.candidateIds.clear();
    EXPECT_FALSE(ruledCounterSelectionStillLegal(payment, stale));
}

TEST_F(RuledClientTest, PluralSacrificeUsesOneExactGenerationBoundCohort)
{
    ruled::v1::RuledEventBatch batch;
    auto *costs = &(*(*batch.mutable_legal_by_player())[kLocalPlayer].mutable_cost_choices_by_ability())[900ull << 32];
    auto *choice = costs->add_choices();
    choice->set_cost_index(1);
    choice->set_zone(ruled::v1::COST_CHOICE_ZONE_BATTLEFIELD);
    choice->set_kind(ruled::v1::COST_CHOICE_KIND_SACRIFICE);
    choice->set_min(3);
    choice->set_max(3);
    for (quint32 id : {900u, 901u, 902u}) {
        choice->add_candidate_ids(id);
        auto *object = choice->add_candidate_objects()->mutable_object();
        object->set_object_id(id);
        object->set_zone_change_generation(id - 888);
    }
    apply(batch);
    const auto choices = state->abilityCostChoices(900, 0);
    ASSERT_EQ(choices.size(), 1);
    const auto &parsed = choices.front();
    EXPECT_TRUE(ruledCostUsesObjectRefs(parsed));
    EXPECT_TRUE(ruledCostNeedsConfirmation(parsed));
    EXPECT_TRUE(ruledCostSelectionPrompt(parsed, QStringLiteral("Kuldotha Forgemaster")).contains("3"));
    PendingActivatedAbility pending;
    pending.valid = true;
    pending.waitingForCost = true;
    pending.costChoices = choices;
    pending.costSelections = {{1, RuledCostChoiceZone::Battlefield, {}, {}}};
    for (int count = 0; count <= 3; ++count) {
        auto &selected = pending.costSelections.front();
        selected.selectedIds = {902, 900, 901};
        selected.selectedGenerations = {14, 12, 13};
        selected.selectedIds.resize(count);
        selected.selectedGenerations.resize(count);
        const auto progress = ruledPendingGraveyardCostSelectionProgress(pending);
        ASSERT_TRUE(progress.has_value());
        EXPECT_EQ(progress->confirmable, count == 3);
    }
    ruled::v1::CostSelection command;
    ruledWriteCostObjectRefs(pending.costSelections.front(), command);
    ASSERT_EQ(command.battlefield_objects().objects_size(), 3);
    EXPECT_EQ(command.battlefield_objects().objects(0).object_id(), 902u);
    EXPECT_EQ(command.battlefield_objects().objects(0).zone_change_generation(), 14u);
    EXPECT_EQ(command.battlefield_objects().objects(1).object_id(), 900u);
    EXPECT_EQ(command.battlefield_objects().objects(1).zone_change_generation(), 12u);
    EXPECT_EQ(command.battlefield_objects().objects(2).object_id(), 901u);
    EXPECT_EQ(command.battlefield_objects().objects(2).zone_change_generation(), 13u);
    pending.costSelections.front().selectedGenerations[1] = 11;
    EXPECT_FALSE(ruledPendingGraveyardCostSelectionProgress(pending)->confirmable);
    RuledCostChoice singleton = parsed;
    singleton.min = singleton.max = 1;
    EXPECT_FALSE(ruledCostUsesObjectRefs(singleton));
    EXPECT_FALSE(ruledCostNeedsConfirmation(singleton));
}

TEST_F(RuledClientTest, BlightCostsUseAnAuthoritativeCreaturePicker)
{
    ruled::v1::RuledEventBatch batch;
    auto *hand = (*batch.mutable_legal_by_player())[kLocalPlayer].add_hand_actions();
    hand->set_kind(ruled::v1::HAND_ACTION_CAST_SPELL);
    hand->set_cast_method(ruled::v1::CAST_METHOD_NORMAL);
    hand->set_hand_index(3);
    auto *choice = hand->mutable_cost_choices()->add_choices();
    choice->set_cost_index(1);
    choice->set_zone(ruled::v1::COST_CHOICE_ZONE_BATTLEFIELD);
    choice->set_kind(ruled::v1::COST_CHOICE_KIND_BLIGHT);
    choice->set_blight_count(2);
    choice->set_min(1);
    choice->set_max(1);
    choice->add_candidate_ids(900);
    auto *ref = choice->add_candidate_objects();
    ref->mutable_object()->set_object_id(900);
    ref->mutable_object()->set_zone_change_generation(12);
    auto *group = hand->mutable_cost_choices()->add_cast_cost_groups();
    group->set_group_index(0);
    group->set_max(1);
    auto *option = group->add_options();
    option->set_kind(ruled::v1::CAST_COST_OPTION_KIND_BLIGHT);
    option->set_label("Blight 2");
    option->set_selectable(true);
    option->add_valid_permanent_ids(900);
    option->add_valid_permanent_generations(12);
    apply(batch);
    const auto costs = state->spellCostData(3, 0, RuledCastSource::Hand);
    ASSERT_EQ(costs.choices.size(), 1);
    EXPECT_EQ(costs.choices[0].kind, RuledCostChoiceKind::Blight);
    EXPECT_EQ(costs.choices[0].blightCount, 2u);
    EXPECT_TRUE(ruledCostSelectionPrompt(costs.choices[0], QStringLiteral("Wild Unraveling")).contains("Blight 2"));
    PendingRuledSpellCast pending;
    pending.valid = true;
    pending.waitingForCost = true;
    pending.costChoices = costs.choices;
    EXPECT_TRUE(ruledPendingGraveyardCostSelectionProgress(pending).has_value());
    pending.costSelections = {{1, RuledCostChoiceZone::Battlefield, {900}, {12}}};
    EXPECT_TRUE(ruledPendingGraveyardCostSelectionProgress(pending)->confirmable);
    ruled::v1::CostSelection command;
    ruledWriteCostObjectRefs(pending.costSelections[0], command);
    ASSERT_EQ(command.battlefield_objects().objects_size(), 1);
    EXPECT_EQ(command.battlefield_objects().objects(0).object_id(), 900u);
    EXPECT_EQ(command.battlefield_objects().objects(0).zone_change_generation(), 12u);
    pending.costSelections[0].selectedGenerations[0] = 11;
    EXPECT_FALSE(ruledPendingGraveyardCostSelectionProgress(pending)->confirmable);
    pending.waitingForCastCostObject = true;
    pending.activeCastCostOption = 0;
    pending.castCostGroups = costs.castCostGroups;
    EXPECT_EQ(ruledCastCostObjectEligibility(pending, RuledCastCostCandidateKind::Permanent, 900),
              RuledTargetClickEligibility::Legal);
    EXPECT_EQ(ruledCastCostObjectEligibility(pending, RuledCastCostCandidateKind::Permanent, 901),
              RuledTargetClickEligibility::Illegal);
    EXPECT_EQ(ruledCastCostObjectEligibility(pending, RuledCastCostCandidateKind::Hand, 900),
              RuledTargetClickEligibility::Illegal);
}

TEST(RuledPendingCostSelectionTest, BlightCanShareACreatureWithTapAndSacrifice)
{
    RuledCostChoice blight;
    blight.costIndex = 1;
    blight.kind = RuledCostChoiceKind::Blight;
    RuledCostChoice sacrifice;
    sacrifice.costIndex = 2;
    sacrifice.kind = RuledCostChoiceKind::Sacrifice;
    const QVector<RuledCostChoice> choices{blight, sacrifice};
    EXPECT_FALSE(
        ruledCostSelectionConflicts(sacrifice, choices, {1, RuledCostChoiceZone::Battlefield, {900}, {12}}, 900));
    EXPECT_FALSE(ruledCostSelectionConflicts(blight, choices, {2, RuledCostChoiceZone::Battlefield, {900}}, 900));
}

TEST_F(RuledClientTest, ParsesEngineAuthoredOptionalCastCostGroups)
{
    ruled::v1::RuledEventBatch batch;
    auto *hand = (*batch.mutable_legal_by_player())[kLocalPlayer].add_hand_actions();
    hand->set_kind(ruled::v1::HAND_ACTION_CAST_SPELL);
    hand->set_cast_method(ruled::v1::CAST_METHOD_NORMAL);
    hand->set_hand_index(3);
    auto *group = hand->mutable_cost_choices()->add_cast_cost_groups();
    group->set_group_index(0);
    group->set_prompt("Choose an additional cost.");
    group->set_min(0);
    group->set_max(1);
    auto *kicker = group->add_options();
    kicker->set_option_index(0);
    kicker->set_label("Kicker {2}");
    kicker->set_kind(ruled::v1::CAST_COST_OPTION_KIND_MANA);
    kicker->set_additional_mana_cost("{2}");
    kicker->set_selectable(true);
    auto *behold = group->add_options();
    behold->set_option_index(1);
    behold->set_label("Behold a Dragon");
    behold->set_kind(ruled::v1::CAST_COST_OPTION_KIND_BEHOLD);
    behold->add_valid_hand_indices(7);
    behold->add_valid_permanent_ids(900);
    behold->add_valid_permanent_generations(12);
    behold->set_selectable(true);
    auto *discard = group->add_options();
    discard->set_option_index(2);
    discard->set_label("Discard a card");
    discard->set_kind(ruled::v1::CAST_COST_OPTION_KIND_DISCARD_CARD);
    discard->add_valid_hand_indices(8);
    discard->set_selectable(true);
    auto *payLife = group->add_options();
    payLife->set_option_index(3);
    payLife->set_label("Pay 3 life");
    payLife->set_kind(ruled::v1::CAST_COST_OPTION_KIND_PAY_LIFE);
    payLife->set_selectable(true);
    apply(batch);

    const auto costs = state->spellCostData(3, 0, RuledCastSource::Hand);
    ASSERT_EQ(costs.castCostGroups.size(), 1);
    EXPECT_EQ(costs.castCostGroups.first().prompt, QStringLiteral("Choose an additional cost."));
    ASSERT_EQ(costs.castCostGroups.first().options.size(), 4);
    EXPECT_EQ(costs.castCostGroups.first().options.at(0).additionalManaCost, QStringLiteral("{2}"));
    EXPECT_EQ(costs.castCostGroups.first().options.at(1).validHandIndices, QSet<quint32>({7}));
    EXPECT_EQ(costs.castCostGroups.first().options.at(1).validPermanentIds, QSet<quint32>({900}));
    EXPECT_EQ(costs.castCostGroups.first().options.at(1).validPermanentGenerations.value(900), 12u);
    EXPECT_EQ(costs.castCostGroups.first().options.at(2).kind, RuledCastCostOptionKind::DiscardCard);
    EXPECT_EQ(costs.castCostGroups.first().options.at(2).validHandIndices, QSet<quint32>({8}));
    EXPECT_EQ(costs.castCostGroups.first().options.at(3).kind, RuledCastCostOptionKind::PayLife);
}

TEST_F(RuledClientTest, MultikickerOfferPreservesEngineQuantityBound)
{
    ruled::v1::RuledEventBatch batch;
    auto *hand = (*batch.mutable_legal_by_player())[kLocalPlayer].add_hand_actions();
    hand->set_kind(ruled::v1::HAND_ACTION_CAST_SPELL);
    hand->set_cast_method(ruled::v1::CAST_METHOD_NORMAL);
    hand->set_hand_index(3);
    auto *group = hand->mutable_cost_choices()->add_cast_cost_groups();
    group->set_max(1);
    auto *option = group->add_options();
    option->set_kind(ruled::v1::CAST_COST_OPTION_KIND_MANA);
    option->set_additional_mana_cost("{2}");
    option->set_maximum_repetitions(1073741823u);
    option->set_selectable(true);
    apply(batch);
    const auto costs = state->spellCostData(3, 0, RuledCastSource::Hand);
    ASSERT_EQ(costs.castCostGroups.size(), 1);
    ASSERT_EQ(costs.castCostGroups.first().options.size(), 1);
    EXPECT_EQ(costs.castCostGroups.first().options.first().maximumRepetitions, std::optional<quint32>(1073741823u));
}

TEST(RuledPendingCastTest, MultikickerStagesOneCountThenDeclineRemovesItsWholeFee)
{
    RuledPendingCast pending;
    auto &spell = pending.beginSpell();
    RuledCastCostOption option;
    option.optionIndex = 0;
    option.kind = RuledCastCostOptionKind::Mana;
    option.additionalManaCost = "{2}";
    option.maximumRepetitions = 1073741823u;
    option.selectable = true;
    RuledCastCostGroup group;
    group.groupIndex = 0;
    group.options = {option};
    spell.castCostGroups = {group};
    spell.remainingCost = {{'X', 1}, {'R', 1}};
    EXPECT_TRUE(pending.stageRepeatedCastCost(0, 3));
    EXPECT_EQ(spell.remainingCost.value('X'), 7);
    ASSERT_EQ(spell.castCostSelections.size(), 1);
    EXPECT_EQ(spell.castCostSelections.first().repetitions, std::optional<quint32>(3));
    EXPECT_TRUE(ruledCastCostGroupSelectionCompletesImmediately(spell, group));
    EXPECT_FALSE(pending.stageRepeatedCastCost(0, 2));
    EXPECT_EQ(spell.remainingCost.value('X'), 7);
    EXPECT_TRUE(pending.declineCastCostGroup());
    EXPECT_TRUE(spell.valid);
    EXPECT_EQ(spell.remainingCost.value('X'), 1);
    EXPECT_EQ(spell.remainingCost.value('R'), 1);
    EXPECT_EQ(spell.nextCastCostGroup, 1);
    EXPECT_TRUE(spell.castCostSelections.isEmpty());
}

TEST(RuledPendingCastTest, MultikickerRejectsOversizedNumbersAndCombinedTotals)
{
    RuledCastCostOption option;
    option.kind = RuledCastCostOptionKind::Mana;
    option.additionalManaCost = "{2}";
    option.maximumRepetitions = 1073741823u;
    EXPECT_EQ(RuledPendingCast::repeatedCastCostAmount(option, 1073741823u), std::optional<int>(2147483646));
    EXPECT_FALSE(RuledPendingCast::repeatedCastCostAmount(option, 0));
    EXPECT_FALSE(RuledPendingCast::repeatedCastCostAmount(option, 1073741824u));
    EXPECT_FALSE(RuledPendingCast::repeatedCastCostAmount(option, std::numeric_limits<quint32>::max()));
    EXPECT_FALSE(RuledPendingCast::parseRepresentableManaCost("{2147483648}"));
    EXPECT_FALSE(RuledPendingCast::parseRepresentableManaCost("{2147483647}{1}"));
    EXPECT_FALSE(RuledPendingCast::parseRepresentableManaCost("{2147483647}{R}"));
    EXPECT_EQ(RuledPendingCast::parseRepresentableManaCost("{2147483646}{R}")->value('X'), 2147483646);

    RuledPendingCast pending;
    auto &spell = pending.beginSpell();
    spell.remainingCost = {{'X', 2147483646}};
    EXPECT_FALSE(pending.finalizeRepeatedCastManaCost(2, 2));
    EXPECT_EQ(spell.remainingCost.value('X'), 2147483646);
    EXPECT_FALSE(spell.manaCostFinalized);
    EXPECT_TRUE(pending.finalizeRepeatedCastManaCost(1, 2147483647));
    EXPECT_EQ(spell.remainingCost.value('X'), 0);
}

TEST_F(RuledClientTest, MultikickerPromptRevalidatesRefreshedOfferAndReplacedDraft)
{
    auto offer = [](int groupIndex, bool selectable) {
        ruled::v1::RuledEventBatch batch;
        auto *hand = (*batch.mutable_legal_by_player())[kLocalPlayer].add_hand_actions();
        hand->set_kind(ruled::v1::HAND_ACTION_CAST_SPELL);
        hand->set_cast_method(ruled::v1::CAST_METHOD_NORMAL);
        hand->set_hand_index(3);
        auto *group = hand->mutable_cost_choices()->add_cast_cost_groups();
        group->set_group_index(groupIndex);
        group->set_max(1);
        auto *option = group->add_options();
        option->set_kind(ruled::v1::CAST_COST_OPTION_KIND_MANA);
        option->set_additional_mana_cost("{2}");
        option->set_maximum_repetitions(1073741823u);
        option->set_selectable(selectable);
        return batch;
    };
    apply(offer(0, true));
    RuledPendingCast pending;
    auto &spell = pending.beginSpell();
    spell.handIndex = 3;
    spell.castCostGroups = state->spellCostData(3, 0, RuledCastSource::Hand).castCostGroups;
    const auto before = spell;
    apply(offer(0, true));
    ASSERT_TRUE(pending.reconcileSpellCosts(*state, kLocalPlayer));
    EXPECT_TRUE(pending.repeatedCastCostPromptStillCurrent(before, 0));
    EXPECT_TRUE(pending.stageRepeatedCastCost(0, 3));
    EXPECT_TRUE(ruledCastCostGroupSelectionCompletesImmediately(spell, spell.castCostGroups.first()));

    spell.castCostSelections.clear();
    spell.remainingCost.clear();
    apply(offer(7, true));
    ASSERT_TRUE(pending.reconcileSpellCosts(*state, kLocalPlayer));
    EXPECT_FALSE(pending.repeatedCastCostPromptStillCurrent(before, 0));
    apply(offer(0, false));
    ASSERT_TRUE(pending.reconcileSpellCosts(*state, kLocalPlayer));
    EXPECT_FALSE(pending.repeatedCastCostPromptStillCurrent(before, 0));
    pending.clearSpell();
    auto &replacement = pending.beginSpell();
    replacement.handIndex = before.handIndex;
    replacement.castCostGroups = before.castCostGroups;
    EXPECT_FALSE(pending.repeatedCastCostPromptStillCurrent(before, 0));
}

TEST(RuledPendingCastTest, MultikickerXExpansionChecksAggregateBeforeMutation)
{
    RuledPendingCast pending;
    auto &spell = pending.beginSpell();
    spell.remainingCost = {{'X', std::numeric_limits<int>::max()}};
    spell.xPips = 1;
    const auto before = spell;
    EXPECT_FALSE(pending.expandRepeatedCastX(2));
    EXPECT_EQ(spell.remainingCost, before.remainingCost);
    EXPECT_EQ(spell.xPips, before.xPips);
    EXPECT_EQ(spell.xValue, before.xValue);
    spell = before;
    spell.remainingCost = {{'X', 2147483646}, {'R', 1}};
    const auto coloredBefore = spell.remainingCost;
    EXPECT_FALSE(pending.expandRepeatedCastX(2));
    EXPECT_EQ(spell.remainingCost, coloredBefore);
    EXPECT_EQ(spell.xPips, 1);
    spell = before;
    spell.remainingCost = {{'X', 2147483645}, {'R', 1}};
    EXPECT_TRUE(pending.expandRepeatedCastX(2));
    EXPECT_EQ(spell.remainingCost.value('X'), 2147483646);
    EXPECT_EQ(spell.xValue, 2);
    EXPECT_EQ(spell.xPips, 0);
    spell = before;
    EXPECT_TRUE(pending.expandRepeatedCastX(0));
    EXPECT_EQ(spell.remainingCost.value('X'), 2147483646);
}

TEST_F(RuledClientTest, ParsesTeamworkCohortsTargetRequirementsAndAllModesCost)
{
    ruled::v1::RuledEventBatch batch;
    auto *hand = (*batch.mutable_legal_by_player())[kLocalPlayer].add_hand_actions();
    hand->set_kind(ruled::v1::HAND_ACTION_CAST_SPELL);
    hand->set_cast_method(ruled::v1::CAST_METHOD_NORMAL);
    hand->set_hand_index(4);
    hand->set_min_modes(1);
    hand->set_max_modes(2);
    hand->add_modes()->set_selectable(true);
    hand->mutable_all_modes_cast_cost()->set_group_index(3);
    hand->mutable_all_modes_cast_cost()->set_option_index(7);

    auto *group = hand->mutable_cost_choices()->add_cast_cost_groups();
    group->set_group_index(3);
    group->set_min(0);
    group->set_max(1);
    auto *teamwork = group->add_options();
    teamwork->set_option_index(7);
    teamwork->set_kind(ruled::v1::CAST_COST_OPTION_KIND_TAP_PERMANENTS);
    teamwork->set_label("Teamwork 4");
    teamwork->set_selectable(true);
    teamwork->set_object_min(1);
    teamwork->set_object_max(2);
    teamwork->mutable_aggregate_minimum()->set_contribution_kind(ruled::v1::OBJECT_CONTRIBUTION_KIND_CURRENT_POWER);
    teamwork->mutable_aggregate_minimum()->set_minimum(4);
    auto *candidate = teamwork->add_candidate_objects();
    candidate->mutable_object()->set_object_id(900);
    candidate->mutable_object()->set_zone_change_generation(12);
    candidate->set_contribution(4);

    auto *targets = &(*(*batch.mutable_legal_by_player())[kLocalPlayer].mutable_valid_targets_by_hand_slot())[4u << 8];
    auto *requirement = targets->add_cast_cost_requirements();
    requirement->set_group_index(0);
    requirement->mutable_required_cost()->set_group_index(3);
    requirement->mutable_required_cost()->set_option_index(7);
    auto *affected = requirement->add_affected_targets();
    affected->set_kind(ruled::v1::TARGET_REF_KIND_PERMANENT);
    affected->set_object_id(901);
    apply(batch);

    const auto costs = state->spellCostData(4, 0, RuledCastSource::Hand);
    ASSERT_EQ(costs.castCostGroups.size(), 1);
    const auto &parsed = costs.castCostGroups.first().options.first();
    EXPECT_EQ(parsed.kind, RuledCastCostOptionKind::TapPermanents);
    EXPECT_EQ(parsed.objectMin, 1);
    EXPECT_EQ(parsed.objectMax, 2);
    EXPECT_EQ(parsed.aggregateMinimum, 4);
    EXPECT_EQ(parsed.contributionKind, RuledObjectContributionKind::CurrentPower);
    EXPECT_EQ(parsed.validPermanentGenerations.value(900), 12u);
    EXPECT_EQ(parsed.candidateContributions.value(900), 4);

    const auto targetData = state->spellTargetData(4, 0, RuledCastSource::Hand);
    ASSERT_EQ(targetData.castCostRequirements.size(), 1);
    EXPECT_EQ(targetData.castCostRequirements.first().groupIndex, 0);
    EXPECT_EQ(targetData.castCostRequirements.first().costGroupIndex, 3);
    EXPECT_EQ(targetData.castCostRequirements.first().costOptionIndex, 7);
    ASSERT_EQ(targetData.castCostRequirements.first().affectedTargets.size(), 1);
    EXPECT_EQ(targetData.castCostRequirements.first().affectedTargets.first().kind,
              ruled::v1::TARGET_REF_KIND_PERMANENT);
    EXPECT_EQ(targetData.castCostRequirements.first().affectedTargets.first().oid, 901u);

    const RuledCastActionKey key{4, 0, RuledCastSource::Hand, ruled::v1::CAST_METHOD_NORMAL, 0};
    const QPair<int, int> expectedAllModesCost{3, 7};
    EXPECT_EQ(state->handActions.value(ruled::v1::HAND_ACTION_CAST_SPELL).allModesCastCostByCastKey.value(key),
              expectedAllModesCost);
}

TEST_F(RuledClientTest, ParsesModeLinkedCastCostCoordinates)
{
    ruled::v1::RuledEventBatch batch;
    auto *hand = (*batch.mutable_legal_by_player())[kLocalPlayer].add_hand_actions();
    hand->set_kind(ruled::v1::HAND_ACTION_CAST_SPELL);
    hand->set_cast_method(ruled::v1::CAST_METHOD_NORMAL);
    hand->set_hand_index(3);
    auto *mode = hand->add_modes();
    mode->set_mode_index(7);
    mode->set_label("Create a Spirit token.");
    mode->set_selectable(true);
    mode->mutable_linked_cast_cost()->set_group_index(2);
    mode->mutable_linked_cast_cost()->set_option_index(5);
    apply(batch);

    const RuledCastActionKey key{3, 0, RuledCastSource::Hand, ruled::v1::CAST_METHOD_NORMAL, 0};
    const auto modes = state->handActions.value(ruled::v1::HAND_ACTION_CAST_SPELL).modalOptionsByCastKey.value(key);
    ASSERT_EQ(modes.size(), 1);
    EXPECT_EQ(modes.first().linkedCastCostGroupIndex, 2);
    EXPECT_EQ(modes.first().linkedCastCostOptionIndex, 5);
}

TEST_F(RuledClientTest, ParsesPublicPermanentResolutionChoiceAsUntargetedBoardClick)
{
    ruled::v1::RuledEventBatch batch;
    auto *choice = batch.add_events()->mutable_resolution_choice_required();
    choice->set_deciding_player_id(kLocalPlayer);
    choice->set_choice_kind(ruled::v1::CHOICE_KIND_PERMANENT_OBJECTS);
    choice->set_min(1);
    choice->set_max(1);
    choice->set_prompt_text("Choose a creature you control.");
    choice->add_candidate_object_ids(901);
    apply(batch);

    EXPECT_TRUE(state->hasPendingChoiceOfKind(RuledClientState::ChoiceKind::PermanentChoice));
    EXPECT_TRUE(state->isPendingChoiceCandidate(RuledClientState::ChoiceKind::PermanentChoice, 901));
    EXPECT_EQ(ruledTargetClickEligibility({}, {}, *state, RuledTargetCandidateKind::Battlefield, 901, kLocalPlayer),
              RuledTargetClickEligibility::Legal);
    EXPECT_EQ(ruledTargetClickEligibility({}, {}, *state, RuledTargetCandidateKind::Stack, 901, kLocalPlayer),
              RuledTargetClickEligibility::Illegal);
}

TEST_F(RuledClientTest, ActiveRevealsShareHistoryAndBecomeDismissibleWhenTheirSourceLeaves)
{
    ruled::v1::RuledEventBatch batch;
    auto *snapshot = batch.add_events()->mutable_active_public_reveal_snapshot();
    for (int i = 0; i < 2; ++i) {
        auto *reveal = snapshot->add_reveals();
        reveal->set_reveal_id("stack:" + std::to_string(700 + i));
        reveal->set_source_object_id(700 + i);
        reveal->set_zone_owner_player_id(i == 0 ? kLocalPlayer : kOpponent);
        reveal->set_source_zone(ruled::v1::CHOICE_CANDIDATE_SOURCE_ZONE_HAND);
        reveal->set_source_description(i == 0 ? "Caustic Exhale" : "Osseous Exhale");
        auto *card = reveal->add_cards();
        card->set_card_id(i == 0 ? "adult_gold_dragon" : "shivan_dragon");
        card->set_card_name(i == 0 ? "Adult Gold Dragon" : "Shivan Dragon");
    }
    QSignalSpy changed(&state->reveals, &RuledRevealState::changed);
    apply(batch);
    ASSERT_EQ(state->reveals.entries().size(), 2);
    EXPECT_EQ(state->reveals.entries().first().sourceDescription, QStringLiteral("Caustic Exhale"));
    EXPECT_EQ(state->reveals.entries().first().phase, RuledRevealState::Phase::Active);
    state->reveals.dismiss("stack:700");
    EXPECT_EQ(state->reveals.entries().size(), 2);
    apply(batch);
    EXPECT_EQ(changed.count(), 1);
    ruled::v1::RuledEventBatch completed;
    completed.add_events()->mutable_active_public_reveal_snapshot();
    apply(completed);
    EXPECT_EQ(state->reveals.entries().first().phase, RuledRevealState::Phase::Completed);
    state->reveals.dismiss("stack:700");
    EXPECT_EQ(state->reveals.entries().size(), 1);
}

TEST_F(RuledClientTest, CancelledHandActivationCanBeRevealedAgainAfterDismissal)
{
    const auto activeSnapshot = [](const std::string &revealId) {
        ruled::v1::RuledEventBatch batch;
        auto *reveal = batch.add_events()
                           ->mutable_active_public_reveal_snapshot()
                           ->add_reveals();
        reveal->set_reveal_id(revealId);
        reveal->set_source_object_id(702);
        reveal->set_zone_owner_player_id(kLocalPlayer);
        reveal->set_source_zone(ruled::v1::CHOICE_CANDIDATE_SOURCE_ZONE_HAND);
        reveal->set_source_description("Urban Retreat");
        auto *card = reveal->add_cards();
        card->set_object_id(702);
        card->set_zone_change_generation(9);
        card->set_card_id("urban_retreat");
        card->set_card_name("Urban Retreat");
        return batch;
    };

    apply(activeSnapshot("activation:702:0:1:1001"));
    ASSERT_EQ(state->reveals.entries().size(), 1);
    EXPECT_EQ(state->reveals.entries().first().phase, RuledRevealState::Phase::Active);

    ruled::v1::RuledEventBatch canceled;
    canceled.add_events()->mutable_active_public_reveal_snapshot();
    apply(canceled);
    ASSERT_EQ(state->reveals.entries().first().phase, RuledRevealState::Phase::Completed);
    state->reveals.dismiss(QStringLiteral("activation:702:0:1:1001"));
    EXPECT_TRUE(state->reveals.entries().isEmpty());

    apply(activeSnapshot("activation:702:0:1:1002"));
    ASSERT_EQ(state->reveals.entries().size(), 1);
    EXPECT_EQ(state->reveals.entries().first().id, QStringLiteral("activation:702:0:1:1002"));
    EXPECT_EQ(state->reveals.entries().first().phase, RuledRevealState::Phase::Active);
}

TEST_F(RuledClientTest, MandatoryResolutionBranchesCannotSubmitDecline)
{
    ruled::v1::RuledEventBatch batch;
    auto *choice = batch.add_events()->mutable_resolution_choice_required();
    choice->set_deciding_player_id(kLocalPlayer);
    choice->set_choice_kind(ruled::v1::CHOICE_KIND_RESOLUTION_BRANCH);
    choice->set_prompt_text("Put the permanent on top or bottom.");
    choice->set_min(1);
    choice->set_max(1);
    for (int index = 0; index < 2; ++index) {
        auto *branch = choice->add_resolution_branches();
        branch->set_branch_index(index);
        branch->set_label(index == 0 ? "Top" : "Bottom");
        branch->set_selectable(true);
    }
    apply(batch);

    ASSERT_TRUE(state->hasPendingChoiceOfKind(RuledClientState::ChoiceKind::ResolutionBranch));
    EXPECT_FALSE(state->pendingClickChoiceMayDecline());
    state->declinePendingClickChoice();
    EXPECT_TRUE(host.sentCommands.isEmpty());
    EXPECT_TRUE(state->hasPendingChoiceOfKind(RuledClientState::ChoiceKind::ResolutionBranch));
}

TEST_F(RuledClientTest, SearchZoneBranchesRemainStructuredAndSubmitTheirOpaqueCombinationIndex)
{
    ruled::v1::RuledEventBatch batch;
    auto *choice = batch.add_events()->mutable_resolution_choice_required();
    choice->set_deciding_player_id(kLocalPlayer);
    choice->set_choice_kind(ruled::v1::CHOICE_KIND_RESOLUTION_BRANCH);
    choice->set_prompt_text("Choose which zones to search.");
    for (int index = 0; index < 7; ++index) {
        auto *branch = choice->add_resolution_branches();
        branch->set_branch_index(index);
        branch->set_label(QStringLiteral("branch %1").arg(index).toStdString());
        branch->set_selectable(true);
        if (index == 0 || index == 3 || index == 4 || index == 6) {
            branch->add_search_zones(ruled::v1::CHOICE_CANDIDATE_SOURCE_ZONE_HAND);
        }
        if (index == 1 || index == 3 || index == 5 || index == 6) {
            branch->add_search_zones(ruled::v1::CHOICE_CANDIDATE_SOURCE_ZONE_GRAVEYARD);
        }
        if (index == 2 || index == 4 || index == 5 || index == 6) {
            branch->add_search_zones(ruled::v1::CHOICE_CANDIDATE_SOURCE_ZONE_LIBRARY);
        }
    }
    apply(batch);

    ASSERT_TRUE(state->hasPendingZoneScopeChoice());
    ASSERT_EQ(state->pendingChoiceOptions().size(), 7);
    EXPECT_EQ(
        state->pendingChoiceOptions().at(4).searchZones,
        QSet<int>({ruled::v1::CHOICE_CANDIDATE_SOURCE_ZONE_HAND, ruled::v1::CHOICE_CANDIDATE_SOURCE_ZONE_LIBRARY}));
    state->submitPendingChoiceOption(4);
    ASSERT_EQ(host.sentCommands.size(), 1);
    EXPECT_EQ(host.sentCommands[0].submit_resolution_choice().selected_branch_index(), 4u);
}

TEST_F(RuledClientTest, MixedStructuredAndOrdinaryResolutionBranchesFailClosed)
{
    ruled::v1::RuledEventBatch batch;
    auto *choice = batch.add_events()->mutable_resolution_choice_required();
    choice->set_deciding_player_id(kLocalPlayer);
    choice->set_choice_kind(ruled::v1::CHOICE_KIND_RESOLUTION_BRANCH);
    auto *structured = choice->add_resolution_branches();
    structured->set_branch_index(0);
    structured->set_selectable(true);
    structured->add_search_zones(ruled::v1::CHOICE_CANDIDATE_SOURCE_ZONE_HAND);
    auto *missing = choice->add_resolution_branches();
    missing->set_branch_index(1);
    missing->set_selectable(true);
    apply(batch);

    EXPECT_FALSE(state->hasPendingChoiceOptions());
}

TEST_F(RuledClientTest, PrivateHandChoiceMakesTheNonDecidingPlayerWait)
{
    ruled::v1::RuledEventBatch batch;
    auto *rcr = batch.add_events()->mutable_resolution_choice_required();
    rcr->set_deciding_player_id(kLocalPlayer + 1);
    rcr->set_choice_kind(ruled::v1::CHOICE_KIND_HAND_CARDS);
    rcr->set_prompt_text("Opponent is making a resolution choice.");
    apply(batch);

    EXPECT_FALSE(state->isResolutionHandPickActive());
    EXPECT_TRUE(state->isWaitingForChoice());
    EXPECT_EQ(state->choiceWaitingPlayer(), kOpponent);

    ruled::v1::RuledEventBatch completed;
    apply(completed);
    EXPECT_FALSE(state->isWaitingForChoice());
}

TEST_F(RuledClientTest, AcceptedPermanentSelectionsClearWithoutChangingTheKeptGeneration)
{
    constexpr quint32 sourceOid = 7000;
    constexpr quint32 chosenOid = 8100;
    constexpr quint64 generation = 12;
    state->battlefieldGenerationByOid.insert(chosenOid, generation);

    ruled::v1::RuledEventBatch pending;
    auto *choice = pending.add_events()->mutable_resolution_choice_required();
    choice->set_deciding_player_id(kOpponent);
    choice->set_source_object_id(sourceOid);
    choice->set_choice_kind(ruled::v1::CHOICE_KIND_PERMANENT_OBJECTS);
    choice->set_prompt_text("Opponent is choosing creatures to sacrifice.");
    auto *selection = choice->add_accepted_permanent_selections();
    selection->set_selecting_player_id(kLocalPlayer + 1);
    selection->set_object_id(chosenOid);
    selection->set_zone_change_generation(generation);
    apply(pending);

    EXPECT_TRUE(state->isWaitingForChoice());
    EXPECT_EQ(state->acceptedSacrificeChooserForEngineOid(chosenOid), kLocalPlayer + 1);
    state->battlefieldGenerationByOid.insert(chosenOid, generation + 1);
    EXPECT_EQ(state->acceptedSacrificeChooserForEngineOid(chosenOid), -1);
    state->battlefieldGenerationByOid.insert(chosenOid, generation);
    EXPECT_EQ(state->acceptedSacrificeChooserForEngineOid(chosenOid), kLocalPlayer + 1);

    ruled::v1::RuledEventBatch completed;
    auto *progress = completed.add_events()->mutable_resolution_choice_progress();
    progress->set_source_object_id(sourceOid);
    apply(completed);
    EXPECT_EQ(state->acceptedSacrificeChooserForEngineOid(chosenOid), -1);
    EXPECT_EQ(state->acceptedSacrificeSelectionSourceOid, 0u);
    EXPECT_EQ(state->battlefieldGenerationByOid.value(chosenOid), generation);
}

TEST_F(RuledClientTest, AlternativeDiscardRequiresOneMatchingCardOrTwoCards)
{
    ruled::v1::RuledEventBatch batch;
    auto *choice = batch.add_events()->mutable_resolution_choice_required();
    choice->set_deciding_player_id(kLocalPlayer);
    choice->set_choice_kind(ruled::v1::CHOICE_KIND_HAND_CARDS);
    choice->set_min(1);
    choice->set_max(2);
    for (int i = 0; i < 3; ++i) {
        choice->add_candidate_object_ids(501u + i);
        choice->add_candidate_server_card_ids(21 + i);
    }
    auto *one = choice->add_selection_alternatives();
    one->set_count(1);
    one->add_candidate_indices(0);
    auto *two = choice->add_selection_alternatives();
    two->set_count(2);
    for (int i = 0; i < 3; ++i) {
        two->add_candidate_indices(i);
    }
    apply(batch);
    ASSERT_TRUE(state->isResolutionHandPickActive());
    state->toggleResolutionHandPickCard(22);
    EXPECT_FALSE(state->resolutionHandPickConfirmable());
    state->submitResolutionHandPick();
    EXPECT_TRUE(host.sentCommands.isEmpty());
    state->toggleResolutionHandPickCard(23);
    EXPECT_TRUE(state->resolutionHandPickConfirmable());
    state->submitResolutionHandPick();
    ASSERT_EQ(host.sentCommands.size(), 1);
    EXPECT_EQ(host.sentCommands[0].submit_resolution_choice().chosen_object_ids_size(), 2);
    host.sentCommands.clear();
    apply(batch);
    state->toggleResolutionHandPickCard(21);
    EXPECT_TRUE(state->resolutionHandPickConfirmable());
    EXPECT_TRUE(host.sentCommands.isEmpty());
    state->submitResolutionHandPick();
    ASSERT_EQ(host.sentCommands.size(), 1);
    EXPECT_EQ(host.sentCommands[0].submit_resolution_choice().chosen_object_ids(0), 501u);
}

TEST_F(RuledClientTest, MalformedDiscardAlternativesRetireThePreviousSelection)
{
    ruled::v1::RuledEventBatch valid;
    auto *choice = valid.add_events()->mutable_resolution_choice_required();
    choice->set_deciding_player_id(kLocalPlayer);
    choice->set_choice_kind(ruled::v1::CHOICE_KIND_HAND_CARDS);
    choice->set_min(1);
    choice->set_max(1);
    choice->add_candidate_object_ids(501u);
    choice->add_candidate_server_card_ids(21);
    auto *one = choice->add_selection_alternatives();
    one->set_count(1);
    one->add_candidate_indices(0);
    for (int malformed = 0; malformed < 4; ++malformed) {
        apply(valid);
        state->toggleResolutionHandPickCard(21);
        ASSERT_TRUE(state->resolutionHandPickConfirmable());
        auto invalid = valid;
        auto *alternative =
            invalid.mutable_events(0)->mutable_resolution_choice_required()->mutable_selection_alternatives(0);
        if (malformed == 0)
            alternative->add_candidate_indices(99);
        if (malformed == 1)
            alternative->add_candidate_indices(0);
        if (malformed == 2)
            alternative->set_count(0);
        if (malformed == 3)
            invalid.mutable_events(0)->mutable_resolution_choice_required()->set_max(0);
        apply(invalid);
        EXPECT_FALSE(state->isResolutionHandPickActive());
        EXPECT_FALSE(state->resolutionHandPickConfirmable());
        state->submitResolutionHandPick();
        EXPECT_TRUE(host.sentCommands.isEmpty());
    }
}

TEST_F(RuledClientTest, LibrarySearchAlternativesAllowFailToFindSingletonAndCompatiblePair)
{
    ruled::v1::RuledEventBatch batch;
    auto *choice = batch.add_events()->mutable_resolution_choice_required();
    choice->set_deciding_player_id(kLocalPlayer);
    choice->set_choice_kind(ruled::v1::CHOICE_KIND_LIBRARY_SEARCH);
    choice->set_min(0);
    choice->set_max(2);
    for (int i = 0; i < 4; ++i) {
        choice->add_candidate_object_ids(801u + i);
        choice->add_candidate_server_card_ids(11 + i);
        choice->add_candidate_names(i < 2 ? "Forest" : (i == 2 ? "Island" : "Wastes"));
    }
    auto *one = choice->add_selection_alternatives();
    one->set_count(1);
    for (int i = 0; i < 4; ++i)
        one->add_candidate_indices(i);
    auto *two = choice->add_selection_alternatives();
    two->set_count(2);
    two->add_candidate_indices(0);
    two->add_candidate_indices(1);
    apply(batch);
    ASSERT_TRUE(state->isResolutionHandPickActive());
    EXPECT_EQ(state->resolutionHandPickZone(), RuledClientState::PickZone::Deck);
    EXPECT_TRUE(state->resolutionHandPickConfirmable());
    state->submitResolutionHandPick();
    ASSERT_EQ(host.sentCommands.size(), 1);
    EXPECT_EQ(host.sentCommands.last().submit_resolution_choice().chosen_object_ids_size(), 0);
    host.sentCommands.clear();
    apply(batch);
    state->toggleResolutionHandPickCard(14);
    ASSERT_EQ(state->resolutionHandPickSelected(), 1);
    EXPECT_TRUE(state->resolutionHandPickConfirmable());
    EXPECT_FALSE(state->isResolutionHandPickCardSelectable(11));
    state->toggleResolutionHandPickCard(11);
    EXPECT_EQ(state->resolutionHandPickSelected(), 1);
    state->submitResolutionHandPick();
    ASSERT_EQ(host.sentCommands.size(), 1);
    EXPECT_EQ(host.sentCommands.last().submit_resolution_choice().chosen_object_ids(0), 804u);
    host.sentCommands.clear();
    apply(batch);
    state->toggleResolutionHandPickCard(11);
    EXPECT_TRUE(state->resolutionHandPickConfirmable());
    EXPECT_FALSE(state->isResolutionHandPickCardSelectable(13));
    EXPECT_TRUE(state->isResolutionHandPickCardSelectable(12));
    state->toggleResolutionHandPickCard(12);
    EXPECT_EQ(state->resolutionHandPickSelected(), 2);
    EXPECT_TRUE(state->resolutionHandPickConfirmable());
    state->submitResolutionHandPick();
    ASSERT_EQ(host.sentCommands.size(), 1);
    EXPECT_EQ(host.sentCommands.last().submit_resolution_choice().chosen_object_ids_size(), 2);
}

TEST_F(RuledClientTest, MalformedLibraryAlternativesRetirePreviousPrivatePicker)
{
    ruled::v1::RuledEventBatch valid;
    auto *choice = valid.add_events()->mutable_resolution_choice_required();
    choice->set_deciding_player_id(kLocalPlayer);
    choice->set_choice_kind(ruled::v1::CHOICE_KIND_LIBRARY_SEARCH);
    choice->set_min(0);
    choice->set_max(1);
    choice->add_candidate_object_ids(801u);
    choice->add_candidate_server_card_ids(11);
    choice->add_candidate_names("Wastes");
    auto *one = choice->add_selection_alternatives();
    one->set_count(1);
    one->add_candidate_indices(0);
    for (int malformed = 0; malformed < 8; ++malformed) {
        apply(valid);
        ASSERT_TRUE(state->isResolutionHandPickActive());
        state->toggleResolutionHandPickCard(11);
        auto invalid = valid;
        auto *bad = invalid.mutable_events(0)->mutable_resolution_choice_required();
        if (malformed == 0)
            bad->mutable_selection_alternatives(0)->add_candidate_indices(99);
        if (malformed == 1)
            bad->mutable_selection_alternatives(0)->add_candidate_indices(0);
        if (malformed == 2)
            bad->mutable_selection_alternatives(0)->set_count(0);
        if (malformed == 3)
            bad->clear_candidate_server_card_ids();
        if (malformed == 4)
            bad->set_candidate_server_card_ids(0, -1);
        if (malformed == 5)
            bad->clear_candidate_names();
        if (malformed == 6 || malformed == 7) {
            bad->add_candidate_object_ids(malformed == 7 ? 801u : 802u);
            bad->add_candidate_server_card_ids(malformed == 6 ? 11 : 12);
            bad->add_candidate_names("Forest");
        }
        apply(invalid);
        EXPECT_FALSE(state->isResolutionHandPickActive());
        EXPECT_FALSE(state->resolutionHandPickConfirmable());
        state->submitResolutionHandPick();
        EXPECT_TRUE(host.sentCommands.isEmpty());
    }
}

TEST_F(RuledClientTest, LibraryAlternativesRejectedAckRestoresSelectionAndCannotReplaceNewAuthority)
{
    for (const bool newerChoice : {false, true}) {
        ruled::v1::RuledEventBatch batch;
        auto *choice = batch.add_events()->mutable_resolution_choice_required();
        choice->set_deciding_player_id(kLocalPlayer);
        choice->set_choice_kind(ruled::v1::CHOICE_KIND_LIBRARY_SEARCH);
        choice->set_min(0);
        choice->set_max(1);
        choice->add_candidate_object_ids(801u);
        choice->add_candidate_server_card_ids(11);
        choice->add_candidate_names("Wastes");
        auto *one = choice->add_selection_alternatives();
        one->set_count(1);
        one->add_candidate_indices(0);
        apply(batch);
        state->toggleResolutionHandPickCard(11);
        state->submitResolutionHandPick();
        ASSERT_EQ(host.sentCommands.size(), 1);
        if (newerChoice) {
            choice->set_candidate_object_ids(0, 901u);
            choice->set_candidate_server_card_ids(0, 21);
            choice->set_candidate_names(0, "Forest");
            const quint64 newerPromptId = nextSyntheticResolutionPromptId++;
            auto *snapshot = batch.mutable_resolution_prompt_state();
            snapshot->set_resolution_prompt_id(newerPromptId);
            choice->set_resolution_prompt_id(newerPromptId);
            apply(batch);
            state->toggleResolutionHandPickCard(21);
        }
        host.answerPendingAck(false);
        ASSERT_TRUE(state->isResolutionHandPickActive());
        EXPECT_EQ(state->resolutionHandPickSelected(), 1);
        EXPECT_TRUE(state->resolutionHandPickConfirmable());
        EXPECT_TRUE(state->isResolutionHandPickCardSelectable(newerChoice ? 21 : 11));
        EXPECT_FALSE(state->isResolutionHandPickCardSelectable(newerChoice ? 11 : 21));
        state->submitResolutionHandPick();
        ASSERT_EQ(host.sentCommands.size(), 2);
        EXPECT_EQ(host.sentCommands.last().submit_resolution_choice().chosen_object_ids(0), newerChoice ? 901u : 801u);
        host.answerPendingAck(true);
        host.sentCommands.clear();
    }
}

TEST_F(RuledClientTest, SequentialPlayerSetDiscardReplacesPickWithWaitAndNextPrivatePick)
{
    ruled::v1::RuledEventBatch first;
    auto *firstChoice = first.add_events()->mutable_resolution_choice_required();
    firstChoice->set_deciding_player_id(kLocalPlayer);
    firstChoice->set_choice_kind(ruled::v1::CHOICE_KIND_HAND_CARDS);
    firstChoice->set_prompt_text("Choose one card to discard.");
    firstChoice->set_min(1);
    firstChoice->set_max(1);
    firstChoice->add_candidate_object_ids(501u);
    firstChoice->add_candidate_server_card_ids(1);
    apply(first);
    ASSERT_TRUE(state->isResolutionHandPickActive());
    state->toggleResolutionHandPickCard(1);
    EXPECT_EQ(state->resolutionHandPickSelected(), 1);

    ruled::v1::RuledEventBatch waiting;
    auto *waitChoice = waiting.add_events()->mutable_resolution_choice_required();
    waitChoice->set_deciding_player_id(kOpponent);
    waitChoice->set_choice_kind(ruled::v1::CHOICE_KIND_HAND_CARDS);
    waitChoice->set_prompt_text("Opponent is making a resolution choice.");
    apply(waiting);
    EXPECT_FALSE(state->isResolutionHandPickActive());
    EXPECT_EQ(state->resolutionHandPickSelected(), 0);
    EXPECT_TRUE(state->isWaitingForChoice());
    EXPECT_EQ(state->choiceWaitingPlayer(), kOpponent);

    ruled::v1::RuledEventBatch nextPrivate;
    auto *nextChoice = nextPrivate.add_events()->mutable_resolution_choice_required();
    nextChoice->set_deciding_player_id(kLocalPlayer);
    nextChoice->set_choice_kind(ruled::v1::CHOICE_KIND_HAND_CARDS);
    nextChoice->set_prompt_text("Choose one card to discard.");
    nextChoice->set_min(1);
    nextChoice->set_max(1);
    nextChoice->add_candidate_object_ids(601u);
    nextChoice->add_candidate_server_card_ids(2);
    apply(nextPrivate);
    ASSERT_TRUE(state->isResolutionHandPickActive());
    EXPECT_FALSE(state->isWaitingForChoice());
    EXPECT_FALSE(state->isResolutionHandPickCardSelectable(1));
    EXPECT_TRUE(state->isResolutionHandPickCardSelectable(2));

    state->toggleResolutionHandPickCard(2);
    host.sentCommands.clear();
    state->submitResolutionHandPick();
    ASSERT_EQ(host.sentCommands.size(), 1);
    ASSERT_EQ(host.sentCommands[0].submit_resolution_choice().chosen_object_ids_size(), 1);
    EXPECT_EQ(host.sentCommands[0].submit_resolution_choice().chosen_object_ids(0), 601u);
}

TEST(RuledManaPoolTrackerTest, OptimisticStagingDoesNotTurnOldManaIntoNewProduction)
{
    RuledManaPoolTracker tracker;

    const auto first = tracker.observe(7, 0, 1, 0);
    EXPECT_EQ(first.newlyProduced, 1);
    EXPECT_EQ(first.displayedBeforeNewStaging, 1);

    // The first produced pip is staged and hidden locally. The next engine value is 2, but only
    // one pip is new and only that one may be auto-applied.
    const auto second = tracker.observe(7, 0, 2, 1);
    EXPECT_EQ(second.newlyProduced, 1);
    EXPECT_EQ(second.displayedBeforeNewStaging, 1);

    const auto undoSecond = tracker.observe(7, 0, 1, 1);
    EXPECT_EQ(undoSecond.newlyProduced, 0);
    EXPECT_EQ(undoSecond.displayedBeforeNewStaging, 0);
    const auto undoFirst = tracker.observe(7, 0, 0, 0);
    EXPECT_EQ(undoFirst.newlyProduced, 0);
    EXPECT_EQ(undoFirst.displayedBeforeNewStaging, 0);
}

TEST_F(RuledClientTest, HandCardsChoiceStartsAClickToPickAndSubmitsInClickOrder)
{
    ruled::v1::RuledEventBatch batch;
    auto *rcr = batch.add_events()->mutable_resolution_choice_required();
    rcr->set_deciding_player_id(kLocalPlayer);
    rcr->set_choice_kind(ruled::v1::CHOICE_KIND_HAND_CARDS); // Brainstorm
    rcr->set_prompt_text("Put two cards from your hand on top of your library.");
    rcr->set_min(2);
    rcr->set_max(2);
    rcr->set_ordered(true);
    for (const quint32 oid : {11u, 12u, 13u}) {
        rcr->add_candidate_object_ids(oid);
    }
    for (const int scid : {1, 2, 3}) {
        rcr->add_candidate_server_card_ids(scid);
    }
    apply(batch);

    ASSERT_TRUE(state->isResolutionHandPickActive());
    EXPECT_EQ(state->resolutionHandPickZone(), RuledClientState::PickZone::Hand);
    EXPECT_EQ(state->resolutionHandPickRequired(), 2);
    EXPECT_TRUE(state->isResolutionHandPickCardSelectable(1));
    EXPECT_FALSE(state->isResolutionHandPickCardSelectable(99));

    state->toggleResolutionHandPickCard(3);
    state->toggleResolutionHandPickCard(1);
    EXPECT_EQ(state->resolutionHandPickClickOrderFor(3), 1);
    EXPECT_EQ(state->resolutionHandPickClickOrderFor(1), 2);
    // Capped at max.
    state->toggleResolutionHandPickCard(2);
    EXPECT_EQ(state->resolutionHandPickSelected(), 2);

    host.sentCommands.clear();
    state->submitResolutionHandPick();
    ASSERT_EQ(host.sentCommands.size(), 1);
    const auto &sub = host.sentCommands[0].submit_resolution_choice();
    ASSERT_EQ(sub.chosen_object_ids_size(), 2);
    // Click order is preserved — Brainstorm's ordering is load-bearing.
    EXPECT_EQ(sub.chosen_object_ids(0), 13u);
    EXPECT_EQ(sub.chosen_object_ids(1), 11u);
    EXPECT_FALSE(state->isResolutionHandPickActive());
}

TEST_F(RuledClientTest, HandCardsChoiceHonorsEngineAuthoredEligibility)
{
    ruled::v1::RuledEventBatch batch;
    auto *choice = batch.add_events()->mutable_resolution_choice_required();
    choice->set_deciding_player_id(kLocalPlayer);
    choice->set_choice_kind(ruled::v1::CHOICE_KIND_HAND_CARDS);
    choice->set_prompt_text("You may put a land card from your hand onto the battlefield.");
    choice->set_min(0);
    choice->set_max(1);
    choice->add_candidate_object_ids(21u);
    choice->add_candidate_object_ids(22u);
    choice->add_candidate_server_card_ids(201);
    choice->add_candidate_server_card_ids(202);
    choice->add_candidate_names("Forest");
    choice->add_candidate_names("Ornithopter");
    choice->add_candidate_selectable(true);
    choice->add_candidate_selectable(false);
    apply(batch);

    ASSERT_TRUE(state->isResolutionHandPickActive());
    EXPECT_TRUE(state->isResolutionHandPickCardSelectable(201));
    EXPECT_FALSE(state->isResolutionHandPickCardSelectable(202));

    state->toggleResolutionHandPickCard(202);
    EXPECT_EQ(state->resolutionHandPickSelected(), 0);
    state->toggleResolutionHandPickCard(201);
    EXPECT_EQ(state->resolutionHandPickSelected(), 1);

    host.sentCommands.clear();
    state->submitResolutionHandPick();
    ASSERT_EQ(host.sentCommands.size(), 1);
    const auto &selected = host.sentCommands[0].submit_resolution_choice();
    ASSERT_EQ(selected.chosen_object_ids_size(), 1);
    EXPECT_EQ(selected.chosen_object_ids(0), 21u);
}

TEST_F(RuledClientTest, HandCardsChoiceRejectsMalformedEligibilityMask)
{
    ruled::v1::RuledEventBatch batch;
    auto *choice = batch.add_events()->mutable_resolution_choice_required();
    choice->set_deciding_player_id(kLocalPlayer);
    choice->set_choice_kind(ruled::v1::CHOICE_KIND_HAND_CARDS);
    choice->set_min(0);
    choice->set_max(1);
    choice->add_candidate_object_ids(21u);
    choice->add_candidate_server_card_ids(201);
    choice->add_candidate_selectable(true);
    choice->add_candidate_selectable(false);
    apply(batch);

    EXPECT_FALSE(state->isResolutionHandPickActive());
    EXPECT_EQ(host.dialogRequests, 0);
}

TEST_F(RuledClientTest, EntryRevealHandPickRejectedAckRestoresSelectionAndDecline)
{
    for (const bool selectCard : {false, true}) {
        ruled::v1::RuledEventBatch batch;
        (*batch.mutable_legal_by_player())[kLocalPlayer];
        batch.add_events()->mutable_active_public_reveal_snapshot();
        auto *choice = batch.add_events()->mutable_resolution_choice_required();
        choice->set_deciding_player_id(kLocalPlayer);
        choice->set_choice_kind(ruled::v1::CHOICE_KIND_HAND_CARDS);
        choice->set_prompt_text("As Game Trail enters, reveal a qualifying card or choose none.");
        choice->set_min(0);
        choice->set_max(1);
        choice->add_candidate_object_ids(501u);
        choice->add_candidate_server_card_ids(41);
        choice->add_candidate_names("Forest");
        apply(batch);
        ASSERT_TRUE(state->isResolutionHandPickActive());
        if (selectCard)
            state->toggleResolutionHandPickCard(41);
        state->submitResolutionHandPick();
        EXPECT_FALSE(state->isResolutionHandPickActive());
        const auto &answer = host.sentCommands.last().submit_resolution_choice();
        EXPECT_EQ(answer.chosen_object_ids_size(), selectCard ? 1 : 0);
        host.answerPendingAck(false);
        ASSERT_TRUE(state->isResolutionHandPickActive());
        EXPECT_EQ(state->resolutionHandPickSelected(), selectCard ? 1 : 0);
        EXPECT_TRUE(state->resolutionHandPickConfirmable());
        state->submitResolutionHandPick();
        host.answerPendingAck(true);
        EXPECT_FALSE(state->isResolutionHandPickActive());
    }
}

TEST_F(RuledClientTest, EntryRevealEmptyOptionalHandPickCanDeclineAndRestoreRejectedAck)
{
    for (quint32 max = 1; max <= 7; ++max) {
        state->clearSessionState();
        host.sentCommands.clear();

        ruled::v1::RuledEventBatch batch;
        (*batch.mutable_legal_by_player())[kLocalPlayer];
        auto *choice = batch.add_events()->mutable_resolution_choice_required();
        choice->set_deciding_player_id(kLocalPlayer);
        choice->set_choice_kind(ruled::v1::CHOICE_KIND_HAND_CARDS);
        choice->set_prompt_text("As Game Trail enters, reveal a qualifying card or choose none.");
        choice->set_min(0);
        choice->set_max(max);
        apply(batch);

        ASSERT_TRUE(state->isResolutionHandPickActive()) << "max=" << max;
        ASSERT_TRUE(state->pendingChoice.has_value());
        EXPECT_EQ(state->pendingChoice->min, 0);
        EXPECT_EQ(state->pendingChoice->max, static_cast<int>(max));
        EXPECT_TRUE(state->pendingClickChoiceMayDecline());
        EXPECT_TRUE(state->resolutionHandPickConfirmable());

        state->declinePendingClickChoice();
        ASSERT_EQ(host.sentCommands.size(), 1);
        ASSERT_TRUE(host.sentCommands.last().has_submit_resolution_choice());
        EXPECT_EQ(host.sentCommands.last().submit_resolution_choice().chosen_object_ids_size(), 0);
        host.answerPendingAck(false);

        ASSERT_TRUE(state->isResolutionHandPickActive()) << "max=" << max;
        ASSERT_TRUE(state->pendingChoice.has_value());
        EXPECT_EQ(state->pendingChoice->min, 0);
        EXPECT_EQ(state->pendingChoice->max, static_cast<int>(max));
        EXPECT_EQ(state->resolutionHandPickSelected(), 0);
        EXPECT_TRUE(state->pendingClickChoiceMayDecline());
        EXPECT_TRUE(state->resolutionHandPickConfirmable());

        state->submitResolutionHandPick();
        ASSERT_EQ(host.sentCommands.size(), 2);
        EXPECT_EQ(host.sentCommands.last().submit_resolution_choice().chosen_object_ids_size(), 0);
        host.answerPendingAck(true);
        EXPECT_FALSE(state->isResolutionHandPickActive()) << "max=" << max;
    }
}

TEST_F(RuledClientTest, EntryRevealMalformedEmptyHandPickIsRejected)
{
    for (int variant = 0; variant < 25; ++variant) {
        state->clearSessionState();
        host.sentCommands.clear();

        ruled::v1::RuledEventBatch batch;
        (*batch.mutable_legal_by_player())[kLocalPlayer];
        auto *choice = batch.add_events()->mutable_resolution_choice_required();
        choice->set_deciding_player_id(kLocalPlayer);
        choice->set_choice_kind(ruled::v1::CHOICE_KIND_HAND_CARDS);
        choice->set_min(0);
        choice->set_max(1);
        switch (variant) {
            case 0:
                choice->set_min(1);
                break;
            case 1:
                choice->set_max(0);
                break;
            case 2:
                choice->set_max(8);
                break;
            case 3:
                choice->set_ordered(true);
                break;
            case 4:
                choice->set_unique_names(true);
                break;
            case 5:
                choice->add_candidate_object_ids(101u);
                break;
            case 6:
                choice->add_candidate_card_ids("forest");
                break;
            case 7:
                choice->add_candidate_names("Forest");
                break;
            case 8:
                choice->add_candidate_server_card_ids(41);
                break;
            case 9:
                choice->add_candidate_selectable(true);
                break;
            case 10:
                choice->add_candidate_source_zones(ruled::v1::CHOICE_CANDIDATE_SOURCE_ZONE_HAND);
                break;
            case 11:
                choice->add_candidate_player_ids(kLocalPlayer);
                break;
            case 12:
                choice->add_candidate_token_identities();
                break;
            case 13:
                choice->add_selection_slots()->set_label("slot");
                break;
            case 14:
                choice->add_replacement_options()->set_effect_summary("Replacement");
                break;
            case 15:
                choice->add_accepted_permanent_selections();
                break;
            case 16:
                choice->add_selection_alternatives()->set_count(1);
                break;
            case 17:
                choice->add_resolution_branches();
                break;
            case 18:
                choice->add_combat_defender_options();
                break;
            case 19:
                choice->set_waterbend(true);
                break;
            case 20:
                choice->set_variable_mana_contribution(true);
                break;
            case 21:
                choice->set_generic_mana_cost(1);
                break;
            case 22:
                choice->set_payment_currently_legal(true);
                break;
            case 23:
                choice->set_mana_cost("{U}");
                break;
            case 24:
                choice->mutable_public_reveal()->set_zone_owner_player_id(kOpponent);
                break;
        }
        apply(batch);
        EXPECT_FALSE(state->isResolutionHandPickActive()) << "variant=" << variant;
        EXPECT_FALSE(state->pendingChoice.has_value()) << "variant=" << variant;
        EXPECT_TRUE(host.sentCommands.isEmpty()) << "variant=" << variant;
    }
}

TEST_F(RuledClientTest, NonemptyHandPickStillHonorsPrivateEligibilityAndMaximum)
{
    ruled::v1::RuledEventBatch batch;
    auto *choice = batch.add_events()->mutable_resolution_choice_required();
    choice->set_deciding_player_id(kLocalPlayer);
    choice->set_choice_kind(ruled::v1::CHOICE_KIND_HAND_CARDS);
    choice->set_min(0);
    choice->set_max(2);
    for (int i = 0; i < 4; ++i) {
        choice->add_candidate_object_ids(501u + static_cast<quint32>(i));
        choice->add_candidate_server_card_ids(41 + i);
        choice->add_candidate_selectable(i != 3);
    }
    apply(batch);

    ASSERT_TRUE(state->isResolutionHandPickActive());
    ASSERT_TRUE(state->pendingChoice.has_value());
    EXPECT_EQ(state->pendingChoice->max, 2);
    state->toggleResolutionHandPickCard(41);
    state->toggleResolutionHandPickCard(42);
    EXPECT_EQ(state->resolutionHandPickSelected(), 2);
    state->toggleResolutionHandPickCard(43);
    state->toggleResolutionHandPickCard(44);
    EXPECT_EQ(state->resolutionHandPickSelected(), 2);

    state->submitResolutionHandPick();
    ASSERT_EQ(host.sentCommands.size(), 1);
    const auto &answer = host.sentCommands.last().submit_resolution_choice();
    ASSERT_EQ(answer.chosen_object_ids_size(), 2);
    EXPECT_EQ(answer.chosen_object_ids(0), 501u);
    EXPECT_EQ(answer.chosen_object_ids(1), 502u);
    host.answerPendingAck(true);
    EXPECT_FALSE(state->isResolutionHandPickActive());
}

TEST_F(RuledClientTest, EntryRevealHandPickLateRejectedAckCannotRestoreSupersededAuthority)
{
    for (const int interruption : {0, 1, 2}) {
        ruled::v1::RuledEventBatch batch;
        (*batch.mutable_legal_by_player())[kLocalPlayer];
        auto *choice = batch.add_events()->mutable_resolution_choice_required();
        choice->set_deciding_player_id(kLocalPlayer);
        choice->set_choice_kind(ruled::v1::CHOICE_KIND_HAND_CARDS);
        choice->set_prompt_text("First entry reveal");
        choice->set_min(0);
        choice->set_max(1);
        choice->add_candidate_object_ids(501u);
        choice->add_candidate_server_card_ids(41);
        apply(batch);
        state->submitResolutionHandPick();
        if (interruption == 0) {
            choice->set_prompt_text("New entry reveal");
            apply(batch);
        } else if (interruption == 1) {
            ruled::v1::RuledEventBatch revoked;
            revoked.add_events()->mutable_active_public_reveal_snapshot();
            apply(revoked);
        } else
            state->clearSessionState();
        const auto revision = state->pendingChoiceRevision;
        host.answerPendingAck(false);
        EXPECT_EQ(state->pendingChoiceRevision, revision);
        EXPECT_EQ(state->isResolutionHandPickActive(), interruption == 0);
        if (interruption == 0)
            EXPECT_EQ(state->resolutionHandPickPromptText(), "New entry reveal");
    }
}

TEST_F(RuledClientTest, SubmitIsRefusedBelowTheMinimum)
{
    ruled::v1::RuledEventBatch batch;
    auto *rcr = batch.add_events()->mutable_resolution_choice_required();
    rcr->set_deciding_player_id(kLocalPlayer);
    rcr->set_choice_kind(ruled::v1::CHOICE_KIND_HAND_CARDS);
    rcr->set_min(2);
    rcr->set_max(2);
    rcr->add_candidate_object_ids(11);
    rcr->add_candidate_server_card_ids(1);
    apply(batch);

    state->toggleResolutionHandPickCard(1);
    host.sentCommands.clear();
    state->submitResolutionHandPick();
    EXPECT_TRUE(host.sentCommands.isEmpty());
    EXPECT_TRUE(state->isResolutionHandPickActive());
}

TEST_F(RuledClientTest, LibrarySearchChoiceEnforcesUniqueNamesAndOpensTheDeckView)
{
    QSignalSpy started(state, &RuledClientState::librarySearchPickStarted);
    ruled::v1::RuledEventBatch batch;
    auto *rcr = batch.add_events()->mutable_resolution_choice_required();
    rcr->set_deciding_player_id(kLocalPlayer);
    rcr->set_choice_kind(ruled::v1::CHOICE_KIND_LIBRARY_SEARCH); // Gifts Ungiven step 1
    rcr->set_min(4);
    rcr->set_max(4);
    rcr->set_unique_names(true);
    for (const quint32 oid : {11u, 12u, 13u}) {
        rcr->add_candidate_object_ids(oid);
    }
    for (const int scid : {1, 2, 3}) {
        rcr->add_candidate_server_card_ids(scid);
    }
    rcr->add_candidate_names("Forest");
    rcr->add_candidate_names("Forest");
    rcr->add_candidate_names("Island");
    apply(batch);

    ASSERT_TRUE(state->isResolutionHandPickActive());
    EXPECT_EQ(state->resolutionHandPickZone(), RuledClientState::PickZone::Deck);
    EXPECT_EQ(state->resolutionHandPickViewTitle(), QStringLiteral("Search your library"));
    EXPECT_TRUE(state->resolutionHandPickShowViewControls());
    ASSERT_EQ(started.count(), 1);
    EXPECT_EQ(started.at(0).at(0).toStringList(),
              QStringList({QStringLiteral("Forest"), QStringLiteral("Forest"), QStringLiteral("Island")}));

    state->toggleResolutionHandPickCard(1); // Forest
    // The second Forest is no longer selectable ("four cards with different names").
    EXPECT_FALSE(state->isResolutionHandPickCardSelectable(2));
    state->toggleResolutionHandPickCard(2);
    EXPECT_EQ(state->resolutionHandPickSelected(), 1);
    EXPECT_TRUE(state->isResolutionHandPickCardSelectable(3));
    state->toggleResolutionHandPickCard(3);
    EXPECT_EQ(state->resolutionHandPickSelected(), 2);
}

TEST_F(RuledClientTest, OrderedLibrarySearchSubmitsCardsInClickOrder)
{
    QSignalSpy started(state, &RuledClientState::librarySearchPickStarted);
    ruled::v1::RuledEventBatch batch;
    auto *rcr = batch.add_events()->mutable_resolution_choice_required();
    rcr->set_deciding_player_id(kLocalPlayer);
    rcr->set_choice_kind(ruled::v1::CHOICE_KIND_LIBRARY_SEARCH);
    rcr->set_prompt_text("Search your library for up to two basic lands. Choose them in order: the first enters the "
                         "battlefield tapped, and any remaining chosen card goes into your hand.");
    rcr->set_min(0);
    rcr->set_max(2);
    rcr->set_ordered(true);
    for (const quint32 oid : {81u, 82u}) {
        rcr->add_candidate_object_ids(oid);
    }
    for (const int scid : {0, 1}) {
        rcr->add_candidate_server_card_ids(scid);
    }
    rcr->add_candidate_names("Forest");
    rcr->add_candidate_names("Island");
    apply(batch);

    ASSERT_TRUE(state->isResolutionHandPickActive());
    EXPECT_EQ(state->resolutionHandPickZone(), RuledClientState::PickZone::Deck);
    EXPECT_EQ(state->resolutionHandPickViewTitle(), QStringLiteral("Search your library"));
    ASSERT_EQ(started.count(), 1);
    EXPECT_TRUE(state->resolutionHandPickPromptText().contains(QStringLiteral("first enters the battlefield tapped")));

    state->toggleResolutionHandPickCard(1);
    state->toggleResolutionHandPickCard(0);
    EXPECT_EQ(state->resolutionHandPickClickOrderFor(1), 1);
    EXPECT_EQ(state->resolutionHandPickClickOrderFor(0), 2);

    host.sentCommands.clear();
    state->submitResolutionHandPick();
    ASSERT_EQ(host.sentCommands.size(), 1);
    const auto &submitted = host.sentCommands.first().submit_resolution_choice();
    ASSERT_EQ(submitted.chosen_object_ids_size(), 2);
    EXPECT_EQ(submitted.chosen_object_ids(0), 82u);
    EXPECT_EQ(submitted.chosen_object_ids(1), 81u);
}

TEST_F(RuledClientTest, HeterogeneousLibrarySearchUsesTheEngineAuthoredDistinctSlotGraph)
{
    ruled::v1::RuledEventBatch batch;
    auto *rcr = batch.add_events()->mutable_resolution_choice_required();
    rcr->set_deciding_player_id(kLocalPlayer);
    rcr->set_choice_kind(ruled::v1::CHOICE_KIND_LIBRARY_SEARCH);
    rcr->set_prompt_text("Search for up to two cards.");
    rcr->set_min(0);
    rcr->set_max(2);
    for (const quint32 oid : {101u, 102u, 103u, 104u}) {
        rcr->add_candidate_object_ids(oid);
    }
    for (const int scid : {0, 1, 2, 3}) {
        rcr->add_candidate_server_card_ids(scid);
    }
    for (const char *name : {"Dryad Shrine", "Forest", "Sanctum", "Island"}) {
        rcr->add_candidate_names(name);
    }
    auto *basic = rcr->add_selection_slots();
    basic->set_label("a basic land card");
    for (const uint32_t index : {0u, 1u, 3u}) {
        basic->add_candidate_indices(index);
    }
    auto *shrine = rcr->add_selection_slots();
    shrine->set_label("a Shrine card");
    for (const uint32_t index : {0u, 2u}) {
        shrine->add_candidate_indices(index);
    }
    apply(batch);

    ASSERT_TRUE(state->isResolutionHandPickActive());
    EXPECT_TRUE(state->resolutionHandPickPromptText().contains(QStringLiteral("a basic land card")));
    EXPECT_TRUE(state->resolutionHandPickPromptText().contains(QStringLiteral("a Shrine card")));
    state->toggleResolutionHandPickCard(1);                     // Basic-only candidate occupies the first slot.
    EXPECT_FALSE(state->isResolutionHandPickCardSelectable(3)); // Another basic-only card has no slot.
    EXPECT_TRUE(state->isResolutionHandPickCardSelectable(0));  // Overlap can occupy the Shrine slot.
    EXPECT_TRUE(state->isResolutionHandPickCardSelectable(2));  // Shrine-only candidate is also valid.
    state->toggleResolutionHandPickCard(2);

    host.sentCommands.clear();
    state->submitResolutionHandPick();
    ASSERT_EQ(host.sentCommands.size(), 1);
    const auto &submitted = host.sentCommands.first().submit_resolution_choice();
    ASSERT_EQ(submitted.chosen_object_ids_size(), 2);
    EXPECT_EQ(submitted.chosen_object_ids(0), 102u);
    EXPECT_EQ(submitted.chosen_object_ids(1), 103u);
}

TEST_F(RuledClientTest, MalformedHeterogeneousLibrarySearchIsRejected)
{
    ruled::v1::RuledEventBatch batch;
    auto *rcr = batch.add_events()->mutable_resolution_choice_required();
    rcr->set_deciding_player_id(kLocalPlayer);
    rcr->set_choice_kind(ruled::v1::CHOICE_KIND_LIBRARY_SEARCH);
    rcr->set_max(1);
    rcr->add_candidate_object_ids(101u);
    rcr->add_candidate_server_card_ids(0);
    rcr->add_candidate_names("Forest");
    auto *slot = rcr->add_selection_slots();
    slot->set_label("a basic land card");
    slot->add_candidate_indices(1);
    apply(batch);

    EXPECT_FALSE(state->isResolutionHandPickActive());
}

// CR 701.18 scry reuses the library-search deck popup, retitled. The ordering step submits in
// click order, which is how the engine reads `ordered: true` (same convention as Brainstorm).
TEST_F(RuledClientTest, LibraryTopChoiceOpensTheScryViewAndSubmitsInClickOrder)
{
    QSignalSpy started(state, &RuledClientState::librarySearchPickStarted);
    ruled::v1::RuledEventBatch batch;
    auto *rcr = batch.add_events()->mutable_resolution_choice_required();
    rcr->set_deciding_player_id(kLocalPlayer);
    rcr->set_choice_kind(ruled::v1::CHOICE_KIND_LIBRARY_TOP); // Preordain, ordering step
    rcr->set_min(2);
    rcr->set_max(2);
    rcr->set_ordered(true);
    for (const quint32 oid : {21u, 22u}) {
        rcr->add_candidate_object_ids(oid);
    }
    for (const int scid : {0, 1}) {
        rcr->add_candidate_server_card_ids(scid);
    }
    rcr->add_candidate_names("Island");
    rcr->add_candidate_names("Island");
    apply(batch);

    ASSERT_TRUE(state->isResolutionHandPickActive());
    EXPECT_EQ(state->resolutionHandPickZone(), RuledClientState::PickZone::Deck);
    EXPECT_EQ(state->resolutionHandPickViewTitle(), QStringLiteral("Scry"));
    ASSERT_EQ(started.count(), 1);
    EXPECT_EQ(started.at(0).at(0).toStringList(), QStringList({QStringLiteral("Island"), QStringLiteral("Island")}));
    // Duplicate names must both stay pickable — scry never sets unique_names.
    EXPECT_TRUE(state->isResolutionHandPickCardSelectable(0));
    EXPECT_TRUE(state->isResolutionHandPickCardSelectable(1));

    state->toggleResolutionHandPickCard(1);
    state->toggleResolutionHandPickCard(0);
    EXPECT_EQ(state->resolutionHandPickSelected(), 2);

    host.sentCommands.clear();
    state->submitResolutionHandPick();
    ASSERT_EQ(host.sentCommands.size(), 1);
    const auto &sub = host.sentCommands[0].submit_resolution_choice();
    ASSERT_EQ(sub.chosen_object_ids_size(), 2);
    // Click order reaches the engine verbatim; it reads the list bottom-first, so the last card
    // clicked here (21) is the one that ends up on top.
    EXPECT_EQ(sub.chosen_object_ids(0), 22u);
    EXPECT_EQ(sub.chosen_object_ids(1), 21u);
    EXPECT_FALSE(state->isResolutionHandPickActive());
}

// A scry step that keeps every card on top submits nothing: min 0 must be answerable.
TEST_F(RuledClientTest, LibraryTopChoiceAllowsSubmittingAnEmptyBottomPile)
{
    ruled::v1::RuledEventBatch batch;
    auto *rcr = batch.add_events()->mutable_resolution_choice_required();
    rcr->set_deciding_player_id(kLocalPlayer);
    rcr->set_choice_kind(ruled::v1::CHOICE_KIND_LIBRARY_TOP); // Opt, scry 1
    rcr->set_min(0);
    rcr->set_max(1);
    rcr->add_candidate_object_ids(31u);
    rcr->add_candidate_server_card_ids(0);
    rcr->add_candidate_names("Island");
    apply(batch);

    ASSERT_TRUE(state->isResolutionHandPickActive());
    EXPECT_EQ(state->resolutionHandPickRequired(), 0);

    host.sentCommands.clear();
    state->submitResolutionHandPick();
    ASSERT_EQ(host.sentCommands.size(), 1);
    EXPECT_EQ(host.sentCommands[0].submit_resolution_choice().chosen_object_ids_size(), 0);
    EXPECT_FALSE(state->isResolutionHandPickActive());
}

TEST_F(RuledClientTest, EmptyLibrarySearchOpensABlankDeckViewAndSubmitsFailToFind)
{
    QSignalSpy started(state, &RuledClientState::librarySearchPickStarted);
    ruled::v1::RuledEventBatch batch;
    auto *rcr = batch.add_events()->mutable_resolution_choice_required();
    rcr->set_deciding_player_id(kLocalPlayer);
    rcr->set_choice_kind(ruled::v1::CHOICE_KIND_LIBRARY_SEARCH);
    rcr->set_prompt_text("Search your library for a Plains card.");
    rcr->set_min(0);
    rcr->set_max(1);
    apply(batch);

    ASSERT_TRUE(state->isResolutionHandPickActive());
    EXPECT_EQ(state->resolutionHandPickZone(), RuledClientState::PickZone::Deck);
    EXPECT_TRUE(state->resolutionHandPickCandidateNames().isEmpty());
    ASSERT_EQ(started.count(), 1);
    EXPECT_TRUE(started.at(0).at(0).toStringList().isEmpty());
    EXPECT_TRUE(started.at(0).at(1).value<QVector<int>>().isEmpty());

    host.sentCommands.clear();
    state->submitResolutionHandPick();
    ASSERT_EQ(host.sentCommands.size(), 1);
    EXPECT_EQ(host.sentCommands.first().submit_resolution_choice().chosen_object_ids_size(), 0);
    EXPECT_FALSE(state->isResolutionHandPickActive());
}

TEST_F(RuledClientTest, ManifestDreadChoiceUsesAControlFreeCardImageView)
{
    QSignalSpy started(state, &RuledClientState::librarySearchPickStarted);
    ruled::v1::RuledEventBatch batch;
    auto *rcr = batch.add_events()->mutable_resolution_choice_required();
    rcr->set_deciding_player_id(kLocalPlayer);
    rcr->set_choice_kind(ruled::v1::CHOICE_KIND_MANIFEST_DREAD);
    rcr->set_prompt_text("Choose one of the top two cards to manifest.");
    rcr->set_min(1);
    rcr->set_max(1);
    for (const quint32 oid : {35u, 36u}) {
        rcr->add_candidate_object_ids(oid);
    }
    for (const int scid : {0, 1}) {
        rcr->add_candidate_server_card_ids(scid);
    }
    rcr->add_candidate_names("Hill Giant");
    rcr->add_candidate_names("Forest");
    apply(batch);

    ASSERT_TRUE(state->isResolutionHandPickActive());
    EXPECT_EQ(state->resolutionHandPickZone(), RuledClientState::PickZone::Deck);
    EXPECT_EQ(state->resolutionHandPickViewTitle(), QStringLiteral("Manifest dread"));
    EXPECT_FALSE(state->resolutionHandPickShowViewControls());
    ASSERT_EQ(started.count(), 1);
    EXPECT_EQ(started.at(0).at(0).toStringList(),
              QStringList({QStringLiteral("Hill Giant"), QStringLiteral("Forest")}));
}

TEST_F(RuledClientTest, PopupLocalIdsCannotInheritBattlefieldDisplayState)
{
    ruled::v1::RuledEventBatch battlefield;
    addPermanent(battlefield.add_events(), kLocalPlayer, 10, 3);
    auto *view = battlefield.add_events()->mutable_zone_view()->add_per_player();
    view->set_player_id(kLocalPlayer);
    auto *planeswalker = view->add_battlefield_objects();
    planeswalker->set_object_id(10);
    planeswalker->set_is_planeswalker(true);
    planeswalker->set_loyalty(5);
    apply(battlefield);

    QObject physicalZone;
    QObject popupZone;
    markRuledChoiceLocalIds(popupZone);
    EXPECT_EQ(ruledPhysicalDisplayOid(*state, kLocalPlayer, 3, &physicalZone), 10u);
    EXPECT_EQ(state->loyaltyForPermanentOid(10), 5);
    EXPECT_EQ(ruledPhysicalDisplayOid(*state, kLocalPlayer, 3, &popupZone), 0u);

    ruled::v1::RuledEventBatch look;
    auto *choice = look.add_events()->mutable_resolution_choice_required();
    choice->set_deciding_player_id(kLocalPlayer);
    choice->set_choice_kind(ruled::v1::CHOICE_KIND_LIBRARY_LOOK);
    choice->set_min(0);
    choice->set_max(1);
    choice->add_candidate_object_ids(2);
    choice->add_candidate_server_card_ids(3);
    choice->add_candidate_names("Chandra, Novice Pyromancer");
    choice->add_candidate_selectable(true);
    apply(look);
    ASSERT_TRUE(state->isResolutionHandPickActive());
    state->toggleResolutionHandPickCard(3);
    EXPECT_EQ(state->resolutionHandPickClickOrderFor(3), 1);
    state->submitResolutionHandPick();
    ASSERT_EQ(host.sentCommands.size(), 1);
    ASSERT_EQ(host.sentCommands.last().submit_resolution_choice().chosen_object_ids_size(), 1);
    EXPECT_EQ(host.sentCommands.last().submit_resolution_choice().chosen_object_ids(0), 2u);
    state->clearPendingChoice();
    EXPECT_FALSE(state->isResolutionHandPickActive());
    EXPECT_EQ(ruledPhysicalDisplayOid(*state, kLocalPlayer, 3, &popupZone), 0u);
    EXPECT_EQ(ruledPhysicalDisplayOid(*state, kLocalPlayer, 3, &physicalZone), 10u);
    EXPECT_EQ(state->loyaltyForPermanentOid(10), 5);

    for (const char *marker : {"ruledReplacementPicker", "ruledRevealSnapshot"}) {
        QObject otherPopup;
        otherPopup.setProperty(marker, true);
        EXPECT_EQ(ruledPhysicalDisplayOid(*state, kLocalPlayer, 3, &otherPopup), 0u) << marker;
    }
}

TEST_F(RuledClientTest, PendingAbilityActivationIsAuthoritativePerBatchState)
{
    ruled::v1::RuledEventBatch begun;
    auto *pending = (*begun.mutable_legal_by_player())[kLocalPlayer].mutable_pending_ability_activation();
    pending->set_transaction_id(82u);
    pending->set_revision(3u);
    pending->set_actor_player_id(kLocalPlayer);
    pending->set_deciding_player_id(kLocalPlayer + 1);
    pending->set_stage(ruled::v1::ABILITY_ACTIVATION_STAGE_OPPONENT_TARGET);
    pending->set_source_object_id(700u);
    pending->add_announced_targets()->set_object_id(701u);
    apply(begun);
    ASSERT_TRUE(state->pendingAbilityActivation.has_value());
    EXPECT_EQ(state->pendingAbilityActivation->transaction_id(), 82u);
    EXPECT_EQ(state->pendingAbilityActivation->announced_targets(0).object_id(), 701u);
    ruled::v1::RuledEventBatch previewOnly;
    previewOnly.add_events()->mutable_attackers_preview();
    apply(previewOnly);
    ASSERT_TRUE(state->pendingAbilityActivation.has_value());
    ruled::v1::RuledEventBatch completed;
    (*completed.mutable_legal_by_player())[kLocalPlayer];
    apply(completed);
    EXPECT_FALSE(state->pendingAbilityActivation.has_value());
}

TEST_F(RuledClientTest, HandReturnTappedAbilityRemainsSelectableForManaWindow)
{
    ruled::v1::RuledEventBatch batch;
    auto &actions = (*batch.mutable_legal_by_player())[kLocalPlayer];
    auto *zoneAbility = actions.add_zone_ability_actions();
    zoneAbility->set_source_zone(ruled::v1::ABILITY_SOURCE_ZONE_HAND);
    zoneAbility->set_object_id(700u);
    zoneAbility->set_zone_change_generation(9u);
    zoneAbility->set_hand_index(2u);
    zoneAbility->set_ability_index(0u);
    auto *info = zoneAbility->mutable_ability();
    info->set_text("Activate this ability only as a sorcery.");
    info->set_activatable(true);
    const quint64 key = (static_cast<quint64>(700u) << 32) | 0u;
    auto &costChoices = (*actions.mutable_cost_choices_by_ability())[key];
    costChoices.set_non_mana_costs_payable(true);
    auto *returnChoice = costChoices.add_choices();
    returnChoice->set_cost_index(1u);
    returnChoice->set_zone(ruled::v1::COST_CHOICE_ZONE_BATTLEFIELD);
    returnChoice->set_kind(ruled::v1::COST_CHOICE_KIND_RETURN_TAPPED_CREATURE);
    returnChoice->set_min(1u);
    returnChoice->set_max(1u);
    apply(batch);

    EXPECT_EQ(state->abilitySourceZone(700u), ruled::v1::ABILITY_SOURCE_ZONE_HAND);
    EXPECT_TRUE(state->abilityCostChoices(700u, 0).first().candidateIds.isEmpty());
    EXPECT_TRUE(state->abilityActivatable(700u, 0));
}

TEST(RuledPaymentTest, StagedAbilityCommitPreviewAndPaymentKeepTransactionAndRevision)
{
    RuledPayment payment;
    payment.begin();
    ruled::v1::RuledCommand commit;
    commit.mutable_commit_ability_activation()->set_transaction_id(7002u);
    commit.mutable_commit_ability_activation()->set_expected_revision(4u);
    const auto request = payment.requestAction(commit);
    ASSERT_TRUE(request.has_commit_ability_activation());
    EXPECT_EQ(request.commit_ability_activation().transaction_id(), 7002u);
    EXPECT_EQ(request.commit_ability_activation().expected_revision(), 4u);
    EXPECT_TRUE(request.commit_ability_activation().has_payment());
    EXPECT_FALSE(request.has_activate_ability());
    payment.writePayment(commit);
    EXPECT_TRUE(commit.commit_ability_activation().has_payment());
    EXPECT_EQ(commit.commit_ability_activation().expected_revision(), 4u);
}

TEST_F(RuledClientTest, LibraryLookChoiceShowsEveryCardImageButOnlyMatchingCardsAreClickable)
{
    QSignalSpy started(state, &RuledClientState::librarySearchPickStarted);
    ruled::v1::RuledEventBatch batch;
    auto *rcr = batch.add_events()->mutable_resolution_choice_required();
    rcr->set_deciding_player_id(kLocalPlayer);
    rcr->set_choice_kind(ruled::v1::CHOICE_KIND_LIBRARY_LOOK);
    rcr->set_prompt_text("Look at the top five cards. Choose a creature card.");
    rcr->set_min(0);
    rcr->set_max(1);
    for (const quint32 oid : {41u, 42u, 43u, 44u, 45u}) {
        rcr->add_candidate_object_ids(oid);
    }
    for (const int scid : {0, 1, 2, 3, 4}) {
        rcr->add_candidate_server_card_ids(scid);
    }
    for (const char *name : {"Forest", "Grizzly Bears", "Island", "Hill Giant", "Mountain"}) {
        rcr->add_candidate_names(name);
    }
    for (const bool selectable : {false, true, false, true, false}) {
        rcr->add_candidate_selectable(selectable);
    }
    apply(batch);

    ASSERT_TRUE(state->isResolutionHandPickActive());
    EXPECT_EQ(state->resolutionHandPickZone(), RuledClientState::PickZone::Deck);
    EXPECT_EQ(state->resolutionHandPickViewTitle(), QStringLiteral("Look at cards"));
    EXPECT_FALSE(state->resolutionHandPickShowViewControls());
    ASSERT_EQ(started.count(), 1);
    EXPECT_EQ(started.at(0).at(0).toStringList(),
              QStringList({QStringLiteral("Forest"), QStringLiteral("Grizzly Bears"), QStringLiteral("Island"),
                           QStringLiteral("Hill Giant"), QStringLiteral("Mountain")}));
    EXPECT_FALSE(state->isResolutionHandPickCardSelectable(0));
    EXPECT_TRUE(state->isResolutionHandPickCardSelectable(1));
    EXPECT_FALSE(state->isResolutionHandPickCardSelectable(2));
    EXPECT_TRUE(state->isResolutionHandPickCardSelectable(3));

    state->toggleResolutionHandPickCard(0);
    EXPECT_EQ(state->resolutionHandPickSelected(), 0);
    state->toggleResolutionHandPickCard(3);
    EXPECT_EQ(state->resolutionHandPickSelected(), 1);
    EXPECT_EQ(state->resolutionHandPickClickOrderFor(3), 1);
}

TEST_F(RuledClientTest, LibraryLookNonlandImageIsUnselectableAndEmptyConfirmationWorks)
{
    QSignalSpy started(state, &RuledClientState::librarySearchPickStarted);
    ruled::v1::RuledEventBatch batch;
    auto *choice = batch.add_events()->mutable_resolution_choice_required();
    choice->set_deciding_player_id(kLocalPlayer);
    choice->set_choice_kind(ruled::v1::CHOICE_KIND_LIBRARY_LOOK);
    choice->set_min(0);
    choice->set_max(0);
    choice->set_prompt_text("Look at the top card. You may put it onto the battlefield if it is a land.");
    choice->add_candidate_object_ids(41);
    choice->add_candidate_server_card_ids(0);
    choice->add_candidate_names("Divination");
    choice->add_candidate_selectable(false);
    apply(batch);
    ASSERT_TRUE(state->isResolutionHandPickActive());
    ASSERT_EQ(started.count(), 1);
    EXPECT_EQ(started.at(0).at(0).toStringList(), QStringList({QStringLiteral("Divination")}));
    EXPECT_FALSE(state->isResolutionHandPickCardSelectable(0));
    state->toggleResolutionHandPickCard(0);
    EXPECT_EQ(state->resolutionHandPickSelected(), 0);
    host.sentCommands.clear();
    state->submitResolutionHandPick();
    ASSERT_EQ(host.sentCommands.size(), 1);
    EXPECT_EQ(host.sentCommands[0].submit_resolution_choice().chosen_object_ids_size(), 0);
    EXPECT_EQ(host.sentCommands[0].submit_resolution_choice().decision(),
              ruled::v1::RESOLUTION_CHOICE_DECISION_UNSPECIFIED);
}

TEST_F(RuledClientTest, MandatoryLibraryLookRequiresTwoDistinctImagesAndCannotDecline)
{
    ruled::v1::RuledEventBatch batch;
    auto *choice = batch.add_events()->mutable_resolution_choice_required();
    choice->set_deciding_player_id(kLocalPlayer);
    choice->set_choice_kind(ruled::v1::CHOICE_KIND_LIBRARY_LOOK);
    choice->set_min(2);
    choice->set_max(2);
    for (int i = 0; i < 3; ++i) {
        choice->add_candidate_object_ids(71u + i);
        choice->add_candidate_server_card_ids(i);
        choice->add_candidate_names("Forest");
        choice->add_candidate_selectable(true);
    }
    apply(batch);
    ASSERT_TRUE(state->isResolutionHandPickActive());
    EXPECT_EQ(state->resolutionHandPickRequired(), 2);
    EXPECT_FALSE(state->pendingClickChoiceMayDecline());
    host.sentCommands.clear();
    state->declinePendingClickChoice();
    state->submitResolutionHandPick();
    EXPECT_TRUE(host.sentCommands.empty());
    state->toggleResolutionHandPickCard(2);
    state->submitResolutionHandPick();
    EXPECT_TRUE(host.sentCommands.empty());
    state->toggleResolutionHandPickCard(0);
    state->toggleResolutionHandPickCard(1);
    EXPECT_EQ(state->resolutionHandPickSelected(), 2);
    state->submitResolutionHandPick();
    ASSERT_EQ(host.sentCommands.size(), 1);
    const auto &submission = host.sentCommands[0].submit_resolution_choice();
    ASSERT_EQ(submission.chosen_object_ids_size(), 2);
    EXPECT_EQ(submission.chosen_object_ids(0), 73u);
    EXPECT_EQ(submission.chosen_object_ids(1), 71u);
    EXPECT_FALSE(state->isResolutionHandPickActive());
}

TEST_F(RuledClientTest, PublicExploreLibraryLookIsInteractiveOnlyForItsController)
{
    QSignalSpy publicReveal(&state->reveals, &RuledRevealState::changed);
    QSignalSpy started(state, &RuledClientState::librarySearchPickStarted);
    ruled::v1::RuledEventBatch batch;
    auto *rcr = batch.add_events()->mutable_resolution_choice_required();
    rcr->set_deciding_player_id(kLocalPlayer);
    rcr->set_source_object_id(206);
    rcr->set_choice_kind(ruled::v1::CHOICE_KIND_LIBRARY_LOOK);
    rcr->set_prompt_text("Put Storm Crow into your graveyard?");
    auto *reveal = rcr->mutable_public_reveal();
    reveal->set_reveal_id("explore:206:cursor:0");
    reveal->set_source_object_id(206);
    reveal->set_source_zone(ruled::v1::CHOICE_CANDIDATE_SOURCE_ZONE_LIBRARY);
    reveal->set_zone_owner_player_id(kLocalPlayer);
    reveal->set_source_description("Explore");
    rcr->set_min(0);
    rcr->set_max(1);
    rcr->add_candidate_object_ids(41);
    rcr->add_candidate_card_ids("storm_crow");
    rcr->add_candidate_names("Storm Crow");
    rcr->add_candidate_server_card_ids(0);
    rcr->add_candidate_selectable(true);
    auto *revealedCard = reveal->add_cards();
    revealedCard->set_object_id(41);
    revealedCard->set_zone_change_generation(1);
    revealedCard->set_card_id("storm_crow");
    revealedCard->set_card_name("Storm Crow");
    apply(batch);

    ASSERT_TRUE(state->reveals.hasChoice());
    EXPECT_EQ(state->reveals.choice()->cardNames(), QStringList({QStringLiteral("Storm Crow")}));
    ASSERT_TRUE(state->isResolutionHandPickActive());
    EXPECT_EQ(state->resolutionHandPickZone(), RuledClientState::PickZone::Revealed);
    EXPECT_TRUE(state->isResolutionHandPickCardSelectable(0));
    EXPECT_EQ(publicReveal.count(), 1);
    EXPECT_EQ(started.count(), 0) << "A public reveal must use one shared choice window";

    ruled::v1::RuledEventBatch completed;
    completed.add_events()->mutable_log()->set_text("Explore completed.");
    apply(completed);
    EXPECT_FALSE(state->reveals.hasChoice());
    ASSERT_EQ(state->reveals.entries().size(), 1);
    EXPECT_EQ(state->reveals.entries().first().phase, RuledRevealState::Phase::Completed);
    EXPECT_TRUE(state->reveals.entries().first().choiceCardIds.isEmpty());

    ruled::v1::RuledEventBatch observer;
    auto *observed = observer.add_events()->mutable_resolution_choice_required();
    observed->CopyFrom(*rcr);
    observed->set_deciding_player_id(kOpponent);
    observed->clear_candidate_selectable();
    observed->set_prompt_text("Opponent is making a resolution choice.");
    apply(observer);
    EXPECT_FALSE(state->reveals.hasChoice());
    EXPECT_FALSE(state->isResolutionHandPickActive());
    EXPECT_EQ(started.count(), 0);
    EXPECT_EQ(state->reveals.entries().first().phase, RuledRevealState::Phase::Completed);
}

TEST_F(RuledClientTest, LibraryLookOrderingUsesImageClickOrderAndMalformedEligibilityFailsClosed)
{
    QSignalSpy started(state, &RuledClientState::librarySearchPickStarted);
    ruled::v1::RuledEventBatch malformed;
    auto *bad = malformed.add_events()->mutable_resolution_choice_required();
    bad->set_deciding_player_id(kLocalPlayer);
    bad->set_choice_kind(ruled::v1::CHOICE_KIND_LIBRARY_LOOK);
    for (const quint32 oid : {51u, 52u}) {
        bad->add_candidate_object_ids(oid);
    }
    for (const int scid : {0, 1}) {
        bad->add_candidate_server_card_ids(scid);
    }
    for (const char *name : {"Forest", "Island"}) {
        bad->add_candidate_names(name);
    }
    bad->add_candidate_selectable(true);
    apply(malformed);
    EXPECT_FALSE(state->isResolutionHandPickActive());
    EXPECT_EQ(started.count(), 0);
    EXPECT_EQ(host.dialogRequests, 0);

    ruled::v1::RuledEventBatch ordered;
    auto *rcr = ordered.add_events()->mutable_resolution_choice_required();
    rcr->set_deciding_player_id(kLocalPlayer);
    rcr->set_choice_kind(ruled::v1::CHOICE_KIND_LIBRARY_LOOK);
    rcr->set_min(2);
    rcr->set_max(2);
    rcr->set_ordered(true);
    for (const quint32 oid : {61u, 62u}) {
        rcr->add_candidate_object_ids(oid);
    }
    for (const int scid : {0, 1}) {
        rcr->add_candidate_server_card_ids(scid);
    }
    for (const char *name : {"Forest", "Island"}) {
        rcr->add_candidate_names(name);
        rcr->add_candidate_selectable(true);
    }
    apply(ordered);
    ASSERT_TRUE(state->isResolutionHandPickActive());
    EXPECT_EQ(state->resolutionHandPickViewTitle(), QStringLiteral("Look at cards"));
    EXPECT_EQ(started.count(), 1);

    state->toggleResolutionHandPickCard(1);
    state->toggleResolutionHandPickCard(0);
    EXPECT_EQ(state->resolutionHandPickClickOrderFor(1), 1);
    EXPECT_EQ(state->resolutionHandPickClickOrderFor(0), 2);
    host.sentCommands.clear();
    state->submitResolutionHandPick();
    ASSERT_EQ(host.sentCommands.size(), 1);
    const auto &submission = host.sentCommands[0].submit_resolution_choice();
    ASSERT_EQ(submission.chosen_object_ids_size(), 2);
    EXPECT_EQ(submission.chosen_object_ids(0), 62u);
    EXPECT_EQ(submission.chosen_object_ids(1), 61u);
}

TEST_F(RuledClientTest, RevealedChoiceAnnouncesAndClosesThePopup)
{
    QSignalSpy revealed(state, &RuledClientState::revealedPickChanged);
    ruled::v1::RuledEventBatch batch;
    auto *rcr = batch.add_events()->mutable_resolution_choice_required();
    rcr->set_deciding_player_id(kLocalPlayer);
    rcr->set_choice_kind(ruled::v1::CHOICE_KIND_REVEALED); // Gifts Ungiven step 2
    rcr->set_min(2);
    rcr->set_max(2);
    for (const quint32 oid : {11u, 12u}) {
        rcr->add_candidate_object_ids(oid);
    }
    for (const int scid : {1, 2}) {
        rcr->add_candidate_server_card_ids(scid);
    }
    rcr->add_candidate_names("Forest");
    rcr->add_candidate_names("Island");
    apply(batch);

    ASSERT_EQ(revealed.count(), 1);
    EXPECT_TRUE(revealed.at(0).at(0).toBool());
    EXPECT_EQ(state->resolutionHandPickZone(), RuledClientState::PickZone::Revealed);
    EXPECT_EQ(state->resolutionHandPickViewTitle(), QStringLiteral("Revealed cards"));

    state->toggleResolutionHandPickCard(1);
    state->toggleResolutionHandPickCard(2);
    state->submitResolutionHandPick();
    ASSERT_EQ(revealed.count(), 2);
    EXPECT_FALSE(revealed.at(1).at(0).toBool());
}

/// Thoughtseize/Coercion (CR 701.7): the popup renders like a revealed set but is a hand, and
/// must say so — it is built on the deck zone as a scaffold, which used to name the window.
TEST_F(RuledClientTest, OpponentHandChoiceRendersAsARevealedPickTitledAsAHand)
{
    QSignalSpy revealed(state, &RuledClientState::revealedPickChanged);
    ruled::v1::RuledEventBatch batch;
    auto *rcr = batch.add_events()->mutable_resolution_choice_required();
    rcr->set_deciding_player_id(kLocalPlayer);
    rcr->set_choice_kind(ruled::v1::CHOICE_KIND_OPPONENT_HAND);
    rcr->set_min(1);
    rcr->set_max(1);
    for (const quint32 oid : {21u, 22u}) {
        rcr->add_candidate_object_ids(oid);
    }
    for (const int scid : {0, 1}) {
        rcr->add_candidate_server_card_ids(scid);
    }
    rcr->add_candidate_names("Black Lotus");
    rcr->add_candidate_names("Swamp");
    rcr->add_candidate_selectable(true);
    rcr->add_candidate_selectable(false);
    apply(batch);

    ASSERT_EQ(revealed.count(), 1);
    EXPECT_TRUE(revealed.at(0).at(0).toBool());
    EXPECT_EQ(state->resolutionHandPickZone(), RuledClientState::PickZone::Revealed);
    EXPECT_EQ(state->resolutionHandPickViewTitle(), QStringLiteral("Target player's hand"));
    EXPECT_EQ(revealed.at(0).at(1).toStringList(),
              QStringList({QStringLiteral("Black Lotus"), QStringLiteral("Swamp")}));

    EXPECT_TRUE(state->isResolutionHandPickCardSelectable(0));
    EXPECT_FALSE(state->isResolutionHandPickCardSelectable(1));
    state->toggleResolutionHandPickCard(1);
    EXPECT_EQ(state->resolutionHandPickSelected(), 0);
    state->toggleResolutionHandPickCard(0);
    EXPECT_EQ(state->resolutionHandPickSelected(), 1);
}

TEST_F(RuledClientTest, PublicHandRevealIsReadOnlyForObserversAndReconcilesExactSnapshots)
{
    QSignalSpy publicReveal(&state->reveals, &RuledRevealState::changed);
    QSignalSpy timeline(state, &RuledClientState::engineTimeline);
    auto makeReveal = [](quint32 sourceOid, int ownerId, std::initializer_list<const char *> names) {
        ruled::v1::RuledEventBatch batch;
        auto *rcr = batch.add_events()->mutable_resolution_choice_required();
        rcr->set_deciding_player_id(kLocalPlayer + 1);
        rcr->set_source_object_id(sourceOid);
        rcr->set_choice_kind(ruled::v1::CHOICE_KIND_OPPONENT_HAND);
        rcr->mutable_public_reveal();
        rcr->mutable_public_reveal()->set_zone_owner_player_id(ownerId);
        int index = 0;
        for (const char *name : names) {
            rcr->add_candidate_object_ids(100 + index);
            rcr->add_candidate_card_ids(name);
            rcr->add_candidate_names(name);
            rcr->add_candidate_server_card_ids(index);
            ++index;
        }
        return batch;
    };

    apply(makeReveal(700, 2, {"Forest", "Grizzly Bears"}));
    EXPECT_FALSE(state->reveals.hasChoice());
    ASSERT_EQ(state->reveals.order().size(), 1);
    const auto firstReveal = state->reveals.entries().value(state->reveals.order().last());
    EXPECT_EQ(firstReveal.sourceObjectId, 700u);
    EXPECT_EQ(firstReveal.owner, 2);
    EXPECT_EQ(firstReveal.cardNames(), QStringList({QStringLiteral("Forest"), QStringLiteral("Grizzly Bears")}));
    EXPECT_EQ(firstReveal.phase, RuledRevealState::Phase::Completed);
    EXPECT_FALSE(state->isResolutionHandPickActive());
    ASSERT_EQ(publicReveal.count(), 1);
    ASSERT_EQ(timeline.count(), 1);
    EXPECT_EQ(timeline.at(0).at(0).toString(), QStringLiteral("P2 reveals: Forest, Grizzly Bears.\n"));

    // An unchanged authoritative repeat (for example after a rejected stale submission) refreshes
    // the popup but must not spam the game log.
    apply(makeReveal(700, 2, {"Forest", "Grizzly Bears"}));
    ASSERT_EQ(timeline.count(), 2);
    EXPECT_TRUE(timeline.at(1).at(0).toString().isEmpty());

    // A new reveal occurrence from the same source retains the previous snapshot.
    apply(makeReveal(700, 2, {"Island"}));
    EXPECT_FALSE(state->reveals.hasChoice());
    EXPECT_EQ(state->reveals.entries().value(state->reveals.order().last()).cardNames(),
              QStringList({QStringLiteral("Island")}));
    ASSERT_EQ(publicReveal.count(), 2);
    ASSERT_EQ(timeline.count(), 3);
    EXPECT_EQ(timeline.at(2).at(0).toString(), QStringLiteral("P2 reveals: Island.\n"));

    // A different source also retains earlier completed reveals.
    apply(makeReveal(701, 1, {"Swamp", "Thoughtseize"}));
    EXPECT_FALSE(state->reveals.hasChoice());
    const auto lastReveal = state->reveals.entries().value(state->reveals.order().last());
    EXPECT_EQ(lastReveal.sourceObjectId, 701u);
    EXPECT_EQ(lastReveal.owner, 1);
    EXPECT_EQ(lastReveal.cardNames(), QStringList({QStringLiteral("Swamp"), QStringLiteral("Thoughtseize")}));
    EXPECT_FALSE(state->isResolutionHandPickActive());
    ASSERT_EQ(timeline.count(), 4);
    EXPECT_EQ(timeline.at(3).at(0).toString(), QStringLiteral("P1 reveals: Swamp, Thoughtseize.\n"));

    ruled::v1::RuledEventBatch completed;
    completed.add_events()->mutable_log()->set_text("Choice completed.");
    apply(completed);
    EXPECT_FALSE(state->reveals.hasChoice());
    ASSERT_EQ(publicReveal.count(), 3)
        << "the repeated exact receipt is immutable and does not emit a second state change";
    EXPECT_EQ(state->reveals.entries().size(), 3);
}

TEST_F(RuledClientTest, WardAnnotationAndPaymentUseTheExistingRuledPromptModes)
{
    ruled::v1::RuledEventBatch manaWard;
    auto *pushed = manaWard.add_events()->mutable_stack_pushed();
    pushed->set_object_id(700u);
    pushed->set_description("Dirgur Island Dragon");
    pushed->set_ability_annotation("Ward {2}");
    pushed->set_is_triggered(true);
    auto *payment = manaWard.add_events()->mutable_resolution_choice_required();
    payment->set_deciding_player_id(kLocalPlayer);
    payment->set_choice_kind(ruled::v1::CHOICE_KIND_MANA_PAYMENT);
    payment->set_prompt_text("Ward {2}: Pay {2} or decline.");
    payment->set_generic_mana_cost(2);
    payment->set_payment_currently_legal(false);
    apply(manaWard);

    EXPECT_EQ(state->stackAnnotation(700u), QStringLiteral("Ward {2}"));
    EXPECT_TRUE(state->isResolutionPaymentActive());
    EXPECT_EQ(state->resolutionPaymentGenericCost(), 2);
    EXPECT_FALSE(state->resolutionPaymentCurrentlyLegal());

    ruled::v1::RuledEventBatch discardWardObserver;
    auto *discard = discardWardObserver.add_events()->mutable_resolution_choice_required();
    discard->set_deciding_player_id(kOpponent);
    discard->set_choice_kind(ruled::v1::CHOICE_KIND_HAND_CARDS);
    discard->set_prompt_text("Opponent is making a resolution choice.");
    apply(discardWardObserver);
    EXPECT_FALSE(state->isResolutionHandPickActive());
    EXPECT_TRUE(state->isWaitingForChoice());
    EXPECT_EQ(state->choiceWaitingPlayer(), kOpponent);
}

TEST_F(RuledClientTest, CounterEventRemovesOnlyTheExactSyntheticStackObject)
{
    ruled::v1::RuledEventBatch push;
    auto *first = push.add_events()->mutable_stack_pushed();
    first->set_object_id(900);
    first->set_description("Ward target ability");
    first->set_ability_annotation("Target ability");
    auto *second = push.add_events()->mutable_stack_pushed();
    second->set_object_id(901);
    second->set_description("Unrelated ability");
    second->set_ability_annotation("Unrelated ability");
    apply(push);
    ASSERT_EQ(state->getStackOidOrder().size(), 2);

    ruled::v1::RuledEventBatch counter;
    counter.add_events()->mutable_stack_object_countered()->set_object_id(900);
    apply(counter);

    EXPECT_EQ(state->getStackOidOrder(), QVector<quint32>({901}));
    EXPECT_EQ(host.removedSyntheticCards, QVector<quint32>({900}));
    EXPECT_EQ(state->stackAnnotation(901), QStringLiteral("Unrelated ability"));
}

TEST_F(RuledClientTest, PrivateOpponentHandLookDoesNotEnterThePublicGameLog)
{
    QSignalSpy timeline(state, &RuledClientState::engineTimeline);
    ruled::v1::RuledEventBatch batch;
    auto *rcr = batch.add_events()->mutable_resolution_choice_required();
    rcr->set_deciding_player_id(kLocalPlayer);
    rcr->set_choice_kind(ruled::v1::CHOICE_KIND_OPPONENT_HAND);
    rcr->add_candidate_object_ids(21);
    rcr->add_candidate_server_card_ids(0);
    rcr->add_candidate_names("Black Lotus");
    rcr->add_candidate_selectable(true);
    apply(batch);

    ASSERT_EQ(timeline.count(), 1);
    EXPECT_TRUE(timeline.at(0).at(0).toString().isEmpty());
}

TEST_F(RuledClientTest, PersistentPublicHandsReconcileEvenWhenBattlefieldsAreUnchanged)
{
    auto applyHands = [&](bool secondSnapshot, bool effectActive) {
        ruled::v1::RuledEventBatch batch;
        auto *zoneView = batch.add_events()->mutable_zone_view();
        zoneView->set_battlefields_unchanged(true);
        auto *owner = zoneView->add_per_player();
        owner->set_player_id(kLocalPlayer + 1);
        if (effectActive) {
            auto *publicHand = owner->mutable_public_hand();
            auto *card = publicHand->add_cards();
            card->set_object_id(700u);
            card->set_zone_change_generation(2u);
            card->set_card_id(secondSnapshot ? "island" : "forest");
            card->set_card_name(secondSnapshot ? "Island" : "Forest");
            if (!secondSnapshot) {
                card = publicHand->add_cards();
                card->set_object_id(701u);
                card->set_zone_change_generation(1u);
                card->set_card_id("grizzly_bears");
                card->set_card_name("Grizzly Bears");
            }
        }
        auto *emptyOwner = zoneView->add_per_player();
        emptyOwner->set_player_id(kLocalPlayer + 2);
        if (effectActive)
            emptyOwner->mutable_public_hand();
        apply(batch);
    };

    applyHands(false, true);
    ASSERT_TRUE(state->reveals.publicHands().contains(kLocalPlayer + 1));
    ASSERT_EQ(state->reveals.publicHands().value(kLocalPlayer + 1).size(), 2);
    EXPECT_EQ(state->reveals.publicHands().value(kLocalPlayer + 1).first().name, QStringLiteral("Forest"));
    ASSERT_TRUE(state->reveals.publicHands().contains(kLocalPlayer + 2));
    EXPECT_TRUE(state->reveals.publicHands().value(kLocalPlayer + 2).isEmpty());

    applyHands(true, true);
    ASSERT_EQ(state->reveals.publicHands().value(kLocalPlayer + 1).size(), 1);
    EXPECT_EQ(state->reveals.publicHands().value(kLocalPlayer + 1).first().cardId, QStringLiteral("island"));
    EXPECT_TRUE(state->reveals.publicHands().contains(kLocalPlayer + 2));

    applyHands(false, false);
    EXPECT_TRUE(state->reveals.publicHands().isEmpty()) << "absence clears the live hand after the effect ends";
}

TEST_F(RuledClientTest, PublicHandRevealIsInteractiveOnlyForDeciderAndIgnoresPreviewBatches)
{
    QSignalSpy publicReveal(&state->reveals, &RuledRevealState::changed);
    ruled::v1::RuledEventBatch batch;
    auto *rcr = batch.add_events()->mutable_resolution_choice_required();
    rcr->set_deciding_player_id(kLocalPlayer);
    rcr->set_source_object_id(800);
    rcr->set_choice_kind(ruled::v1::CHOICE_KIND_OPPONENT_HAND);
    rcr->mutable_public_reveal();
    rcr->mutable_public_reveal()->set_zone_owner_player_id(kLocalPlayer + 1);
    for (int i = 0; i < 2; ++i) {
        rcr->add_candidate_object_ids(200 + i);
        rcr->add_candidate_card_ids(i == 0 ? "forest" : "grizzly_bears");
        rcr->add_candidate_names(i == 0 ? "Forest" : "Grizzly Bears");
        rcr->add_candidate_server_card_ids(i);
    }
    rcr->add_candidate_selectable(false);
    rcr->add_candidate_selectable(true);
    rcr->set_min(1);
    rcr->set_max(1);
    apply(batch);

    ASSERT_TRUE(state->reveals.hasChoice());
    ASSERT_TRUE(state->isResolutionHandPickActive());
    EXPECT_FALSE(state->isResolutionHandPickCardSelectable(0));
    EXPECT_TRUE(state->isResolutionHandPickCardSelectable(1));

    ruled::v1::RuledEventBatch preview;
    preview.add_events()->mutable_attackers_preview()->set_declaring_player_id(kLocalPlayer);
    apply(preview);
    EXPECT_TRUE(state->reveals.hasChoice());
    EXPECT_EQ(publicReveal.count(), 1);
}

TEST(RuledPickSurfaceTest, PublicRevealAcceptsNonlocalHandViewButOtherPicksRemainLocal)
{
    EXPECT_TRUE(isRuledPickSurfaceCard(RuledPickZone::Revealed, RuledPickScaffoldZone::Hand, true, false));
    EXPECT_TRUE(isRuledPickSurfaceCard(RuledPickZone::Revealed, RuledPickScaffoldZone::Deck, true, false));
    EXPECT_FALSE(isRuledPickSurfaceCard(RuledPickZone::Revealed, RuledPickScaffoldZone::Hand, false, false));

    EXPECT_FALSE(isRuledPickSurfaceCard(RuledPickZone::Hand, RuledPickScaffoldZone::Hand, false, false));
    EXPECT_TRUE(isRuledPickSurfaceCard(RuledPickZone::Hand, RuledPickScaffoldZone::Hand, false, true));
    EXPECT_FALSE(isRuledPickSurfaceCard(RuledPickZone::Deck, RuledPickScaffoldZone::Deck, true, false));
    EXPECT_TRUE(isRuledPickSurfaceCard(RuledPickZone::Deck, RuledPickScaffoldZone::Deck, true, true));
}

TEST_F(RuledClientTest, MalformedPublicHandRevealFailsClosed)
{
    QSignalSpy publicReveal(&state->reveals, &RuledRevealState::changed);
    ruled::v1::RuledEventBatch batch;
    auto *rcr = batch.add_events()->mutable_resolution_choice_required();
    rcr->set_deciding_player_id(kLocalPlayer + 1);
    rcr->set_choice_kind(ruled::v1::CHOICE_KIND_OPPONENT_HAND);
    rcr->mutable_public_reveal();
    rcr->mutable_public_reveal()->set_zone_owner_player_id(2);
    rcr->add_candidate_object_ids(100);
    rcr->add_candidate_card_ids("forest");
    rcr->add_candidate_names("Forest");
    // Missing the parallel transient popup id.
    apply(batch);

    EXPECT_FALSE(state->reveals.hasChoice());
    EXPECT_FALSE(state->isResolutionHandPickActive());
    EXPECT_EQ(publicReveal.count(), 1)
        << "a shape-valid immutable receipt remains displayable even when candidate bindings are malformed";
    EXPECT_EQ(host.dialogRequests, 0);
}

TEST_F(RuledClientTest, InstantRevealOccurrencesRetainSnapshotsAndDismissIndependently)
{
    ruled::v1::RuledEventBatch batch;
    auto *reveal = batch.add_events()->mutable_cards_revealed();
    reveal->set_reveal_id("event:20:0");
    reveal->set_source_description("Explore");
    reveal->set_source_zone(ruled::v1::CHOICE_CANDIDATE_SOURCE_ZONE_LIBRARY);
    reveal->set_zone_owner_player_id(kLocalPlayer);
    auto *card = reveal->add_cards();
    card->set_object_id(44);
    card->set_zone_change_generation(3);
    card->set_card_id("forest");
    card->set_card_name("Forest");
    apply(batch);
    apply(batch);
    ASSERT_EQ(state->reveals.entries().size(), 1);
    EXPECT_EQ(state->reveals.entries().first().cards.first().generation, 3);
    EXPECT_TRUE(state->reveals.entries().first().choiceCardIds.isEmpty());
    state->reveals.dismiss("event:20:0");
    apply(batch);
    EXPECT_TRUE(state->reveals.entries().isEmpty()) << "a repeated batch must not undo local dismissal";
    reveal->set_reveal_id("event:21:0");
    apply(batch);
    ASSERT_EQ(state->reveals.entries().size(), 1) << "revealing the same card again is a new occurrence";
    EXPECT_EQ(state->reveals.entries().first().cardNames(), QStringList({"Forest"}));
}

TEST_F(RuledClientTest, PublicChoiceUsesSharedSurfaceRegardlessOfChoiceKind)
{
    ruled::v1::RuledEventBatch batch;
    auto *choice = batch.add_events()->mutable_resolution_choice_required();
    choice->set_deciding_player_id(kLocalPlayer);
    choice->set_source_object_id(801);
    choice->set_choice_kind(ruled::v1::CHOICE_KIND_HAND_CARDS);
    choice->set_min(1);
    choice->set_max(1);
    choice->add_candidate_object_ids(44);
    choice->add_candidate_card_ids("forest");
    choice->add_candidate_names("Forest");
    choice->add_candidate_server_card_ids(3);
    choice->mutable_public_reveal()->set_zone_owner_player_id(kLocalPlayer);
    apply(batch);
    ASSERT_TRUE(state->isResolutionHandPickActive());
    EXPECT_EQ(state->resolutionHandPickZone(), RuledClientState::PickZone::Revealed);
    EXPECT_TRUE(state->pendingChoice->publicReveal);
}

TEST_F(RuledClientTest, ActiveSnapshotAfterResolutionOfferKeepsRevealChoiceInteractive)
{
    host.local = 2;
    ruled::v1::RuledEventBatch batch;
    auto *reveal = batch.add_events()->mutable_cards_revealed();
    reveal->set_reveal_id("gifts:accepted-command:stack:77:cursor:0");
    reveal->set_source_object_id(77);
    reveal->set_source_zone(ruled::v1::CHOICE_CANDIDATE_SOURCE_ZONE_LIBRARY);
    reveal->set_zone_owner_player_id(1);
    reveal->set_source_description("Gifts Ungiven");
    const char *names[] = {"Forest", "Island", "Plains", "Swamp"};
    for (int i = 0; i < 4; ++i) {
        auto *card = reveal->add_cards();
        card->set_object_id(500u + static_cast<quint32>(i));
        card->set_zone_change_generation(3u);
        card->set_card_id(QString::fromLatin1(names[i]).toLower().toStdString());
        card->set_card_name(names[i]);
    }
    const ruled::v1::CardsRevealed receipt = *reveal;

    auto *choice = batch.add_events()->mutable_resolution_choice_required();
    choice->set_deciding_player_id(host.localPlayerId());
    choice->set_source_object_id(77);
    choice->set_choice_kind(ruled::v1::CHOICE_KIND_HAND_CARDS);
    choice->set_prompt_text("Choose two cards to put into the graveyard.");
    choice->set_min(2);
    choice->set_max(2);
    *choice->mutable_public_reveal() = receipt;
    for (int i = 0; i < 4; ++i) {
        choice->add_candidate_object_ids(receipt.cards(i).object_id());
        choice->add_candidate_card_ids(receipt.cards(i).card_id());
        choice->add_candidate_names(receipt.cards(i).card_name());
        choice->add_candidate_server_card_ids(70 + i);
    }

    // The engine appends this authoritative active set after the offer in the same batch.
    *batch.add_events()->mutable_active_public_reveal_snapshot()->add_reveals() = receipt;
    (*batch.mutable_legal_by_player())[host.localPlayerId()].set_undoable_mana_abilities(0);
    apply(batch);

    ASSERT_TRUE(state->reveals.hasChoice());
    const auto *activeChoice = state->reveals.choice();
    ASSERT_NE(activeChoice, nullptr);
    EXPECT_EQ(activeChoice->id, QString::fromStdString(receipt.reveal_id()));
    EXPECT_EQ(activeChoice->phase, RuledRevealState::Phase::Choice);
    EXPECT_EQ(activeChoice->choiceCardIds, QVector<int>({70, 71, 72, 73}));
    EXPECT_EQ(activeChoice->cardNames(), QStringList({QStringLiteral("Forest"), QStringLiteral("Island"),
                                                      QStringLiteral("Plains"), QStringLiteral("Swamp")}));
    ASSERT_TRUE(state->isResolutionHandPickActive());
    EXPECT_EQ(state->resolutionHandPickZone(), RuledClientState::PickZone::Revealed);
    EXPECT_EQ(state->resolutionHandPickCandidateNames(), QStringList({"Forest", "Island", "Plains", "Swamp"}));
}

TEST_F(RuledClientTest, PublicRevealObserverKeepsReadOnlyActiveReceipt)
{
    host.local = 2;
    ruled::v1::RuledEventBatch batch;
    auto *reveal = batch.add_events()->mutable_cards_revealed();
    reveal->set_reveal_id("gifts:observer:stack:77:cursor:0");
    reveal->set_source_object_id(77);
    reveal->set_source_zone(ruled::v1::CHOICE_CANDIDATE_SOURCE_ZONE_LIBRARY);
    reveal->set_zone_owner_player_id(1);
    reveal->set_source_description("Gifts Ungiven");
    const char *names[] = {"Forest", "Island", "Plains", "Swamp"};
    for (int i = 0; i < 4; ++i) {
        auto *card = reveal->add_cards();
        card->set_object_id(600u + static_cast<quint32>(i));
        card->set_zone_change_generation(4u);
        card->set_card_id(QString::fromLatin1(names[i]).toLower().toStdString());
        card->set_card_name(names[i]);
    }
    const ruled::v1::CardsRevealed receipt = *reveal;

    auto *choice = batch.add_events()->mutable_resolution_choice_required();
    choice->set_deciding_player_id(1);
    choice->set_source_object_id(77);
    choice->set_choice_kind(ruled::v1::CHOICE_KIND_REVEALED);
    choice->set_min(2);
    choice->set_max(2);
    *choice->mutable_public_reveal() = receipt;
    for (const auto &card : receipt.cards()) {
        choice->add_candidate_object_ids(card.object_id());
        choice->add_candidate_card_ids(card.card_id());
        choice->add_candidate_names(card.card_name());
        choice->add_candidate_server_card_ids(80 + static_cast<int>(card.object_id() - 600u));
    }

    *batch.add_events()->mutable_active_public_reveal_snapshot()->add_reveals() = receipt;
    (*batch.mutable_legal_by_player())[host.localPlayerId()].set_undoable_mana_abilities(0);
    apply(batch);

    EXPECT_FALSE(state->reveals.hasChoice());
    EXPECT_FALSE(state->pendingChoice.has_value());
    EXPECT_FALSE(state->isResolutionHandPickActive());
    const auto entry = state->reveals.entries().value(QString::fromStdString(receipt.reveal_id()));
    EXPECT_EQ(entry.phase, RuledRevealState::Phase::Active);
    EXPECT_TRUE(entry.choiceCardIds.isEmpty());
    EXPECT_EQ(entry.cardNames(), QStringList({"Forest", "Island", "Plains", "Swamp"}));
}

TEST_F(RuledClientTest, PublicRevealCandidateSubsetMapsByIdentityAndPopupSelection)
{
    ruled::v1::RuledEventBatch batch;
    auto *reveal = batch.add_events()->mutable_cards_revealed();
    reveal->set_reveal_id("gifts:subset:stack:78:cursor:0");
    reveal->set_source_object_id(78);
    reveal->set_source_zone(ruled::v1::CHOICE_CANDIDATE_SOURCE_ZONE_LIBRARY);
    reveal->set_zone_owner_player_id(kLocalPlayer);
    reveal->set_source_description("Gifts Ungiven");
    const char *names[] = {"Forest", "Island", "Plains", "Swamp"};
    for (int i = 0; i < 4; ++i) {
        auto *card = reveal->add_cards();
        card->set_object_id(700u + static_cast<quint32>(i));
        card->set_zone_change_generation(5u);
        card->set_card_id(QString::fromLatin1(names[i]).toLower().toStdString());
        card->set_card_name(names[i]);
    }
    const ruled::v1::CardsRevealed receipt = *reveal;

    auto *choice = batch.add_events()->mutable_resolution_choice_required();
    choice->set_deciding_player_id(host.localPlayerId());
    choice->set_source_object_id(78);
    choice->set_choice_kind(ruled::v1::CHOICE_KIND_REVEALED);
    choice->set_prompt_text("Choose one of the revealed cards.");
    choice->set_min(1);
    choice->set_max(1);
    choice->set_unique_names(false);
    *choice->mutable_public_reveal() = receipt;
    // Candidate order deliberately differs from both reveal and displayed zone order.
    for (const int revealIndex : {2, 0}) {
        const auto &card = receipt.cards(revealIndex);
        choice->add_candidate_object_ids(card.object_id());
        choice->add_candidate_card_ids(card.card_id());
        choice->add_candidate_names(card.card_name());
    }
    choice->add_candidate_server_card_ids(27); // Plains (reveal index 2).
    choice->add_candidate_server_card_ids(29); // Forest (reveal index 0).
    *batch.add_events()->mutable_active_public_reveal_snapshot()->add_reveals() = receipt;
    (*batch.mutable_legal_by_player())[host.localPlayerId()].set_undoable_mana_abilities(0);
    apply(batch);

    ASSERT_TRUE(state->reveals.hasChoice());
    const auto *entry = state->reveals.choice();
    ASSERT_NE(entry, nullptr);
    EXPECT_TRUE(entry->hasInteractiveChoice());
    EXPECT_EQ(entry->choiceCardIds, QVector<int>({29, -1, 27, -1}));
    EXPECT_EQ(entry->choiceCardIdForIndex(0), 29);
    EXPECT_EQ(entry->choiceCardIdForIndex(1), -1);
    EXPECT_EQ(entry->choiceCardIdForIndex(2), 27);
    EXPECT_EQ(entry->choiceCardIdForIndex(3), -1);
    ASSERT_TRUE(state->isResolutionHandPickActive());
    EXPECT_EQ(state->resolutionHandPickCandidateNames(), QStringList({"Plains", "Forest"}));

    // The UI submits the transient popup identity; it must resolve to the matching revealed OID,
    // even when the engine candidate list used a different order from the reveal.
    state->toggleResolutionHandPickCard(27);
    state->submitResolutionHandPick();
    ASSERT_EQ(host.sentCommands.size(), 1);
    const auto &submission = host.sentCommands.last().submit_resolution_choice();
    ASSERT_EQ(submission.chosen_object_ids_size(), 1);
    EXPECT_EQ(submission.chosen_object_ids(0), 702u);
}

TEST_F(RuledClientTest, PublicRevealLifecycleRetiresOmittedAndConflictingOccurrences)
{
    ruled::v1::CardsRevealed receipt;
    receipt.set_reveal_id("gifts:lifecycle:stack:79:cursor:0");
    receipt.set_source_object_id(79);
    receipt.set_source_zone(ruled::v1::CHOICE_CANDIDATE_SOURCE_ZONE_LIBRARY);
    receipt.set_zone_owner_player_id(kLocalPlayer);
    receipt.set_source_description("Gifts Ungiven");
    auto *card = receipt.add_cards();
    card->set_object_id(900);
    card->set_zone_change_generation(6);
    card->set_card_id("forest");
    card->set_card_name("Forest");
    const QString id = QString::fromStdString(receipt.reveal_id());
    const QVector<int> popupIds{41};

    ASSERT_TRUE(state->reveals.publish(receipt));
    ruled::v1::ActivePublicRevealSnapshot active;
    *active.add_reveals() = receipt;
    state->reveals.applyActiveSnapshot(active);
    ASSERT_EQ(state->reveals.entries().value(id).phase, RuledRevealState::Phase::Active);

    ASSERT_TRUE(state->reveals.beginChoice(receipt, popupIds));
    state->reveals.applyActiveSnapshot(active);
    ASSERT_EQ(state->reveals.entries().value(id).phase, RuledRevealState::Phase::Choice);
    EXPECT_EQ(state->reveals.entries().value(id).choiceCardIds, popupIds);

    state->reveals.applyActiveSnapshot(ruled::v1::ActivePublicRevealSnapshot{});
    EXPECT_FALSE(state->reveals.hasChoice());
    EXPECT_EQ(state->reveals.entries().value(id).phase, RuledRevealState::Phase::Completed);
    state->reveals.applyActiveSnapshot(active);
    EXPECT_FALSE(state->reveals.hasChoice()) << "a stale snapshot cannot resurrect an omitted occurrence";
    EXPECT_EQ(state->reveals.entries().value(id).phase, RuledRevealState::Phase::Completed);

    ASSERT_TRUE(state->reveals.beginChoice(receipt, popupIds))
        << "a fresh validated offer may re-associate the same immutable receipt";
    state->reveals.applyActiveSnapshot(active);
    ASSERT_TRUE(state->reveals.hasChoice());
    EXPECT_EQ(state->reveals.entries().value(id).phase, RuledRevealState::Phase::Choice);

    ruled::v1::ActivePublicRevealSnapshot conflicting;
    auto *altered = conflicting.add_reveals();
    *altered = receipt;
    altered->mutable_cards(0)->set_card_name("Island");
    state->reveals.applyActiveSnapshot(conflicting);
    EXPECT_FALSE(state->reveals.hasChoice());
    ASSERT_EQ(state->reveals.entries().value(id).cards.size(), 1);
    EXPECT_EQ(state->reveals.entries().value(id).cards.first().name, QStringLiteral("Forest"));
    EXPECT_EQ(state->reveals.entries().value(id).phase, RuledRevealState::Phase::Completed);
    state->reveals.applyActiveSnapshot(active);
    EXPECT_EQ(state->reveals.entries().value(id).cards.first().name, QStringLiteral("Forest"));
    EXPECT_EQ(state->reveals.entries().value(id).phase, RuledRevealState::Phase::Completed);
}

TEST_F(RuledClientTest, PublicRevealReceiptOnResolutionBranchStillStagesTheBranch)
{
    ruled::v1::RuledEventBatch batch;
    auto *reveal = batch.add_events()->mutable_cards_revealed();
    reveal->set_reveal_id("gifts:commander-branch:stack:80:cursor:0");
    reveal->set_source_object_id(80);
    reveal->set_source_zone(ruled::v1::CHOICE_CANDIDATE_SOURCE_ZONE_LIBRARY);
    reveal->set_zone_owner_player_id(kLocalPlayer);
    auto *card = reveal->add_cards();
    card->set_object_id(950);
    card->set_zone_change_generation(7);
    card->set_card_id("sol_ring");
    card->set_card_name("Sol Ring");
    const ruled::v1::CardsRevealed receipt = *reveal;

    auto *choice = batch.add_events()->mutable_resolution_choice_required();
    choice->set_deciding_player_id(host.localPlayerId());
    choice->set_source_object_id(80);
    choice->set_choice_kind(ruled::v1::CHOICE_KIND_RESOLUTION_BRANCH);
    choice->set_prompt_text("Put the revealed commander into the command zone?");
    choice->set_min(1);
    choice->set_max(1);
    *choice->mutable_public_reveal() = receipt;
    auto *command = choice->add_resolution_branches();
    command->set_branch_index(0);
    command->set_label("Command zone");
    command->set_selectable(true);
    auto *hand = choice->add_resolution_branches();
    hand->set_branch_index(1);
    hand->set_label("Hand");
    hand->set_selectable(true);
    *batch.add_events()->mutable_active_public_reveal_snapshot()->add_reveals() = receipt;
    (*batch.mutable_legal_by_player())[host.localPlayerId()].set_undoable_mana_abilities(0);
    apply(batch);

    ASSERT_TRUE(state->pendingChoice.has_value());
    EXPECT_EQ(state->pendingChoice->kind, RuledClientState::ChoiceKind::ResolutionBranch);
    ASSERT_EQ(state->pendingChoice->choiceOptions.size(), 2);
    EXPECT_FALSE(state->reveals.hasChoice());
    EXPECT_EQ(state->reveals.entries().value(QString::fromStdString(receipt.reveal_id())).phase,
              RuledRevealState::Phase::Active);
}

TEST_F(RuledClientTest, PublicRevealReceiptOn4043OrderUsesOrderedImagePickerForItsSubset)
{
    ruled::v1::RuledEventBatch batch;
    auto *reveal = batch.add_events()->mutable_cards_revealed();
    reveal->set_reveal_id("gifts:4043-order:stack:81:cursor:0");
    reveal->set_source_object_id(81);
    reveal->set_source_zone(ruled::v1::CHOICE_CANDIDATE_SOURCE_ZONE_LIBRARY);
    reveal->set_zone_owner_player_id(1);
    reveal->set_source_description("Gifts Ungiven");
    const char *names[] = {"Forest", "Island", "Plains", "Swamp"};
    for (int i = 0; i < 4; ++i) {
        auto *card = reveal->add_cards();
        card->set_object_id(800u + static_cast<quint32>(i));
        card->set_zone_change_generation(8u);
        card->set_card_id(QString::fromLatin1(names[i]).toLower().toStdString());
        card->set_card_name(names[i]);
    }
    const ruled::v1::CardsRevealed receipt = *reveal;

    auto *choice = batch.add_events()->mutable_resolution_choice_required();
    choice->set_deciding_player_id(host.localPlayerId());
    choice->set_source_object_id(81);
    choice->set_choice_kind(ruled::v1::CHOICE_KIND_SIMULTANEOUS_ENTRY_ORDER);
    const QString graveyardOrderPrompt =
        QStringLiteral("P%1: choose the order for the cards put into your graveyard. "
                       "The last card you click will be on top.")
            .arg(host.localPlayerId());
    choice->set_prompt_text(graveyardOrderPrompt.toStdString());
    choice->set_min(2);
    choice->set_max(2);
    choice->set_ordered(true);
    *choice->mutable_public_reveal() = receipt;
    // CR 404.3 orders only the two-card graveyard side; it is a strict subset of the four-card
    // public Gifts receipt and must use the existing image-order surface.
    for (const int revealIndex : {3, 1}) {
        const auto &card = receipt.cards(revealIndex);
        choice->add_candidate_object_ids(card.object_id());
        choice->add_candidate_card_ids(card.card_id());
        choice->add_candidate_names(card.card_name());
    }
    choice->add_candidate_server_card_ids(0);
    choice->add_candidate_server_card_ids(1);
    *batch.add_events()->mutable_active_public_reveal_snapshot()->add_reveals() = receipt;
    (*batch.mutable_legal_by_player())[host.localPlayerId()].set_undoable_mana_abilities(0);
    apply(batch);

    ASSERT_TRUE(state->pendingChoice.has_value());
    EXPECT_EQ(state->pendingChoice->kind, RuledClientState::ChoiceKind::ResolutionPick);
    EXPECT_EQ(state->pendingChoice->viewTitle, QStringLiteral("Order cards"));
    EXPECT_EQ(state->resolutionHandPickPromptText(), graveyardOrderPrompt);
    EXPECT_EQ(state->pendingChoice->pickZone, RuledClientState::PickZone::Deck);
    EXPECT_EQ(state->pendingChoice->candidateNames, QStringList({"Swamp", "Island"}));
    EXPECT_EQ(state->pendingChoice->reverseSelectionOrder, false);
    EXPECT_EQ(state->pendingChoice->showViewControls, false);
    EXPECT_EQ(state->pendingChoice->serverCardIdToOid.value(0), 803u);
    EXPECT_EQ(state->pendingChoice->serverCardIdToOid.value(1), 801u);
    EXPECT_FALSE(state->reveals.hasChoice());
    EXPECT_EQ(state->reveals.entries().value(QString::fromStdString(receipt.reveal_id())).phase,
              RuledRevealState::Phase::Active);
}

TEST_F(RuledClientTest, ZurPaymentBranchUsesSeparateActiveLibraryReveal)
{
    ruled::v1::RuledEventBatch batch;
    auto *reveal = batch.add_events()->mutable_cards_revealed();
    reveal->set_reveal_id("draw:41:801:2");
    reveal->set_source_object_id(801);
    reveal->set_source_zone(ruled::v1::CHOICE_CANDIDATE_SOURCE_ZONE_LIBRARY);
    reveal->set_zone_owner_player_id(kLocalPlayer + 1);
    auto *card = reveal->add_cards();
    card->set_object_id(44);
    card->set_zone_change_generation(3);
    card->set_card_id("island");
    card->set_card_name("Island");

    auto *choice = batch.add_events()->mutable_resolution_choice_required();
    choice->set_deciding_player_id(kLocalPlayer);
    choice->set_source_object_id(801);
    choice->set_choice_kind(ruled::v1::CHOICE_KIND_RESOLUTION_BRANCH);
    choice->set_prompt_text("Pay 2 life to put the revealed card into its owner's graveyard?");
    choice->set_min(1);
    choice->set_max(1);
    auto *pay = choice->add_resolution_branches();
    pay->set_branch_index(0);
    pay->set_label("Pay 2 life");
    pay->set_selectable(true);
    auto *decline = choice->add_resolution_branches();
    decline->set_branch_index(1);
    decline->set_label("Decline");
    decline->set_selectable(true);

    auto *active = batch.add_events()->mutable_active_public_reveal_snapshot();
    *active->add_reveals() = *reveal;
    (*batch.mutable_legal_by_player())[kLocalPlayer].set_undoable_mana_abilities(0);
    apply(batch);

    ASSERT_TRUE(state->hasPendingChoiceOfKind(RuledClientState::ChoiceKind::ResolutionBranch));
    ASSERT_EQ(state->pendingChoice->choiceOptions.size(), 2);
    EXPECT_EQ(state->pendingChoice->choiceOptions[0].label, QStringLiteral("Pay 2 life"));
    EXPECT_TRUE(state->pendingChoice->choiceOptions[0].enabled);
    EXPECT_EQ(state->pendingChoice->choiceOptions[1].label, QStringLiteral("Decline"));
    EXPECT_FALSE(state->pendingChoice->publicReveal);
    EXPECT_EQ(host.dialogRequests, 0);

    ASSERT_EQ(state->reveals.entries().size(), 1);
    const auto &entry = state->reveals.entries().first();
    EXPECT_EQ(entry.id, QStringLiteral("draw:41:801:2"));
    EXPECT_EQ(entry.phase, RuledRevealState::Phase::Active);
    EXPECT_EQ(entry.cardNames(), QStringList({QStringLiteral("Island")}));
}

TEST(RuledRevealStateTest, ReusingAnOccurrenceIdCannotReplaceItsRevealedIdentity)
{
    RuledRevealState state;
    ruled::v1::CardsRevealed reveal;
    reveal.set_reveal_id("event:30:0");
    auto *card = reveal.add_cards();
    card->set_object_id(44);
    card->set_card_id("forest");
    card->set_card_name("Forest");
    ASSERT_TRUE(state.beginChoice(reveal, {0}));
    ASSERT_TRUE(state.beginChoice(reveal, {0})) << "an unchanged authoritative snapshot is accepted";
    reveal.mutable_cards(0)->set_card_id("island");
    reveal.mutable_cards(0)->set_card_name("Island");
    EXPECT_FALSE(state.beginChoice(reveal, {0}));
    ASSERT_NE(state.choice(), nullptr);
    EXPECT_EQ(state.choice()->cardNames(), QStringList({"Forest"}));
}

TEST_F(RuledClientTest, ReplaySeekingSuppressesExpiredWindowsAndRestoresActiveReveals)
{
    ruled::v1::RuledEventBatch instant;
    auto *reveal = instant.add_events()->mutable_cards_revealed();
    reveal->set_reveal_id("event:40:0");
    auto *card = reveal->add_cards();
    card->set_card_id("forest");
    card->set_card_name("Forest");
    ASSERT_TRUE(dispatcher->processPayload(instant.SerializeAsString(), true));
    EXPECT_TRUE(state->reveals.windowsSuppressed());
    EXPECT_TRUE(state->reveals.entries().isEmpty());
    ruled::v1::RuledEventBatch active;
    auto *snapshot = active.add_events()->mutable_active_public_reveal_snapshot();
    *snapshot->add_reveals() = *reveal;
    snapshot->mutable_reveals(0)->set_reveal_id("stack:40:cost:0");
    ASSERT_TRUE(dispatcher->processPayload(active.SerializeAsString(), true));
    ASSERT_EQ(state->reveals.entries().size(), 1);
    EXPECT_TRUE(state->reveals.windowsSuppressed());
    apply(active);
    EXPECT_FALSE(state->reveals.windowsSuppressed());
    EXPECT_EQ(state->reveals.entries().first().phase, RuledRevealState::Phase::Active);
}

TEST_F(RuledClientTest, TargetObjectAndLegendKeepChoicesUseClickToSelect)
{
    {
        ruled::v1::RuledEventBatch batch;
        auto *rcr = batch.add_events()->mutable_resolution_choice_required();
        rcr->set_deciding_player_id(kLocalPlayer);
        rcr->set_choice_kind(ruled::v1::CHOICE_KIND_TARGET_OBJECTS); // CR 707.10c copy retarget
        rcr->set_prompt_text("Choose new targets for the copy.");
        rcr->add_candidate_object_ids(100);
        apply(batch);
        ASSERT_TRUE(state->hasPendingChoiceOfKind(RuledClientState::ChoiceKind::CopyTarget));
        EXPECT_TRUE(state->isPendingChoiceCandidate(RuledClientState::ChoiceKind::CopyTarget, 100));
        EXPECT_FALSE(state->isPendingChoiceCandidate(RuledClientState::ChoiceKind::CopyTarget, 101));
        EXPECT_FALSE(state->isResolutionHandPickActive()); // no list dialog for this kind
        EXPECT_EQ(host.dialogRequests, 0);

        host.sentCommands.clear();
        state->submitPendingChoiceObject(100);
        ASSERT_EQ(host.sentCommands.size(), 1);
        EXPECT_EQ(host.sentCommands[0].submit_resolution_choice().chosen_object_ids(0), 100u);
        EXPECT_FALSE(state->hasPendingChoiceOfKind(RuledClientState::ChoiceKind::CopyTarget));
    }
    {
        ruled::v1::RuledEventBatch batch;
        auto *rcr = batch.add_events()->mutable_resolution_choice_required();
        rcr->set_deciding_player_id(kLocalPlayer);
        rcr->set_choice_kind(ruled::v1::CHOICE_KIND_LEGEND_KEEP); // CR 704.5j legend rule
        rcr->add_candidate_object_ids(100);
        rcr->add_candidate_object_ids(101);
        apply(batch);
        ASSERT_TRUE(state->hasPendingChoiceOfKind(RuledClientState::ChoiceKind::LegendKeep));
        EXPECT_TRUE(state->isPendingChoiceCandidate(RuledClientState::ChoiceKind::LegendKeep, 101));

        host.sentCommands.clear();
        state->submitPendingChoiceObject(101);
        ASSERT_EQ(host.sentCommands.size(), 1);
        EXPECT_EQ(host.sentCommands[0].submit_resolution_choice().chosen_object_ids(0), 101u);
        EXPECT_FALSE(state->hasPendingChoiceOfKind(RuledClientState::ChoiceKind::LegendKeep));
    }
}

TEST_F(RuledClientTest, CopyTargetKeepsPlayerIdentityAndAllowsEmptyKeepOriginal)
{
    {
        ruled::v1::RuledEventBatch batch;
        auto *rcr = batch.add_events()->mutable_resolution_choice_required();
        rcr->set_deciding_player_id(kLocalPlayer);
        rcr->set_choice_kind(ruled::v1::CHOICE_KIND_TARGET_OBJECTS);
        rcr->set_min(0);
        rcr->set_max(1);
        rcr->set_prompt_text("Target 1 of 1 for the copy: choose a new target or keep the original");
        rcr->add_candidate_player_ids(kOpponent);
        apply(batch);

        ASSERT_TRUE(state->hasPendingChoiceOfKind(RuledClientState::ChoiceKind::CopyTarget));
        EXPECT_TRUE(state->pendingClickChoiceMayDecline());
        EXPECT_TRUE(state->isPendingCopyTargetPlayerCandidate(kOpponent));
        EXPECT_FALSE(state->isPendingCopyTargetPlayerCandidate(kLocalPlayer));
        EXPECT_EQ(host.dialogRequests, 0);

        host.sentCommands.clear();
        state->submitPendingCopyTargetPlayer(kOpponent);
        ASSERT_EQ(host.sentCommands.size(), 1);
        const auto &submission = host.sentCommands[0].submit_resolution_choice();
        EXPECT_EQ(submission.chosen_player_ids_size(), 1);
        EXPECT_EQ(submission.chosen_player_ids(0), kOpponent);
        EXPECT_EQ(submission.chosen_object_ids_size(), 0);
    }
    {
        ruled::v1::RuledEventBatch batch;
        auto *rcr = batch.add_events()->mutable_resolution_choice_required();
        rcr->set_deciding_player_id(kLocalPlayer);
        rcr->set_choice_kind(ruled::v1::CHOICE_KIND_TARGET_OBJECTS);
        rcr->set_min(0);
        rcr->set_max(1);
        rcr->set_prompt_text("Target 1 of 1 for the copy: choose a new target or keep the original");
        apply(batch);

        ASSERT_TRUE(state->hasPendingChoiceOfKind(RuledClientState::ChoiceKind::CopyTarget));
        EXPECT_TRUE(state->pendingClickChoiceMayDecline());
        EXPECT_TRUE(state->pendingChoice->candidateOids.isEmpty());
        EXPECT_TRUE(state->pendingChoice->candidatePlayerIds.isEmpty());

        host.sentCommands.clear();
        state->declinePendingClickChoice();
        ASSERT_EQ(host.sentCommands.size(), 1);
        const auto &submission = host.sentCommands[0].submit_resolution_choice();
        EXPECT_EQ(submission.chosen_object_ids_size(), 0);
        EXPECT_EQ(submission.chosen_player_ids_size(), 0);
        EXPECT_FALSE(state->hasPendingChoiceOfKind(RuledClientState::ChoiceKind::CopyTarget));
    }
}

TEST_F(RuledClientTest, CopyTargetGraveyardCardUsesOwnerScopedEngineObjectId)
{
    ruled::v1::RuledEventBatch batch;
    auto *graveyardMap = batch.add_events()->mutable_graveyard_object_map();
    auto *ownCard = graveyardMap->add_entries();
    ownCard->set_player_id(kLocalPlayer);
    ownCard->set_engine_object_id(500);
    ownCard->set_server_card_id(11);
    auto *opponentCard = graveyardMap->add_entries();
    opponentCard->set_player_id(kOpponent);
    opponentCard->set_engine_object_id(501);
    opponentCard->set_server_card_id(11); // Server_Card ids are owner-local.

    auto *choice = batch.add_events()->mutable_resolution_choice_required();
    choice->set_deciding_player_id(kLocalPlayer);
    choice->set_choice_kind(ruled::v1::CHOICE_KIND_TARGET_OBJECTS);
    choice->add_candidate_object_ids(500);
    choice->add_candidate_object_ids(501);
    apply(batch);

    ASSERT_TRUE(state->hasPendingChoiceOfKind(RuledClientState::ChoiceKind::CopyTarget));
    EXPECT_EQ(state->graveyardEngineOidForOwnedCard(kLocalPlayer, 11), 500u);
    const quint32 opponentObjectId = state->graveyardEngineOidForOwnedCard(kOpponent, 11);
    EXPECT_EQ(opponentObjectId, 501u);
    EXPECT_TRUE(state->isPendingChoiceCandidate(RuledClientState::ChoiceKind::CopyTarget, opponentObjectId));

    host.sentCommands.clear();
    state->submitPendingChoiceObject(opponentObjectId);
    ASSERT_EQ(host.sentCommands.size(), 1);
    const auto &submission = host.sentCommands[0].submit_resolution_choice();
    ASSERT_EQ(submission.chosen_object_ids_size(), 1);
    EXPECT_EQ(submission.chosen_object_ids(0), 501u);
    EXPECT_EQ(submission.chosen_player_ids_size(), 0);
}

TEST_F(RuledClientTest, CopySourceChoiceUsesBoardSelectionAndEmptyResolutionChoiceDeclines)
{
    ruled::v1::RuledEventBatch batch;
    auto *rcr = batch.add_events()->mutable_resolution_choice_required();
    rcr->set_deciding_player_id(kLocalPlayer);
    rcr->set_choice_kind(ruled::v1::CHOICE_KIND_COPY_SOURCE);
    rcr->set_min(0);
    rcr->set_max(1);
    rcr->set_prompt_text("Choose a creature for Clone to copy, or Decline to enter as Clone.");
    rcr->add_candidate_object_ids(100);
    rcr->add_candidate_object_ids(101);
    apply(batch);

    ASSERT_TRUE(state->hasPendingChoiceOfKind(RuledClientState::ChoiceKind::CopySource));
    EXPECT_TRUE(state->isPendingChoiceCandidate(RuledClientState::ChoiceKind::CopySource, 100));
    EXPECT_FALSE(state->isPendingChoiceCandidate(RuledClientState::ChoiceKind::CopySource, 102));
    EXPECT_TRUE(state->pendingClickChoiceMayDecline());
    EXPECT_EQ(state->pendingChoicePromptText(RuledClientState::ChoiceKind::CopySource),
              QStringLiteral("Choose a creature for Clone to copy, or Decline to enter as Clone."));

    host.sentCommands.clear();
    state->declinePendingClickChoice();
    ASSERT_EQ(host.sentCommands.size(), 1);
    EXPECT_TRUE(host.sentCommands[0].has_submit_resolution_choice());
    EXPECT_EQ(host.sentCommands[0].submit_resolution_choice().chosen_object_ids_size(), 0);
    EXPECT_FALSE(state->hasPendingChoiceOfKind(RuledClientState::ChoiceKind::CopySource));
}

TEST_F(RuledClientTest, PopulateUsesMandatoryCopySourceSelection)
{
    ruled::v1::RuledEventBatch batch;
    auto *choice = batch.add_events()->mutable_resolution_choice_required();
    choice->set_deciding_player_id(kLocalPlayer);
    choice->set_choice_kind(ruled::v1::CHOICE_KIND_COPY_SOURCE);
    choice->set_min(1);
    choice->set_max(1);
    choice->set_prompt_text("Choose a creature token you control to populate.");
    choice->add_candidate_object_ids(501);
    choice->add_candidate_object_ids(502);
    apply(batch);
    ASSERT_TRUE(state->hasPendingChoiceOfKind(RuledClientState::ChoiceKind::CopySource));
    EXPECT_FALSE(state->pendingClickChoiceMayDecline());
    host.sentCommands.clear();
    state->declinePendingClickChoice();
    EXPECT_TRUE(host.sentCommands.isEmpty());
    EXPECT_TRUE(state->hasPendingChoiceOfKind(RuledClientState::ChoiceKind::CopySource));
    state->submitPendingChoiceObject(502);
    ASSERT_EQ(host.sentCommands.size(), 1);
    ASSERT_TRUE(host.sentCommands[0].has_submit_resolution_choice());
    EXPECT_EQ(host.sentCommands[0].submit_resolution_choice().chosen_object_ids(0), 502u);
    EXPECT_FALSE(state->hasPendingChoiceOfKind(RuledClientState::ChoiceKind::CopySource));

    choice->set_deciding_player_id(kLocalPlayer + 1);
    apply(batch);
    EXPECT_FALSE(state->hasPendingChoiceOfKind(RuledClientState::ChoiceKind::CopySource));
}

TEST_F(RuledClientTest, ResolutionCostObjectsToggleAsABoundedCohortBeforeSubmission)
{
    QSignalSpy progressSpy(state, &RuledClientState::resolutionCostSelectionChanged);
    ruled::v1::RuledEventBatch batch;
    auto *rcr = batch.add_events()->mutable_resolution_choice_required();
    rcr->set_deciding_player_id(kLocalPlayer);
    rcr->set_choice_kind(ruled::v1::CHOICE_KIND_COST_OBJECTS);
    rcr->set_min(2);
    rcr->set_max(2);
    rcr->set_prompt_text("Choose two untapped permanents.");
    rcr->add_candidate_object_ids(100);
    rcr->add_candidate_object_ids(101);
    apply(batch);

    ASSERT_TRUE(state->hasPendingChoiceOfKind(RuledClientState::ChoiceKind::CostObjects));
    EXPECT_EQ(progressSpy.count(), 1);
    progressSpy.clear();
    state->toggleResolutionCostObject(999);
    EXPECT_EQ(progressSpy.count(), 0);
    state->toggleResolutionCostObject(100);
    EXPECT_EQ(progressSpy.count(), 1);
    EXPECT_TRUE(state->isResolutionCostObjectSelected(100));
    EXPECT_EQ(state->resolutionCostObjectSelectedCount(), 1);
    host.sentCommands.clear();
    state->submitResolutionCostObjects();
    EXPECT_TRUE(host.sentCommands.isEmpty());

    state->toggleResolutionCostObject(101);
    QSignalSpy repaintSpy(state, &RuledClientState::combatStateChanged);
    state->submitResolutionCostObjects();
    EXPECT_EQ(repaintSpy.count(), 1);
    EXPECT_FALSE(state->isResolutionCostObjectSelected(100));
    ASSERT_EQ(host.sentCommands.size(), 1);
    const auto &chosen = host.sentCommands[0].submit_resolution_choice().chosen_object_ids();
    ASSERT_EQ(chosen.size(), 2);
    EXPECT_EQ(chosen.Get(0), 100u);
    EXPECT_EQ(chosen.Get(1), 101u);
    EXPECT_EQ(progressSpy.count(), 3); // two selections and clearing the submitted choice
}

TEST_F(RuledClientTest, OptionalSacrificeCostObjectsPreserveEmptyAndPaidObjectAnswers)
{
    ruled::v1::RuledEventBatch batch;
    auto *choice = batch.add_events()->mutable_resolution_choice_required();
    choice->set_deciding_player_id(kLocalPlayer);
    choice->set_choice_kind(ruled::v1::CHOICE_KIND_COST_OBJECTS);
    choice->set_min(0);
    choice->set_max(1);
    choice->set_prompt_text("Choose a permanent to sacrifice, or decline.");
    choice->add_candidate_object_ids(101);
    apply(batch);

    ASSERT_TRUE(state->hasPendingChoiceOfKind(RuledClientState::ChoiceKind::CostObjects));
    ASSERT_TRUE(state->pendingChoice.has_value());
    EXPECT_EQ(state->pendingChoice->min, 0);
    EXPECT_EQ(state->pendingChoice->max, 1);
    EXPECT_TRUE(state->pendingClickChoiceMayDecline());
    EXPECT_TRUE(state->pendingChoice->candidateOids.contains(101));

    host.sentCommands.clear();
    state->submitResolutionCostObjects();
    ASSERT_EQ(host.sentCommands.size(), 1);
    const auto &declined = host.sentCommands[0].submit_resolution_choice();
    EXPECT_EQ(declined.chosen_object_ids_size(), 0);
    EXPECT_EQ(declined.chosen_player_ids_size(), 0);

    batch.Clear();
    choice = batch.add_events()->mutable_resolution_choice_required();
    choice->set_deciding_player_id(kLocalPlayer);
    choice->set_choice_kind(ruled::v1::CHOICE_KIND_COST_OBJECTS);
    choice->set_min(0);
    choice->set_max(1);
    choice->add_candidate_object_ids(101);
    apply(batch);
    state->toggleResolutionCostObject(101);
    state->submitResolutionCostObjects();
    ASSERT_EQ(host.sentCommands.size(), 2);
    const auto &paid = host.sentCommands[1].submit_resolution_choice();
    ASSERT_EQ(paid.chosen_object_ids_size(), 1);
    EXPECT_EQ(paid.chosen_object_ids(0), 101u);
    EXPECT_EQ(paid.chosen_player_ids_size(), 0);
}

TEST_F(RuledClientTest, CancellingResolutionCostObjectsRepaintsClearedSelectionImmediately)
{
    RuledClientState::RuledPendingChoice choice;
    choice.kind = RuledClientState::ChoiceKind::CostObjects;
    choice.mayDecline = true;
    choice.min = 0;
    choice.max = 1;
    choice.candidateOids = {100};
    stageChoiceForTest(choice);
    state->toggleResolutionCostObject(100);
    ASSERT_TRUE(state->isResolutionCostObjectSelected(100));

    QSignalSpy repaintSpy(state, &RuledClientState::combatStateChanged);
    QObject::connect(state, &RuledClientState::combatStateChanged, state,
                     [this]() { EXPECT_FALSE(state->isResolutionCostObjectSelected(100)); });
    state->declinePendingClickChoice();

    EXPECT_EQ(repaintSpy.count(), 1);
    ASSERT_EQ(host.sentCommands.size(), 1);
    EXPECT_EQ(host.sentCommands[0].submit_resolution_choice().chosen_object_ids_size(), 0);
}

TEST_F(RuledClientTest, SimultaneousEntryTimestampOrderUsesImagesAndClickOrder)
{
    ruled::v1::RuledEventBatch batch;
    auto *choice = batch.add_events()->mutable_resolution_choice_required();
    choice->set_deciding_player_id(kLocalPlayer);
    choice->set_choice_kind(ruled::v1::CHOICE_KIND_SIMULTANEOUS_ENTRY_ORDER);
    choice->set_prompt_text("Choose timestamp order");
    choice->set_min(2);
    choice->set_max(2);
    choice->set_ordered(true);
    choice->add_candidate_object_ids(77);
    choice->add_candidate_object_ids(78);
    choice->add_candidate_names("Folio of Fancies");
    choice->add_candidate_names("Twenty-Toed Toad");
    choice->add_candidate_card_ids("folio_of_fancies");
    choice->add_candidate_card_ids("twenty-toed_toad");
    choice->add_candidate_server_card_ids(0);
    choice->add_candidate_server_card_ids(1);
    apply(batch);
    EXPECT_EQ(host.dialogRequests, 0);
    ASSERT_TRUE(state->isResolutionHandPickActive());
    ASSERT_TRUE(state->pendingChoice.has_value());
    EXPECT_EQ(state->pendingChoice->viewTitle, QStringLiteral("Order cards"));
    EXPECT_EQ(state->resolutionHandPickPromptText(), QStringLiteral("Choose timestamp order"));
    EXPECT_EQ(state->resolutionHandPickZone(), RuledClientState::PickZone::Deck);
    EXPECT_FALSE(state->resolutionHandPickShowViewControls());
    state->toggleResolutionHandPickCard(1);
    EXPECT_FALSE(state->resolutionHandPickConfirmable());
    state->toggleResolutionHandPickCard(0);
    ASSERT_TRUE(state->resolutionHandPickConfirmable());
    state->submitResolutionHandPick();
    ASSERT_EQ(host.sentCommands.size(), 1);
    const auto &submitted = host.sentCommands[0].submit_resolution_choice();
    ASSERT_EQ(submitted.chosen_object_ids_size(), 2);
    EXPECT_EQ(submitted.chosen_object_ids(0), 78u);
    EXPECT_EQ(submitted.chosen_object_ids(1), 77u);
}

TEST_F(RuledClientTest, SimultaneousEntryOrderUsesFallbackWhenEnginePromptIsEmpty)
{
    ruled::v1::RuledEventBatch batch;
    auto *choice = batch.add_events()->mutable_resolution_choice_required();
    choice->set_deciding_player_id(kLocalPlayer);
    choice->set_choice_kind(ruled::v1::CHOICE_KIND_SIMULTANEOUS_ENTRY_ORDER);
    choice->set_min(2);
    choice->set_max(2);
    choice->set_ordered(true);
    choice->add_candidate_object_ids(79);
    choice->add_candidate_object_ids(80);
    choice->add_candidate_names("Folio of Fancies");
    choice->add_candidate_names("Twenty-Toed Toad");
    choice->add_candidate_card_ids("folio_of_fancies");
    choice->add_candidate_card_ids("twenty-toed_toad");
    choice->add_candidate_server_card_ids(0);
    choice->add_candidate_server_card_ids(1);
    apply(batch);

    ASSERT_TRUE(state->pendingChoice.has_value());
    EXPECT_EQ(state->pendingChoice->viewTitle, QStringLiteral("Order cards"));
    EXPECT_EQ(state->resolutionHandPickPromptText(), QStringLiteral("Choose the order for these cards."));
}

TEST_F(RuledClientTest, ProposedSameNameTokensRemainDistinctAndMalformedIdentityFailsClosed)
{
    ruled::v1::RuledEventBatch batch;
    auto *choice = batch.add_events()->mutable_resolution_choice_required();
    choice->set_deciding_player_id(kLocalPlayer);
    choice->set_choice_kind(ruled::v1::CHOICE_KIND_SIMULTANEOUS_ENTRY_ORDER);
    choice->set_prompt_text("Choose your simultaneous battlefield entries from earliest to latest timestamp.");
    choice->set_min(3);
    choice->set_max(3);
    choice->set_ordered(true);
    int i = 0;
    for (const char *keyword : {"Flying", "Vigilance", "Trample"}) {
        choice->add_candidate_object_ids(77u + i);
        choice->add_candidate_server_card_ids(i);
        choice->add_candidate_card_ids(std::string("golem_variant_") + keyword);
        choice->add_candidate_names("Golem");
        auto *identity = choice->add_candidate_token_identities();
        identity->set_name("Golem");
        identity->set_pt("3/3");
        identity->set_is_creature(true);
        for (const char *type : {"Artifact", "Creature", "Golem"})
            identity->add_types(type);
        identity->add_keywords(keyword);
        ++i;
    }
    apply(batch);
    ASSERT_TRUE(state->isResolutionHandPickActive());
    EXPECT_EQ(host.dialogRequests, 0);
    EXPECT_EQ(state->resolutionHandPickCandidateNames(), QStringList({"Golem", "Golem", "Golem"}));
    const auto identities = state->resolutionHandPickCandidateTokenIdentities();
    ASSERT_EQ(identities.size(), 3);
    const auto labels = state->resolutionHandPickCandidateAnnotations();
    ASSERT_EQ(labels.size(), 3);
    for (int index = 0; index < 3; ++index) {
        EXPECT_TRUE(labels[index].contains("Golem"));
        EXPECT_TRUE(labels[index].contains("3/3"));
        EXPECT_TRUE(labels[index].contains("Colorless"));
        EXPECT_TRUE(labels[index].contains("Artifact Creature Golem"));
        EXPECT_TRUE(labels[index].contains(QString::fromStdString(identities[index].keywords(0))));
    }
    EXPECT_EQ(state->pendingChoice->viewTitle, QStringLiteral("Order cards"));
    EXPECT_EQ(state->resolutionHandPickPromptText(),
              QStringLiteral("Choose your simultaneous battlefield entries from earliest to latest timestamp."));
    state->toggleResolutionHandPickCard(2);
    state->toggleResolutionHandPickCard(0);
    EXPECT_FALSE(state->resolutionHandPickConfirmable());
    state->toggleResolutionHandPickCard(1);
    ASSERT_TRUE(state->resolutionHandPickConfirmable());
    state->submitResolutionHandPick();
    ASSERT_EQ(host.sentCommands.size(), 1);
    EXPECT_EQ(host.sentCommands[0].submit_resolution_choice().chosen_object_ids(0), 79u);
    EXPECT_EQ(host.sentCommands[0].submit_resolution_choice().chosen_object_ids(1), 77u);
    EXPECT_EQ(host.sentCommands[0].submit_resolution_choice().chosen_object_ids(2), 78u);
    for (int defect = 0; defect < 5; ++defect) {
        auto malformed = batch;
        auto *bad = malformed.mutable_events(0)->mutable_resolution_choice_required();
        if (defect == 0)
            bad->mutable_candidate_token_identities()->RemoveLast();
        if (defect == 1)
            bad->mutable_candidate_token_identities(0)->clear_types();
        if (defect == 2)
            bad->mutable_candidate_token_identities(0)->set_name("Wrong name");
        if (defect == 3)
            bad->set_candidate_server_card_ids(1, 0);
        if (defect == 4)
            bad->set_candidate_object_ids(1, 77u);
        apply(malformed);
        EXPECT_FALSE(state->isResolutionHandPickActive());
        EXPECT_EQ(host.dialogRequests, 0);
    }
}

TEST_F(RuledClientTest, ProliferateChoiceMixesPermanentAndPlayerIdsWithinOneBoundedSelection)
{
    QSignalSpy progressSpy(state, &RuledClientState::resolutionCostSelectionChanged);
    ruled::v1::RuledEventBatch batch;
    auto *rcr = batch.add_events()->mutable_resolution_choice_required();
    rcr->set_deciding_player_id(kLocalPlayer);
    rcr->set_choice_kind(ruled::v1::CHOICE_KIND_PROLIFERATE);
    rcr->set_min(0);
    rcr->set_max(4);
    rcr->set_prompt_text("Choose any number of permanents and/or players with counters.");
    rcr->add_candidate_object_ids(100);
    rcr->add_candidate_names("Grizzly Bears");
    rcr->add_candidate_object_ids(101);
    rcr->add_candidate_names("Timber Wolves");
    rcr->add_candidate_player_ids(kLocalPlayer);
    rcr->add_candidate_player_ids(kLocalPlayer + 1);
    apply(batch);

    ASSERT_TRUE(state->hasPendingChoiceOfKind(RuledClientState::ChoiceKind::Proliferate));
    EXPECT_TRUE(state->isPendingChoiceCandidate(RuledClientState::ChoiceKind::Proliferate, 100));
    EXPECT_TRUE(state->isProliferatePlayerCandidate(kLocalPlayer + 1));
    EXPECT_EQ(state->proliferateSelectionRequired(), 0);
    EXPECT_EQ(state->proliferateSelectionMaximum(), 4);
    EXPECT_EQ(progressSpy.count(), 1);

    state->toggleProliferateObject(100);
    state->toggleProliferatePlayer(kLocalPlayer + 1);
    state->toggleProliferateObject(101);
    state->toggleProliferatePlayer(kLocalPlayer);
    EXPECT_EQ(state->proliferateSelectionSelectedCount(), 4);
    EXPECT_TRUE(state->isProliferateObjectSelected(100));
    EXPECT_TRUE(state->isProliferatePlayerSelected(kLocalPlayer + 1));
    state->toggleProliferateObject(999);
    EXPECT_EQ(state->proliferateSelectionSelectedCount(), 4);

    host.sentCommands.clear();
    state->submitProliferateChoice();
    ASSERT_EQ(host.sentCommands.size(), 1);
    const auto &choice = host.sentCommands[0].submit_resolution_choice();
    ASSERT_EQ(choice.chosen_object_ids_size(), 2);
    EXPECT_EQ(choice.chosen_object_ids(0), 100u);
    EXPECT_EQ(choice.chosen_object_ids(1), 101u);
    ASSERT_EQ(choice.chosen_player_ids_size(), 2);
    EXPECT_EQ(choice.chosen_player_ids(0), kLocalPlayer + 1);
    EXPECT_EQ(choice.chosen_player_ids(1), kLocalPlayer);
    EXPECT_EQ(choice.decision(), ruled::v1::RESOLUTION_CHOICE_DECISION_UNSPECIFIED);
    EXPECT_FALSE(state->hasPendingChoiceOfKind(RuledClientState::ChoiceKind::Proliferate));
}

TEST_F(RuledClientTest, DecliningProliferateSubmitsBothDomainsEmpty)
{
    RuledClientState::RuledPendingChoice choice;
    choice.kind = RuledClientState::ChoiceKind::Proliferate;
    choice.mayDecline = true;
    choice.min = 0;
    choice.max = 2;
    choice.candidateOids = {100};
    choice.candidatePlayerIds = {kLocalPlayer + 1};
    stageChoiceForTest(choice);
    state->toggleProliferateObject(100);
    state->toggleProliferatePlayer(kLocalPlayer + 1);
    host.sentCommands.clear();

    state->declinePendingClickChoice();

    ASSERT_EQ(host.sentCommands.size(), 1);
    const auto &answer = host.sentCommands[0].submit_resolution_choice();
    EXPECT_EQ(answer.chosen_object_ids_size(), 0);
    EXPECT_EQ(answer.chosen_player_ids_size(), 0);
    EXPECT_FALSE(state->hasPendingChoiceOfKind(RuledClientState::ChoiceKind::Proliferate));
}

TEST_F(RuledClientTest, AuraReturnChoicesUseTypedPermanentAndPlayerClickSurfaces)
{
    for (const auto &[protoKind, stateKind] :
         std::initializer_list<std::pair<ruled::v1::ChoiceKind, RuledClientState::ChoiceKind>>{
             {ruled::v1::CHOICE_KIND_AURA_PERMANENT, RuledClientState::ChoiceKind::AuraPermanent},
             {ruled::v1::CHOICE_KIND_AURA_PLAYER, RuledClientState::ChoiceKind::AuraPlayer}}) {
        ruled::v1::RuledEventBatch batch;
        auto *rcr = batch.add_events()->mutable_resolution_choice_required();
        rcr->set_deciding_player_id(kLocalPlayer);
        rcr->set_choice_kind(protoKind);
        rcr->set_min(1);
        rcr->set_max(1);
        rcr->set_prompt_text("Choose what the Aura will enchant.");
        rcr->add_candidate_object_ids(100);
        apply(batch);

        ASSERT_TRUE(state->hasPendingChoiceOfKind(stateKind));
        EXPECT_TRUE(state->isPendingChoiceCandidate(stateKind, 100));
        EXPECT_FALSE(state->pendingClickChoiceMayDecline());

        host.sentCommands.clear();
        state->submitPendingChoiceObject(100);
        ASSERT_EQ(host.sentCommands.size(), 1);
        EXPECT_EQ(host.sentCommands[0].submit_resolution_choice().chosen_object_ids(0), 100u);
    }
}

TEST_F(RuledClientTest, CopySourceChoiceIsInteractiveOnlyForTheDecidingPlayer)
{
    QSignalSpy promptFeed(state, &RuledClientState::enginePromptFeed);
    ruled::v1::RuledEventBatch batch;
    auto *rcr = batch.add_events()->mutable_resolution_choice_required();
    rcr->set_deciding_player_id(kLocalPlayer + 1);
    rcr->set_choice_kind(ruled::v1::CHOICE_KIND_COPY_SOURCE);
    rcr->set_min(0);
    rcr->set_max(1);
    rcr->set_prompt_text("Choose a creature for Clone to copy, or Decline to enter as Clone.");
    rcr->add_candidate_object_ids(100);
    apply(batch);

    EXPECT_FALSE(state->hasPendingChoiceOfKind(RuledClientState::ChoiceKind::CopySource));
    ASSERT_EQ(promptFeed.count(), 1);
    EXPECT_TRUE(promptFeed.at(0).at(0).toString().isEmpty());
    EXPECT_TRUE(state->isWaitingForChoice());
    EXPECT_EQ(state->choiceWaitingPlayer(), kLocalPlayer + 1);
}

// The engine parks one choice at a time, so the holder is exclusive: installing a new choice
// tears the previous one down — including the revealed-cards popup a pick had opened.
TEST_F(RuledClientTest, InstallingAChoiceTearsDownTheOneItReplaces)
{
    ruled::v1::RuledEventBatch reveal;
    auto *rcr = reveal.add_events()->mutable_resolution_choice_required();
    rcr->set_deciding_player_id(kLocalPlayer);
    rcr->set_choice_kind(ruled::v1::CHOICE_KIND_REVEALED);
    rcr->set_min(1);
    rcr->set_max(1);
    rcr->add_candidate_object_ids(10);
    rcr->add_candidate_server_card_ids(0);
    rcr->add_candidate_names("Swamp");
    apply(reveal);
    ASSERT_TRUE(state->isResolutionHandPickActive());

    QSignalSpy revealed(state, &RuledClientState::revealedPickChanged);
    ruled::v1::RuledEventBatch trigger;
    auto *tnt = trigger.add_events()->mutable_trigger_needs_target();
    tnt->set_source_permanent_id(100);
    tnt->set_ability_text("Draw a card.");
    tnt->set_controller_player_id(kLocalPlayer);
    apply(trigger);

    EXPECT_TRUE(state->hasPendingTriggerTarget());
    EXPECT_FALSE(state->isResolutionHandPickActive());
    ASSERT_EQ(revealed.count(), 1);
    EXPECT_FALSE(revealed.at(0).at(0).toBool()); // popup told to close
}

TEST_F(RuledClientTest, UnrecognisedChoiceKindFallsBackToTheModalDialog)
{
    ruled::v1::RuledEventBatch batch;
    auto *rcr = batch.add_events()->mutable_resolution_choice_required();
    rcr->set_deciding_player_id(kLocalPlayer);
    rcr->set_choice_kind(ruled::v1::CHOICE_KIND_HAND_CARDS); // no parallel server card ids
    rcr->set_prompt_text("Choose one.");
    rcr->add_candidate_object_ids(11);
    rcr->add_candidate_names("Forest");
    apply(batch);

    EXPECT_FALSE(state->isResolutionHandPickActive());
    EXPECT_EQ(host.dialogRequests, 1);
    EXPECT_EQ(host.lastDialogPrompt, QStringLiteral("Choose one."));
}

TEST_F(RuledClientTest, ReplacementEffectChoiceNeverUsesModalFallback)
{
    host.autoSubmitDialogChoice = true;
    ruled::v1::RuledEventBatch batch;
    auto *rcr = batch.add_events()->mutable_resolution_choice_required();
    rcr->set_deciding_player_id(kLocalPlayer);
    rcr->set_choice_kind(ruled::v1::CHOICE_KIND_REPLACEMENT_EFFECT);
    rcr->set_prompt_text("Choose the next replacement effect for Diregraf Ghoul entering the battlefield.");
    rcr->set_min(1);
    rcr->set_max(1);
    rcr->add_candidate_object_ids(7001);
    rcr->add_candidate_names("Orb of Dreams - permanents enter tapped");
    auto *option = rcr->add_replacement_options();
    option->set_application_id(7001);
    option->set_source_card_name("Orb of Dreams");
    option->set_effect_summary("Permanents enter tapped");
    apply(batch);

    EXPECT_EQ(host.dialogRequests, 0);
    EXPECT_TRUE(host.sentCommands.isEmpty());
    ASSERT_TRUE(state->hasPendingReplacementEffect());
    ASSERT_EQ(state->pendingChoice->replacementOptions.size(), 1);
    EXPECT_EQ(state->pendingChoice->replacementOptions[0].source_card_name(), "Orb of Dreams");
    EXPECT_EQ(state->pendingChoice->replacementOptions[0].effect_summary(), "Permanents enter tapped");
    const auto images = state->replacementEffectImages();
    ASSERT_EQ(images.size(), 1);
    EXPECT_EQ(images[0].tileIndex, 0);
    EXPECT_EQ(images[0].cardName, "Orb of Dreams");
    EXPECT_EQ(images[0].annotation, "Permanents enter tapped");
    const auto revision = state->pendingChoiceRevision;
    state->submitReplacementEffect(0, revision - 1);
    state->submitReplacementEffect(1, revision);
    state->submitReplacementEffect(7001, revision); // Application and physical ids are not tile indices.
    EXPECT_TRUE(host.sentCommands.isEmpty());
    state->submitReplacementEffect(0, revision);
    state->submitReplacementEffect(0, revision);
    ASSERT_EQ(host.sentCommands.size(), 1);
    EXPECT_EQ(host.sentCommands[0].submit_resolution_choice().chosen_object_ids(0), 7001u);
    EXPECT_TRUE(state->pendingChoice->replacementSubmitting);
    host.answerPendingAck(false);
    EXPECT_FALSE(state->pendingChoice->replacementSubmitting);
    state->submitReplacementEffect(0, revision);
    ASSERT_EQ(host.sentCommands.size(), 2);
    ruled::v1::RuledEventBatch preview;
    preview.add_events()->mutable_attackers_preview()->set_declaring_player_id(kLocalPlayer);
    apply(preview);
    EXPECT_TRUE(state->hasPendingReplacementEffect());
    EXPECT_EQ(state->pendingChoiceRevision, revision);
    apply(batch);
    const auto refreshedRevision = state->pendingChoiceRevision;
    EXPECT_EQ(refreshedRevision, revision);
    host.answerPendingAck(false);
    EXPECT_EQ(state->pendingChoiceRevision, refreshedRevision);
    EXPECT_FALSE(state->pendingChoice->replacementSubmitting);
    EXPECT_EQ(host.sentCommands.size(), 2);
    apply(ruled::v1::RuledEventBatch{});
    EXPECT_FALSE(state->hasPendingReplacementEffect());
    apply(batch);
    state->submitReplacementEffect(0, state->pendingChoiceRevision);
    state->clearSessionState();
    host.answerPendingAck(false);
    EXPECT_FALSE(state->hasPendingReplacementEffect());
    EXPECT_TRUE(state->replacementEffectImages().isEmpty());
}

TEST_F(RuledClientTest, ReplacementImagesPreserveDuplicateSourcesAndRejectMalformedSnapshots)
{
    ruled::v1::RuledEventBatch batch;
    auto *choice = batch.add_events()->mutable_resolution_choice_required();
    choice->set_deciding_player_id(kLocalPlayer);
    choice->set_choice_kind(ruled::v1::CHOICE_KIND_REPLACEMENT_EFFECT);
    choice->set_min(1);
    choice->set_max(1);
    for (quint32 id : {7001u, 7002u}) {
        choice->add_candidate_object_ids(id);
        auto *option = choice->add_replacement_options();
        option->set_application_id(id);
        option->set_source_card_name("Dragonstorm Globe");
        option->set_effect_summary("Enters with counters");
    }
    apply(batch);
    ASSERT_TRUE(state->hasPendingReplacementEffect());
    EXPECT_EQ(state->pendingChoice->replacementOptions.size(), 2);
    state->submitReplacementEffect(1, state->pendingChoiceRevision);
    EXPECT_EQ(host.sentCommands.last().submit_resolution_choice().chosen_object_ids(0), 7002u);
    host.answerPendingAck(true);
    choice->mutable_replacement_options(0)->clear_source_card_name();
    apply(batch);
    ASSERT_TRUE(state->hasPendingReplacementEffect());
    EXPECT_TRUE(state->pendingChoice->replacementOptions[0].source_card_name().empty());
    EXPECT_EQ(state->replacementEffectImages()[0].cardName, "Replacement effect");
    EXPECT_EQ(state->replacementEffectImages()[0].annotation, "Enters with counters");
    choice->mutable_replacement_options(1)->set_application_id(7001);
    apply(batch);
    EXPECT_FALSE(state->hasPendingReplacementEffect());
    EXPECT_EQ(host.dialogRequests, 0);
    choice->mutable_replacement_options(1)->set_application_id(7002);
    choice->set_deciding_player_id(kOpponent);
    choice->set_resolution_prompt_id(0);
    batch.mutable_resolution_prompt_state()->set_deciding_player_id(kOpponent);
    batch.mutable_resolution_prompt_state()->clear_resolution_prompt_id();
    apply(batch);
    EXPECT_FALSE(state->hasPendingReplacementEffect());
    EXPECT_EQ(state->choiceWaitingPlayerId, kOpponent);
}

TEST_F(RuledClientTest, ChoicesForAnotherPlayerNeverPromptUs)
{
    ruled::v1::RuledEventBatch batch;
    auto *rcr = batch.add_events()->mutable_resolution_choice_required();
    rcr->set_deciding_player_id(kOpponent);
    rcr->set_choice_kind(ruled::v1::CHOICE_KIND_HAND_CARDS);
    rcr->add_candidate_object_ids(11);
    rcr->add_candidate_server_card_ids(1);
    apply(batch);

    EXPECT_FALSE(state->isResolutionHandPickActive());
    EXPECT_EQ(host.dialogRequests, 0);
}

// ---------------------------------------------------------------------------------------
// Opening sequence commands
// ---------------------------------------------------------------------------------------

TEST_F(RuledClientTest, OpeningBottomSendsIndicesAdjustedForPriorRemovals)
{
    ruled::v1::RuledEventBatch batch;
    auto &actions = (*batch.mutable_legal_by_player())[kLocalPlayer];
    auto *opening = actions.mutable_opening();
    opening->set_stage(ruled::v1::OPENING_STAGE_BOTTOM);
    opening->set_deciding_player_id(kLocalPlayer);
    opening->set_mulligans_taken(1);
    opening->set_bottom_cards_remaining(1);
    for (int i = 0; i < 7; ++i) {
        addHandAction(actions, ruled::v1::HAND_ACTION_OPENING_BOTTOM, i, "Card");
    }
    apply(batch);
    ASSERT_EQ(state->getOpeningUiKind(), RuledClientState::RuledOpeningUiKind::BottomLibrary);

    ASSERT_EQ(state->openingBottomRequiredCount(), 1);

    state->toggleOpeningBottomHandIndex(2);
    EXPECT_EQ(state->openingBottomClickOrderFor(2), 1);
    host.sentCommands.clear();
    state->openingBottomDone();
    ASSERT_EQ(host.sentCommands.size(), 1);
    EXPECT_EQ(host.sentCommands[0].put_opening_hand_on_bottom().hand_card_index(), 2u);
}

TEST_F(RuledClientTest, OpeningBottomAdjustsLaterIndicesForEarlierRemovals)
{
    ruled::v1::RuledEventBatch batch;
    auto &actions = (*batch.mutable_legal_by_player())[kLocalPlayer];
    auto *opening = actions.mutable_opening();
    opening->set_stage(ruled::v1::OPENING_STAGE_BOTTOM);
    opening->set_deciding_player_id(kLocalPlayer);
    opening->set_mulligans_taken(2);
    opening->set_bottom_cards_remaining(2);
    for (int i = 0; i < 7; ++i) {
        addHandAction(actions, ruled::v1::HAND_ACTION_OPENING_BOTTOM, i, "Card");
    }
    apply(batch);
    ASSERT_EQ(state->openingBottomRequiredCount(), 2);

    // Click low index first: the later index shifts down by one when it is sent.
    state->toggleOpeningBottomHandIndex(1);
    state->toggleOpeningBottomHandIndex(4);
    host.sentCommands.clear();
    state->openingBottomDone();
    ASSERT_EQ(host.sentCommands.size(), 1);
    EXPECT_EQ(host.sentCommands[0].put_opening_hand_on_bottom().hand_card_index(), 1u);
    host.answerPendingAck(true);
    ASSERT_EQ(host.sentCommands.size(), 2);
    EXPECT_EQ(host.sentCommands[1].put_opening_hand_on_bottom().hand_card_index(), 3u);
}

// ---------------------------------------------------------------------------------------
// Session lifecycle
// ---------------------------------------------------------------------------------------

TEST_F(RuledClientTest, ClearSessionStateResetsEverythingCarriedBetweenGames)
{
    state->payment.begin();
    const auto obsoletePayment = state->payment.request({});
    ruled::v1::RuledEventBatch batch;
    auto *sp = batch.add_events()->mutable_stack_pushed();
    sp->set_object_id(900);
    sp->set_ability_annotation("some ability");
    auto *gy = batch.add_events()->mutable_graveyard_object_map()->add_entries();
    gy->set_engine_object_id(500);
    gy->set_server_card_id(11);
    auto *rcr = batch.add_events()->mutable_resolution_choice_required();
    rcr->set_deciding_player_id(kLocalPlayer);
    rcr->set_choice_kind(ruled::v1::CHOICE_KIND_TARGET_OBJECTS);
    rcr->add_candidate_object_ids(100);
    auto &actions = (*batch.mutable_legal_by_player())[kLocalPlayer];
    addHandAction(actions, ruled::v1::HAND_ACTION_CAST_SPELL, 0, "Grizzly Bears");
    apply(batch);
    EXPECT_FALSE(state->payment.active); // a blocking engine choice replaces local staging
    ruled::v1::PaymentPreview obsoleteReply;
    obsoleteReply.set_transaction_id(obsoletePayment.transaction_id());
    obsoleteReply.set_revision(obsoletePayment.revision());
    EXPECT_FALSE(state->payment.apply(obsoleteReply));
    ASSERT_TRUE(state->hasStackItems());
    ASSERT_TRUE(state->hasPendingChoiceOfKind(RuledClientState::ChoiceKind::CopyTarget));

    QSignalSpy resetSpy(state, &RuledClientState::sessionReset);
    state->payment.begin();
    state->payment.payMana('U');
    state->clearSessionState();

    EXPECT_EQ(resetSpy.count(), 1);
    EXPECT_FALSE(state->payment.active);
    EXPECT_EQ(state->payment.selection.mana().u(), 0u);
    EXPECT_FALSE(state->hasStackItems());
    EXPECT_TRUE(state->stackAnnotation(900).isEmpty());
    EXPECT_FALSE(state->hasPendingChoiceOfKind(RuledClientState::ChoiceKind::CopyTarget));
    EXPECT_FALSE(state->hasPendingTriggerTarget());
    EXPECT_FALSE(state->isHandActionLegal(ruled::v1::HAND_ACTION_CAST_SPELL, 0));
    EXPECT_EQ(state->getOpeningUiKind(), RuledClientState::RuledOpeningUiKind::None);
    EXPECT_EQ(state->getOpeningMulliganCount(), 0);
    // Phantom graveyard targets are the reason this map must not survive a game boundary.
    EXPECT_EQ(state->graveyardEngineOidForOwnedCard(kLocalPlayer, 11), 0u);
}

// The server broadcasts a new session's first RuledEventBatch *before* the Event_GameStateChanged
// that flips game_started, so the game-start teardown runs with the incoming opening prompt already
// applied. Clearing it there strands the opening: the engine is blocked on ChooseStartingPlayer and
// never re-sends the prompt.
TEST_F(RuledClientTest, GameStartResetKeepsTheIncomingSessionsOpeningPrompt)
{
    ruled::v1::RuledEventBatch batch;
    // Residue from the finished game that must still be cleared…
    batch.add_events()->mutable_stack_pushed()->set_object_id(900);
    // …alongside the incoming session's opening prompt, which must survive.
    auto &actions = (*batch.mutable_legal_by_player())[kLocalPlayer];
    actions.mutable_opening()->set_stage(ruled::v1::OPENING_STAGE_CHOOSE_STARTING_PLAYER);
    actions.mutable_opening()->set_deciding_player_id(kLocalPlayer);
    actions.mutable_opening()->add_eligible_starting_player_ids(kLocalPlayer);
    actions.mutable_opening()->add_eligible_starting_player_ids(kOpponent);
    apply(batch);
    ASSERT_EQ(state->getOpeningUiKind(), RuledClientState::RuledOpeningUiKind::ChooseFirst);

    state->clearSessionState(RuledSessionResetScope::KeepCurrentBatch);

    EXPECT_EQ(state->getOpeningUiKind(), RuledClientState::RuledOpeningUiKind::ChooseFirst);
    EXPECT_FALSE(state->hasStackItems()); // the finished game's residue still goes

    // The game-stop transition is the symmetric case: nothing survives it.
    state->clearSessionState(RuledSessionResetScope::All);
    EXPECT_EQ(state->getOpeningUiKind(), RuledClientState::RuledOpeningUiKind::None);
}

TEST_F(RuledClientTest, EveryBatchSchedulesAnArrowResync)
{
    apply(phaseBatch(ruled::v1::PHASE_ID_MAIN1, kLocalPlayer));
    EXPECT_EQ(host.arrowSyncRequests, 1);
    apply(phaseBatch(ruled::v1::PHASE_ID_MAIN2, kLocalPlayer));
    EXPECT_EQ(host.arrowSyncRequests, 2);
}

TEST_F(RuledClientTest, MalformedPayloadIsRejectedWithoutCrashing)
{
    EXPECT_FALSE(dispatcher->processPayload(std::string("\xff\xff\xff\xff", 4)));
}

// ---------------------------------------------------------------------------------------------
// Dev console grammar (RuledDevCommandParser)
//
// The console is the one ruled input that starts as free text, so the grammar is the only place a
// typo turns into a wrong command rather than a compile error. Seats here are ids {7, 9} rather
// than {0, 1} so an ordinal can never accidentally equal the id it resolves to.
// ---------------------------------------------------------------------------------------------

namespace
{
const QVector<int> kSeats = {7, 9};
constexpr int kMe = 7;

RuledDevCommandParser::Result parseLine(const QString &line)
{
    return RuledDevCommandParser::parse(line, kMe, kSeats);
}
} // namespace

TEST(RuledDevCommandParserTest, PutDefaultsToTheLocalSeat)
{
    const auto r = parseLine(QStringLiteral("put hand Serra Angel"));
    ASSERT_TRUE(r.ok) << r.error.toStdString();
    const auto &dev = r.command.dev_command();
    EXPECT_EQ(dev.target_player_id(), kMe);
    EXPECT_EQ(dev.put_card_in_zone().card_name(), "Serra Angel");
    EXPECT_EQ(dev.put_card_in_zone().zone(), ruled::v1::DEV_ZONE_HAND);
    EXPECT_FALSE(dev.put_card_in_zone().ready());
}

TEST(RuledDevCommandParserTest, SeatIsAOneBasedOrdinalNotAPlayerId)
{
    const auto r = parseLine(QStringLiteral("put 2 gy Lightning Bolt"));
    ASSERT_TRUE(r.ok) << r.error.toStdString();
    EXPECT_EQ(r.command.dev_command().target_player_id(), 9) << "ordinal 2 is the second seat";
    EXPECT_EQ(r.command.dev_command().put_card_in_zone().zone(), ruled::v1::DEV_ZONE_GRAVEYARD);
}

TEST(RuledDevCommandParserTest, OutOfRangeSeatIsRejected)
{
    const auto r = parseLine(QStringLiteral("put 5 hand Serra Angel"));
    EXPECT_FALSE(r.ok);
    EXPECT_FALSE(r.error.isEmpty());
}

TEST(RuledDevCommandParserTest, EveryZoneAliasResolves)
{
    const QVector<QPair<QString, ruled::v1::DevZone>> cases = {
        {QStringLiteral("hand"), ruled::v1::DEV_ZONE_HAND},
        {QStringLiteral("bf"), ruled::v1::DEV_ZONE_BATTLEFIELD},
        {QStringLiteral("battlefield"), ruled::v1::DEV_ZONE_BATTLEFIELD},
        {QStringLiteral("board"), ruled::v1::DEV_ZONE_BATTLEFIELD},
        {QStringLiteral("gy"), ruled::v1::DEV_ZONE_GRAVEYARD},
        {QStringLiteral("graveyard"), ruled::v1::DEV_ZONE_GRAVEYARD},
        {QStringLiteral("exile"), ruled::v1::DEV_ZONE_EXILE},
        {QStringLiteral("lib"), ruled::v1::DEV_ZONE_LIBRARY},
        {QStringLiteral("library"), ruled::v1::DEV_ZONE_LIBRARY},
        {QStringLiteral("deck"), ruled::v1::DEV_ZONE_LIBRARY},
        {QStringLiteral("cmd"), ruled::v1::DEV_ZONE_COMMAND},
        {QStringLiteral("command"), ruled::v1::DEV_ZONE_COMMAND},
    };
    for (const auto &[word, zone] : cases) {
        const auto r = parseLine(QStringLiteral("put %1 Grizzly Bears").arg(word));
        ASSERT_TRUE(r.ok) << word.toStdString() << ": " << r.error.toStdString();
        EXPECT_EQ(r.command.dev_command().put_card_in_zone().zone(), zone) << word.toStdString();
    }
}

TEST(RuledDevCommandParserTest, UnknownZoneIsRejected)
{
    EXPECT_FALSE(parseLine(QStringLiteral("put sideboard Serra Angel")).ok);
}

TEST(RuledDevCommandParserTest, ReadyIsStrippedOnlyAsATrailingToken)
{
    const auto readied = parseLine(QStringLiteral("put bf Grizzly Bears ready"));
    ASSERT_TRUE(readied.ok);
    EXPECT_TRUE(readied.command.dev_command().put_card_in_zone().ready());
    EXPECT_EQ(readied.command.dev_command().put_card_in_zone().card_name(), "Grizzly Bears");

    // A card named "ready" is still a card name: the flag is only consumed when something
    // precedes it, so the grammar does not depend on what is in the card pool.
    const auto named = parseLine(QStringLiteral("put bf ready"));
    ASSERT_TRUE(named.ok);
    EXPECT_FALSE(named.command.dev_command().put_card_in_zone().ready());
    EXPECT_EQ(named.command.dev_command().put_card_in_zone().card_name(), "ready");
}

TEST(RuledDevCommandParserTest, PutWithoutACardNameIsRejected)
{
    EXPECT_FALSE(parseLine(QStringLiteral("put bf")).ok);
    EXPECT_FALSE(parseLine(QStringLiteral("put")).ok);
}

TEST(RuledDevCommandParserTest, MoveSharesPutsGrammarButBuildsTheOtherPayload)
{
    const auto r = parseLine(QStringLiteral("move 2 gy Serra Angel"));
    ASSERT_TRUE(r.ok) << r.error.toStdString();
    const auto &dev = r.command.dev_command();
    EXPECT_EQ(dev.target_player_id(), 9);
    ASSERT_TRUE(dev.has_move_card()) << "move must not build a put payload";
    EXPECT_FALSE(dev.has_put_card_in_zone());
    EXPECT_EQ(dev.move_card().card_name(), "Serra Angel");
    EXPECT_EQ(dev.move_card().zone(), ruled::v1::DEV_ZONE_GRAVEYARD);

    // `ready` applies to move too: relocating onto the battlefield re-sickens the permanent.
    const auto readied = parseLine(QStringLiteral("move bf Grizzly Bears ready"));
    ASSERT_TRUE(readied.ok);
    EXPECT_TRUE(readied.command.dev_command().move_card().ready());

    EXPECT_FALSE(parseLine(QStringLiteral("move gy")).ok);
    EXPECT_FALSE(parseLine(QStringLiteral("move")).ok);
}

TEST(RuledDevCommandParserTest, PutAndMoveAreDistinctVerbs)
{
    // The whole point of the split: the same line with a different verb produces a different
    // payload, so "give me a card" and "relocate this one" can no longer be confused.
    const auto put = parseLine(QStringLiteral("put bf Serra Angel"));
    const auto moved = parseLine(QStringLiteral("move bf Serra Angel"));
    ASSERT_TRUE(put.ok);
    ASSERT_TRUE(moved.ok);
    EXPECT_TRUE(put.command.dev_command().has_put_card_in_zone());
    EXPECT_TRUE(moved.command.dev_command().has_move_card());
}

TEST(RuledDevCommandParserTest, ManaCountsColourPipsAndGenericDigits)
{
    const auto r = parseLine(QStringLiteral("mana 3RR"));
    ASSERT_TRUE(r.ok) << r.error.toStdString();
    const auto &m = r.command.dev_command().add_mana();
    EXPECT_EQ(m.c(), 3u);
    EXPECT_EQ(m.r(), 2u);
    EXPECT_EQ(m.w(), 0u);

    const auto multi = parseLine(QStringLiteral("mana WWU"));
    ASSERT_TRUE(multi.ok);
    EXPECT_EQ(multi.command.dev_command().add_mana().w(), 2u);
    EXPECT_EQ(multi.command.dev_command().add_mana().u(), 1u);

    // Digits form one number, so this is twelve generic rather than one and two. It also has to
    // survive the seat rule: a lone leading number is symbols, not an out-of-range seat.
    const auto twelve = parseLine(QStringLiteral("mana 12"));
    ASSERT_TRUE(twelve.ok) << twelve.error.toStdString();
    EXPECT_EQ(twelve.command.dev_command().add_mana().c(), 12u);
    EXPECT_EQ(twelve.command.dev_command().target_player_id(), kMe);
}

TEST(RuledDevCommandParserTest, ManaSeatOnlyWinsWhenItIsAValidOrdinalWithSymbolsAfterIt)
{
    // "2 UU" is the second seat; "3 RR" cannot be (there is no seat 3), so it falls back to
    // reading as mana rather than erroring — three generic and two red.
    const auto notASeat = parseLine(QStringLiteral("mana 3 RR"));
    ASSERT_TRUE(notASeat.ok) << notASeat.error.toStdString();
    EXPECT_EQ(notASeat.command.dev_command().target_player_id(), kMe);
    EXPECT_EQ(notASeat.command.dev_command().add_mana().c(), 3u);
    EXPECT_EQ(notASeat.command.dev_command().add_mana().r(), 2u);

    // And the unambiguous spelling of the same thing.
    const auto joined = parseLine(QStringLiteral("mana 3RR"));
    ASSERT_TRUE(joined.ok);
    EXPECT_EQ(joined.command.dev_command().target_player_id(), kMe);
}

TEST(RuledDevCommandParserTest, ManaAcceptsASeatAndRejectsJunkSymbols)
{
    const auto seated = parseLine(QStringLiteral("mana 2 UU"));
    ASSERT_TRUE(seated.ok) << seated.error.toStdString();
    EXPECT_EQ(seated.command.dev_command().target_player_id(), 9);
    EXPECT_EQ(seated.command.dev_command().add_mana().u(), 2u);

    EXPECT_FALSE(parseLine(QStringLiteral("mana XYZ")).ok);
    EXPECT_FALSE(parseLine(QStringLiteral("mana")).ok);
}

TEST(RuledDevCommandParserTest, LeadingSlashAndCaseAreTolerated)
{
    const auto slashed = parseLine(QStringLiteral("/PUT Hand Serra Angel"));
    ASSERT_TRUE(slashed.ok) << slashed.error.toStdString();
    EXPECT_EQ(slashed.command.dev_command().put_card_in_zone().zone(), ruled::v1::DEV_ZONE_HAND);
    // The card name keeps its typed casing — the engine matches Oracle names case-insensitively,
    // but the log should read the way the user wrote it.
    EXPECT_EQ(slashed.command.dev_command().put_card_in_zone().card_name(), "Serra Angel");
}

TEST(RuledDevCommandParserTest, HelpIsHandledLocallyAndUnknownVerbsAreNot)
{
    const auto help = parseLine(QStringLiteral("help"));
    EXPECT_FALSE(help.ok);
    EXPECT_TRUE(help.handledLocally);
    EXPECT_FALSE(help.message.isEmpty());

    const auto unknown = parseLine(QStringLiteral("summon Serra Angel"));
    EXPECT_FALSE(unknown.ok);
    EXPECT_FALSE(unknown.handledLocally);
    EXPECT_FALSE(unknown.error.isEmpty());
}

} // namespace

TEST(RuledDiagnostics, CapturesLocalStagingAndLosslessIdentityWithoutChangingState)
{
    RuledClientState state(nullptr);
    state.engineOidToCardId.insert(42, 7);
    state.engineOidOwner.insert(42, 29);
    state.battlefieldGenerationByOid.insert(42, std::numeric_limits<quint64>::max());
    state.pendingAttackerOids.insert(42);
    RuledClientState::RuledPendingChoice choice;
    choice.kind = RuledClientState::RuledPendingChoice::Kind::ResolutionPick;
    choice.selectedServerCardIds = {7};
    choice.serverCardIdToOid.insert(7, 42);
    choice.promptText = "Select a card";
    state.pendingChoice = choice;
    const auto snapshot = state.diagnosticSnapshot();
    const auto objects = snapshot.value("battlefield_objects").toArray();
    ASSERT_EQ(objects.size(), 1);
    EXPECT_EQ(objects[0].toObject().value("zone_change_generation").toString(), "18446744073709551615");
    EXPECT_EQ(objects[0].toObject().value("server_card_id").toInt(), 7);
    EXPECT_EQ(objects[0].toObject().value("owner_player_id").toInt(), 29);
    EXPECT_EQ(snapshot.value("pendingChoice").toObject().value("kind").toString(), "ResolutionPick");
    EXPECT_EQ(snapshot.value("pendingAttackerOids").toArray().size(), 1);
    EXPECT_EQ(state.battlefieldGenerationByOid.value(42), std::numeric_limits<quint64>::max());
    EXPECT_EQ(snapshot, state.diagnosticSnapshot());
    state.payment.begin();
    EXPECT_TRUE(state.payment.diagnosticSnapshot().value("active").toBool());
    EXPECT_EQ(state.payment.diagnosticSnapshot().value("transactionId").toString(),
              QString::number(state.payment.transaction()));
}

TEST_F(RuledClientTest, AbilityLogResolvesOracleTextAndPreservesFallbackAndSurroundingText)
{
    host.presentationCards = readPresentationCards(R"(<cockatrice_carddatabase version="4"><cards><card>
      <name>Spyglass Siren</name><text>Ordinary display</text><ruled-oracle>
      <face card-name="Spyglass Siren" face-name="Spyglass Siren"><text>Flying
When Spyglass Siren enters, create a Map token.</text></face>
      </ruled-oracle></card></cards></cockatrice_carddatabase>)");
    const QString oracle = QStringLiteral("Flying\nWhen Spyglass Siren enters, create a Map token.");
    ruled::v1::RuledEventBatch batch;
    auto *log = batch.add_events()->mutable_log();
    log->set_text("Complete engine fallback");
    auto *rendered = log->mutable_ability_presentation();
    rendered->set_prefix("Triggered: Spyglass Siren - ");
    rendered->set_suffix(" [target retained]");
    auto *ability = rendered->mutable_ability();
    ability->set_external_card_name("Spyglass Siren");
    ability->set_external_face_name("Spyglass Siren");
    ability->set_oracle_text_sha256(RuledOracleText::textSha256(oracle).toStdString());
    ability->add_oracle_line_indices(2);
    ability->set_fallback_text("Siren fallback ability");
    QSignalSpy timeline(state, &RuledClientState::engineTimeline);
    apply(batch);
    ASSERT_EQ(timeline.count(), 1);
    EXPECT_EQ(timeline.last().at(0).toString(),
              QStringLiteral(
                  "Triggered: Spyglass Siren - When Spyglass Siren enters, create a Map token. [target retained]\n"));
    host.presentationCards.clear();
    apply(batch);
    EXPECT_EQ(timeline.last().at(0).toString(),
              QStringLiteral("Triggered: Spyglass Siren - Siren fallback ability [target retained]\n"));
    rendered->clear_ability();
    apply(batch);
    EXPECT_EQ(timeline.last().at(0).toString(), QStringLiteral("Complete engine fallback\n"));
    log->clear_ability_presentation();
    apply(batch);
    EXPECT_EQ(timeline.last().at(0).toString(), QStringLiteral("Complete engine fallback\n"));
}

TEST_F(RuledClientTest, SpecialCastSubmissionPreservesTheCompleteAnnouncementAndPromptId)
{
    ruled::v1::RuledEventBatch offerBatch;
    auto *rcr = offerBatch.add_events()->mutable_resolution_choice_required();
    rcr->set_deciding_player_id(kLocalPlayer);
    rcr->set_choice_kind(ruled::v1::CHOICE_KIND_SPECIAL_CAST);
    rcr->add_candidate_object_ids(70);
    rcr->add_candidate_selectable(true);
    auto *zoneOffer = (*offerBatch.mutable_legal_by_player())[kLocalPlayer].add_zone_cast_actions();
    zoneOffer->set_object_id(70);
    zoneOffer->set_source_zone(ruled::v1::CAST_SOURCE_ZONE_EXILE);
    zoneOffer->set_zone_change_generation(6);
    zoneOffer->set_cast_method(ruled::v1::CAST_METHOD_MADNESS);
    zoneOffer->set_casting_permission_id(123456);
    zoneOffer->set_cost("{R}");
    apply(offerBatch);

    RuledPendingCast pending;
    auto &spell = pending.beginSpell();
    spell.handIndex = 70;
    spell.source = RuledCastSource::Exile;
    spell.castMethod = ruled::v1::CAST_METHOD_MADNESS;
    spell.castingPermissionId = 123456;
    spell.sourceZoneChangeGeneration = 6;
    spell.resolutionPromptId = state->resolutionPromptId;

    ruled::v1::RuledCommand proposal;
    auto *announcement = proposal.mutable_begin_spell_cast()->mutable_announcement();
    announcement->set_cast_method(ruled::v1::CAST_METHOD_MADNESS);
    announcement->set_casting_permission_id(123456);
    announcement->mutable_source()->set_exile_object_id(70);
    announcement->mutable_source()->set_expected_zone_change_generation(6);
    announcement->add_targets()->set_object_id(12);
    auto submitted = RuledPendingCast::submissionCommand(proposal, &spell, state);
    ASSERT_TRUE(submitted.has_value());
    ASSERT_TRUE(submitted->has_submit_resolution_choice());
    EXPECT_EQ(submitted->submit_resolution_choice().decision(), ruled::v1::RESOLUTION_CHOICE_DECISION_CAST_SPELL);
    EXPECT_EQ(submitted->submit_resolution_choice().resolution_prompt_id(), state->resolutionPromptId);
    EXPECT_EQ(submitted->submit_resolution_choice().spell_cast_announcement().SerializeAsString(),
              announcement->SerializeAsString());
    EXPECT_TRUE(proposal.has_begin_spell_cast());
    announcement->set_cast_method(ruled::v1::CAST_METHOD_NORMAL);
    ASSERT_TRUE(RuledPendingCast::submissionCommand(proposal, &spell, state).has_value());
    EXPECT_TRUE(RuledPendingCast::submissionCommand(proposal, &spell, state)->has_begin_spell_cast());
}

TEST(RuledDevCommandParserTest, PublicZoneChoiceFixtureHasNoCallerControlledFields)
{
    const auto r = parseLine(QStringLiteral("choice public-zones"));
    ASSERT_TRUE(r.ok) << r.error.toStdString();
    const auto &dev = r.command.dev_command();
    EXPECT_EQ(dev.target_player_id(), kMe);
    EXPECT_TRUE(dev.has_open_public_zone_search_fixture());
    EXPECT_FALSE(dev.has_put_card_in_zone());
    EXPECT_FALSE(dev.has_move_card());

    EXPECT_FALSE(parseLine(QStringLiteral("choice public-zones 2")).ok);
    EXPECT_FALSE(parseLine(QStringLiteral("choice public-zones hand Grizzly Bears")).ok);
}

TEST_F(RuledClientTest, PermissionCastConversionRequiresTheCapturedExactSpecialCastOffer)
{
    const auto makeOffer = [this](quint64 promptId) {
        ruled::v1::RuledEventBatch batch;
        auto *snapshot = batch.mutable_resolution_prompt_state();
        snapshot->set_active(true);
        snapshot->set_deciding_player_id(kLocalPlayer);
        snapshot->set_resolution_prompt_id(promptId);
        auto *rcr = batch.add_events()->mutable_resolution_choice_required();
        rcr->set_deciding_player_id(kLocalPlayer);
        rcr->set_choice_kind(ruled::v1::CHOICE_KIND_SPECIAL_CAST);
        rcr->set_resolution_prompt_id(promptId);
        rcr->add_candidate_object_ids(70);
        rcr->add_candidate_selectable(true);
        auto *zoneOffer = (*batch.mutable_legal_by_player())[kLocalPlayer].add_zone_cast_actions();
        zoneOffer->set_object_id(70);
        zoneOffer->set_source_zone(ruled::v1::CAST_SOURCE_ZONE_EXILE);
        zoneOffer->set_zone_change_generation(6);
        zoneOffer->set_cast_method(ruled::v1::CAST_METHOD_PERMISSION);
        zoneOffer->set_casting_permission_id(123456);
        zoneOffer->set_cost("{R}");
        applyRaw(batch);
    };
    makeOffer(9001);

    RuledPendingCast pending;
    auto &spell = pending.beginSpell();
    spell.handIndex = 70;
    spell.source = RuledCastSource::Exile;
    spell.castMethod = ruled::v1::CAST_METHOD_PERMISSION;
    spell.castingPermissionId = 123456;
    spell.sourceZoneChangeGeneration = 6;
    spell.resolutionPromptId = 9001;

    ruled::v1::RuledCommand proposal;
    auto *announcement = proposal.mutable_begin_spell_cast()->mutable_announcement();
    announcement->set_cast_method(ruled::v1::CAST_METHOD_PERMISSION);
    announcement->set_casting_permission_id(123456);
    announcement->mutable_source()->set_exile_object_id(70);
    announcement->mutable_source()->set_expected_zone_change_generation(6);
    auto submitted = RuledPendingCast::submissionCommand(proposal, &spell, state);
    ASSERT_TRUE(submitted.has_value());
    ASSERT_TRUE(submitted->has_submit_resolution_choice());
    EXPECT_EQ(submitted->submit_resolution_choice().resolution_prompt_id(), 9001u);

    // Reissue the same source, permission and generation under a new prompt ID. The captured
    // offer no longer matches, so the old pending cast cannot answer the replacement.
    makeOffer(9002);
    EXPECT_FALSE(RuledPendingCast::submissionCommand(proposal, &spell, state).has_value());

    spell.resolutionPromptId = 0;
    EXPECT_FALSE(RuledPendingCast::submissionCommand(proposal, &spell, state).has_value());
}

TEST_F(RuledClientTest, PermissionDoesNotDowngradeWhileAnActivePromptHasNoValidatedOffer)
{
    ruled::v1::RuledEventBatch active;
    active.mutable_resolution_prompt_state()->set_active(true);
    active.mutable_resolution_prompt_state()->set_deciding_player_id(kLocalPlayer);
    active.mutable_resolution_prompt_state()->set_resolution_prompt_id(9305);
    applyRaw(active);

    ASSERT_TRUE(state->resolutionPromptSnapshotValid);
    ASSERT_TRUE(state->resolutionPromptActive);
    ASSERT_EQ(state->resolutionPromptId, 0u);

    const auto applyPermissionOffer = [this](quint32 candidateOid) {
        constexpr quint64 promptId = 9305;
        ruled::v1::RuledEventBatch batch;
        auto *snapshot = batch.mutable_resolution_prompt_state();
        snapshot->set_active(true);
        snapshot->set_deciding_player_id(kLocalPlayer);
        snapshot->set_resolution_prompt_id(promptId);
        auto *rcr = batch.add_events()->mutable_resolution_choice_required();
        rcr->set_deciding_player_id(kLocalPlayer);
        rcr->set_choice_kind(ruled::v1::CHOICE_KIND_SPECIAL_CAST);
        rcr->set_resolution_prompt_id(promptId);
        rcr->add_candidate_object_ids(candidateOid);
        rcr->add_candidate_selectable(true);
        auto *offer = (*batch.mutable_legal_by_player())[kLocalPlayer].add_zone_cast_actions();
        offer->set_object_id(70);
        offer->set_source_zone(ruled::v1::CAST_SOURCE_ZONE_EXILE);
        offer->set_zone_change_generation(6);
        offer->set_cast_method(ruled::v1::CAST_METHOD_PERMISSION);
        offer->set_casting_permission_id(123457);
        offer->set_cost("{R}");
        applyRaw(batch);
    };
    applyPermissionOffer(70);
    ASSERT_TRUE(state->isCurrentResolutionPrompt(9305));
    applyPermissionOffer(71);
    EXPECT_TRUE(state->resolutionPromptSnapshotValid);
    EXPECT_TRUE(state->resolutionPromptActive);
    EXPECT_EQ(state->resolutionPromptDeciderPlayerId, kLocalPlayer);
    EXPECT_EQ(state->resolutionPromptId, 0u);
    EXPECT_TRUE(state->hasActiveLocalResolutionPrompt());

    RuledPendingCast pending;
    auto &spell = pending.beginSpell();
    spell.handIndex = 70;
    spell.source = RuledCastSource::Exile;
    spell.castMethod = ruled::v1::CAST_METHOD_PERMISSION;
    spell.castingPermissionId = 123457;
    spell.sourceZoneChangeGeneration = 6;

    ruled::v1::RuledCommand proposal;
    auto *announcement = proposal.mutable_begin_spell_cast()->mutable_announcement();
    announcement->set_cast_method(ruled::v1::CAST_METHOD_PERMISSION);
    announcement->set_casting_permission_id(123457);
    announcement->mutable_source()->set_exile_object_id(70);
    announcement->mutable_source()->set_expected_zone_change_generation(6);
    EXPECT_FALSE(RuledPendingCast::submissionCommand(proposal, &spell, state).has_value());
    EXPECT_TRUE(pending.resolutionChoiceBlocksSpell(*state));

    // The same conversion gate must preserve the engine-owned child activation exception.
    spell.engineTransactionId = 93;
    state->pendingSpellCast.emplace().set_transaction_id(93);
    EXPECT_FALSE(pending.resolutionChoiceBlocksSpell(*state));
    state->pendingSpellCast.reset();
    spell.engineTransactionId = 0;

    ruled::v1::RuledEventBatch inactive;
    inactive.mutable_resolution_prompt_state()->set_active(false);
    inactive.mutable_resolution_prompt_state()->set_deciding_player_id(0);
    applyRaw(inactive);
    ASSERT_FALSE(state->resolutionPromptActive);
    const auto ordinary = RuledPendingCast::submissionCommand(proposal, &spell, state);
    ASSERT_TRUE(ordinary.has_value());
    EXPECT_TRUE(ordinary->has_begin_spell_cast());
    EXPECT_FALSE(pending.resolutionChoiceBlocksSpell(*state));
}

TEST_F(RuledClientTest, PaymentReconciliationRebindsHandCostsAndRejectsMissingCards)
{
    RuledPendingCast pending;
    auto &spell = pending.beginSpell();
    spell.handIndex = 3;
    auto &offers = state->handActions[ruled::v1::HAND_ACTION_CAST_SPELL];
    offers.faceOptionsByIndex[3] = {RuledFaceOption{}};
    const auto key = RuledClientState::handCastActionKey(3, 0, ruled::v1::CAST_METHOD_NORMAL);
    RuledCostChoice discard;
    discard.costIndex = 4;
    discard.zone = RuledCostChoiceZone::Hand;
    discard.kind = RuledCostChoiceKind::Discard;
    discard.min = discard.max = 1;
    discard.candidateIds = {7};
    offers.costDataByCastKey[key].choices = {discard};
    // Physical card 91 has changed hand slot; the retained selection is not slot 91.
    state->ownedCardToEngineHandSlot[RuledClientState::makeOwnedCardKey(8, 91)] = 7;
    spell.costSelections = {{4, RuledCostChoiceZone::Hand, {91}}};
    EXPECT_TRUE(pending.reconcileSpellCosts(*state, 8));
    EXPECT_EQ(spell.costChoices.front().candidateIds, QSet<quint32>({7}));
    EXPECT_FALSE(pending.reconcileSpellCosts(*state, 19));
    state->ownedCardToEngineHandSlot.remove(RuledClientState::makeOwnedCardKey(8, 91));
    EXPECT_FALSE(pending.reconcileSpellCosts(*state, 8));
    EXPECT_TRUE(spell.valid); // The UI owns cancellation and its signals, not this decision helper.
}

TEST_F(RuledClientTest, SplitAttackOffersBlockerControlsOnlyToCurrentDefender)
{
    constexpr int attacker = 1;
    constexpr int firstDefender = 2;
    constexpr int secondDefender = 3;
    host.local = firstDefender;

    ruled::v1::RuledEventBatch declared;
    auto *attackers = declared.add_events()->mutable_attackers_declared();
    *attackers->add_assignments() = playerAttackAssignment(100, firstDefender);
    *attackers->add_assignments() = playerAttackAssignment(101, secondDefender);
    apply(declared);

    auto firstTurn = phaseBatch(ruled::v1::PHASE_ID_DECLARE_BLOCKERS, attacker);
    firstTurn.add_events()->mutable_priority_changed()->set_player_id(firstDefender);
    addLegalBlockPair((*firstTurn.mutable_legal_by_player())[firstDefender], 200, 100);
    apply(firstTurn);
    EXPECT_TRUE(state->localPlayerIsDefender());
    EXPECT_TRUE(state->isLegalBlockPair(200, 100));
    EXPECT_FALSE(state->isLegalBlockPair(200, 101));

    host.local = secondDefender;
    EXPECT_FALSE(state->localPlayerIsDefender());
    ruled::v1::RuledEventBatch secondTurn;
    secondTurn.add_events()->mutable_priority_changed()->set_player_id(secondDefender);
    // The next attacked defender gets controls when the engine offers a legal block;
    // defenders with no legal pairs are skipped by the engine before this priority change.
    addLegalBlockPair((*secondTurn.mutable_legal_by_player())[secondDefender], 201, 101);
    QSignalSpy combatChanged(state, &RuledClientState::combatStateChanged);
    apply(secondTurn);
    EXPECT_GT(combatChanged.count(), 0);
    EXPECT_TRUE(state->localPlayerIsDefender());
    EXPECT_TRUE(state->isLegalBlockPair(201, 101));

    host.local = firstDefender;
    EXPECT_FALSE(state->localPlayerIsDefender());
}

TEST_F(RuledClientTest, PaymentReconciliationKeepsTheMatchingAnnouncedSpellDuringPayment)
{
    RuledPendingCast pending;
    auto &spell = pending.beginSpell();
    spell.handIndex = 3;
    spell.engineTransactionId = 17;
    spell.stage = PendingRuledSpellCast::Stage::Paying;

    auto &engineCast = state->pendingSpellCast.emplace();
    engineCast.set_transaction_id(spell.engineTransactionId);

    EXPECT_FALSE(state->isHandCastActionLegal(spell.handIndex, spell.faceIndex, spell.castMethod));
    EXPECT_TRUE(pending.reconcileSpellCosts(*state, kLocalPlayer));

    engineCast.set_transaction_id(spell.engineTransactionId + 1);
    EXPECT_FALSE(pending.reconcileSpellCosts(*state, kLocalPlayer));
}

TEST_F(RuledClientTest, PaymentReconciliationPreservesIncompleteCohortsButRejectsChangedGenerations)
{
    RuledPendingCast pending;
    auto &spell = pending.beginSpell();
    spell.handIndex = 3;
    spell.waitingForCastCostObject = true;
    spell.activeCastCostOption = 5;
    RuledCastCostOption option;
    option.optionIndex = 5;
    option.kind = RuledCastCostOptionKind::TapPermanents;
    option.selectable = true;
    option.objectMin = 1;
    option.objectMax = 2;
    option.aggregateMinimum = 4;
    option.validPermanentIds = {900};
    option.validPermanentGenerations[900] = 12;
    option.candidateContributions[900] = 3;
    RuledCastCostGroup group;
    group.groupIndex = 2;
    group.options = {option};
    spell.castCostGroups = {group};
    RuledPendingCastCostSelection selection;
    selection.groupIndex = 2;
    selection.optionIndex = 5;
    selection.objectKind = RuledPendingCastCostSelection::ObjectKind::Permanent;
    selection.selectedObjectIds = {900};
    selection.selectedObjectGenerations[900] = 12;
    selection.selectedObjectContributions[900] = 3;
    spell.castCostSelections = {selection};
    auto &offers = state->handActions[ruled::v1::HAND_ACTION_CAST_SPELL];
    offers.faceOptionsByIndex[3] = {RuledFaceOption{}};
    const auto key = RuledClientState::handCastActionKey(3, 0, ruled::v1::CAST_METHOD_NORMAL);
    offers.costDataByCastKey[key].castCostGroups = {group};
    EXPECT_TRUE(pending.reconcileSpellCosts(*state, 8));
    EXPECT_FALSE(pending.pendingRuledCastCostObjectCanConfirm());
    EXPECT_TRUE(pending.pendingRuledCastCostObjectUsesExplicitConfirmation());
    EXPECT_TRUE(pending.pendingRuledSpellPromptText().contains(QStringLiteral("total power: 3 / 4")));
    spell.waitingForCastCostObject = false;
    EXPECT_FALSE(pending.reconcileSpellCosts(*state, 8));
    spell.waitingForCastCostObject = true;
    offers.costDataByCastKey[key].castCostGroups[0].options[0].validPermanentGenerations[900] = 13;
    EXPECT_FALSE(pending.reconcileSpellCosts(*state, 8));
    offers.costDataByCastKey[key].castCostGroups[0].options[0].validPermanentGenerations[900] = 12;
    offers.costDataByCastKey[key].castCostGroups[0].options[0].candidateContributions[900] = 4;
    EXPECT_FALSE(pending.reconcileSpellCosts(*state, 8));
}

TEST_F(RuledClientTest, SinglePermanentCastCostDoesNotRequireRedundantConfirmation)
{
    RuledPendingCast pending;
    auto &spell = pending.beginSpell();
    spell.waitingForCastCostObject = true;
    spell.activeCastCostOption = 5;

    RuledCastCostOption option;
    option.optionIndex = 5;
    option.kind = RuledCastCostOptionKind::SacrificePermanent;
    option.selectable = true;
    option.objectMin = 1;
    option.objectMax = 1;
    option.validPermanentIds = {900};

    RuledCastCostGroup group;
    group.groupIndex = 2;
    group.max = 1;
    group.options = {option};
    spell.castCostGroups = {group};

    RuledPendingCastCostSelection selection;
    selection.groupIndex = 2;
    selection.optionIndex = 5;
    selection.objectKind = RuledPendingCastCostSelection::ObjectKind::Permanent;
    selection.selectedObjectIds = {900};
    spell.castCostSelections = {selection};

    EXPECT_TRUE(pending.pendingRuledCastCostObjectCanConfirm());
    EXPECT_FALSE(pending.pendingRuledCastCostObjectUsesExplicitConfirmation());
}

TEST(RuledPendingPaymentTest, PaymentPromptKeepsTheExistingTextAndTargetingPrecedence)
{
    RuledPendingCast pending;
    auto &spell = pending.beginSpell();
    spell.cardName = QStringLiteral("Example");
    spell.remainingCost = RuledPendingCast::parseSimpleManaCost("{2}{U}");
    EXPECT_EQ(pending.pendingRuledSpellPromptText(),
              QStringLiteral("Pay mana for Example: {2}{U} remaining (click mana counters)."));
    spell.waitingForTarget = true;
    EXPECT_TRUE(pending.pendingRuledSpellPromptText().isEmpty());
    spell.waitingForTarget = false;
    spell.inDamageAllocationMode = true;
    EXPECT_TRUE(pending.pendingRuledSpellPromptText().isEmpty());
    auto &ability = pending.beginAbility();
    ability.cardName = QStringLiteral("Example");
    ability.remainingCost = RuledPendingCast::parseSimpleManaCost("{2}{U}");
    EXPECT_TRUE(pending.pendingRuledAbilityPromptText().isEmpty());
    ability.waitingForMana = true;
    EXPECT_EQ(pending.pendingRuledAbilityPromptText(),
              QStringLiteral("Pay mana for Example: {2}{U} remaining (click mana counters)."));
}

TEST_F(RuledClientTest, OptionalHandPickDeclinesWithEmptySelectionAndClosesThePicker)
{
    ruled::v1::RuledEventBatch batch;
    auto *choice = batch.add_events()->mutable_resolution_choice_required();
    choice->set_deciding_player_id(kLocalPlayer);
    choice->set_choice_kind(ruled::v1::CHOICE_KIND_HAND_CARDS);
    choice->set_min(0);
    choice->set_max(1);
    choice->add_candidate_object_ids(101);
    choice->add_candidate_server_card_ids(10);
    apply(batch);
    EXPECT_TRUE(state->pendingClickChoiceMayDecline());
    QSignalSpy closed(state, &RuledClientState::resolutionHandPickUiChanged);
    state->declinePendingClickChoice();
    ASSERT_EQ(host.sentCommands.size(), 1);
    EXPECT_EQ(host.sentCommands[0].submit_resolution_choice().chosen_object_ids_size(), 0);
    EXPECT_FALSE(state->isResolutionHandPickActive());
    ASSERT_GT(closed.count(), 0);
    EXPECT_EQ(closed.last().at(0).toInt(), -1);
}

TEST_F(RuledClientTest, PermanentChoiceRetainsEngineBoundsForFranticSearch)
{
    ruled::v1::RuledEventBatch batch;
    auto *choice = batch.add_events()->mutable_resolution_choice_required();
    choice->set_deciding_player_id(kLocalPlayer);
    choice->set_choice_kind(ruled::v1::CHOICE_KIND_PERMANENT_OBJECTS);
    choice->set_min(0);
    choice->set_max(3);
    choice->set_prompt_text("Choose 0–3 lands.");
    for (quint32 oid : {100u, 101u, 102u, 103u})
        choice->add_candidate_object_ids(oid);
    apply(batch);
    ASSERT_TRUE(state->hasPendingChoiceOfKind(RuledClientState::ChoiceKind::PermanentChoice));
    EXPECT_EQ(state->pendingChoice->min, 0);
    EXPECT_EQ(state->pendingChoice->max, 3);
    EXPECT_EQ(state->pendingChoice->candidateOids.size(), 4);
}

TEST_F(RuledClientTest, FranticLandClicksToggleUniqueBoundedSelectionUntilConfirmation)
{
    RuledClientState::RuledPendingChoice choice;
    choice.kind = RuledClientState::ChoiceKind::PermanentChoice;
    choice.min = 0;
    choice.max = 3;
    choice.candidateOids = {100, 101, 102, 103};
    stageChoiceForTest(choice);
    QSignalSpy repaint(state, &RuledClientState::combatStateChanged);
    state->submitPendingChoiceObject(999);
    EXPECT_TRUE(state->pendingChoice->selectedObjectOids.isEmpty());
    for (quint32 oid : {100u, 101u, 102u, 103u})
        state->submitPendingChoiceObject(oid);
    EXPECT_EQ(state->pendingChoice->selectedObjectOids, QVector<quint32>({100, 101, 102}));
    EXPECT_TRUE(host.sentCommands.isEmpty());
    state->submitPendingChoiceObject(101);
    EXPECT_FALSE(state->isPermanentChoiceObjectSelected(101));
    state->submitPendingChoiceObject(103);
    EXPECT_EQ(state->pendingChoice->selectedObjectOids, QVector<quint32>({100, 102, 103}));
    EXPECT_EQ(repaint.count(), 5);
    state->submitPermanentChoiceObjects();
    ASSERT_EQ(host.sentCommands.size(), 1);
    const auto &submitted = host.sentCommands.last().submit_resolution_choice();
    ASSERT_EQ(submitted.chosen_object_ids_size(), 3);
    EXPECT_EQ(submitted.chosen_object_ids(0), 100u);
    EXPECT_EQ(submitted.chosen_object_ids(1), 102u);
    EXPECT_EQ(submitted.chosen_object_ids(2), 103u);
    EXPECT_TRUE(state->isPermanentChoiceObjectSelected(103));
    state->submitPendingChoiceObject(100);
    state->submitPermanentChoiceObjects();
    EXPECT_EQ(host.sentCommands.size(), 1);
    host.answerPendingAck(true);
    EXPECT_FALSE(state->hasPendingChoiceOfKind(RuledClientState::ChoiceKind::PermanentChoice));
    EXPECT_FALSE(state->isPermanentChoiceObjectSelected(103));
}

TEST_F(RuledClientTest, FranticZeroOneThreeRejectedChoicesRetainSelectionForRetry)
{
    for (int count : {0, 1, 3}) {
        RuledClientState::RuledPendingChoice choice;
        choice.kind = RuledClientState::ChoiceKind::PermanentChoice;
        choice.min = 0;
        choice.max = 3;
        choice.candidateOids = {100, 101, 102};
        stageChoiceForTest(choice);
        for (int index = 0; index < count; ++index)
            state->submitPendingChoiceObject(100 + index);
        const auto selected = state->pendingChoice->selectedObjectOids;
        const quint64 revision = state->pendingChoiceRevision;
        host.sentCommands.clear();
        state->submitPermanentChoiceObjects();
        ASSERT_EQ(host.sentCommands.size(), 1);
        EXPECT_EQ(host.sentCommands.last().submit_resolution_choice().chosen_object_ids_size(), count);
        host.answerPendingAck(false);
        ASSERT_TRUE(state->hasPermanentChoiceCohort());
        EXPECT_EQ(state->pendingChoiceRevision, revision);
        EXPECT_EQ(state->pendingChoice->selectedObjectOids, selected);
        EXPECT_FALSE(state->pendingChoice->permanentChoiceSubmitting);
        state->submitPermanentChoiceObjects();
        EXPECT_EQ(host.sentCommands.size(), 2);
        host.answerPendingAck(true);
    }
}

TEST_F(RuledClientTest, FranticAcknowledgementCannotReplaceANewerEngineChoice)
{
    for (bool accepted : {false, true}) {
        RuledClientState::RuledPendingChoice old;
        old.kind = RuledClientState::ChoiceKind::PermanentChoice;
        old.max = 3;
        old.candidateOids = {100};
        stageChoiceForTest(old);
        state->submitPermanentChoiceObjects();
        RuledClientState::RuledPendingChoice next;
        next.kind = RuledClientState::ChoiceKind::PermanentChoice;
        next.min = 1;
        next.max = 2;
        next.promptText = "A newer engine choice";
        next.candidateOids = {200, 201};
        stageChoiceForTest(next);
        state->submitPendingChoiceObject(200);
        const quint64 revision = state->pendingChoiceRevision;
        host.answerPendingAck(accepted);
        ASSERT_TRUE(state->hasPermanentChoiceCohort());
        EXPECT_EQ(state->pendingChoiceRevision, revision);
        EXPECT_EQ(state->pendingChoice->promptText, next.promptText);
        EXPECT_EQ(state->pendingChoice->selectedObjectOids, QVector<quint32>({200}));
        EXPECT_FALSE(state->pendingChoice->permanentChoiceSubmitting);
    }
}

TEST_F(RuledClientTest, PermanentCohortConfirmationHonorsMandatoryMinimum)
{
    RuledClientState::RuledPendingChoice choice;
    choice.kind = RuledClientState::ChoiceKind::PermanentChoice;
    choice.min = 2;
    choice.max = 3;
    choice.candidateOids = {100, 101};
    stageChoiceForTest(choice);
    state->submitPermanentChoiceObjects();
    state->submitPendingChoiceObject(100);
    state->submitPermanentChoiceObjects();
    EXPECT_TRUE(host.sentCommands.isEmpty());
    state->submitPendingChoiceObject(101);
    state->submitPermanentChoiceObjects();
    ASSERT_EQ(host.sentCommands.size(), 1);
    host.answerPendingAck(true);
}

TEST_F(RuledClientTest, PermanentOneOfOneRetainsImmediateClickSubmission)
{
    for (int minimum : {0, 1}) {
        ruled::v1::RuledEventBatch batch;
        auto *choice = batch.add_events()->mutable_resolution_choice_required();
        choice->set_deciding_player_id(kLocalPlayer);
        choice->set_choice_kind(ruled::v1::CHOICE_KIND_PERMANENT_OBJECTS);
        choice->set_min(minimum);
        choice->set_max(1);
        choice->add_candidate_object_ids(101);
        apply(batch);
        host.sentCommands.clear();
        EXPECT_FALSE(state->hasPermanentChoiceCohort());
        state->submitPendingChoiceObject(101);
        ASSERT_EQ(host.sentCommands.size(), 1);
        ASSERT_EQ(host.sentCommands.last().submit_resolution_choice().chosen_object_ids_size(), 1);
        EXPECT_EQ(host.sentCommands.last().submit_resolution_choice().chosen_object_ids(0), 101u);
        EXPECT_FALSE(state->hasPendingChoiceOfKind(RuledClientState::ChoiceKind::PermanentChoice));
    }
}

TEST_F(RuledClientTest, OptionalPermanentChoiceDeclinesButMandatoryChoiceDoesNot)
{
    for (int minimum : {0, 1}) {
        ruled::v1::RuledEventBatch batch;
        auto *choice = batch.add_events()->mutable_resolution_choice_required();
        choice->set_deciding_player_id(kLocalPlayer);
        choice->set_choice_kind(ruled::v1::CHOICE_KIND_PERMANENT_OBJECTS);
        choice->set_min(minimum);
        choice->set_max(1);
        choice->add_candidate_object_ids(101);
        apply(batch);
        host.sentCommands.clear();
        state->declinePendingClickChoice();
        EXPECT_EQ(host.sentCommands.size(), minimum == 0 ? 1 : 0);
        EXPECT_EQ(state->hasPendingChoiceOfKind(RuledClientState::ChoiceKind::PermanentChoice), minimum != 0);
    }
}

TEST(RuledPendingCastTest, DeclineOptionalCostClearsOnlyItsStagedPaymentsAndContinuesCasting)
{
    RuledPendingCast pending;
    auto &spell = pending.beginSpell();
    RuledCastCostGroup group;
    group.groupIndex = 4;
    group.min = 0;
    group.max = 2;
    RuledCastCostOption mana;
    mana.optionIndex = 7;
    mana.kind = RuledCastCostOptionKind::Mana;
    mana.additionalManaCost = "{2}{R}";
    group.options = {mana};
    spell.castCostGroups = {group};
    spell.remainingCost = RuledPendingCast::parseSimpleManaCost("{5}{R}{U}");
    spell.castCostGenericReduction = 3;
    spell.castCostSelections = {{9, 1, RuledPendingCastCostSelection::ObjectKind::None, 0, 0, 1},
                                {4, 7, RuledPendingCastCostSelection::ObjectKind::None, 0, 0, 2}};
    spell.waitingForCastCostObject = true;
    spell.activeCastCostOption = 8;
    EXPECT_TRUE(pending.pendingRuledCastCostGroupIsOptional());
    EXPECT_TRUE(pending.declineCastCostGroup());
    EXPECT_TRUE(spell.valid);
    EXPECT_EQ(spell.nextCastCostGroup, 1);
    EXPECT_FALSE(spell.waitingForCastCostObject);
    EXPECT_EQ(spell.activeCastCostOption, -1);
    EXPECT_EQ(spell.castCostGenericReduction, 1);
    EXPECT_EQ(RuledPendingCast::totalRemainingForCost(spell.remainingCost, spell.flexPips), 4);
    ASSERT_EQ(spell.castCostSelections.size(), 1);
    EXPECT_EQ(spell.castCostSelections.front().groupIndex, 9);
    EXPECT_FALSE(pending.declineCastCostGroup());
}

TEST(RuledPendingCastTest, DeclineCannotBypassMandatoryOrModeLinkedCastCosts)
{
    for (bool linked : {false, true}) {
        RuledPendingCast pending;
        auto &spell = pending.beginSpell();
        RuledCastCostGroup group;
        group.groupIndex = 4;
        group.min = linked ? 0 : 1;
        group.max = 1;
        spell.castCostGroups = {group};
        if (linked)
            spell.selectedModeLinkedCastCosts.append({4, 0});
        EXPECT_FALSE(pending.pendingRuledCastCostGroupIsOptional());
        EXPECT_FALSE(pending.declineCastCostGroup());
        EXPECT_TRUE(spell.valid);
        EXPECT_EQ(spell.nextCastCostGroup, 0);
    }
}

TEST(RuledPendingTargetProgressionTest, PerTargetDamageSkipsAllocationAndDisplaysEachAmount)
{
    RuledPendingCast pending;
    auto &spell = pending.spell;
    spell.valid = true;
    spell.isDamageTargets = true;
    spell.damageDivision = ruled::v1::DAMAGE_DIVISION_PER_TARGET;
    spell.fixedDamage = 1;
    spell.minTargets = 1;
    spell.maxTargets = 2;
    spell.selectedTargetOids = {101, 205};
    EXPECT_EQ(pending.effectiveDamageTargetsMax(), 2);
    EXPECT_EQ(pending.prepareSpellDamageAllocation(), RuledPendingCast::DamageAllocationStep::Ready);
    EXPECT_FALSE(spell.inDamageAllocationMode);
    EXPECT_EQ(spell.selectedTargetDamages, QVector<quint32>({0, 0}));
    EXPECT_EQ(pending.spellDamageAllocationForOid(101), 1);
    EXPECT_EQ(pending.spellDamageAllocationForOid(205), 1);
    EXPECT_EQ(pending.spellDamageAllocationForOid(999), 0);
    spell.minTargets = 0;
    spell.selectedTargetOids.clear();
    EXPECT_EQ(pending.prepareSpellDamageAllocation(), RuledPendingCast::DamageAllocationStep::Ready);
    EXPECT_TRUE(spell.selectedTargetDamages.isEmpty());
    pending.clearSpell();
    EXPECT_FALSE(pending.isSpellDamageAllocationDisplayActive());
    EXPECT_EQ(spell.damageDivision, ruled::v1::DAMAGE_DIVISION_CHOOSE_AT_CAST);
}

namespace
{
struct GraveyardOrderCard
{
    int id;
    QString name;
    QString annotation;
    int getId() const
    {
        return id;
    }
};

ServerInfo_Zone graveyardOrderSnapshot(std::initializer_list<int> ids)
{
    ServerInfo_Zone info;
    info.set_name(ZoneNames::GRAVE);
    info.set_type(ServerInfo_Zone::PublicZone);
    info.set_card_count(static_cast<int>(ids.size()));
    for (int id : ids) {
        info.add_card_list()->set_id(id);
    }
    return info;
}
} // namespace

TEST(RuledPublicZoneOrderTest, OwnerPermutationPreservesDuplicateNamesPointersAndAnnotations)
{
    GraveyardOrderCard first{40, "Grizzly Bears", "first physical card"};
    GraveyardOrderCard second{41, "Grizzly Bears", "second physical card"};
    GraveyardOrderCard hill{42, "Hill Giant", "chosen oldest arrival"};
    GraveyardOrderCard older{44, "Island", "older graveyard prefix"};
    const QList<GraveyardOrderCard *> original{&hill, &second, &first, &older};
    const auto info = graveyardOrderSnapshot({41, 40, 42, 44});
    const auto ordered = RuledPublicZoneOrderDetail::snapshot(true, true, QStringLiteral("grave"), original, info);
    ASSERT_TRUE(ordered);
    EXPECT_EQ(*ordered, QList<GraveyardOrderCard *>({&second, &first, &hill, &older}));
    EXPECT_EQ(original, QList<GraveyardOrderCard *>({&hill, &second, &first, &older}));
    EXPECT_EQ(second.annotation, QStringLiteral("second physical card"));
    EXPECT_EQ(first.id, 40);
    const auto repeated = RuledPublicZoneOrderDetail::snapshot(true, true, QStringLiteral("grave"), *ordered, info);
    ASSERT_TRUE(repeated);
    EXPECT_EQ(*repeated, *ordered);
}

TEST(RuledPublicZoneOrderTest, MalformedOrPartialSnapshotsAreAtomic)
{
    GraveyardOrderCard first{40, "Grizzly Bears", "retained"};
    GraveyardOrderCard second{41, "Grizzly Bears", "retained"};
    const QList<GraveyardOrderCard *> original{&first, &second};
    QList<ServerInfo_Zone> invalid;
    invalid.append(graveyardOrderSnapshot({40}));
    invalid.append(graveyardOrderSnapshot({40, 40}));
    invalid.append(graveyardOrderSnapshot({40, 99}));
    auto countMismatch = graveyardOrderSnapshot({41, 40});
    countMismatch.set_card_count(3);
    invalid.append(countMismatch);
    auto missingCount = graveyardOrderSnapshot({41, 40});
    missingCount.clear_card_count();
    invalid.append(missingCount);
    auto missingId = graveyardOrderSnapshot({41, 40});
    missingId.mutable_card_list(0)->clear_id();
    invalid.append(missingId);
    for (const auto &info : invalid) {
        EXPECT_FALSE(RuledPublicZoneOrderDetail::snapshot(true, true, QStringLiteral("grave"), original, info));
        EXPECT_EQ(original, QList<GraveyardOrderCard *>({&first, &second}));
    }
    const auto valid = graveyardOrderSnapshot({41, 40});
    EXPECT_FALSE(RuledPublicZoneOrderDetail::snapshot(true, true, QStringLiteral("grave"),
                                                      QList<GraveyardOrderCard *>({&first, &first}), valid));
    EXPECT_FALSE(RuledPublicZoneOrderDetail::snapshot(true, true, QStringLiteral("grave"),
                                                      QList<GraveyardOrderCard *>({&first, nullptr}), valid));
}

TEST(RuledPublicZoneOrderTest, FullAndPartialTrackedViewsPreserveCopiesOrInvalidateChangedMembership)
{
    GraveyardOrderCard first{40, "Grizzly Bears", "view first"};
    GraveyardOrderCard second{41, "Grizzly Bears", "view second"};
    GraveyardOrderCard hill{42, "Hill Giant", "view hill"};
    const QList<int> originOrder{41, 40, 42};
    const auto full =
        RuledPublicZoneOrderDetail::view(QList<GraveyardOrderCard *>({&hill, &first, &second}), originOrder, -1);
    ASSERT_TRUE(full);
    EXPECT_EQ(*full, QList<GraveyardOrderCard *>({&second, &first, &hill}));
    const auto partial =
        RuledPublicZoneOrderDetail::view(QList<GraveyardOrderCard *>({&first, &second}), originOrder, 2);
    ASSERT_TRUE(partial);
    EXPECT_EQ(*partial, QList<GraveyardOrderCard *>({&second, &first}));
    EXPECT_FALSE(RuledPublicZoneOrderDetail::view(QList<GraveyardOrderCard *>({&hill, &first}), originOrder, 2));
    EXPECT_FALSE(
        RuledPublicZoneOrderDetail::view(QList<GraveyardOrderCard *>({&first, &first, &hill}), originOrder, -1));
    EXPECT_EQ(first.annotation, QStringLiteral("view first"));
}

TEST(RuledPublicZoneOrderTest, EmptyCompletePublicGraveyardIsValid)
{
    const auto ordered = RuledPublicZoneOrderDetail::snapshot(
        true, true, QStringLiteral("grave"), QList<GraveyardOrderCard *>(), graveyardOrderSnapshot({}));
    ASSERT_TRUE(ordered);
    EXPECT_TRUE(ordered->isEmpty());
    const auto view = RuledPublicZoneOrderDetail::view(QList<GraveyardOrderCard *>(), QList<int>(), -1);
    ASSERT_TRUE(view);
    EXPECT_TRUE(view->isEmpty());
}

TEST(RuledPublicZoneOrderTest, FreeformHiddenAndOtherZonesRetainExistingPaths)
{
    GraveyardOrderCard synthetic{999, "Triggered ability", "synthetic stack object"};
    const QList<GraveyardOrderCard *> cards{&synthetic};
    const auto info = graveyardOrderSnapshot({999});
    EXPECT_FALSE(RuledPublicZoneOrderDetail::snapshot(false, true, QStringLiteral("grave"), cards, info));
    EXPECT_FALSE(RuledPublicZoneOrderDetail::snapshot(true, false, QStringLiteral("grave"), cards, info));
    for (const char *name : {"stack", "exile", "command", "table", "hand", "deck"}) {
        auto other = info;
        other.set_name(name);
        EXPECT_FALSE(RuledPublicZoneOrderDetail::snapshot(true, true, QString::fromLatin1(name), cards, other));
    }
    auto hidden = info;
    hidden.set_type(ServerInfo_Zone::HiddenZone);
    EXPECT_FALSE(RuledPublicZoneOrderDetail::snapshot(true, true, QStringLiteral("grave"), cards, hidden));
    hidden.set_type(ServerInfo_Zone::PrivateZone);
    EXPECT_FALSE(RuledPublicZoneOrderDetail::snapshot(true, true, QStringLiteral("grave"), cards, hidden));
    auto coordinates = info;
    coordinates.set_with_coords(true);
    EXPECT_FALSE(RuledPublicZoneOrderDetail::snapshot(true, true, QStringLiteral("grave"), cards, coordinates));
    EXPECT_EQ(synthetic.annotation, QStringLiteral("synthetic stack object"));
}

TEST_F(RuledClientTest, PrivateCardLookViewerCheckLeavesPendingPromptUntouched)
{
    host.local = kOpponent;
    QSignalSpy privateLook(state, &RuledClientState::privateCardLookReceived);
    QSignalSpy timeline(state, &RuledClientState::engineTimeline);
    const QString secretName = QStringLiteral("Fire & Ice / 龍 50%: \"Choice\"");

    ruled::v1::RuledEventBatch batch;
    auto *log = batch.add_events()->mutable_log();
    log->set_text((QStringLiteral("fallback secret: ") + secretName).toStdString());
    auto *look = log->mutable_private_card_look();
    look->set_viewer_player_id(kOpponent);
    look->add_external_card_names(secretName.toStdString());
    auto *promptState = batch.mutable_resolution_prompt_state();
    promptState->set_active(true);
    promptState->set_deciding_player_id(kLocalPlayer);
    auto *choice = batch.add_events()->mutable_resolution_choice_required();
    choice->set_deciding_player_id(kLocalPlayer);
    choice->set_choice_kind(ruled::v1::CHOICE_KIND_PRIVATE_REPLACEMENT);
    choice->set_prompt_text("The other player is choosing.");

    applyRaw(batch);

    ASSERT_EQ(privateLook.size(), 1);
    ASSERT_EQ(privateLook.first().size(), 1);
    EXPECT_EQ(privateLook.first().first().toStringList(), QStringList{secretName});
    ASSERT_EQ(timeline.size(), 1);
    EXPECT_TRUE(timeline.first().first().toString().trimmed().isEmpty());
    EXPECT_TRUE(state->resolutionPromptActive);
    EXPECT_EQ(state->resolutionPromptDeciderPlayerId, kLocalPlayer);
    EXPECT_EQ(state->resolutionPromptId, 0u);
    EXPECT_EQ(state->choiceWaitingPlayer(), kLocalPlayer);
    EXPECT_TRUE(host.sentCommands.isEmpty());

    privateLook.clear();
    timeline.clear();
    ruled::v1::RuledEventBatch wrongViewer = batch;
    wrongViewer.mutable_events(0)->mutable_log()->mutable_private_card_look()->set_viewer_player_id(kLocalPlayer);
    applyRaw(wrongViewer);
    ASSERT_EQ(privateLook.size(), 0);
    ASSERT_EQ(timeline.size(), 1);
    EXPECT_TRUE(timeline.first().first().toString().trimmed().isEmpty());
    EXPECT_TRUE(state->resolutionPromptActive);
    EXPECT_EQ(state->resolutionPromptDeciderPlayerId, kLocalPlayer);
    EXPECT_EQ(state->resolutionPromptId, 0u);
    EXPECT_TRUE(host.sentCommands.isEmpty());

    privateLook.clear();
    timeline.clear();
    ruled::v1::RuledEventBatch malformed = batch;
    malformed.mutable_events(0)->mutable_log()->mutable_private_card_look()->set_external_card_names(
        0, std::string(513, 'X'));
    applyRaw(malformed);
    ASSERT_EQ(privateLook.size(), 0);
    ASSERT_EQ(timeline.size(), 1);
    EXPECT_TRUE(timeline.first().first().toString().trimmed().isEmpty());
    EXPECT_TRUE(state->resolutionPromptActive);
    EXPECT_EQ(state->resolutionPromptDeciderPlayerId, kLocalPlayer);
    EXPECT_EQ(state->resolutionPromptId, 0u);
    EXPECT_TRUE(host.sentCommands.isEmpty());
}

TEST(RuledPrivateCardLookTest, EncodedHrefResolvesForPreviewAndKeepsRawHrefBehavior)
{
    const QString cardName = QStringLiteral("Fire & Ice / 龍 50%: \"Choice\"");
    const QString html = ruledPrivateCardLookMessageHtml({cardName});
    EXPECT_NE(html.indexOf(QStringLiteral("Fire &amp; Ice")), -1);
    EXPECT_NE(html.indexOf(QStringLiteral("50%")), -1);
    EXPECT_EQ(html.indexOf(QStringLiteral("\"Choice\"")), -1);
    EXPECT_NE(html.indexOf(QStringLiteral("&quot;Choice&quot;")), -1);

    const QRegularExpression hrefExpression(QStringLiteral("href=\\\"([^\\\"]+)\\\""));
    const auto match = hrefExpression.match(html);
    ASSERT_TRUE(match.hasMatch());
    QString resolvedName;
    EXPECT_EQ(ruledResolveCardPreviewHref(match.captured(1), &resolvedName), RuledCardPreviewHrefKind::PrivateLookCard);
    EXPECT_EQ(resolvedName, cardName);

    EXPECT_EQ(ruledResolveCardPreviewHref(QStringLiteral("card://Legacy Raw Name"), &resolvedName),
              RuledCardPreviewHrefKind::LegacyCard);
    EXPECT_EQ(resolvedName, QStringLiteral("Legacy Raw Name"));
    EXPECT_EQ(ruledResolveCardPreviewHref(QStringLiteral("card://ruled-private-look/%GG"), &resolvedName),
              RuledCardPreviewHrefKind::InvalidPrivateLookCard);
}
