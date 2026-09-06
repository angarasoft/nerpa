//! The desired-state graph, and the order it has to be applied in.
//!
//! Expands: 0006
//!
//! Resources are nodes, declared dependencies are edges. The graph exists so
//! that two questions can be answered before anything is touched: is this
//! configuration coherent, and in what order does it have to happen.
//!
//! # Stages
//!
//! A stage is one topological level: every resource whose dependencies are all
//! satisfied by earlier stages. Stages order *execution* — a VM is created
//! before it is configured because the graph says so. They are not planning
//! rounds. The whole graph is known here, before the first stage runs, and
//! nothing about a later stage is left to be worked out after an earlier one
//! finishes.
//!
//! # Determinism
//!
//! Every collection in this module is a `BTree*`, and every result is sorted.
//! Two runs over the same configuration must produce byte-identical stages,
//! because those stages are hashed and the hash is signed. Hash map iteration
//! order would make that untrue on alternate Tuesdays.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use crate::address::{Address, Target};
use crate::domain::ExecutionDomain;
use crate::ensures;
use crate::inventory::{Inventory, Provenance};
use crate::resource::Resource;

/// Why a graph could not be built or ordered.
///
/// The variants carry whole addresses rather than identifiers, because the
/// message a person reads at three in the morning should not require a second
/// lookup to be useful. That makes the error type large, and clippy says so at
/// each returning function; boxing it would trade a real diagnostic for a lint
/// on a path that runs once per plan, not once per resource.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum GraphError {
    /// Two declarations claimed one address.
    #[error("{0} was declared twice")]
    Duplicate(Address),
    /// A declared dependency names something that is not here.
    #[error("{from} depends on {to}, which is not in the graph")]
    Dangling {
        /// The resource that declared the dependency.
        from: Address,
        /// The address it named, which is absent.
        to: Address,
    },
    /// An attribute reads from a resource that is not here.
    #[error("{from} reads {attribute} from {to}, which is not in the graph")]
    MissingReference {
        /// The resource whose attribute could not be resolved.
        from: Address,
        /// The attribute that named the missing resource.
        attribute: String,
        /// The address it named.
        to: Address,
    },
    /// A resource lives on a node the inventory has never heard of.
    #[error("{resource} lives on {target}, which nothing in the inventory provides")]
    UnknownNode {
        /// The resource that could not be placed.
        resource: Address,
        /// The node it names.
        target: Target,
    },
    /// Ordering is impossible. Carries everything a cycle holds up, not only
    /// the cycle proper.
    #[error("dependencies form a cycle: {}", .0.iter().map(ToString::to_string).collect::<Vec<_>>().join(" -> "))]
    Cycle(Vec<Address>),
}

/// What kind of work one stage is made of.
///
/// Reported rather than decided: it falls out of which domains the resources in
/// the stage belong to. A mixed stage is legitimate — two independent subtrees
/// can be at the same depth — and is worth naming so that plan output can say so
/// instead of implying an order that is not there.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum StageKind {
    /// Everything in this stage is reachable through an API.
    Control,
    /// Everything in this stage runs on nodes, named here.
    Target(BTreeSet<Target>),
    /// Both, at the same depth and independent of each other.
    Mixed,
}

/// One topological level of the graph.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stage {
    kind: StageKind,
    addresses: Vec<Address>,
}

impl Stage {
    /// What sort of work this stage is made of.
    pub fn kind(&self) -> &StageKind {
        &self.kind
    }

    /// The resources in this stage, in a stable order.
    pub fn addresses(&self) -> &[Address] {
        &self.addresses
    }
}

/// A set of resources and the ordering between them.
#[derive(Debug, Clone, Default)]
pub struct Graph {
    resources: BTreeMap<Address, Resource>,
    inventory: Inventory,
}

