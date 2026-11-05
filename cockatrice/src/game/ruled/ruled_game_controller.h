#ifndef RULED_GAME_CONTROLLER_H
#define RULED_GAME_CONTROLLER_H
#include "ruled_client_host.h"
#include "ruled_mana_pool_tracker.h"

#include <QHash>
#include <QObject>
#include <QPointer>
#include <libcockatrice/game/ruled_game_hooks.h>
#include <libcockatrice/protocol/pb/response.pb.h>
class AbstractGame;
class CardItem;
class ArrowItem;
class PlayerLogic;
class RuledClientState;
class RuledEventDispatcher;
class RuledClientDiagnostics;
class RuledPreparationDisplay;
class PendingCommand;
class CommandContainer;
enum class RuledSessionResetScope : int;
class RuledGameController : public QObject, public RuledClientHost, public RuledGameHooks
{
    Q_OBJECT
    friend class RuledDiagnosticViewer;

public:
    explicit RuledGameController(AbstractGame *game);
    ~RuledGameController() override;
    PlayerActions *createPlayerActions(PlayerLogic *player,
                                       IInterfaceSettingsProvider *settings,
                                       ICardPreferenceProvider *preferences) override;
    int restoreManaCounterDebit(int playerId, int counterId, int displayedValue, int staged) const;
    RuledClientState *state() const override
    {
        return ruledState;
    }
    RuledClientState *ruled() const
    {
        return ruledState;
    }
    RuledClientDiagnostics *diagnostics() const override
    {
        return ruledDiagnostics;
    }
    void refreshRuledSpellTargetArrows() override;
    void sendGameCommand(PendingCommand *pending, int playerId = -1) override;
    PendingCommand *prepareGameCommand(const google::protobuf::Message &command);
    void prepared(PendingCommand *pending) override;
    void received(const GameEventContainer &events) override;
    void processPayload(const std::string &payload, bool skipReveal) override;
    bool preparePlayerEvent(int playerId, GameEvent &event, int options) override;
    void playerEventProcessed(int playerId, const GameEvent &event) override;
    void playerSnapshotStarted(int playerId) override;
    void playerSnapshotFinished(int playerId) override;
    void reset(bool keepCurrentBatch) override;

private:
    // --- RuledClientHost -------------------------------------------------------------------
    [[nodiscard]] int localPlayerId() const override;
    [[nodiscard]] int currentActivePlayerId() const override;
    void setActivePlayerId(int playerId) override;
    void setPriorityPlayerId(int playerId) override;
    void setToolbarPhase(int toolbarPhase) override;
    void createSyntheticStackCard(quint32 virtualOid,
                                  const QString &displayName,
                                  int controllerPlayerId,
                                  const QString &setName,
                                  const std::optional<TokenStackIdentity> &sourceTokenIdentity) override;
    void removeSyntheticStackCard(quint32 virtualOid) override;
    void reconcilePreparationCopies(const QVector<PreparationCopy> &copies) override;
    [[nodiscard]] QString stackCardProviderId(quint32 oid) const override;
    [[nodiscard]] bool fallbackCreaturePt(quint32 engineOid, int *power, int *toughness) const override;
    [[nodiscard]] QString battlefieldCardName(quint32 engineOid) const override;
    [[nodiscard]] CardInfoPtr presentationCard(const QString &name) const override;
    void sendRuledCommand(const ruled::v1::RuledCommand &command) override;
    void sendRuledCommandExpectingAck(const ruled::v1::RuledCommand &command,
                                      std::function<void(bool accepted)> onFinished) override;
    [[nodiscard]] bool beginRuledGameplayCommand();
    void finishRuledGameplayCommand(quint64 token);
    void requestResolutionChoiceDialog(const QString &prompt,
                                       quint64 resolutionPromptId,
                                       const QVector<quint32> &candidateOids,
                                       const QStringList &candidateNames,
                                       int minCount,
                                       int maxCount,
                                       bool ordered,
                                       bool uniqueNames) override;
    void scheduleSpellTargetArrowSync() override;

    void clearRuledSpellTargetArrows();
    void syncRuledSpellTargetingArrows();
    /// Clears ruled engine-session tracking state and the two zones no game-state snapshot
    /// resets in ruled mode. Call on game stop and before a new game starts on the same handler;
    /// see RuledClientState::SessionResetScope for why the two transitions differ.
    void clearRuledSessionState(RuledSessionResetScope scope);

    AbstractGame *game;
    RuledClientState *ruledState;
    RuledEventDispatcher *ruledDispatcher;
    RuledClientDiagnostics *ruledDiagnostics;
    RuledPreparationDisplay *ruledPreparationDisplay;

    // Synthetic CardItems inserted into the stack zone to represent ability / copy stack items.
    // QPointer auto-nullifies if the CardItem is deleted outside our cleanup path.
    QHash<quint32, QPointer<CardItem>> syntheticAbilityStackCards;
    QList<QPointer<ArrowItem>> ruledSpellTargetSyntheticArrows;
    int nextRuledSpellTargetArrowId = -2;
    quint64 ruledPendingCommandToken = 0;
    QHash<int, RuledManaPoolTracker> manaPoolTrackers;
    QHash<int, QHash<int, int>> snapshotTapAngles;
};
#endif
