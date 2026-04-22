#ifndef RULES_RELAY_H
#define RULES_RELAY_H

#include <libcockatrice/protocol/pb/ruled_v1.pb.h>

#include <QByteArray>
#include <QList>
#include <QObject>
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

    bool sessionStart(quint64 gameId, quint64 seed, const QList<int> &playerIds, ruled::v1::IpcResponse &out);
    bool playerCommand(int playerId, const QByteArray &ruledCommandBytes, ruled::v1::IpcResponse &out);
    bool sessionEnd();

private:
    bool writeFrame(const google::protobuf::Message &msg);
    bool readFrame(QByteArray &out);
    QString engineHost() const;
    quint16 enginePort() const;

    QTcpSocket *socket;
};

#endif
