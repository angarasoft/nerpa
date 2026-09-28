//! Files written from templates that look like the files they produce.
//!
//! Writing configuration files is most of what configuration management does,
//! and Ansible is genuinely good at it for one reason: somebody opens
//! `nginx.conf.j2` and sees nginx.conf with holes in it. Building the same file
//! by concatenating strings loses that, and with it most of the reason a person
//! can read the configuration at all. So this is Jinja, by way of `minijinja` —
//! see `0023`.
//!
//! # What it will not do
//!
//! No clock, no environment, no filesystem beyond the configuration's own
//! directory, no process, no network. There is no `lookup` and nothing like one.
//! A template that reads the world renders differently every time, and a plan
//! computed from it could not be compared, hashed or signed — the same reasoning
//! that put Starlark rather than a scripting language in `0005`.
//!
//! Values are handed over rather than inherited. There is no ambient scope and
//! no precedence to memorise; Ansible's twenty-two levels are refused here as
//! they are everywhere else.
//!
//! Execution is bounded. `minijinja` counts fuel and limits recursion, and both
//! are set, because a template arriving from somebody's configuration repository
//! is exactly where an unbounded loop should be impossible.
//!
//! # Why the filters are a list rather than whatever the library has
//!
//! Every filter this accepts is part of what nerpa promises not to break
//! (`0022`). Taking whatever `minijinja` ships would mean an upgrade silently
//! widening what a configuration may say, and a configuration written against
//! the wider set failing on an older nerpa with no explanation. So the set is
//! named here, and adding to it is a decision somebody makes on purpose.
//!
//! Method syntax is a **second** surface, not the same one. `pycompat` answers
//! `value.method()` through a callback of its own, which the filter list does
//! not touch — taking it whole put around twenty-five methods nobody had looked
//! at inside the sandbox, and one of them, `str.count("")`, never returns: it
//! advances by the length of what it searched for. Fuel does not stop that,
//! because fuel is charged per instruction of the template's own machine and
//! that loop is inside a call the machine is waiting on. So the methods are a
//! named list too, and it is checked before the callback is reached.

use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};

use minijinja::Environment;
use nerpa_core::Value as CoreValue;

/// How much work one template may do before it is stopped.
///
/// Generous for anything anybody writes on purpose and far below a loop that
/// does not end.
const FUEL: u64 = 2_000_000;

/// How deep `include` and macros may go.
const DEPTH: usize = 32;

/// The methods a template may call on a value.
///
/// Named for the same reason the filters are, and separately, because they
/// arrive by a different door. Deliberately absent: `count`, whose
/// implementation does not terminate when asked to count the empty string, and
/// `format`, which is a second templating language inside this one.
const METHODS: [&str; 17] = [
    // What a mapping offers.
    "items",
    "keys",
    "values",
    "get",
    // What a string offers.
    "upper",
    "lower",
    "title",
    "capitalize",
    "strip",
    "lstrip",
    "rstrip",
    "split",
    "splitlines",
    "replace",
    "startswith",
    "endswith",
    "join",
];

/// Where a template's text comes from.
///
/// A trait rather than a path so that the boundary has a fake behind it, for the
/// same reason `Provider` does: a boundary nothing can stand in for is one every
/// test has to go around.
pub trait Templates: std::fmt::Debug + 'static {
    /// The text of a template, or `None` if there is no such template.
    ///
    /// # Errors
    ///
    /// Returns a message if the name is one this source refuses to resolve, or
    /// if it is there and could not be read.
    fn get(&self, name: &str) -> Result<Option<String>, String>;
}

/// Templates read from the directory the configuration lives in.
#[derive(Debug, Clone)]
pub struct Directory {
    root: PathBuf,
}

impl Directory {
    /// Templates under this directory and nowhere else.
    #[must_use]
    pub fn at(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }
}

