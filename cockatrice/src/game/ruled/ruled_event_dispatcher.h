/**
 * @file ruled_event_dispatcher.h
 * @ingroup GameLogic
 * @brief Applies a `ruled::v1::RuledEventBatch` to `RuledClientState`.
 *
 * One private method per engine event kind: adding a mechanic means adding a method plus its
 * `has_*()` line in `processBatch`, not growing an inline block inside an upstream switch.
 * The dispatcher is the ONLY writer of the engine-authoritative fields on `RuledClientState`;
 * local click staging is mutated by the state's own toggle methods.
 *
 * Like the state, it depends on nothing from the Qt game UI — everything reaches the client
 * through `RuledClientHost`, which is what makes `tests/ruled_client_tests/` possible.
 */

#ifndef COCKATRICE_RULED_EVENT_DISPATCHER_H
#define COCKATRICE_RULED_EVENT_DISPATCHER_H

#include <QObject>
#include <QString>
#include <string>

namespace ruled::v1
{
class AttackersDeclared;
class AttackersPreview;
class BattlefieldObjectMap;
class BlockersDeclared;
class BlockersPreview;
class CombatDamageAssigned;
class CreaturesRemovedFromCombat;
class ExileObjectMap;
class GraveyardObjectMap;
class HandSlotMap;
class LegalActions;
class LifeChanged;
class PhaseChanged;
class ResolutionChoiceRequired;
class RuledEventBatch;
class StackPushed;
class StackResolved;
class TriggerNeedsTarget;
class TriggerOrderRequired;
class ZoneViewSync;
} // namespace ruled::v1

class RuledClientHost;
class RuledClientState;

class RuledEventDispatcher : public QObject
{
    Q_OBJECT

public:
    RuledEventDispatcher(RuledClientState *state, RuledClientHost *host, QObject *parent = nullptr);

    /// Parse a serialized `RuledEventBatch` and apply it. Returns false when the payload does not
    /// parse; in that case the pre-batch reset has already run (matching legacy behaviour).
    bool processPayload(const std::string &payload);

    /// Apply an already-parsed batch. Exposed for tests.
    void processBatch(const ruled::v1::RuledEventBatch &batch);

private:
    /// Per-batch accumulators: what to emit once every event in the batch has been applied.
    struct BatchContext
    {
        QString timeline;
        QString promptFeed;
        bool combatStateDirty = false;
        bool battlefieldMapDirty = false;
        bool stackTrackingDirty = false;
        /// CR 603.3b: the ordering prompt changed somewhere in this batch. Emitted once at the end
        /// from the final state rather than per event, because a single batch both places the
        /// trigger just picked (which ends the old prompt) and raises the next one — emitting on
        /// each would tear the popup down and rebuild it, losing its position mid-choice.
        bool triggerOrderDirty = false;
    };

    /// Legal actions and the opening-UI kind are rebuilt from scratch every batch; clear them
    /// before applying so a batch without a `legal_by_player` entry leaves nothing stale.
    void resetPerBatchLegalActions();

    void applyPhaseChanged(const ruled::v1::PhaseChanged &pc, BatchContext &ctx);
    void applyStackPushed(const ruled::v1::StackPushed &sp, BatchContext &ctx);
    void applyStackResolved(const ruled::v1::StackResolved &sr, BatchContext &ctx);
    void applyTriggerNeedsTarget(const ruled::v1::TriggerNeedsTarget &tnt, BatchContext &ctx);
    void applyTriggerOrderRequired(const ruled::v1::TriggerOrderRequired &tor, BatchContext &ctx);
    void applyResolutionChoiceRequired(const ruled::v1::ResolutionChoiceRequired &rcr, BatchContext &ctx);
    void applyBattlefieldObjectMap(const ruled::v1::BattlefieldObjectMap &map, BatchContext &ctx);
    void applyHandSlotMap(const ruled::v1::HandSlotMap &map);
    void applyGraveyardObjectMap(const ruled::v1::GraveyardObjectMap &map);
    void applyExileObjectMap(const ruled::v1::ExileObjectMap &map);
    void applyZoneView(const ruled::v1::ZoneViewSync &view, BatchContext &ctx);
    void applyAttackersDeclared(const ruled::v1::AttackersDeclared &ad, BatchContext &ctx);
    void applyAttackersPreview(const ruled::v1::AttackersPreview &ap, BatchContext &ctx);
    void applyBlockersDeclared(const ruled::v1::BlockersDeclared &bd, BatchContext &ctx);
    void applyBlockersPreview(const ruled::v1::BlockersPreview &bp, BatchContext &ctx);
    void applyCombatDamageAssigned(const ruled::v1::CombatDamageAssigned &cda, BatchContext &ctx);
    void applyRemovedFromCombat(const ruled::v1::CreaturesRemovedFromCombat &rfc, BatchContext &ctx);
    void applyLifeChanged(const ruled::v1::LifeChanged &lc, BatchContext &ctx);

    /// The local player's `LegalActions` entry: hand-action label parsing, targeting tables,
    /// opening-UI kind, and the must-attack / must-block requirement sets.
    void applyLegalActions(const ruled::v1::LegalActions &actions, BatchContext &ctx);
    /// No `legal_by_player` entry for us this batch (e.g. a Servatrice-synthesized combat
    /// preview echo). Deliberately does NOT clear the requirement sets — see the body.
    void applyNoLegalActions();

    /// Emits everything the batch accumulated, in the legacy order.
    void finishBatch(BatchContext &ctx);

    RuledClientState *state;
    RuledClientHost *host;
};

#endif // COCKATRICE_RULED_EVENT_DISPATCHER_H
