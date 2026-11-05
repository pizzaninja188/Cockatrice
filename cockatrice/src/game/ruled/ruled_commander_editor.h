#pragma once

#include <QGroupBox>

class DeckStateManager;
class QHBoxLayout;

// Edits the canonical declarations; commanders never become ordinary mainboard nodes.
class RuledCommanderEditor : public QGroupBox
{
    Q_OBJECT
public:
    explicit RuledCommanderEditor(DeckStateManager *manager, QWidget *parent = nullptr);

private:
    void refresh();
    DeckStateManager *manager;
    QWidget *cards;
    QHBoxLayout *cardsLayout;
};
