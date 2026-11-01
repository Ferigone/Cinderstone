//! Climate.ParameterList's exact seven-dimensional RTree search.
//!
//! The tree and ordered values are immutable. Each worker retains its own
//! previous leaf for that same tree: ties can depend on that query history.
//! This module selects a supplied value, not a biome registry or chunk status.
use crate::density::ClimateTargetPoint;
use anyhow::{bail, ensure, Result};
use std::{cmp::Ordering, marker::PhantomData, rc::Rc, sync::Arc};

const DIMENSIONS: usize = 7;
const CHILDREN_PER_NODE: usize = 6;

/// A quantized Climate.Parameter interval, including the offset dimension.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClimateParameter {
    pub min: i64,
    pub max: i64,
}
impl ClimateParameter {
    /// Climate.Parameter.distance(long), retaining Java signed-long wrapping.
    pub fn distance(self, value: i64) -> i64 {
        let above = value.wrapping_sub(self.max);
        if above > 0 {
            above
        } else {
            self.min.wrapping_sub(value).max(0)
        }
    }

    fn span(self, other: Self) -> Self {
        Self {
            min: self.min.min(other.min),
            max: self.max.max(other.max),
        }
    }

    fn center(self) -> i64 {
        // Java divides signed longs with truncation toward zero.
        self.min.wrapping_add(self.max) / 2
    }
}

/// Original parameter-space order: temperature, humidity, continentalness,
/// erosion, depth, weirdness, then [offset, offset]. Values are already the
/// original quantized longs; construction performs no float conversion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClimateParameterPoint {
    pub parameters: [ClimateParameter; DIMENSIONS],
}

#[derive(Debug)]
enum NodeKind {
    Leaf(usize),
    SubTree(Vec<usize>),
}
#[derive(Debug)]
struct Node {
    parameters: [ClimateParameter; DIMENSIONS],
    kind: NodeKind,
}
impl Node {
    fn distance(&self, target: &[i64; DIMENSIONS]) -> i64 {
        self.parameters
            .iter()
            .zip(target)
            .fold(0_i64, |total, (parameter, target)| {
                let distance = parameter.distance(*target);
                total.wrapping_add(distance.wrapping_mul(distance))
            })
    }
}

struct Tree<T> {
    values: Vec<(ClimateParameterPoint, T)>,
    nodes: Vec<Node>,
    root: usize,
}

/// Immutable ordered parameter values and the exact original RTree geometry.
/// Clones share this tree identity; they do not rebuild or reseed anything.
pub struct ClimateParameterList<T> {
    tree: Arc<Tree<T>>,
}
impl<T> Clone for ClimateParameterList<T> {
    fn clone(&self) -> Self {
        Self {
            tree: Arc::clone(&self.tree),
        }
    }
}
impl<T> ClimateParameterList<T> {
    pub fn new(values: Vec<(ClimateParameterPoint, T)>) -> Result<Self> {
        ensure!(
            !values.is_empty(),
            "Need at least one value to build the search tree."
        );
        ensure!(
            values.len() <= i32::MAX as usize,
            "climate parameter list exceeds Java List size"
        );
        ensure!(
            values
                .iter()
                .all(|(point, _)| { point.parameters[6].min == point.parameters[6].max }),
            "Climate.ParameterPoint offset must be a point interval"
        );
        let nodes = values
            .iter()
            .enumerate()
            .map(|(index, (point, _))| Node {
                parameters: point.parameters,
                kind: NodeKind::Leaf(index),
            })
            .collect::<Vec<_>>();
        let mut tree = Tree {
            values,
            nodes,
            root: 0,
        };
        tree.root = tree.build((0..tree.values.len()).collect())?;
        Ok(Self {
            tree: Arc::new(tree),
        })
    }

    /// Preserve the source list order, including duplicate points and values.
    pub fn values(&self) -> &[(ClimateParameterPoint, T)] {
        &self.tree.values
    }

    /// Start the equivalent of a fresh ThreadLocal for this tree. Reuse the
    /// returned worker across successive queries/chunks; recreating it discards
    /// observable tie prehistory. Different tree identities need different workers.
    pub fn worker(&self) -> ClimateSearch<T> {
        ClimateSearch {
            tree: Arc::clone(&self.tree),
            last_leaf: None,
            worker_local: PhantomData,
        }
    }
}

/// One worker's search prehistory, bound to its immutable parameter-list owner.
/// This owner is deliberately neither Send nor Sync, like a source ThreadLocal;
/// the immutable list may be shared when its values are Send + Sync.
pub struct ClimateSearch<T> {
    tree: Arc<Tree<T>>,
    last_leaf: Option<usize>,
    worker_local: PhantomData<Rc<()>>,
}
impl<T> ClimateSearch<T> {
    /// Return the original ordered entry index selected by production RTree
    /// search. The target's seventh (offset) coordinate is always zero.
    pub fn find_index(&mut self, target: ClimateTargetPoint) -> usize {
        let leaf = self
            .tree
            .search(self.tree.root, &target.parameter_array(), self.last_leaf)
            .expect("original climate search returned no leaf");
        self.last_leaf = Some(leaf);
        match self.tree.nodes[leaf].kind {
            NodeKind::Leaf(index) => index,
            NodeKind::SubTree(_) => unreachable!("climate search must return a leaf"),
        }
    }

