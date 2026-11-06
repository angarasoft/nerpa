use dacc_core::NonEmptyStr;

dacc_work::limitation!("l-empty-folder-reads-as-refusal",
    title: NonEmptyStr::new("An empty folder's list is read as a refusal rather than an empty world"),
    violated: NonEmptyStr::new("A folder that holds nothing is a world like any other, and a read of it answers that nothing is there. The provider's list calls demand their field — instances, networks, subnets — and the API omits a list that is empty, because proto3 JSON omits the zero value, so an empty folder answers `{}` and the read takes it for a transport failure naming a missing field. A first plan against a fresh folder cannot even be written: the observation of an empty world is a refusal. The bucket list met the same answer and was fixed with the bucket kind; the three older kinds were not, because every test gave them a list with something in it."),
);
