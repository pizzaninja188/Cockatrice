#ifndef RULED_GAME_HOOKS_H
#define RULED_GAME_HOOKS_H
#include <string>
class PlayerActions;
class PlayerLogic;
class IInterfaceSettingsProvider;
class ICardPreferenceProvider;
class RuledClientState;
class RuledClientDiagnostics;
class PendingCommand;
class GameEventContainer;
class GameEvent;
// The game library owns model updates and transport; the client installs its ruled UI adapter.
class RuledGameHooks
{
public:
    virtual ~RuledGameHooks() = default;
    virtual PlayerActions *createPlayerActions(PlayerLogic *player,
                                               IInterfaceSettingsProvider *settings,
                                               ICardPreferenceProvider *preferences) = 0;
    virtual RuledClientState *state() const = 0;
    virtual RuledClientDiagnostics *diagnostics() const = 0;
    virtual void sendGameCommand(PendingCommand *pending, int playerId) = 0;
    virtual void prepared(PendingCommand *pending) = 0;
    virtual void received(const GameEventContainer &events) = 0;
    virtual void processPayload(const std::string &payload, bool skipReveal) = 0;
    virtual bool preparePlayerEvent(int playerId, GameEvent &event, int options) = 0;
    virtual void playerEventProcessed(int playerId, const GameEvent &event) = 0;
    virtual void playerSnapshotStarted(int playerId) = 0;
    virtual void playerSnapshotFinished(int playerId) = 0;
    virtual void reset(bool keepCurrentBatch) = 0;
    virtual void refreshRuledSpellTargetArrows() = 0;
};
#endif