    pub fn find_value(&mut self, target: ClimateTargetPoint) -> &T {
        let index = self.find_index(target);
        &self.tree.values[index].1
    }

    /// The prior result belongs to this exact tree, not merely an equal set of
    /// parameters. Exposing its input index does not permit replacing the owner.
    pub fn last_index(&self) -> Option<usize> {
        self.last_leaf.map(|leaf| match self.tree.nodes[leaf].kind {
            NodeKind::Leaf(index) => index,
            NodeKind::SubTree(_) => unreachable!("climate cursor must retain a leaf"),
        })
    }
}

struct Bucket {
    parameters: [ClimateParameter; DIMENSIONS],
    children: Vec<usize>,
}

impl<T> Tree<T> {
    fn build(&mut self, mut children: Vec<usize>) -> Result<usize> {
        if children.len() == 1 {
            return Ok(children[0]);
        }
        if children.len() <= CHILDREN_PER_NODE {
            // List.sort is stable. Identical center sums retain input order.
            children.sort_by_key(|child| {
                self.nodes[*child]
                    .parameters
                    .iter()
                    .fold(0_i64, |total, parameter| {
                        total.wrapping_add(parameter.center().wrapping_abs())
                    })
            });
            return Ok(self.push_subtree(children));
        }
        let mut best_cost = i64::MAX;
        let mut best_axis = None;
        let mut best_buckets = Vec::new();
        for axis in 0..DIMENSIONS {
            children.sort_by(|a, b| {
                compare_parameters(
                    &self.nodes[*a].parameters,
                    &self.nodes[*b].parameters,
                    axis,
                    false,
                )
            });
            let width = bucket_size(children.len());
            let buckets = children
                .chunks(width)
                .map(|children| Bucket {
                    parameters: self.span(children),
                    children: children.to_vec(),
                })
                .collect::<Vec<_>>();
            let cost = buckets.iter().fold(0_i64, |total, bucket| {
                total.wrapping_add(parameter_cost(&bucket.parameters))
            });
            // The original retains the first axis on equal costs.
            if best_cost > cost {
                best_cost = cost;
                best_axis = Some(axis);
                best_buckets = buckets;
            }
        }
        let Some(axis) = best_axis else {
            // The source would dereference a null candidate here if arbitrary
            // overflowing parameters made every cost Long.MAX_VALUE. No tree or
            // fallback selection is synthesized for that failed construction.
            bail!("original climate tree could not select a bucket axis");
        };
        best_buckets.sort_by(|a, b| compare_parameters(&a.parameters, &b.parameters, axis, true));
        let mut branches = Vec::with_capacity(best_buckets.len());
        for bucket in best_buckets {
            branches.push(self.build(bucket.children)?);
        }
        Ok(self.push_subtree(branches))
    }

    fn span(&self, children: &[usize]) -> [ClimateParameter; DIMENSIONS] {
        let mut parameters = self.nodes[children[0]].parameters;
        for child in &children[1..] {
            for (span, interval) in parameters.iter_mut().zip(self.nodes[*child].parameters) {
                *span = span.span(interval);
            }
        }
        parameters
    }

    fn push_subtree(&mut self, children: Vec<usize>) -> usize {
        let parameters = self.span(&children);
        let index = self.nodes.len();
        self.nodes.push(Node {
            parameters,
            kind: NodeKind::SubTree(children),
        });
        index
    }

    fn search(
        &self,
        node: usize,
        target: &[i64; DIMENSIONS],
        previous: Option<usize>,
    ) -> Option<usize> {
        let NodeKind::SubTree(children) = &self.nodes[node].kind else {
            return Some(node);
        };
        let mut best = previous;
        let mut best_distance = previous.map_or(i64::MAX, |leaf| self.nodes[leaf].distance(target));
        for child in children {
            let lower_bound = self.nodes[*child].distance(target);
            if best_distance > lower_bound {
                let candidate = self.search(*child, target, best);
                let candidate_distance = if candidate == Some(*child) {
                    lower_bound
                } else {
                    self.nodes[candidate.expect("original climate subtree returned no leaf")]
                        .distance(target)
                };
                if best_distance > candidate_distance {
                    best_distance = candidate_distance;
                    best = candidate;
                }
            }
        }
        best
    }
}

fn compare_parameters(
    a: &[ClimateParameter; DIMENSIONS],
    b: &[ClimateParameter; DIMENSIONS],
    first_axis: usize,
    absolute: bool,
) -> Ordering {
    for step in 0..DIMENSIONS {
        let axis = (first_axis + step) % DIMENSIONS;
        let a = a[axis].center();
        let b = b[axis].center();
        let order = if absolute {
            a.wrapping_abs().cmp(&b.wrapping_abs())
        } else {
            a.cmp(&b)
        };
        if order != Ordering::Equal {
            return order;
        }
    }
    Ordering::Equal
}

