use dacc_core::{BlastRadius, NonEmptyStr};

dacc_work::slice!("s-deb-packages",
    title: NonEmptyStr::new("The os provider manages packages on Debian-like systems"),
    thrust: crate::thrust::t_converges_to_invariants,
    outcome: NonEmptyStr::new(
        "The os provider knows a third family — dpkg to read, apt-get to act — so a Debian or Ubuntu machine converges with an empty second run, like a Rhel or Gentoo one."
    ),
    specification: crate::rfc::rfc_2026_002,
    max_radius: BlastRadius::Local,
);
