//! Cycle detection on arbitrary graphs (`com.tngtech.archunit.library.cycle_detection`).
//!
//! Johnson's algorithm on top of Tarjan's strongly connected components, ported from
//! ArchUnit so that cycles are found in the same order and cut at the same limit
//! (`cycles.max_number_to_detect`).
//!
//! ```
//! use archunit::library::cycle_detection::{CycleDetector, SimpleEdge};
//!
//! let nodes = ["a", "b", "c"];
//! let edges = [SimpleEdge::new("a", "b"), SimpleEdge::new("b", "a"), SimpleEdge::new("b", "c")];
//! let cycles = CycleDetector::detect_cycles(&nodes, &edges);
//! assert_eq!(cycles.len(), 1);
//! assert_eq!(cycles.iter().next().unwrap().edges().len(), 2);
//! ```

pub mod rules;

use std::collections::{HashMap, HashSet};
use std::hash::Hash;

use crate::config::ArchConfiguration;

/// A directed edge between two nodes (`Edge<NODE>`).
pub trait Edge<N> {
    /// The origin node.
    fn origin(&self) -> &N;
    /// The target node.
    fn target(&self) -> &N;
}

/// A plain edge (`Edge.create(origin, target)`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SimpleEdge<N> {
    origin: N,
    target: N,
}

impl<N> SimpleEdge<N> {
    /// Creates an edge.
    pub fn new(origin: N, target: N) -> Self {
        Self { origin, target }
    }
}

impl<N> Edge<N> for SimpleEdge<N> {
    fn origin(&self) -> &N {
        &self.origin
    }

    fn target(&self) -> &N {
        &self.target
    }
}

/// A cycle: a closed path of edges (`Cycle<EDGE>`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cycle<E> {
    edges: Vec<E>,
}

impl<E> Cycle<E> {
    /// The edges in order; the last edge's target is the first edge's origin.
    pub fn edges(&self) -> &[E] {
        &self.edges
    }
}

/// The cycles found in a graph (`Cycles<EDGE>`).
#[derive(Debug, Clone)]
pub struct Cycles<E> {
    cycles: Vec<Cycle<E>>,
    max_number_of_cycles_reached: bool,
}

impl<E> Cycles<E> {
    /// Iterates over the cycles.
    pub fn iter(&self) -> impl Iterator<Item = &Cycle<E>> {
        self.cycles.iter()
    }

    /// The number of cycles found (bounded by `cycles.max_number_to_detect`).
    pub fn len(&self) -> usize {
        self.cycles.len()
    }

    /// Whether no cycle was found.
    pub fn is_empty(&self) -> bool {
        self.cycles.is_empty()
    }

    /// Whether the search stopped because the configured maximum was reached
    /// (`maxNumberOfCyclesReached()`).
    pub fn max_number_of_cycles_reached(&self) -> bool {
        self.max_number_of_cycles_reached
    }
}

impl<E> IntoIterator for Cycles<E> {
    type Item = Cycle<E>;
    type IntoIter = std::vec::IntoIter<Cycle<E>>;

    fn into_iter(self) -> Self::IntoIter {
        self.cycles.into_iter()
    }
}

/// Finds cycles in a graph (`CycleDetector`).
#[derive(Debug)]
pub struct CycleDetector;

impl CycleDetector {
    /// Finds all elementary cycles among `nodes` connected by `edges`
    /// (`CycleDetector.detectCycles(nodes, edges)`).
    ///
    /// # Panics
    /// If an edge refers to a node that is not in `nodes`.
    pub fn detect_cycles<N, E>(nodes: &[N], edges: &[E]) -> Cycles<E>
    where
        N: Hash + Eq + Clone,
        E: Edge<N> + Clone,
    {
        Self::detect_cycles_with_limit(
            nodes,
            edges,
            ArchConfiguration::get().max_number_of_cycles_to_detect(),
        )
    }

    /// Like [`detect_cycles`](Self::detect_cycles) with an explicit maximum number of cycles.
    pub fn detect_cycles_with_limit<N, E>(nodes: &[N], edges: &[E], max_cycles: usize) -> Cycles<E>
    where
        N: Hash + Eq + Clone,
        E: Edge<N> + Clone,
    {
        let mut index_of: HashMap<&N, usize> = HashMap::new();
        for node in nodes {
            let next = index_of.len();
            index_of.entry(node).or_insert(next);
        }
        let size = index_of.len();
        let mut adjacency: Vec<Vec<usize>> = vec![Vec::new(); size];
        let mut edge_by_indexes: HashMap<(usize, usize), E> = HashMap::new();
        for edge in edges {
            let origin = *index_of
                .get(edge.origin())
                .unwrap_or_else(|| panic!("Origin node of an edge is not part of the graph"));
            let target = *index_of
                .get(edge.target())
                .unwrap_or_else(|| panic!("Target node of an edge is not part of the graph"));
            if !adjacency[origin].contains(&target) {
                adjacency[origin].push(target);
            }
            edge_by_indexes
                .entry((origin, target))
                .or_insert_with(|| edge.clone());
        }
        let (raw_cycles, max_reached) =
            JohnsonCycleFinder::new(&adjacency, max_cycles).find_cycles();
        let cycles = raw_cycles
            .into_iter()
            .map(|raw| {
                let mut cycle_edges = Vec::with_capacity(raw.len());
                for window in raw.windows(2) {
                    cycle_edges.push(edge_by_indexes[&(window[0], window[1])].clone());
                }
                cycle_edges.push(edge_by_indexes[&(raw[raw.len() - 1], raw[0])].clone());
                Cycle { edges: cycle_edges }
            })
            .collect();
        Cycles {
            cycles,
            max_number_of_cycles_reached: max_reached,
        }
    }
}

