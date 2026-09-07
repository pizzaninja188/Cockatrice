#include "ruled_resume_client.h"

#include "../phases_toolbar.h"
#include "ruled_auto_pass_policy.h"

#include <QDebug>
#include <QFile>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>

void RuledResumeClient::restoreToolbar(PhasesToolbar *toolbar, int playerId)
{
    const QString path = qEnvironmentVariable("COCKATRICE_RULED_RESUME_POLICY");
    if (path.isEmpty())
        return;
    QFile file(path);
    if (!file.open(QIODevice::ReadOnly) || file.size() > 65536) {
        qWarning() << "Cannot read resume toolbar policy" << path;
        return;
    }
    const auto policy = QJsonDocument::fromJson(file.readAll()).object();
    if (policy.value("player_id").toInt(-1) != playerId) {
        qWarning() << "Resume toolbar policy belongs to a different seat";
        return;
    }
    const auto phases = [](const QJsonValue &value) {
        google::protobuf::RepeatedField<int> result;
        for (const auto &item : value.toArray()) {
            ruled::v1::PhaseId phase;
            if (item.isDouble())
                result.Add(item.toInt());
            else if (ruled::v1::PhaseId_Parse(item.toString().toStdString(), &phase))
                result.Add(phase);
        }
        return result;
    };
    toolbar->stopOnMyTurn = RuledAutoPassPolicy::toToolbarStops(phases(policy.value("stop_on_own_turn")));
    toolbar->stopOnOpponentTurn = RuledAutoPassPolicy::toToolbarStops(phases(policy.value("stop_on_opponent_turn")));
    toolbar->syncButtonStopsFromState();
}
