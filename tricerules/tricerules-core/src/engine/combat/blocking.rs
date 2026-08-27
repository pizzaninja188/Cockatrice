//! CR 509.1b-c complete-declaration feasibility. This graph contains only engine-derived
//! public combat facts; it neither reads card definitions nor mutates game state.
use std::collections::HashMap;

use crate::ObjectId;
use tricerules_proto::ruled::v1::BlockPair;

pub(super) struct BlockGraph {
    pub attackers: Vec<ObjectId>,
    pub blockers: Vec<ObjectId>,
    /// Attacker indices adjacent to each blocker, in deterministic order.
    pub edges: Vec<Vec<usize>>,
    pub minimum: Vec<usize>,
    pub maximum: Vec<Option<usize>>,
    pub must_block: Vec<bool>,
}

pub(super) struct BlockAnalysis {
    pub pairs: Vec<BlockPair>,
    pub required: Vec<ObjectId>,
}

impl BlockGraph {
    pub fn count_is_legal(&self, attacker: usize, count: usize) -> bool {
        count == 0
            || (count >= self.minimum[attacker]
                && self.maximum[attacker].is_none_or(|max| count <= max))
    }

    pub fn maximum_requirements(&self) -> usize {
        self.solve(None, None)
            .expect("empty declaration obeys restrictions")
    }

    pub fn analyze(&self) -> BlockAnalysis {
        let mut pairs = Vec::new();
        let mut required = Vec::new();
        if !self.must_block.iter().any(|must| *must) {
            // With no requirements, every other attacker may remain unblocked. A pair needs
            // only enough neighbors for its own minimum and a compatible maximum.
            let available: Vec<_> = (0..self.attackers.len())
                .map(|a| self.edges.iter().filter(|edges| edges.contains(&a)).count())
                .collect();
            for (b, edges) in self.edges.iter().enumerate() {
                for &a in edges {
                    if available[a] >= self.minimum[a]
                        && self.maximum[a].is_none_or(|max| max >= self.minimum[a])
                    {
                        pairs.push(self.pair(b, a));
                    }
                }
            }
        } else {
            let optimum = self.maximum_requirements();
            for (b, edges) in self.edges.iter().enumerate() {
                for &a in edges {
                    if self.solve(Some((b, a)), None) == Some(optimum) {
                        pairs.push(self.pair(b, a));
                    }
                }
                if self.solve(None, Some(b)) != Some(optimum) {
                    required.push(self.blockers[b]);
                }
            }
        }
        pairs.sort_unstable_by_key(|pair| (pair.blocker_id, pair.attacker_id));
        required.sort_unstable();
        BlockAnalysis { pairs, required }
    }

    fn pair(&self, blocker: usize, attacker: usize) -> BlockPair {
        BlockPair {
            blocker_id: self.blockers[blocker],
            attacker_id: self.attackers[attacker],
        }
    }

    /// Maximize the number of existing block-if-able requirements obeyed, optionally forcing
    /// one edge or forbidding one blocker. No heuristic cutoff may change the answer.
    fn solve(&self, forced: Option<(usize, usize)>, omitted: Option<usize>) -> Option<usize> {
        if !self.must_block.iter().any(|must| *must) && forced.is_none() {
            return Some(0);
        }
        let mut order: Vec<_> = (0..self.blockers.len()).collect();
        order.sort_unstable_by_key(|&b| {
            (
                forced.is_none_or(|(fb, _)| fb != b),
                !self.must_block[b],
                self.edges[b].len(),
                self.blockers[b],
            )
        });
        let mut available = vec![vec![0; self.attackers.len()]; order.len() + 1];
        let mut remaining_required = vec![0; order.len() + 1];
        for position in (0..order.len()).rev() {
            available[position] = available[position + 1].clone();
            let b = order[position];
            remaining_required[position] = remaining_required[position + 1]
                + usize::from(self.must_block[b] && omitted != Some(b));
            if omitted != Some(b) {
                for &a in &self.edges[b] {
                    if forced.is_none_or(|(fb, fa)| fb != b || fa == a) {
                        available[position][a] += 1;
                    }
                }
            }
        }
        let mut search = Search {
            graph: self,
            order,
            available,
            remaining_required,
            forced,
            omitted,
            memo: HashMap::new(),
        };
        search.visit(0, &mut vec![0; self.attackers.len()])
    }
}

struct Search<'a> {
    graph: &'a BlockGraph,
    order: Vec<usize>,
    available: Vec<Vec<usize>>,
    remaining_required: Vec<usize>,
    forced: Option<(usize, usize)>,
    omitted: Option<usize>,
    memo: HashMap<(usize, Vec<usize>), Option<usize>>,
}

