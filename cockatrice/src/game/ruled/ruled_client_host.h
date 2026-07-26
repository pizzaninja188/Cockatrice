/**
 * @file ruled_client_host.h
 * @ingroup GameLogic
 * @brief Abstract seam between the ruled client view-model and the Qt game UI.
 *
 * `RuledClientState` and `RuledEventDispatcher` are deliberately free of any dependency on
 * `AbstractGame`, `Player` or `CardItem`: everything they need from the running client is
 * reached through this interface.  That is what makes the pair testable offscreen — the
 * headless suite in `tests/ruled_client_tests/` supplies a fake host and asserts on the
 * resulting state.  `GameEventHandler` is the production implementation.
 */

#ifndef COCKATRICE_RULED_CLIENT_HOST_H
#define COCKATRICE_RULED_CLIENT_HOST_H

#include <QString>
#include <QStringList>
#include <QVector>
#include <QtGlobal>
#include <functional>

namespace ruled::v1
{
class RuledCommand;
}

class RuledClientHost
{
public:
    virtual ~RuledClientHost() = default;

    /// Cockatrice seat id of the player sitting at this client, or -1 when unknown.
    [[nodiscard]] virtual int localPlayerId() const = 0;

    /// Engine-driven mirror of the freeform turn/phase widgets.
    [[nodiscard]] virtual int currentActivePlayerId() const = 0;
    virtual void setActivePlayerId(int playerId) = 0;
    virtual void setPriorityPlayerId(int playerId) = 0;
    virtual void setToolbarPhase(int toolbarPhase) = 0;

    /// Abilities and spell copies have no physical card on the stack; the host materialises a
    /// synthetic `CardItem` for them (and tears it down again) so the stack window shows them.
    virtual void createSyntheticStackCard(quint32 virtualOid,
                                          const QString &displayName,
                                          int controllerPlayerId,
                                          const QString &setName) = 0;
    virtual void removeSyntheticStackCard(quint32 virtualOid) = 0;
    /// Printing (Scryfall provider id) of the spell currently on the stack under `oid`, so a
    /// copy (CR 707.10) inherits the original's art. Empty when it cannot be resolved.
    [[nodiscard]] virtual QString stackCardProviderId(quint32 oid) const = 0;

    /// P/T fallback for a battlefield creature when the engine zone view is unavailable
    /// (ZoneView is stripped from client broadcasts). Returns false when nothing is known.
    [[nodiscard]] virtual bool fallbackCreaturePt(quint32 engineOid, int *power, int *toughness) const = 0;
    /// Display name of a battlefield permanent, for prompt text. Empty when unresolvable.
    [[nodiscard]] virtual QString battlefieldCardName(quint32 engineOid) const = 0;

    /// Transport for a ruled command produced by the view-model (resolution picks, combat
    /// damage assignment, previews). The host wraps it in `Command_RuledPayload` and sends it.
    virtual void sendRuledCommand(const ruled::v1::RuledCommand &command) = 0;

    /// As `sendRuledCommand`, but invokes `onFinished(accepted)` when the server answers, so the
    /// caller can roll back optimistic local state on rejection (e.g. an illegal block).
    virtual void sendRuledCommandExpectingAck(const ruled::v1::RuledCommand &command,
                                              std::function<void(bool accepted)> onFinished) = 0;

    /// Modal fallback picker for resolution choice kinds that have no bespoke click UI.
    /// The host is expected to defer the dialog past the current batch and then send a
    /// `SubmitResolutionChoice` itself.
    virtual void requestResolutionChoiceDialog(const QString &prompt,
                                               const QVector<quint32> &candidateOids,
                                               const QStringList &candidateNames,
                                               int minCount,
                                               int maxCount,
                                               bool ordered,
                                               bool uniqueNames) = 0;

    /// Ask the host to re-derive the spell→target arrows once layout has settled.
    virtual void scheduleSpellTargetArrowSync() = 0;
};

#endif // COCKATRICE_RULED_CLIENT_HOST_H
