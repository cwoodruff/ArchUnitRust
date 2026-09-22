use std::fmt;

use regex::Regex;

use crate::base::{ArchUnitError, DescribedPredicate, join_single_quoted};

/// Matches module paths against a package identifier (`PackageMatcher`).
///
/// The syntax is ArchUnit's, with `::` as the separator:
///
/// | Pattern | Meaning |
/// |---|---|
/// | `*` | any non-empty sequence of characters without `::`, i.e. (part of) one segment |
/// | `..` | any number of segments, including none |
/// | `(*)` / `(**)` | like `*` / `..` but captured |
/// | `[a\|b]` | alternation |
/// | leading `crate` | matches the root segment of any crate |
///
/// ```
/// use archunit::core::domain::PackageMatcher;
///
/// assert!(PackageMatcher::of("..service..").matches("my_app::service::internal"));
/// assert!(!PackageMatcher::of("my_app::*").matches("my_app::service::internal"));
/// let result = PackageMatcher::of("my_app::(*)..").match_("my_app::service::internal").unwrap();
/// assert_eq!(result.group(1), "service");
/// ```
#[derive(Debug, Clone)]
pub struct PackageMatcher {
    identifier: String,
    regex: Regex,
}

/// The result of a successful match with its captured groups.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchResult {
    groups: Vec<String>,
}

impl MatchResult {
    /// The captured segment(s) of the `number`th capturing group (1-based).
    ///
    /// # Panics
    /// If `number` is 0 or greater than [`number_of_groups`](Self::number_of_groups).
    pub fn group(&self, number: usize) -> &str {
        assert!(
            number >= 1 && number <= self.groups.len(),
            "Group number {number} out of range 1..={}",
            self.groups.len()
        );
        &self.groups[number - 1]
    }

    /// The number of capturing groups.
    pub fn number_of_groups(&self) -> usize {
        self.groups.len()
    }

    /// All captured groups in order.
    pub fn groups(&self) -> &[String] {
        &self.groups
    }
}

const TWO_DOTS_REGEX: &str = r"(?:(?:^\w*)?::(?:\w+::)*(?:\w*$)?)?";
const TWO_STAR_CAPTURE_REGEX: &str = r"(\w+(?:::\w+)*)";
const MARKER: &str = "\u{0}";

impl PackageMatcher {
    /// Creates a matcher; panics on an illegal identifier (use [`try_of`](Self::try_of) for a `Result`).
    pub fn of(package_identifier: &str) -> Self {
        Self::try_of(package_identifier).unwrap_or_else(|e| panic!("{e}"))
    }

    /// Creates a matcher, reporting illegal identifiers as an error.
    pub fn try_of(package_identifier: &str) -> Result<Self, ArchUnitError> {
        validate(package_identifier)?;
        let regex = Regex::new(&convert_to_regex(package_identifier)).map_err(|e| {
            ArchUnitError::IllegalPackageIdentifier(format!(
                "Package Identifier '{package_identifier}' is invalid: {e}"
            ))
        })?;
        Ok(Self {
            identifier: package_identifier.to_owned(),
            regex,
        })
    }

    /// Whether `package` matches.
    pub fn matches(&self, package: &str) -> bool {
        self.regex.is_match(package)
    }

    /// Matches and returns the captured groups, if any match.
    pub fn match_(&self, package: &str) -> Option<MatchResult> {
        self.regex.captures(package).map(|captures| MatchResult {
            groups: captures
                .iter()
                .skip(1)
                .map(|g| g.map(|m| m.as_str().to_owned()).unwrap_or_default())
                .collect(),
        })
    }

    /// The identifier this matcher was created from.
    pub fn identifier(&self) -> &str {
        &self.identifier
    }
}

impl fmt::Display for PackageMatcher {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.identifier)
    }
}

fn validate(identifier: &str) -> Result<(), ArchUnitError> {
    let err = |msg: String| Err(ArchUnitError::IllegalPackageIdentifier(msg));
    if identifier.contains("...") {
        return err(format!(
            "Package Identifier '{identifier}' may not contain more than two consecutive dots"
        ));
    }
    if identifier.replace("(**)", "").contains("**") {
        return err(format!(
            "Package Identifier '{identifier}' may only contain '**' as capturing group '(**)'"
        ));
    }
    if identifier.contains("(..)") {
        return err(format!(
            "Package Identifier '{identifier}' may not contain '(..)'; use '(**)' instead"
        ));
    }
    let nested = Regex::new(r"\[[^\]]*[()]|\([^)]*\[").expect("static regex");
    if nested.is_match(identifier) {
        return err(format!(
            "Package Identifier '{identifier}' may not nest '()' and '[]'"
        ));
    }
    let bracket_without_alternation = Regex::new(r"\[[^|\]]*\]").expect("static regex");
    if bracket_without_alternation.is_match(identifier) {
        return err(format!(
            "Package Identifier '{identifier}' may only use '[..]' for alternations like '[a|b]'"
        ));
    }
    let mut depth = 0i32;
    for ch in identifier.chars() {
        match ch {
            '(' | '[' => depth += 1,
            ')' | ']' => depth -= 1,
            '|' if depth == 0 => {
                return err(format!(
                    "Package Identifier '{identifier}' may only use '|' inside '[..]' or '(..)'"
                ));
            }
            _ => {}
        }
    }
    let allowed = Regex::new(r"^[\w:.*()\[\]|]*$").expect("static regex");
    if !allowed.is_match(identifier) {
        return err(format!(
            "Package Identifier '{identifier}' may only consist of valid Rust identifier parts or the symbols ':.)(*[]|'"
        ));
    }
    Ok(())
}

fn convert_to_regex(identifier: &str) -> String {
    let alternation = Regex::new(r"\[(.*?)\]").expect("static regex");
    let mut pattern = identifier.to_owned();
    // A leading `crate` segment stands for the root segment of any crate.
    if pattern == "crate" {
        pattern = "*".to_owned();
    } else if let Some(rest) = pattern.strip_prefix("crate::") {
        pattern = format!("*::{rest}");
    } else if let Some(rest) = pattern.strip_prefix("crate..") {
        pattern = format!("*..{rest}");
    }
    let pattern = alternation.replace_all(&pattern, "(?:$1)").into_owned();
    let pattern = pattern
        .replace("(**)", MARKER)
        .replace('*', r"\w+")
        .replace(MARKER, TWO_STAR_CAPTURE_REGEX)
        .replace("..", TWO_DOTS_REGEX);
    format!("^{pattern}$")
}

/// Factory for predicates over package names (`PackageMatchers`).
#[derive(Debug)]
pub struct PackageMatchers;

impl PackageMatchers {
    /// A predicate on package names matching any of the identifiers;
    /// described as `matches any of ['a', 'b']`.
    pub fn of(package_identifiers: &[&str]) -> DescribedPredicate<String> {
        let matchers: Vec<PackageMatcher> = package_identifiers
            .iter()
            .map(|id| PackageMatcher::of(id))
            .collect();
        let description = format!(
            "matches any of [{}]",
            join_single_quoted(package_identifiers)
        );
        DescribedPredicate::describe(description, move |package: &String| {
            matchers.iter().any(|m| m.matches(package))
        })
    }
}
