use dacc_core::NonEmptyStr;

dacc_work::limitation!("l-bucket-name-unchecked",
    title: NonEmptyStr::new("A bucket's name goes into a URL path unchecked"),
    violated: NonEmptyStr::new("A name is only a name while the wire it addresses agrees it is one. The address grammar permits a slash in a foreign name — portage paths asked for that — and a bucket's name goes into the URL path as it stands, so yandex:bucket.a/b addresses a/b: another resource's path, not a misspelling. The fleet document refuses an ssh host that starts with a dash, and an executor path with a tilde, for exactly this reason: names arrive from swapped, shared, generated documents. The cloud's names had no such wall."),
);