impl Templates for Directory {
    fn get(&self, name: &str) -> Result<Option<String>, String> {
        let path = Path::new(name);
        // Refused by shape before anything touches the disk. A canonicalised
        // comparison afterwards would also catch these, but only for a path that
        // exists — and the message for one that does not should still say what
        // was wrong with it rather than that it is missing.
        if path.is_absolute() {
            return Err(format!(
                "{name:?} is an absolute path; templates are named relative to the configuration"
            ));
        }
        if path
            .components()
            .any(|part| matches!(part, Component::ParentDir))
        {
            return Err(format!(
                "{name:?} leaves the configuration's directory, and a template it does not contain is not part of it"
            ));
        }

        let full = self.root.join(path);
        match std::fs::read_to_string(&full) {
            Ok(text) => {
                // The shape check cannot see a symlink pointing out of the tree.
                // This can, and only once there is something to look at.
                let inside = full
                    .canonicalize()
                    .ok()
                    .zip(self.root.canonicalize().ok())
                    .is_some_and(|(reached, root)| reached.starts_with(root));
                if inside {
                    Ok(Some(text))
                } else {
                    Err(format!(
                        "{name:?} leads outside the configuration's directory"
                    ))
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(format!("{}: {error}", full.display())),
        }
    }
}

/// Templates held in memory, for tests and for a configuration with none.
#[derive(Debug, Default, Clone)]
pub struct Held {
    entries: BTreeMap<String, String>,
}

impl Held {
    /// Holding nothing.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// And now holding this.
    #[must_use]
    pub fn holding(mut self, name: impl Into<String>, text: impl Into<String>) -> Self {
        self.entries.insert(name.into(), text.into());
        self
    }
}

impl Templates for Held {
    fn get(&self, name: &str) -> Result<Option<String>, String> {
        Ok(self.entries.get(name).cloned())
    }
}

#[dacc_derive::doc_anchor(id = "inv-template-001")]
/// Everything a template may use, added one at a time to an empty environment.
///
/// Named rather than inherited, for the reason in this module's documentation.
/// An empty environment rather than the library's own with the rest taken away,
/// because then an upgrade that adds a filter does not reach a configuration at
/// all — it has to be let in here first, by somebody deciding to.
fn allowed(environment: &mut Environment<'_>) {
    use minijinja::{filters, tests};

    environment.add_filter("abs", filters::abs);
    environment.add_filter("attr", filters::attr);
    environment.add_filter("batch", filters::batch);
    environment.add_filter("capitalize", filters::capitalize);
    environment.add_filter("default", filters::default);
    environment.add_filter("first", filters::first);
    environment.add_filter("float", filters::float);
    environment.add_filter("indent", filters::indent);
    environment.add_filter("int", filters::int);
    environment.add_filter("items", filters::items);
    environment.add_filter("join", filters::join);
    environment.add_filter("last", filters::last);
    environment.add_filter("length", filters::length);
    environment.add_filter("lines", filters::lines);
    environment.add_filter("list", filters::list);
    environment.add_filter("lower", filters::lower);
    environment.add_filter("map", filters::map);
    environment.add_filter("max", filters::max);
    environment.add_filter("min", filters::min);
    environment.add_filter("reject", filters::reject);
    environment.add_filter("rejectattr", filters::rejectattr);
    environment.add_filter("replace", filters::replace);
    environment.add_filter("reverse", filters::reverse);
    environment.add_filter("round", filters::round);
    environment.add_filter("select", filters::select);
    environment.add_filter("selectattr", filters::selectattr);
    environment.add_filter("slice", filters::slice);
    environment.add_filter("sort", filters::sort);
    environment.add_filter("title", filters::title);
    environment.add_filter("trim", filters::trim);
    environment.add_filter("unique", filters::unique);
    environment.add_filter("upper", filters::upper);

    environment.add_test("boolean", tests::is_boolean);
    environment.add_test("defined", tests::is_defined);
    environment.add_test("even", tests::is_even);
    environment.add_test("false", tests::is_false);
    environment.add_test("integer", tests::is_integer);
    environment.add_test("mapping", tests::is_mapping);
    environment.add_test("none", tests::is_none);
    environment.add_test("number", tests::is_number);
    environment.add_test("odd", tests::is_odd);
    environment.add_test("sequence", tests::is_sequence);
    environment.add_test("string", tests::is_string);
    environment.add_test("true", tests::is_true);
    environment.add_test("undefined", tests::is_undefined);

    environment.add_global(
        "range",
        minijinja::Value::from_function(minijinja::functions::range),
    );
}

/// The renderer, built once and asked many times.
pub(crate) struct Renderer {
    environment: Environment<'static>,
    source: Box<dyn Templates>,
}

impl std::fmt::Debug for Renderer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Renderer").finish_non_exhaustive()
    }
}