impl Graph {
    /// An empty graph.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a resource, refusing a second one at the same address.
    ///
    /// Two declarations of one address are not a merge to be resolved but a
    /// question with two answers, and picking either would be a guess about
    /// which one the author meant.
    ///
    /// # Errors
    ///
    /// Returns [`GraphError::Duplicate`] if this address is already present.
    #[allow(
        clippy::result_large_err,
        reason = "diagnostics beat a lint on a cold path"
    )]
    pub fn insert(&mut self, resource: Resource) -> Result<(), GraphError> {
        let address = resource.address().clone();
        if self.resources.contains_key(&address) {
            return Err(GraphError::Duplicate(address));
        }
        self.resources.insert(address, resource);
        Ok(())
    }

    /// How many resources are in the graph.
    #[must_use]
    pub fn len(&self) -> usize {
        self.resources.len()
    }

    /// Whether the graph holds nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.resources.is_empty()
    }

    /// The resource at `address`, if it is here.
    #[must_use]
    pub fn get(&self, address: &Address) -> Option<&Resource> {
        self.resources.get(address)
    }

    /// Declares that `resource` brings `target` into existence.
    pub fn managed_by(&mut self, target: Target, resource: Address) {
        self.inventory.managed_by(target, resource);
    }

    /// Declares that `target` is already there and Nerpa does not create it.
    pub fn preexisting(&mut self, target: Target) {
        self.inventory.preexisting(target);
    }

    /// Where the nodes come from.
    #[must_use]
    pub fn inventory(&self) -> &Inventory {
        &self.inventory
    }

    /// Every resource's dependencies, from one walk of the graph.
    ///
    /// [`Graph::dependencies_of`] answers for one resource by building every edge
    /// in the graph. Asking it once per resource — which is what planning did —
    /// makes the work grow with the square of the number of resources, on
    /// arithmetic that has no network and no disk in it: four thousand resources
    /// took 1.34 seconds and eight thousand took six. Building the map once and
    /// reading it takes 0.06 and 0.12.
    ///
    /// So anything that walks the whole graph asks for the whole map. The single
    /// answer is still there for a caller that genuinely wants one.
    ///
    /// # Errors
    ///
    /// Returns [`GraphError`] if an edge points at a resource that is not
    /// declared.
    #[allow(
        clippy::result_large_err,
        reason = "diagnostics beat a lint on a cold path"
    )]
    pub fn dependencies(&self) -> Result<BTreeMap<&Address, BTreeSet<&Address>>, GraphError> {
        let mut all: BTreeMap<&Address, BTreeSet<&Address>> = BTreeMap::new();
        for (dependent, dependency) in self.edges()? {
            all.entry(dependent).or_default().insert(dependency);
        }
        Ok(all)
    }

    /// What one resource has to come after, however that ordering arose.
    ///
    /// For a single resource. Anything asking about more than one should use
    /// [`Graph::dependencies`], which costs one walk rather than one per answer.
    ///
    /// # Errors
    ///
    /// Returns [`GraphError`] for the same reasons [`Graph::stages`] does: the
    /// edges have to be derived before any of them can be reported.
    pub fn dependencies_of(&self, address: &Address) -> Result<BTreeSet<Address>, GraphError> {
        Ok(self
            .edges()?
            .into_iter()
            .filter(|(dependent, _)| *dependent == address)
            .map(|(_, dependency)| dependency.clone())
            .collect())
    }

    /// Every ordering the graph implies, as `(dependent, dependency)`.
    ///
    /// Three sources, and only one of them is written by hand. An explicit
    /// `after` says what the data does not reveal; an attribute reading from
    /// another resource is an ordering whether or not anybody noticed it; and
    /// anything living on a node comes after whatever creates that node.
    ///
    // ENFORCES: INV-GRAPH-002
    // ENFORCES: INV-GRAPH-003
    // ENFORCES: INV-GRAPH-004
    #[allow(
        clippy::result_large_err,
        reason = "diagnostics beat a lint on a cold path"
    )]
    fn edges(&self) -> Result<Vec<(&Address, &Address)>, GraphError> {
        let mut edges = Vec::new();

        for (address, resource) in &self.resources {
            for dependency in resource.depends_on() {
                let (known, _) = self.resources.get_key_value(dependency).ok_or_else(|| {
                    GraphError::Dangling {
                        from: address.clone(),
                        to: dependency.clone(),
                    }
                })?;
                edges.push((address, known));
            }

            for (name, attribute) in resource.attributes() {
                let Some(reference) = attribute.reference() else {
                    continue;
                };
                let (known, _) = self
                    .resources
                    .get_key_value(reference.source())
                    .ok_or_else(|| GraphError::MissingReference {
                        from: address.clone(),
                        attribute: name.clone(),
                        to: reference.source().clone(),
                    })?;
                edges.push((address, known));
            }

            if let ExecutionDomain::Target(target) = address.domain() {
                // Fail closed. If Nerpa cannot say where a machine comes from it
                // cannot say when to touch it, and ordering the resource last
                // would be a guess dressed up as a decision.
                match self.inventory.provenance(&target) {
                    None => {
                        return Err(GraphError::UnknownNode {
                            resource: address.clone(),
                            target,
                        });
                    }
                    Some(Provenance::Preexisting) => {}
                    Some(Provenance::Managed(provider)) => {
                        let (known, _) =
                            self.resources.get_key_value(provider).ok_or_else(|| {
                                GraphError::Dangling {
                                    from: address.clone(),
                                    to: provider.clone(),
                                }
                            })?;
                        edges.push((address, known));
                    }
                }
            }
        }

        Ok(edges)
    }

    /// Orders the graph into stages, or explains why it cannot be ordered.
    ///
    /// Kahn's algorithm, over sorted collections throughout so that the answer —
    /// including the cycle report — is the same on every machine and every run.
    ///
    /// # Errors
    ///
    /// Returns [`GraphError::Dangling`] if a declared dependency names a
    /// resource that is not in the graph, [`GraphError::MissingReference`] if an
    /// attribute reads from one, [`GraphError::UnknownNode`] if a resource lives
    /// on a node the inventory does not know, and [`GraphError::Cycle`] if the
    /// dependencies cannot be ordered at all.
    ///
    /// # Panics
    ///
    /// Panics if a contract is violated — that is, if this function's own
    /// bookkeeping has become inconsistent. See [`crate::contract`].
    ///
    // ENFORCES: INV-GRAPH-001
    // ENFORCES: INV-GRAPH-002
    #[allow(
        clippy::result_large_err,
        reason = "diagnostics beat a lint on a cold path"
    )]
    pub fn stages(&self) -> Result<Vec<Stage>, GraphError> {
        let mut indegree: BTreeMap<&Address, usize> = self
            .resources
            .keys()
            .map(|address| (address, 0usize))
            .collect();
        let mut dependents: BTreeMap<&Address, BTreeSet<&Address>> = BTreeMap::new();

        // Two references to one resource are one ordering, so an edge already
        // recorded must not be counted twice or the node never becomes ready.
        for (dependent, dependency) in self.edges()? {
            if dependents.entry(dependency).or_default().insert(dependent) {
                *indegree
                    .get_mut(dependent)
                    .expect("address came from this map") += 1;
            }
        }

        let mut ready: VecDeque<&Address> = indegree
            .iter()
            .filter(|(_, degree)| **degree == 0)
            .map(|(address, _)| *address)
            .collect();

        let mut stages = Vec::new();
        let mut placed = 0usize;

        while !ready.is_empty() {
            let level: Vec<&Address> = ready.drain(..).collect();
            let mut next: BTreeSet<&Address> = BTreeSet::new();

            for address in &level {
                for dependent in dependents.get(address).into_iter().flatten() {
                    let degree = indegree
                        .get_mut(*dependent)
                        .expect("dependent came from this map");
                    *degree -= 1;
                    if *degree == 0 {
                        next.insert(dependent);
                    }
                }
            }

            placed += level.len();
            stages.push(Self::stage_from(level));
            ready.extend(next);
        }

        if placed != self.resources.len() {
            // Whatever still has an incoming edge is either in a cycle or
            // downstream of one. Both belong in the report: naming only the
            // cycle proper leaves the reader wondering why unrelated resources
            // went missing from the plan.
            let stuck: Vec<Address> = indegree
                .iter()
                .filter(|(_, degree)| **degree > 0)
                .map(|(address, _)| (*address).clone())
                .collect();
            ensures!(
                !stuck.is_empty(),
                "resources went missing without a cycle to explain it"
            );
            return Err(GraphError::Cycle(stuck));
        }

        ensures!(
            stages
                .iter()
                .map(|stage| stage.addresses.len())
                .sum::<usize>()
                == self.resources.len(),
            "every resource appears in exactly one stage"
        );
        Ok(stages)
    }

    fn stage_from(level: Vec<&Address>) -> Stage {
        let mut targets = BTreeSet::new();
        let mut has_control = false;

        for address in &level {
            match address.domain() {
                ExecutionDomain::Control => has_control = true,
                ExecutionDomain::Target(target) => {
                    targets.insert(target);
                }
            }
        }

        let kind = match (has_control, targets.is_empty()) {
            (true, true) => StageKind::Control,
            (false, false) => StageKind::Target(targets),
            _ => StageKind::Mixed,
        };

        Stage {
            kind,
            addresses: level.into_iter().cloned().collect(),
        }
    }
}

impl FromIterator<Resource> for Result<Graph, GraphError> {
    #[allow(
        clippy::result_large_err,
        reason = "diagnostics beat a lint on a cold path"
    )]
    fn from_iter<I: IntoIterator<Item = Resource>>(resources: I) -> Self {
        let mut graph = Graph::new();
        for resource in resources {
            graph.insert(resource)?;
        }
        Ok(graph)
    }
}
