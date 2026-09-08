#ifndef RULED_E2E_OPENING_DRIVER_H
#define RULED_E2E_OPENING_DRIVER_H
#include "ruled_e2e_client.h"
namespace ruled_e2e
{
// Reused setup policy only. Focused tests submit all subsequent gameplay themselves.
class OpeningDriver : public SmokeClient
{
public:
    OpeningDriver(bool starts, QString name, QStringList *output, bool mulliganOnce = false)
        : SmokeClient(std::move(name), output), starts(starts), mulliganOnce(mulliganOnce)
    {
    }
    bool starts;
    bool mulliganOnce;
    bool didMulligan = false;
    bool sawBottomAction = false;
    bool sentBottom = false;
    bool libraryDetailsStayedConcealed = true;
    quint64 lastActedVersion = 0;
    void onRuledCommandSent() override
    {
        lastActedVersion = stateVersion;
    }
    void act()
    {
        actOpening();
    }
    bool actOpening();
    void onRuledEvent(const ruled::v1::RuledEvent &ev) override;
    void onBatchEventsComplete(const ruled::v1::RuledEventBatch &batch) override;
    void onPaymentPreview(const ruled::v1::RuledEventBatch &batch) override;
};
} // namespace ruled_e2e
#endif
