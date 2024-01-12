use crate::parser::fortran_language;
use crate::rules::{Code, Violation};
/// Defines rules that check whether functions and subroutines are defined within modules,
/// submodules, or interfaces. It is also acceptable to define nested functions or subroutines.
use tree_sitter::{Node, Query};

pub const USE_MODULES_AND_PROGRAMS: &str = "\
    Functions and subroutines should be contained within (sub)modules or programs.
    Fortran compilers are unable to perform type checks and conversions on functions
    defined outside of these scopes, and this is a common source of bugs.";

pub fn use_modules_and_programs(code: Code, root: &Node, src: &str) -> Vec<Violation> {
    let mut violations = Vec::new();
    let query_txt = "(translation_unit [(function) @func (subroutine) @sub])";
    let query = Query::new(fortran_language(), query_txt).unwrap();
    let mut cursor = tree_sitter::QueryCursor::new();
    for match_ in cursor.matches(&query, *root, src.as_bytes()) {
        for capture in match_.captures {
            let node = capture.node;
            violations.push(Violation::from_node(
                &node,
                code,
                format!(
                    "{} not contained within (sub)module or program",
                    node.kind(),
                )
                .as_str(),
            ));
        }
    }
    violations
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::test_utils::{test_tree_method, TEST_CODE};

    #[test]
    fn test_function_not_in_module() {
        let source = "
            integer function double(x)
              integer, intent(in) :: x
              double = 2 * x
            end function

            subroutine triple(x)
              integer, intent(inout) :: x
              x = 3 * x
            end subroutine
            ";
        let expected_violations = [2, 7]
            .iter()
            .zip(["function", "subroutine"])
            .map(|(line, kind)| {
                Violation::new(
                    *line,
                    TEST_CODE,
                    format!("{} not contained within (sub)module or program", kind).as_str(),
                )
            })
            .collect();
        test_tree_method(use_modules_and_programs, source, Some(expected_violations));
    }

    #[test]
    fn test_function_in_module() {
        let source = "
            module my_module
                implicit none
            contains
                integer function double(x)
                  integer, intent(in) :: x
                  double = 2 * x
                end function

                subroutine triple(x)
                  integer, intent(inout) :: x
                  x = 3 * x
                end subroutine
            end module
            ";
        test_tree_method(use_modules_and_programs, source, None);
    }
}