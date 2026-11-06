use dacc_core::NonEmptyStr;

dacc_work::limitation!("l-patch-resets-unmanaged",
    title: NonEmptyStr::new("A bucket update resets the object lock and the private-endpoint policy it never declared"),
    violated: NonEmptyStr::new("An attribute the declaration does not name is reported and never turned into work. The bucket update body carries hard-coded words for the object lock and the private-endpoint policy — a WORM setting among them — and the storage API applies what the body carries whether the mask names it or not, so correcting a tag resets both. The read carries the two fields back in its answer; the constants simply ignored it, and the recording's bucket held the same values the constants name, which is why every test stayed green while the door was open."),
);