fn parameter_cost(parameters: &[ClimateParameter; DIMENSIONS]) -> i64 {
    parameters.iter().fold(0_i64, |total, parameter| {
        total.wrapping_add(parameter.max.wrapping_sub(parameter.min).wrapping_abs())
    })
}

fn bucket_size(length: usize) -> usize {
    // The source is 6^floor(log(length - 0.01) / log(6)). For Java List's integer
    // size this is exactly the largest power of six strictly below length.
    // Integer powers preserve that result without platform log/pow rounding.
    let mut size = 1;
    while size * CHILDREN_PER_NODE < length {
        size *= CHILDREN_PER_NODE;
    }
    size
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Deserialize)]
    struct Entry {
        parameters: [[i64; 2]; DIMENSIONS],
        #[serde(alias = "biome")]
        value: String,
    }
    impl Entry {
        fn into_value(self) -> (ClimateParameterPoint, String) {
            (
                ClimateParameterPoint {
                    parameters: self.parameters.map(|pair| ClimateParameter {
                        min: pair[0],
                        max: pair[1],
                    }),
                },
                self.value,
            )
        }
    }
    #[derive(Deserialize)]
    struct Query {
        target: [i64; 6],
        index: usize,
        #[serde(alias = "biome")]
        value: String,
    }
    #[derive(Deserialize)]
    struct Sequence {
        entries: Vec<Entry>,
        queries: Vec<Query>,
    }
    #[derive(Deserialize)]
    struct PresetSequence {
        preset: String,
        queries: Vec<Query>,
    }
    #[derive(Deserialize)]
    struct Vectors {
        search_sequences: Vec<PresetSequence>,
        tie_sequences: Vec<Sequence>,
    }
    #[derive(Deserialize)]
    struct Preset {
        entries: Vec<Entry>,
    }
    #[derive(Deserialize)]
    struct Presets {
        presets: std::collections::BTreeMap<String, Preset>,
    }
    fn target(values: [i64; 6]) -> ClimateTargetPoint {
        ClimateTargetPoint {
            temperature: values[0],
            humidity: values[1],
            continentalness: values[2],
            erosion: values[3],
            depth: values[4],
            weirdness: values[5],
        }
    }
    fn vectors() -> Vectors {
        serde_json::from_str(include_str!(
            "../../fixtures/world-generation-biome-source-vectors.json"
        ))
        .unwrap()
    }

    #[test]
    fn original_preset_search_indices_preserve_tree_order_and_history() -> Result<()> {
        let mut presets: Presets = serde_json::from_str(include_str!(
            "../../fixtures/world-generation-multinoise-parameters.json"
        ))?;
        for sequence in vectors().search_sequences {
            let entries = presets
                .presets
                .remove(&sequence.preset)
                .expect("one original search history per preset")
                .entries
                .into_iter()
                .map(Entry::into_value)
                .collect();
            let list = ClimateParameterList::new(entries)?;
            let mut search = list.worker();
            let mut value_search = list.worker();
            for query in sequence.queries {
                let target = target(query.target);
                assert_eq!(
                    search.find_index(target),
                    query.index,
                    "{}",
                    sequence.preset
                );
                assert_eq!(search.last_index(), Some(query.index));
                assert_eq!(value_search.find_value(target), &query.value);
                assert_eq!(list.values()[query.index].1, query.value);
            }
        }
        Ok(())
    }

    #[test]
    fn actual_original_ties_retain_the_same_workers_prior_leaf() -> Result<()> {
        for sequence in vectors().tie_sequences {
            let list = ClimateParameterList::new(
                sequence
                    .entries
                    .into_iter()
                    .map(Entry::into_value)
                    .collect(),
            )?;
            let mut worker = list.worker();
            let clone = list.clone();
            assert!(Arc::ptr_eq(&list.tree, &clone.tree));
            for query in sequence.queries {
                assert_eq!(worker.find_index(target(query.target)), query.index);
                assert_eq!(list.values()[query.index].1, query.value);
            }
        }
        Ok(())
    }

    #[test]
    fn stable_bucket_boundaries_and_empty_list_rejection() {
        assert!(ClimateParameterList::<String>::new(Vec::new()).is_err());
        for (length, expected) in [(7, 6), (36, 6), (37, 36), (216, 36), (217, 216)] {
            assert_eq!(bucket_size(length), expected);
        }
        let interval = ClimateParameter { min: -3, max: 0 };
        assert_eq!(interval.center(), -1);
        let overflowing = ClimateParameter {
            min: i64::MIN,
            max: i64::MAX,
        };
        assert_eq!(overflowing.center(), 0);
        assert_eq!(parameter_cost(&[overflowing; DIMENSIONS]), 7);
    }
}