impl Search<'_> {
    fn visit(&mut self, position: usize, counts: &mut [usize]) -> Option<usize> {
        for (a, &count) in counts.iter().enumerate() {
            if count > 0
                && (count + self.available[position][a] < self.graph.minimum[a]
                    || self.graph.maximum[a].is_some_and(|max| max < self.graph.minimum[a]))
            {
                return None;
            }
        }
        if position == self.order.len() {
            return Some(0);
        }
        // Once an uncapped attacker meets its minimum, extra blockers do not change future
        // feasibility. Normalize those counts to share equivalent search states.
        let key = (
            position,
            counts
                .iter()
                .enumerate()
                .map(|(a, &count)| {
                    if self.graph.maximum[a].is_none() {
                        count.min(self.graph.minimum[a])
                    } else {
                        count
                    }
                })
                .collect(),
        );
        if let Some(value) = self.memo.get(&key) {
            return *value;
        }
        let b = self.order[position];
        let mut best = None;
        if self.omitted != Some(b) {
            for edge in 0..self.graph.edges[b].len() {
                let a = self.graph.edges[b][edge];
                if self.forced.is_some_and(|(fb, fa)| fb == b && fa != a)
                    || self.graph.maximum[a].is_some_and(|max| counts[a] >= max)
                {
                    continue;
                }
                counts[a] += 1;
                if let Some(score) = self.visit(position + 1, counts) {
                    best = best.max(Some(score + usize::from(self.graph.must_block[b])));
                }
                counts[a] -= 1;
                if best == Some(self.remaining_required[position]) {
                    break;
                }
            }
        }
        if self.forced.is_none_or(|(fb, _)| fb != b)
            && best != Some(self.remaining_required[position])
        {
            best = best.max(self.visit(position + 1, counts));
        }
        self.memo.insert(key, best);
        best
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    // Independent exhaustive oracle: enumerate every assignment, then apply CR 509.1b-c.
    fn declarations(
        graph: &BlockGraph,
        b: usize,
        selected: &mut Vec<(usize, usize)>,
        out: &mut Vec<Vec<(usize, usize)>>,
    ) {
        if b == graph.blockers.len() {
            if (0..graph.attackers.len()).all(|a| {
                let count = selected.iter().filter(|(_, target)| *target == a).count();
                count == 0
                    || (count >= graph.minimum[a]
                        && graph.maximum[a].is_none_or(|max| count <= max))
            }) {
                out.push(selected.clone());
            }
            return;
        }
        declarations(graph, b + 1, selected, out);
        for &a in &graph.edges[b] {
            selected.push((b, a));
            declarations(graph, b + 1, selected, out);
            selected.pop();
        }
    }

    #[test]
    fn issue_174_graph_matches_exhaustive_declarations() {
        let mut seed = 174_u64;
        let mut next = || {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            (seed >> 32) as usize
        };
        for _ in 0..512 {
            let graph = BlockGraph {
                attackers: vec![10, 20, 30],
                blockers: vec![40, 50, 60, 70],
                edges: (0..4)
                    .map(|_| (0..3).filter(|_| next() % 3 != 0).collect())
                    .collect(),
                minimum: (0..3).map(|_| 1 + next() % 3).collect(),
                maximum: (0..3)
                    .map(|_| match next() % 4 {
                        3 => None,
                        n => Some(n),
                    })
                    .collect(),
                must_block: (0..4).map(|_| next() % 2 == 0).collect(),
            };
            let mut all = Vec::new();
            declarations(&graph, 0, &mut Vec::new(), &mut all);
            let score = |declaration: &Vec<(usize, usize)>| {
                declaration
                    .iter()
                    .filter(|(b, _)| graph.must_block[*b])
                    .count()
            };
            let optimum = all.iter().map(score).max().unwrap();
            all.retain(|d| score(d) == optimum);
            let expected_pairs: BTreeSet<_> = all
                .iter()
                .flatten()
                .map(|&(b, a)| (graph.blockers[b], graph.attackers[a]))
                .collect();
            let expected_required: Vec<_> = (0..4)
                .filter(|b| all.iter().all(|d| d.iter().any(|(chosen, _)| chosen == b)))
                .map(|b| graph.blockers[b])
                .collect();
            let result = graph.analyze();
            assert_eq!(graph.maximum_requirements(), optimum);
            assert_eq!(
                result
                    .pairs
                    .iter()
                    .map(|p| (p.blocker_id, p.attacker_id))
                    .collect::<BTreeSet<_>>(),
                expected_pairs
            );
            assert_eq!(result.required, expected_required);
        }
    }

    #[test]
    fn issue_174_pair_must_extend_to_a_requirement_maximizing_declaration() {
        let graph = BlockGraph {
            attackers: vec![10, 20],
            blockers: vec![30, 40],
            edges: vec![vec![0], vec![0, 1]],
            minimum: vec![2, 1],
            maximum: vec![None, None],
            must_block: vec![true, false],
        };
        let result = graph.analyze();
        assert_eq!(
            result
                .pairs
                .iter()
                .map(|p| (p.blocker_id, p.attacker_id))
                .collect::<Vec<_>>(),
            vec![(30, 10), (40, 10)]
        );
        assert_eq!(result.required, vec![30, 40]);
    }

    #[test]
    fn issue_174_competing_requirements_performance() {
        let graph = BlockGraph {
            attackers: vec![10, 20, 30, 40],
            blockers: (100..118).collect(),
            edges: vec![vec![0, 1, 2, 3]; 18],
            minimum: vec![3; 4],
            maximum: vec![Some(3); 4],
            must_block: vec![true; 18],
        };
        let start = std::time::Instant::now();
        let result = graph.analyze();
        assert_eq!(graph.maximum_requirements(), 12);
        assert_eq!(result.pairs.len(), 72);
        assert!(result.required.is_empty());
        if !cfg!(debug_assertions) {
            assert!(
                start.elapsed() < std::time::Duration::from_secs(2),
                "complete-declaration analysis took {:?}",
                start.elapsed()
            );
        }
    }
}
