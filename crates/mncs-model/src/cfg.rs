//! Deterministic control-flow facts for the supported executable-body subset.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use serde::{Deserialize, Serialize};

use crate::body::{BodyTerminator, FunctionBody};
use crate::canonical::sha256_hex;
use crate::identity::SemanticId;

pub const CFG_SCHEMA_VERSION: &str = "0.1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cfg {
    pub schema_version: String,
    pub identity: SemanticId,
    pub entry: String,
    pub blocks: Vec<CfgBlock>,
    pub reachable: Vec<String>,
    pub unreachable: Vec<String>,
    pub dominators: BTreeMap<String, Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CfgBlock {
    pub id: String,
    pub predecessors: Vec<String>,
    pub successors: Vec<String>,
    pub reachable: bool,
}

impl Cfg {
    pub fn from_body(body: &FunctionBody, body_identity: SemanticId) -> Self {
        let block_ids = body
            .blocks
            .iter()
            .map(|block| block.id.clone())
            .collect::<BTreeSet<_>>();
        let mut successors = BTreeMap::<String, BTreeSet<String>>::new();
        for block in &body.blocks {
            let targets = terminator_targets(&block.terminator)
                .filter(|target| block_ids.contains(*target))
                .cloned()
                .collect::<BTreeSet<_>>();
            successors.insert(block.id.clone(), targets);
        }

        let mut predecessors = block_ids
            .iter()
            .map(|id| (id.clone(), BTreeSet::new()))
            .collect::<BTreeMap<_, _>>();
        for (from, targets) in &successors {
            for target in targets {
                predecessors
                    .entry(target.clone())
                    .or_default()
                    .insert(from.clone());
            }
        }

        let mut reachable = BTreeSet::new();
        if block_ids.contains(&body.entry) {
            let mut queue = VecDeque::from([body.entry.clone()]);
            while let Some(block) = queue.pop_front() {
                if !reachable.insert(block.clone()) {
                    continue;
                }
                if let Some(targets) = successors.get(&block) {
                    queue.extend(targets.iter().cloned());
                }
            }
        }

        let mut dominator_sets = BTreeMap::<String, BTreeSet<String>>::new();
        for block in &reachable {
            if block == &body.entry {
                dominator_sets.insert(block.clone(), BTreeSet::from([block.clone()]));
            } else {
                dominator_sets.insert(block.clone(), reachable.clone());
            }
        }
        let mut changed = true;
        while changed {
            changed = false;
            for block in reachable.iter().filter(|block| *block != &body.entry) {
                let preds = predecessors
                    .get(block)
                    .into_iter()
                    .flat_map(BTreeSet::iter)
                    .filter(|pred| reachable.contains(*pred))
                    .filter_map(|pred| dominator_sets.get(pred));
                let Some(first) = preds.clone().next() else {
                    continue;
                };
                let mut next = first.clone();
                for set in preds.skip(1) {
                    next = next.intersection(set).cloned().collect();
                }
                next.insert(block.clone());
                let previous = dominator_sets
                    .get(block)
                    .expect("reachable block has dominator set");
                if previous != &next {
                    dominator_sets.insert(block.clone(), next);
                    changed = true;
                }
            }
        }

        let mut blocks = block_ids
            .iter()
            .map(|id| CfgBlock {
                id: id.clone(),
                predecessors: predecessors
                    .get(id)
                    .cloned()
                    .unwrap_or_default()
                    .into_iter()
                    .collect(),
                successors: successors
                    .get(id)
                    .cloned()
                    .unwrap_or_default()
                    .into_iter()
                    .collect(),
                reachable: reachable.contains(id),
            })
            .collect::<Vec<_>>();
        blocks.sort_by(|left, right| left.id.cmp(&right.id));
        let reachable = reachable.into_iter().collect::<Vec<_>>();
        let unreachable = block_ids
            .difference(&reachable.iter().cloned().collect())
            .cloned()
            .collect::<Vec<_>>();
        let dominators = dominator_sets
            .into_iter()
            .map(|(block, set)| (block, set.into_iter().collect()))
            .collect::<BTreeMap<_, _>>();
        let material = serde_json::to_string(&(&body_identity, &body.entry, &blocks, &reachable))
            .expect("CFG is serializable");
        Self {
            schema_version: CFG_SCHEMA_VERSION.to_owned(),
            identity: SemanticId(format!("mncs:0.3:cfg:{}", sha256_hex(material.as_bytes()))),
            entry: body.entry.clone(),
            blocks,
            reachable,
            unreachable,
            dominators,
        }
    }

    pub fn canonical_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    pub fn fingerprint(&self) -> Result<String, serde_json::Error> {
        Ok(sha256_hex(self.canonical_json()?.as_bytes()))
    }

    pub fn dominates(&self, definition: &str, use_block: &str) -> bool {
        self.dominators
            .get(use_block)
            .is_some_and(|blocks| blocks.iter().any(|block| block == definition))
    }

    pub fn block(&self, id: &str) -> Option<&CfgBlock> {
        self.blocks.iter().find(|block| block.id == id)
    }
}

fn terminator_targets(terminator: &BodyTerminator) -> impl Iterator<Item = &String> {
    let targets = match terminator {
        BodyTerminator::Return { .. } => Vec::new(),
        BodyTerminator::Branch { target, .. } => vec![target],
        BodyTerminator::Failure { .. } => Vec::new(),
        BodyTerminator::ConditionalBranch {
            then_target,
            else_target,
            ..
        } => vec![then_target, else_target],
    };
    targets.into_iter()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::body::{BodyBlock, BodyParameter, BodyTerminator, BodyType};

    #[test]
    fn cfg_is_deterministic_and_computes_dominance() {
        let body = FunctionBody {
            schema_version: crate::EXECUTABLE_BODY_SCHEMA_VERSION.to_owned(),
            entry: "entry".to_owned(),
            parameters: vec![BodyParameter {
                id: "condition".to_owned(),
                name: "condition".to_owned(),
                ty: BodyType::Named("bool".to_owned()),
            }],
            blocks: vec![
                BodyBlock {
                    id: "entry".to_owned(),
                    parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator: BodyTerminator::ConditionalBranch {
                        condition: "condition".to_owned(),
                        then_target: "left".to_owned(),
                        then_arguments: Vec::new(),
                        else_target: "right".to_owned(),
                        else_arguments: Vec::new(),
                    },
                },
                BodyBlock {
                    id: "left".to_owned(),
                    parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator: BodyTerminator::Branch {
                        target: "join".to_owned(),
                        arguments: Vec::new(),
                    },
                },
                BodyBlock {
                    id: "right".to_owned(),
                    parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator: BodyTerminator::Branch {
                        target: "join".to_owned(),
                        arguments: Vec::new(),
                    },
                },
                BodyBlock {
                    id: "join".to_owned(),
                    parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator: BodyTerminator::Return { values: Vec::new() },
                },
            ],
        };
        let first = Cfg::from_body(&body, SemanticId("body".to_owned()));
        let second = Cfg::from_body(&body, SemanticId("body".to_owned()));
        assert_eq!(first, second);
        assert_eq!(first.reachable, vec!["entry", "join", "left", "right"]);
        assert!(first.dominates("entry", "join"));
        assert!(!first.dominates("left", "join"));
        assert_eq!(
            first.block("entry").expect("entry").successors,
            vec!["left", "right"]
        );
    }
}
