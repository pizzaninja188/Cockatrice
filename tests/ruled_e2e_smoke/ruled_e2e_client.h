#ifndef RULED_E2E_CLIENT_H
#define RULED_E2E_CLIENT_H
#include "ruled_e2e_observed_state.h"
namespace ruled_e2e
{
// Protocol transport/session only. Scenario decisions belong to the caller.
class SmokeClient : public ObservedState
{
public:
    SmokeClient(QString name, QStringList *output) : userName(std::move(name)), transcript(output)
    {
    }
    virtual ~SmokeClient() = default;
    QString userName;
    QStringList *transcript;
    QTcpSocket sock;
    QByteArray inBuf;
    bool sawHandshakeGarbage = false;
    quint64 nextCmdId = 1;
    int roomId = -1;
    int gameId = -1;
    int notifyCustomCount = 0;
    QString lastNotifyContent;
    std::map<quint64, Response> responses;
    std::set<quint64> ruledCmdIds;
    std::vector<ServerInfo_Game> announcedGames;

    struct LegacyCastCommit
    {
        bool hasPayment = false;
        ruled::v1::PaymentSelection payment;
        std::vector<ruled::v1::ManaSpendSelection> restrictedMana;
    };
    std::optional<LegacyCastCommit> legacyCastCommit;
    bool translatedCastCommitInFlight = false;
    bool sendingTranslatedCastCommit = false;
    std::vector<std::pair<ruled::v1::RuledCommand, QString>> commandsAfterTranslatedCast;

    void log(const QString &line);
    bool connectToServer();
    void writeFrame(const std::string &bytes);
    void sendContainer(CommandContainer &cont);
    bool pump(int waitMs);
    void handleServerMessage(const ServerMessage &msg);
    void handleSessionEvent(const SessionEvent &ev);
    void handleRoomEvent(const RoomEvent &ev);
    ::testing::AssertionResult loginAndJoinRoom();
    ::testing::AssertionResult createRuledGame();
    ::testing::AssertionResult joinRuledGame(int targetGameId);
    ::testing::AssertionResult selectDeck(const QString &xml);
    void sendReady();
    void sendRuled(const ruled::v1::RuledCommand &cmd, const QString &what);
    ::testing::AssertionResult publishMain1Stops();
    template <typename Pred>::testing::AssertionResult pumpUntil(Pred pred, int timeoutMs, const char *what)
    {
        QElapsedTimer t;
        t.start();
        while (!pred()) {
            if (t.elapsed() > timeoutMs) {
                return ::testing::AssertionFailure() << userName.toStdString() << ": timeout waiting for " << what;
            }
            pump(50);
        }
        return ::testing::AssertionSuccess();
    }
    void handleGameEventContainer(const GameEventContainer &cont);
    void applyRuledBatch(const ruled::v1::RuledEventBatch &batch);
    void commitTranslatedLegacyCast(const ruled::v1::RuledEventBatch &batch);
    void releaseCommandsAfterTranslatedCast(const ruled::v1::RuledEventBatch &batch);
    // Called synchronously after each observation is decoded, before the next wire event.
    virtual void onResponse(const Response &)
    {
    }
    virtual void onPhysicalEvent(const GameEvent &)
    {
    }
    virtual void onBatchBegin(const ruled::v1::RuledEventBatch &)
    {
    }
    virtual void onRuledEvent(const ruled::v1::RuledEvent &)
    {
    }
    virtual void onBatchEventsComplete(const ruled::v1::RuledEventBatch &)
    {
    }
    virtual void onLegalActions(const ruled::v1::RuledEventBatch &)
    {
    }
    virtual void onPaymentPreview(const ruled::v1::RuledEventBatch &)
    {
    }
    bool labelMatching(const QRegularExpression &re, QRegularExpressionMatch *out = nullptr) const;
    const ruled::v1::LegalHandAction *handAction(ruled::v1::HandActionKind kind,
                                                 const QString &cardName = QString()) const;
    const ruled::v1::LegalZoneAbilityAction *zoneAbilityAction(const QString &cardName,
                                                               ruled::v1::AbilitySourceZone sourceZone) const;
    QList<const ruled::v1::LegalHandAction *> handActions(ruled::v1::HandActionKind kind) const;
    int countOwn(const QString &cardId, bool untappedOnly) const;
    void setBattlefieldAbilitySource(ruled::v1::ActivateAbility *ability, quint32 oid) const;
    virtual void onRuledCommandSent()
    {
    }
};
} // namespace ruled_e2e
#endif
