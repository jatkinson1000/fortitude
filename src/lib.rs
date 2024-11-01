mod ast;
pub mod check;
pub mod cli;
pub mod explain;
mod rules;
mod settings;
use annotate_snippets::{Level, Renderer, Snippet};
use ast::{parse, FortitudeNode};
use colored::{ColoredString, Colorize};
use ruff_diagnostics::{Diagnostic, DiagnosticKind};
use ruff_source_file::{OneIndexed, SourceFile, SourceLocation};
use ruff_text_size::{TextRange, TextSize};
use settings::Settings;
use std::cmp::Ordering;
use std::fmt;
use std::path::Path;
use tree_sitter::Node;

// Rule categories and identity codes
// ----------------------------------
// Helps to sort rules into logical categories, and acts as a unique tag with which
// users can switch rules on and off.

/// The category of each rule defines the sort of problem it intends to solve.
#[allow(dead_code)]
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Category {
    /// Failure to parse a file.
    Error,
    /// Violation of style conventions.
    Style,
    /// Misuse of types and kinds.
    Typing,
    /// Failure to use modules or use them appropriately.
    Modules,
    /// Best practices for setting floating point precision.
    Precision,
    /// Check path names, directory structures, etc.
    FileSystem,
}

#[allow(dead_code)]
impl Category {
    fn from(s: &str) -> anyhow::Result<Self> {
        match s {
            "E" => Ok(Self::Error),
            "S" => Ok(Self::Style),
            "T" => Ok(Self::Typing),
            "M" => Ok(Self::Modules),
            "P" => Ok(Self::Precision),
            "F" => Ok(Self::FileSystem),
            _ => {
                anyhow::bail!("{} is not a rule category.", s)
            }
        }
    }
}

// Violation type
// --------------

/// The location within a file at which a violation was detected
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ViolationPosition {
    None,
    Range(TextRange),
}

// This type is created when a rule is broken. As not all broken rules come with a
// line number or column, it is recommended to use the `violation!` macro to create
// these, or the `from_node()` function when creating them from `tree_sitter` queries.
#[derive(Debug, Eq, PartialEq, Clone)]
pub struct FortitudeViolation {
    /// Description of the error.
    message: String,
    /// The location at which the error was detected.
    range: ViolationPosition,
}

impl FortitudeViolation {
    pub fn new_no_range<T: AsRef<str>>(message: T) -> Self {
        Self {
            message: String::from(message.as_ref()),
            range: ViolationPosition::None,
        }
    }

    pub fn message(&self) -> &str {
        self.message.as_str()
    }

    pub fn range(&self) -> ViolationPosition {
        self.range
    }
}

pub trait FromTSNode {
    fn from_node<T: Into<DiagnosticKind>>(violation: T, node: &Node) -> Self;
}

impl FromTSNode for Diagnostic {
    fn from_node<T: Into<DiagnosticKind>>(violation: T, node: &Node) -> Self {
        Self::new(
            violation,
            TextRange::new(
                TextSize::try_from(node.start_byte()).unwrap(),
                TextSize::try_from(node.end_byte()).unwrap(),
            ),
        )
    }
}

pub trait FromStartEndLineCol {
    /// Create new `Violation` from zero-index start/end line/column numbers
    fn from_start_end_line_col<T: Into<DiagnosticKind>>(
        kind: T,
        source: &SourceFile,
        start_line: usize,
        start_col: usize,
        end_line: usize,
        end_col: usize,
    ) -> Self;
}

impl FromStartEndLineCol for Diagnostic {
    fn from_start_end_line_col<T: Into<DiagnosticKind>>(
        kind: T,
        source: &SourceFile,
        start_line: usize,
        start_col: usize,
        end_line: usize,
        end_col: usize,
    ) -> Self {
        let source_code = source.to_source_code();
        let start_line_offset = source_code.line_start(OneIndexed::from_zero_indexed(start_line));
        let start_offset = start_line_offset + TextSize::try_from(start_col).unwrap();
        let end_line_offset = source_code.line_start(OneIndexed::from_zero_indexed(end_line));
        let end_offset = end_line_offset + TextSize::try_from(end_col).unwrap();
        Diagnostic::new(kind, TextRange::new(start_offset, end_offset))
    }
}

// Rule trait
// ----------

/// Implemented by all rules.
pub trait Rule {
    fn new(settings: &Settings) -> Self
    where
        Self: Sized;
}

/// Implemented by rules that act directly on the file path.
pub trait PathRule: Rule {
    fn check(&self, path: &Path) -> Option<Diagnostic>;
}

/// Implemented by rules that analyse lines of code directly, using regex or otherwise.
pub trait TextRule: Rule {
    fn check(&self, source: &SourceFile) -> Vec<Diagnostic>;
}

/// Implemented by rules that analyse the abstract syntax tree.
pub trait ASTRule: Rule {
    fn check(&self, node: &Node, source: &SourceFile) -> Option<Vec<Diagnostic>>;

    /// Return list of tree-sitter node types on which a rule should trigger.
    fn entrypoints(&self) -> Vec<&'static str>;

