#ifndef RULES_RELAY_H
#define RULES_RELAY_H

#include <libcockatrice/protocol/pb/ruled_v1.pb.h>

#include <QByteArray>
#include <QList>
#include <QObject>
#include <QPair>
#include <QStringList>
#include <QtGlobal>

class QTcpSocket;

/**
 * TCP client to the tricerules-server sidecar (length-prefixed protobuf frames).
 */
class RulesRelay : public QObject
{
    Q_OBJECT
public:
    explicit RulesRelay(QObject *parent = nullptr);
    ~RulesRelay() override;

    bool connectIfNeeded();
    void disconnectRelay();

    /// @param playerDecks optional: one entry per player id with mainboard Oracle card names
    /// (the engine resolves names to its card ids); nullptr = use engine default
    /// @param devCommandsEnabled ask the sidecar to accept debug cheat commands. Only half the
    /// gate: the sidecar grants it solely if its own TRICERULES_DEV_COMMANDS env var is also set.
    bool sessionStart(quint64 gameId, quint64 seed, const QList<int> &playerIds,
                      const QList<QPair<int, QStringList>> *playerDecks, bool devCommandsEnabled,
                      ruled::v1::IpcResponse &out);
    bool playerCommand(int playerId, const QByteArray &ruledCommandBytes, ruled::v1::IpcResponse &out);
    /// Stateless implemented-card check (no engine session): out.ok() iff every Oracle
    /// name resolves; otherwise out.missing_card_names() lists them sorted, deduplicated.
    /// Returns false only on transport failure (sidecar unreachable / bad frame).
    bool validateDeck(const QStringList &cardNames, ruled::v1::IpcResponse &out);
    bool sessionEnd();

private:
    bool writeFrame(const google::protobuf::Message &msg);
    bool readFrame(QByteArray &out);
    QString engineHost() const;
    quint16 enginePort() const;

    QTcpSocket *socket;
    /// True once sessionStart succeeded on the current socket. The sidecar keys the engine session
    /// to the connection, so after this a dropped socket is unrecoverable and must not be silently
    /// reconnected — see connectIfNeeded().
    bool sessionActive = false;
};

#endif
