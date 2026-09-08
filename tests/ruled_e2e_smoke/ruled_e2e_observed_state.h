#ifndef RULED_E2E_OBSERVED_STATE_H
#define RULED_E2E_OBSERVED_STATE_H
#include "ruled_e2e_common.h"

#include <functional>
namespace ruled_e2e
{
// One recipient's ordered observations. No scenario milestones or command submission.
struct ObservedState
{
    struct Permanent
    {
        QString cardId;
        quint32 oid = 0;
        bool tapped = false;
        bool creature = false;
        bool planeswalker = false;
        bool battle = false;
        bool sick = false;
        bool haste = false;
        bool reach = false;
        bool flying = false;
        bool indestructible = false;
        int power = 0;
        int toughness = 0;
        int faceIndex = 0;
        bool faceDown = false;
        quint64 generation = 0;
        int loyalty = -1;
        int defense = -1;
        QString countersAnnotation;
        int battleProtector = -1;
        bool firstAbilityActivatable = false;
        std::vector<quint32> abilityIndices;
        quint32 attachmentObjectId = 0;
        int attachmentPlayerId = -1;
        std::array<bool, 2> roomDoors{false, false};
        int roomDoorCount = 0;
    };
    struct Pool
    {
        int w = 0, u = 0, b = 0, r = 0, g = 0, c = 0;
        int total() const
        {
            return w + u + b + r + g + c;
        }
    };
    int paymentPreviewCount = 0;
    ruled::v1::PaymentPreview paymentPreview;
    int myId = -1;
    // This fixture has two clients; discover the other actual seat ID without arithmetic.
    int oppId = -1;
    bool gameStarted = false;
    quint64 stateVersion = 0;
    ruled::v1::PhaseId phase = ruled::v1::PHASE_ID_UNSPECIFIED;
    int activePlayer = -1;
    int priorityPlayer = -1;
    int stackDepth = 0;
    std::set<quint32> counteredStackObjectIds;
    QStringList labels;
    std::map<int, int> handSizeByPlayer;
    std::map<int, int> lifeByPlayer;
    std::map<int, std::vector<Permanent>> battlefieldByPlayer;
    std::vector<ruled::v1::AttackAssignment> latestAttackPreviewAssignments;
    std::vector<ruled::v1::AttackAssignment> latestDeclaredAttackAssignments;
    std::vector<ruled::v1::AttackAssignment> latestAddedAttackAssignments;
    std::set<int> physicallyTappedCardIds;
    std::set<int> physicallyAttackingCardIds;
    std::map<std::pair<int, int>, std::pair<int, QString>> physicalRowAndPt;
    std::vector<Event_CreateToken> physicalCreateTokenEvents;
    std::vector<Event_MoveCard> physicalMoveEvents;
    std::vector<Event_RevealCards> physicalRevealEvents;
    std::vector<ruled::v1::CardsRevealed> revealEvents;
    Pool myPool;
    std::map<int, int> restrictedBlueByPlayer;
    std::optional<ruled::v1::ResolutionChoiceRequired> pendingChoice;
    std::optional<ruled::v1::ResolutionChoiceRequired> lastResolutionChoice;
    std::optional<ruled::v1::TriggerOrderRequired> pendingTriggerOrder;
    std::optional<ruled::v1::TriggerNeedsTarget> pendingTriggerTarget;
    std::map<quint32, int> graveyardOwnerByEngineOid;
    std::vector<ruled::v1::CardsRevealed> activePublicReveals;
    std::map<quint32, int> serverCardByEngineOid;
    std::map<int, int> handServerCardBySlot;
    std::map<int, QString> annotationByServerCardId;
    ruled::v1::LegalActions latestLegal;

    void observePhysicalEvent(const GameEvent &ev);
    void observeRuledEvent(const ruled::v1::RuledEvent &ev, const std::function<void(const QString &)> &log);
    bool observeLegalActions(const ruled::v1::RuledEventBatch &batch);
};
} // namespace ruled_e2e
#endif