    /// Apply a rule over some text, generating all violations raised as a result.
    fn apply(&self, source: &SourceFile) -> anyhow::Result<Vec<Diagnostic>> {
        let entrypoints = self.entrypoints();
        Ok(parse(source.source_text())?
            .root_node()
            .named_descendants()
            .filter(|x| entrypoints.contains(&x.kind()))
            .filter_map(|x| self.check(&x, source))
            .flatten()
            .collect())
    }
}

// Violation diagnostics
// ---------------------

/// Reports of each violation. They are pretty-printable and sortable.
#[derive(Eq)]
pub struct FortitudeDiagnostic<'a> {
    /// The file where an error was reported.
    file: &'a SourceFile,
    /// The rule code that was violated, expressed as a string.
    code: String,
    /// The specific violation detected
    violation: FortitudeViolation,
}

impl<'a> FortitudeDiagnostic<'a> {
    pub fn from_ruff<S: AsRef<str>>(
        file: &'a SourceFile,
        code: S,
        diagnostic: &Diagnostic,
    ) -> Self {
        Self {
            file,
            code: code.as_ref().to_string(),
            violation: FortitudeViolation {
                message: diagnostic.kind.body.clone(),
                // TODO: handle TextRange::default case for file extensions
                range: ViolationPosition::Range(diagnostic.range),
            },
        }
    }

    fn orderable(&self) -> (&str, usize, usize, &str) {
        match self.violation.range() {
            ViolationPosition::None => (self.file.name(), 0, 0, self.code.as_str()),
            ViolationPosition::Range(range) => (
                self.file.name(),
                range.start().into(),
                range.end().into(),
                self.code.as_str(),
            ),
        }
    }
}

impl<'a> Ord for FortitudeDiagnostic<'a> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.orderable().cmp(&other.orderable())
    }
}

impl<'a> PartialOrd for FortitudeDiagnostic<'a> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<'a> PartialEq for FortitudeDiagnostic<'a> {
    fn eq(&self, other: &Self) -> bool {
        self.orderable() == other.orderable()
    }
}

impl<'a> fmt::Display for FortitudeDiagnostic<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let path = self.file.name().bold();
        let code = self.code.bold().bright_red();
        let message = self.violation.message();
        match self.violation.range() {
            ViolationPosition::None => {
                write!(f, "{}: {} {}", path, code, message)
            }
            ViolationPosition::Range(range) => {
                format_violation(self, f, &range, message, &path, &code)
            }
        }
    }
}

fn format_violation(
    diagnostic: &FortitudeDiagnostic,
    f: &mut fmt::Formatter,
    range: &TextRange,
    message: &str,
    path: &ColoredString,
    code: &ColoredString,
) -> fmt::Result {
    let source_code = diagnostic.file.to_source_code();
    let content_start_index = source_code.line_index(range.start());
    let mut start_index = content_start_index.saturating_sub(2);

    // Trim leading empty lines.
    while start_index < content_start_index {
        if !source_code.line_text(start_index).trim().is_empty() {
            break;
        }
        start_index = start_index.saturating_add(1);
    }

    let content_end_index = source_code.line_index(range.end());
    let mut end_index = content_end_index
        .saturating_add(2)
        .min(OneIndexed::from_zero_indexed(source_code.line_count()));

    // Trim leading empty lines.
    while end_index > content_end_index {
        if !source_code.line_text(end_index).trim().is_empty() {
            break;
        }
        end_index = end_index.saturating_sub(1);
    }

    let start_offset = source_code.line_start(start_index);
    let end_offset = source_code.line_end(end_index);

    let source = source_code.slice(TextRange::new(start_offset, end_offset));
    let message_range = range - start_offset;

    let start_char = source[TextRange::up_to(message_range.start())]
        .chars()
        .count();
    let end_char = source[TextRange::up_to(message_range.end())]
        .chars()
        .count();

    // Some annoyance here: we *have* to have some level prefix to our
    // message. Might be fixed in future version of annotate-snippets
    // -- or we use an earlier version with more control.
    // Also, we could use `.origin(path)` to get the filename and
    // line:col automatically, but there is currently a bug in the
    // library when annotating the start of the line
    let SourceLocation { row, column } = source_code.source_location(range.start());
    let message_line = format!("{path}:{row}:{column}: {code} {message}");
    let snippet = Level::Warning.title(&message_line).snippet(
        Snippet::source(source)
            .line_start(start_index.get())
            .annotation(Level::Error.span(start_char..end_char).label(code)),
    );

    let renderer = Renderer::styled();
    let source_block = renderer.render(snippet);
    writeln!(f, "{}", source_block)
}

pub trait SourceLocationToOffset {
    fn line_location(&self, row: usize, column: u32) -> SourceLocation;
}

impl SourceLocationToOffset for SourceFile {
    fn line_location(&self, row: usize, column: u32) -> SourceLocation {
        let source_code = self.to_source_code();
        let offset =
            source_code.line_start(OneIndexed::from_zero_indexed(row)) + TextSize::new(column);
        source_code.source_location(offset)
    }
}

/// Simplify making a `SourceFile` in tests
#[cfg(test)]
pub fn test_file(source: &str) -> SourceFile {
    use ruff_source_file::SourceFileBuilder;
    use textwrap::dedent;

    SourceFileBuilder::new("test.f90", dedent(source)).finish()
}