/// Port of ArchUnit's `JohnsonCycleFinder` / `JohnsonComponent`.
struct JohnsonCycleFinder<'a> {
    adjacency: &'a [Vec<usize>],
    max_cycles: usize,
    cycles: Vec<Vec<usize>>,
    max_reached: bool,
    component: Vec<usize>,
    blocked: HashSet<usize>,
    dependently_blocked: HashMap<usize, Vec<usize>>,
    stack: Vec<usize>,
}

impl<'a> JohnsonCycleFinder<'a> {
    fn new(adjacency: &'a [Vec<usize>], max_cycles: usize) -> Self {
        Self {
            adjacency,
            max_cycles,
            cycles: Vec::new(),
            max_reached: false,
            component: Vec::new(),
            blocked: HashSet::new(),
            dependently_blocked: HashMap::new(),
            stack: Vec::new(),
        }
    }

    fn find_cycles(mut self) -> (Vec<Vec<usize>>, bool) {
        let mut tarjan = TarjanComponentFinder::new(self.adjacency);
        let mut node_to_process = 0;
        while node_to_process < self.adjacency.len() {
            let Some(component) =
                tarjan.find_non_trivial_component_with_lowest_node_index_above(node_to_process)
            else {
                break;
            };
            let start = component[0];
            self.component = component;
            self.blocked.clear();
            self.dependently_blocked.clear();
            self.find_cycles_from(start);
            node_to_process = start + 1;
        }
        (self.cycles, self.max_reached)
    }

    fn adjacent_in_component(&self, node: usize) -> Vec<usize> {
        self.adjacency[node]
            .iter()
            .copied()
            .filter(|candidate| self.component.binary_search(candidate).is_ok())
            .collect()
    }

    fn find_cycles_from(&mut self, origin: usize) -> bool {
        if self.max_reached {
            return false;
        }
        let mut found = false;
        self.stack.push(origin);
        self.blocked.insert(origin);
        let targets = self.adjacent_in_component(origin);
        for &target in &targets {
            if target == self.component[0] {
                self.record_cycle();
                found = true;
            } else if !self.blocked.contains(&target) {
                found |= self.find_cycles_from(target);
            }
        }
        if found {
            self.unblock(origin);
        } else {
            for &target in &targets {
                self.dependently_blocked
                    .entry(target)
                    .or_default()
                    .push(origin);
            }
        }
        self.stack.pop();
        found
    }

    fn record_cycle(&mut self) {
        if self.max_reached {
            return;
        }
        if self.cycles.len() >= self.max_cycles {
            self.max_reached = true;
            return;
        }
        self.cycles.push(self.stack.clone());
    }

    fn unblock(&mut self, node: usize) {
        if !self.blocked.remove(&node) {
            return;
        }
        let dependents = self.dependently_blocked.remove(&node).unwrap_or_default();
        for dependent in dependents {
            self.unblock(dependent);
        }
    }
}

/// Port of ArchUnit's `TarjanComponentFinder` / `TarjanGraph`.
struct TarjanComponentFinder<'a> {
    adjacency: &'a [Vec<usize>],
    next_index: usize,
    visitation_index: Vec<Option<usize>>,
    low_link: Vec<usize>,
    stack: Vec<usize>,
    on_stack: Vec<bool>,
}

impl<'a> TarjanComponentFinder<'a> {
    fn new(adjacency: &'a [Vec<usize>]) -> Self {
        let size = adjacency.len();
        Self {
            adjacency,
            next_index: 0,
            visitation_index: vec![None; size],
            low_link: vec![0; size],
            stack: Vec::new(),
            on_stack: vec![false; size],
        }
    }

    fn reset(&mut self) {
        self.next_index = 0;
        self.visitation_index.iter_mut().for_each(|v| *v = None);
        self.low_link.iter_mut().for_each(|v| *v = 0);
        self.stack.clear();
        self.on_stack.iter_mut().for_each(|v| *v = false);
    }

    /// The non-trivial strongly connected component with the lowest node index at or above
    /// `lower_bound`, sorted ascending.
    fn find_non_trivial_component_with_lowest_node_index_above(
        &mut self,
        lower_bound: usize,
    ) -> Option<Vec<usize>> {
        let mut result = None;
        for node in lower_bound..self.adjacency.len() {
            if self.visitation_index[node].is_none() {
                let components = self.find_components(node, lower_bound);
                if let Some(mut lowest) = components
                    .into_iter()
                    .min_by_key(|c| c.iter().copied().min().unwrap_or(usize::MAX))
                {
                    lowest.sort_unstable();
                    result = Some(lowest);
                    break;
                }
            }
        }
        self.reset();
        result
    }

    fn find_components(&mut self, node: usize, lower_bound: usize) -> Vec<Vec<usize>> {
        let index = self.next_index;
        self.next_index += 1;
        self.visitation_index[node] = Some(index);
        self.low_link[node] = index;
        self.stack.push(node);
        self.on_stack[node] = true;
        let mut result = Vec::new();
        for target in self.adjacency[node].clone() {
            if target < lower_bound {
                continue;
            }
            if self.visitation_index[target].is_none() {
                result.extend(self.find_components(target, lower_bound));
                self.low_link[node] = self.low_link[node].min(self.low_link[target]);
            } else if self.on_stack[target] {
                let target_index = self.visitation_index[target].expect("visited");
                self.low_link[node] = self.low_link[node].min(target_index);
            }
        }
        if Some(self.low_link[node]) == self.visitation_index[node] {
            let mut component = Vec::new();
            loop {
                let current = self.stack.pop().expect("node on stack");
                self.on_stack[current] = false;
                component.push(current);
                if current == node {
                    break;
                }
            }
            if component.len() > 1 {
                result.push(component);
            }
        }
        result
    }
}