impl Renderer {
    /// A renderer that can reach this source and nothing else.
    pub(crate) fn new(source: Box<dyn Templates>) -> Self {
        let mut environment = Environment::empty();
        allowed(&mut environment);

        // A value that is not there is a mistake, not an empty string. For a
        // configuration file the difference is a directive that silently
        // disappears.
        environment.set_undefined_behavior(minijinja::UndefinedBehavior::Strict);
        // A template's last newline is part of the file it looks like. Jinja drops
        // it by default, and a file without one is not always the same file: cron
        // ignores a last line with no newline after it. Found by rendering one
        // template with this and with Ansible, which keeps it, and watching each
        // restart `chronyd` after the other.
        environment.set_keep_trailing_newline(true);
        environment.set_fuel(Some(FUEL));
        environment.set_recursion_limit(DEPTH);
        // Jinja templates call Python methods on values — `.items()`,
        // `.upper()`, `.split()`. `pycompat` answers those, and it answers more
        // than that, through a path the filter list above never sees.
        environment.set_unknown_method_callback(|state, value, method, arguments| {
            if !METHODS.contains(&method) {
                return Err(minijinja::Error::new(
                    minijinja::ErrorKind::UnknownMethod,
                    format!("{method} is not one of the methods nerpa offers"),
                ));
            }
            minijinja_contrib::pycompat::unknown_method_callback(state, value, method, arguments)
        });

        Self {
            environment,
            source,
        }
    }

    /// Renders one template with these values and nothing else in scope.
    pub(crate) fn render(
        &self,
        name: &str,
        values: &BTreeMap<String, CoreValue>,
    ) -> Result<String, String> {
        let Some(text) = self.source.get(name)? else {
            return Err(format!("there is no template called {name:?}"));
        };

        let context: BTreeMap<&str, minijinja::Value> = values
            .iter()
            .map(|(key, value)| Ok((key.as_str(), as_minijinja(value)?)))
            .collect::<Result<_, String>>()?;

        self.environment
            .render_str(&text, minijinja::Value::from(context))
            .map_err(|error| describe(name, &error))
    }
}

/// What went wrong, with the template's name in front of it.
///
/// `minijinja` reports the line and the offending expression; the name has to
/// come from here, because a rendered string does not know what it was called.
fn describe(name: &str, error: &minijinja::Error) -> String {
    use std::fmt::Write as _;

    let mut message = format!("{name}: {error}");
    let mut cause = std::error::Error::source(error);
    while let Some(next) = cause {
        // Writing into a `String` cannot fail, and the result is dropped rather
        // than unwrapped so that the lint against `unwrap` keeps meaning
        // something.
        let _ = write!(message, ": {next}");
        cause = std::error::Error::source(next);
    }
    message
}

/// A value as a template sees it.
///
/// Written out rather than serialised so that adding a variant to the model is a
/// decision here rather than whatever `serde` happens to do with it.
fn as_minijinja(value: &CoreValue) -> Result<minijinja::Value, String> {
    Ok(match value {
        CoreValue::Text(text) => minijinja::Value::from(text.as_str()),
        CoreValue::Integer(number) => minijinja::Value::from(*number),
        CoreValue::Boolean(flag) => minijinja::Value::from(*flag),
        CoreValue::List(items) => minijinja::Value::from(
            items
                .iter()
                .map(as_minijinja)
                .collect::<Result<Vec<_>, _>>()?,
        ),
        CoreValue::Map(entries) => minijinja::Value::from(
            entries
                .iter()
                .map(|(key, item)| Ok((key.as_str(), as_minijinja(item)?)))
                .collect::<Result<BTreeMap<_, _>, String>>()?,
        ),
        CoreValue::Bytes(bytes) => minijinja::Value::from_bytes(bytes.clone()),
        // Refused rather than shown as something else. A kind of value this
        // version cannot render is a kind it cannot render, and quietly turning
        // it into a string would put whatever `Debug` prints into somebody's
        // configuration file.
        other => {
            return Err(format!(
                "a value of this kind cannot be given to a template: {other}"
            ));
        }
    })
}
