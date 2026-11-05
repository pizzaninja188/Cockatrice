#pragma once
#include <QWidget>
#include <libcockatrice/protocol/pb/ruled_v1.pb.h>
class DeckList;
class QLabel;
class QHBoxLayout;
class RuledDeckLobbyPanel : public QWidget
{
public:
    explicit RuledDeckLobbyPanel(QWidget *parent = nullptr);
    void setDeck(const DeckList &deck);
    void showReport(const ruled::v1::DeckValidationReport &report, bool rejected = false);

private:
    QLabel *status;
    QWidget *commanders;
    QHBoxLayout *cards;
};
