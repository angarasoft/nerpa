use dacc_core::NonEmptyStr;

dacc_work::limitation!("l-fleet-skips-unknown-tables",
    title: NonEmptyStr::new("The fleet document skips a table it does not know, and a known one written as something else"),
    violated: NonEmptyStr::new("The fleet's fields are a closed set where they are read — a node's, the cloud's, the approval table's — because a field silently ignored leaves a document that says something no run does. The document itself is not closed: a table it does not know, such as [aproval] for [approval] or [cloud.alibaba], and a known table written as something else, such as approval = a string, are skipped without a word. A fleet that names no approvers asks for no signature, so a misspelt approval table turns the signature requirement off and says nothing."),
);
