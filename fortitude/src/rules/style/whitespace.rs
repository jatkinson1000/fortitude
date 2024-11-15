use ruff_diagnostics::{Diagnostic, Violation};
use ruff_macros::{derive_message_formats, violation};
use ruff_source_file::SourceFile;

use crate::settings::Settings;
use crate::{FromStartEndLineCol, TextRule};
/// Defines rules that enforce widely accepted whitespace rules.

/// ## What does it do?
/// Checks for tailing whitespace
///
/// ## Why is this bad?
/// Trailing whitespace is difficult to spot, and as some editors will remove it
/// automatically while others leave it, it can cause unwanted 'diff noise' in
/// shared projects.
#[violation]
pub struct TrailingWhitespace {}

impl Violation for TrailingWhitespace {
    #[derive_message_formats]
    fn message(&self) -> String {
        format!("trailing whitespace")
    }
}
impl TextRule for TrailingWhitespace {
    fn check(_settings: &Settings, source: &SourceFile) -> Vec<Diagnostic> {
        let mut violations = Vec::new();
        for (idx, line) in source.source_text().split('\n').enumerate() {
            if line.ends_with([' ', '\t']) {
                violations.push(Diagnostic::from_start_end_line_col(
                    Self {},
                    source,
                    idx,
                    line.trim_end().len(),
                    idx,
                    line.len(),
                ));
            }
        }
        violations
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::default_settings;
    use pretty_assertions::assert_eq;
    use ruff_source_file::SourceFileBuilder;

    #[test]
    fn test_trailing_whitespace() -> anyhow::Result<()> {
        // Careful of trailing whitespace in this string! That's the
        // point of the test! Also not using `dedent` here as it
        // messes with the whitespace-only line
        let source = "
program test  
  implicit none
  integer :: a(3) = [ & 
    1, &
    2, &
    3 &
  ]    
   
end program test
";
        let file = SourceFileBuilder::new("test", source).finish();

        let expected: Vec<Diagnostic> =
            [(0, 13, 0, 15), (3, 23, 3, 24), (7, 3, 7, 7), (8, 0, 8, 3)]
                .iter()
                .map(|(start_line, start_col, end_line, end_col)| {
                    Diagnostic::from_start_end_line_col(
                        TrailingWhitespace {},
                        &file,
                        *start_line,
                        *start_col,
                        *end_line,
                        *end_col,
                    )
                })
                .collect();
        let actual = TrailingWhitespace::check(&default_settings(), &file);
        assert_eq!(actual, expected);
        Ok(())
    }
}
