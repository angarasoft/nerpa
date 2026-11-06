use dacc_core::NonEmptyStr;

dacc_work::limitation!("l-put-optional",
    title: NonEmptyStr::new("The PUT verb is optional on the wire seam"),
    violated: NonEmptyStr::new("A seam's contract is what the compiler enforces of it. The Api seam made its fifth verb a method with a refusing body, so a wire that cannot write the versioning document compiles and meets that fact at apply, on a live cloud, in a transport message. Versioning is a declared function of the provider, not an option; its verb is not an option either. The JSON verbs also append a Content-Type after the caller's headers rather than setting one, so a caller that names one sends two."),
);
