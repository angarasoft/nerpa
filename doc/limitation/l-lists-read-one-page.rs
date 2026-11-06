use dacc_core::NonEmptyStr;

dacc_work::limitation!("l-lists-read-one-page",
    title: NonEmptyStr::new("The three lists read one page of a folder and call it the whole folder"),
    violated: NonEmptyStr::new("A folder larger than one page is a folder like any other, and a read of it answers everything the world holds. The API answers its lists in pages and names the next one; the provider's instance, network and subnet lists take the first page and stop, so a resource past that boundary reads as absent — and the plan proposes creating a second copy of a thing that exists, while the read-back after a create reports the thing it just made as missing. For a fleet of a thousand machines this is the main road, not a corner. The bucket reads by name and never meets the boundary: the right approach is already in the tree, and the three older lists were left as they were."),
);
