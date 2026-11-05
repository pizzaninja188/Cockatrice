#pragma once
#include <QLabel>
#include <libcockatrice/protocol/pb/ruled_v1.pb.h>

class RuledDeckLobbyPanel : public QLabel
{
public:
    explicit RuledDeckLobbyPanel(QWidget *parent = nullptr);
    void resetReport();
    void showReport(const ruled::v1::DeckValidationReport &report, bool rejected = false);
};
