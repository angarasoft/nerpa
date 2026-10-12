//! A control-domain provider for Yandex Cloud.
//!
//! Resources in the `control` domain have no machine: their state lives behind
//! the Yandex Cloud API, reached from where the engine already runs. This
//! provider owns `yandex:instance`, `yandex:disk`, `yandex:network`,
//! `yandex:subnet` and `yandex:security_group`, and the schema that says which
//! of their attributes change in place, replace the resource, were born with
//! it, only grow, or need a stop to change.

use nerpa_core::address::{Address, Kind, Provider as ProviderName};
use nerpa_core::observed::Facts;
use nerpa_core::provider::{
    Afterwards, Answered, Concluded, Concluding, Planned, Prepared, Provider, ProviderError, Reach,
    Read, Reading, Sending,
};
use nerpa_core::schema::Schema;

/// The Yandex Cloud provider, answering to `yandex` in an address.
#[derive(Debug)]
pub struct Yandex {
    name: ProviderName,
}

impl Yandex {
    /// The provider, answering to `yandex` in an address.
    #[must_use]
    pub fn new() -> Self {
        Self {
            name: ProviderName::new("yandex").expect("`yandex` is a valid provider name"),
        }
    }

    /// The schema for the five kinds this provider owns.
    #[must_use]
    fn schema_of(name: ProviderName) -> Schema {
        #[allow(
            clippy::expect_used,
            reason = "literals that stopped parsing are a bug here"
        )]
        let kind = |n: &str| Kind::new(n).expect("a valid kind");
        Schema::new()
            // An instance's zone and platform cannot move; its boot disk and
            // user data describe how it was made; its cores and memory change
            // only while it is stopped.
            .immutable(name.clone(), kind("instance"), "zone")
            .immutable(name.clone(), kind("instance"), "platform_id")
            .at_birth(name.clone(), kind("instance"), "boot_disk")
            .at_birth(name.clone(), kind("instance"), "user_data")
            .identity(name.clone(), kind("instance"), "name", "name")
            .stop_to_change(name.clone(), kind("instance"), "cores")
            .stop_to_change(name.clone(), kind("instance"), "memory")
            // A disk's type and zone cannot move; its size can only grow; its
            // image describes how it was made.
            .immutable(name.clone(), kind("disk"), "type")
            .immutable(name.clone(), kind("disk"), "zone")
            .monotonic(name.clone(), kind("disk"), "size")
            .at_birth(name.clone(), kind("disk"), "image_id")
            .identity(name.clone(), kind("disk"), "name", "name")
            // A network is only a name and a scope.
            .identity(name.clone(), kind("network"), "name", "name")
            // A subnet's zone, network and CIDR cannot move.
            .immutable(name.clone(), kind("subnet"), "zone")
            .immutable(name.clone(), kind("subnet"), "network_id")
            .immutable(name.clone(), kind("subnet"), "v4_cidr_blocks")
            .identity(name.clone(), kind("subnet"), "name", "name")
            // A security group belongs to one network.
            .immutable(name.clone(), kind("security_group"), "network_id")
            .identity(name, kind("security_group"), "name", "name")
    }
}

impl Default for Yandex {
    fn default() -> Self {
        Self::new()
    }
}

impl Provider for Yandex {
    fn name(&self) -> &ProviderName {
        &self.name
    }

    fn schema(&self) -> Schema {
        Self::schema_of(self.name.clone())
    }

    fn read(
        &self,
        _batch: &[Reading],
        _facts: Facts<'_>,
        _reach: &dyn Reach,
    ) -> Result<Read, ProviderError> {
        // The read path arrives with the API client; until then nothing is
        // claimed to exist.
        Ok(Read::new())
    }

    fn prepare(&self, change: &Prepared<'_>) -> Result<Planned, ProviderError> {
        Err(ProviderError::World {
            address: change.address.clone(),
            problem: "the Yandex provider cannot change a resource yet".to_owned(),
        })
    }

    fn conclude(
        &self,
        change: &Concluding<'_>,
        _became: &[Answered],
    ) -> Result<Concluded, ProviderError> {
        Err(ProviderError::World {
            address: change.address.clone(),
            problem: "the Yandex provider cannot change a resource yet".to_owned(),
        })
    }

    fn afterwards(&self, _address: &Address, _kind: Sending) -> Afterwards {
        Afterwards::Read
    }

    /// Nothing to fake yet: the change path is a stub, so the provider is the
    /// fake until the API client exists.
    fn fake() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nerpa_core::address::address;

    #[test]
    fn the_schema_names_what_changes_in_place_and_what_does_not() {
        let schema = Yandex::new().schema();
        let instance = address("yandex:instance.db");
        let disk = address("yandex:disk.data");
        let subnet = address("yandex:subnet.mgmt");
        let group = address("yandex:security_group.web");

        // An instance's zone cannot move; its boot disk was born; its cores
        // need a stop to change.
        assert!(schema.forces_replacement(&instance, "zone"));
        assert!(schema.is_at_birth(&instance, "boot_disk"));
        assert!(schema.requires_stop(&instance, "cores"));
        assert!(!schema.forces_replacement(&instance, "cores"));

        // A disk's size only grows; its type cannot move.
        assert!(schema.grows_only(&disk, "size"));
        assert!(schema.forces_replacement(&disk, "type"));

        // A subnet's zone and network cannot move.
        assert!(schema.forces_replacement(&subnet, "zone"));
        assert!(schema.forces_replacement(&subnet, "network_id"));

        // A security group belongs to one network.
        assert!(schema.forces_replacement(&group, "network_id"));
    }
}
