//! The run as a library: plan and apply over the typed plan.
//!
//! A front end — the CLI today, an HTTP/JSON API next — wraps this: it parses
//! its own arguments, resolves the identity, opens the machines, and renders
//! the result. The engine holds the run itself: the providers the fleet names,
//! the graph the configuration evaluates to, the observed world, and the plan
//! and the apply over it (adr-2026-073).
//!
//! The transport is deliberately not here. Opening a machine — `ssh`, finding
//! the carried executor — is the front end's, and the opened [`Machines`] are
//! handed in for [`Engine::observe`] and [`Engine::apply`].

use std::path::Path;

use nerpa_config::fleet::Fleet;
use nerpa_core::provider::{Machines, Providers};
use nerpa_core::run::{self, Applied, RunError};
use nerpa_core::secret::Secrets;
use nerpa_core::state::State;
use nerpa_core::{Actor, Graph, Observed, Plan, planner};
use nerpa_provider_os::Os;
use nerpa_provider_yandex::{Auth, Http, Yandex};

/// Why a run could not be assembled or carried out.
#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    /// The configuration could not be evaluated into a graph.
    #[error(transparent)]
    Configuration(#[from] nerpa_config::ConfigError),

    /// The credential the fleet names cannot be used: a key file that others can
    /// read, a long-lived credential where a key belongs, a key that cannot sign.
    #[error(transparent)]
    Credential(#[from] nerpa_provider_yandex::AuthError),

    /// The world could not be read.
    #[error(transparent)]
    Observe(#[from] RunError),

    /// The graph could not be ordered into a plan.
    #[error(transparent)]
    Plan(#[from] planner::PlanError),

    /// The plan could not be applied.
    #[error(transparent)]
    Apply(RunError),
}

/// The run: providers and the graph they evaluate, ready to plan and apply.
#[derive(Debug)]
pub struct Engine {
    providers: Providers,
    graph: Graph,
}

impl Engine {
    /// Registers the providers the fleet names and evaluates the configuration.
    ///
    /// `file` is the configuration's path as shown in an error, `source` its
    /// text, and `root` the directory templates resolve against.
    ///
    /// # Errors
    ///
    /// Returns [`EngineError::Credential`] when the credential the fleet names
    /// cannot be used, and [`EngineError::Configuration`] when the configuration
    /// does not evaluate.
    pub fn assemble(
        file: &str,
        source: &str,
        root: &Path,
        fleet: &Fleet,
    ) -> Result<Self, EngineError> {
        let providers = providers(fleet)?;
        let graph = nerpa_config::evaluate_with(
            file,
            source,
            Box::new(nerpa_config::template::Directory::at(root)),
            fleet.clone(),
            providers.schema(),
        )?;
        Ok(Self { providers, graph })
    }

    /// The graph the configuration evaluated to, so a front end can open the
    /// machines the inventory names.
    #[must_use]
    pub fn graph(&self) -> &Graph {
        &self.graph
    }

    /// Reads every resource the configuration declares.
    ///
    /// # Errors
    ///
    /// Returns [`EngineError::Observe`] when a resource could not be read.
    pub fn observe(&self, machines: &Machines, state: &State) -> Result<Observed, EngineError> {
        run::observe(&self.graph, &self.providers, machines, state).map_err(Into::into)
    }

    /// Plans the change from what the world is to what the configuration says,
    /// attributed to `actor`.
    ///
    /// # Errors
    ///
    /// Returns [`EngineError::Plan`] when the graph cannot be ordered.
    pub fn plan(
        &self,
        observed: &Observed,
        state: &State,
        secrets: Secrets<'_>,
        actor: Actor,
    ) -> Result<Plan, EngineError> {
        Ok(planner::plan(
            &self.graph,
            observed,
            state,
            &self.providers.schema(),
            secrets,
        )?
        .attributed_to(actor))
    }

    /// Carries the plan out, stage by stage, and returns what it did.
    ///
    /// # Errors
    ///
    /// Returns [`EngineError::Apply`] at the first change that fails.
    pub fn apply(
        &self,
        plan: &Plan,
        machines: &Machines,
        observed: &Observed,
        state: &mut State,
        secrets: Secrets<'_>,
        record: &mut dyn FnMut(&State) -> Result<(), String>,
    ) -> Result<Applied, EngineError> {
        run::apply(
            plan,
            &self.providers,
            machines,
            observed,
            state,
            secrets,
            record,
        )
        .map_err(EngineError::Apply)
    }
}

/// The providers the fleet names: the os provider always, and Yandex where the
/// fleet carries a cloud.
fn providers(fleet: &Fleet) -> Result<Providers, EngineError> {
    let mut providers = Providers::new().with(Box::new(Os::new()));
    // A cloud provider is registered only where the fleet names a cloud; a
    // configuration that touches no cloud asks for no credential and reaches
    // nothing over HTTP. Where a cloud is named, the credential is the
    // short-lived token the fleet carries, the service-account key in the file
    // it names (adr-2026-079), or the yc profile (adr-2026-071). A key file is
    // read here, so one that cannot be used is refused when the fleet is read
    // and not at the first apply.
    if let Some(cloud) = fleet.cloud() {
        let auth = match (&cloud.iam_token, &cloud.key_file) {
            (Some(token), _) => Auth::IamToken(token.clone()),
            (None, Some(path)) => Auth::service_account_key_file(path)?,
            (None, None) => Auth::YcProfile,
        };
        providers = providers.with(Box::new(Yandex::new(
            cloud.folder_id.clone(),
            "https://compute.api.cloud.yandex.net/compute/v1",
            "https://vpc.api.cloud.yandex.net/vpc/v1",
            "https://operation.api.cloud.yandex.net/operations",
            "https://storage.api.cloud.yandex.net/storage/v1",
            "https://storage.yandexcloud.net",
            auth,
            Box::new(Http::new(std::time::Duration::from_secs(30))),
        )));
    }
    Ok(providers)
}
