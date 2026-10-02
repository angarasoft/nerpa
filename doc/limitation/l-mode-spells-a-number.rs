use dacc_core::NonEmptyStr;

dacc_work::limitation!("l-mode-spells-a-number",
    title: NonEmptyStr::new("A file mode does not converge from text"),
    violated: NonEmptyStr::new("A file mode is written as text and compared as the number it spells; a number, or text that spells no mode a file can have, is refused while planning."),
);
