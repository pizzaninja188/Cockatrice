use super::*;
use std::collections::BTreeMap;
use tricerules_card_model::primitives::AttackLimitAffected;

/// Declaration restrictions, recomputed from active battlefield abilities. These are not
/// occupancy limits: creatures put onto the battlefield attacking bypass declaration.
#[derive(Default)]
pub(in super::super) struct AttackLimits {
    pub global: Option<usize>,
    pub players: BTreeMap<PlayerId, usize>,
}

impl AttackLimits {
    pub fn allows_defender(&self, defender: CombatDefenderTarget) -> bool {
        self.global != Some(0)
            && match defender {
                CombatDefenderTarget::Player(player) => self.players.get(&player) != Some(&0),
                CombatDefenderTarget::Permanent(_) => true,
            }
    }

    pub fn declaration_allowed<'a>(
        &self,
        assignments: impl Iterator<Item = &'a CombatAttackAssignment>,
    ) -> bool {
        let mut players = BTreeMap::<PlayerId, usize>::new();
        for (index, assignment) in assignments.enumerate() {
            if self.global.is_some_and(|maximum| index + 1 > maximum) {
                return false;
            }
            if let CombatDefenderTarget::Player(player) = assignment.defender {
                let count = players.entry(player).or_default();
                *count += 1;
                if self
                    .players
                    .get(&player)
                    .is_some_and(|maximum| *count > *maximum)
                {
                    return false;
                }
            }
        }
        true
    }

    /// Current legal edges form a complete eligible-attacker × defender relation. Each
    /// flagged creature contributes one unweighted requirement (CR 508.1d). Recipient-specific
    /// requirements must extend this solver and its edge producer together.
    pub fn maximum_requirements(
        &self,
        pool_size: usize,
        defenders: &[(CombatDefenderTarget, PlayerId)],
    ) -> usize {
        let mut capacity = 0usize;
        for (defender, _) in defenders {
            match defender {
                CombatDefenderTarget::Player(player) => {
                    let Some(maximum) = self.players.get(player) else {
                        capacity = pool_size;
                        break;
                    };
                    capacity = capacity.saturating_add(*maximum).min(pool_size);
                }
                CombatDefenderTarget::Permanent(_) => {
                    capacity = pool_size;
                    break;
                }
            }
        }
        capacity
            .min(pool_size)
            .min(self.global.unwrap_or(pool_size))
    }
}

impl GameEngine {
    pub(in super::super) fn attack_limits(&self) -> AttackLimits {
        let mut limits = AttackLimits::default();
        for object in self.state.objects.values() {
            for ability in self.active_static_ability_definitions(object.id) {
                let StaticAbilityDef::LimitAttackers { maximum, affected } = ability else {
                    continue;
                };
                let maximum = maximum as usize;
                match affected {
                    AttackLimitAffected::All => {
                        limits.global = Some(limits.global.unwrap_or(maximum).min(maximum));
                    }
                    AttackLimitAffected::AttackingController => {
                        if let Some(controller) = self.controller_of(object.id) {
                            let cap = limits.players.entry(controller).or_insert(maximum);
                            *cap = (*cap).min(maximum);
                        }
                    }
                }
            }
        }
        limits
    }

    pub(in super::super) fn minimum_attack_requirement_count(&self) -> usize {
        let limits = self.attack_limits();
        let untaxed_defenders = self
            .attack_defenders()
            .into_iter()
            .filter(|(defender, _)| {
                limits.allows_defender(*defender) && self.attack_tax_per_attacker(*defender) == 0
            })
            .collect::<Vec<_>>();
        self.attack_limits()
            .maximum_requirements(self.attack_requirement_ids().len(), &untaxed_defenders)
    }
}
