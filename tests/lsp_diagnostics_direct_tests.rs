// Direct LSP diagnostics tests - test check_document logic directly
// These tests work around the need for a full LSP client setup

use pain_compiler::{
    error::ErrorFormatter, parse_with_recovery,
    type_check_program_with_context, type_checker::TypeContext, warnings::WarningCollector,
};
use tower_lsp::lsp_types::*;

/// Helper to create diagnostics like Backend::check_document does
fn check_document_direct(text: &str) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    // Parse with error recovery
    let (parse_result, parse_errors) = parse_with_recovery(text);

    // Add parse errors as diagnostics
    for parse_err in &parse_errors {
        diagnostics.push(Diagnostic {
            range: Range {
                start: Position {
                    line: (parse_err.span.line().saturating_sub(1)) as u32,
                    character: (parse_err.span.column().saturating_sub(1)) as u32,
                },
                end: Position {
                    line: (parse_err.span.line().saturating_sub(1)) as u32,
                    character: (parse_err.span.column().saturating_sub(1) + 1) as u32,
                },
            },
            severity: Some(DiagnosticSeverity::ERROR),
            code: None,
            code_description: None,
            source: Some("pain".to_string()),
            message: parse_err.message.clone(),
            related_information: None,
            tags: None,
            data: None,
        });
    }

    // If parsing succeeded, try type checking
    if let Ok(program) = parse_result {
        let mut ctx = TypeContext::new();
        for item in &program.items {
            match item {
                pain_compiler::ast::Item::Function(func) => {
                    ctx.add_function(func.name.clone(), func.clone());
                }
                pain_compiler::ast::Item::Class(class) => {
                    ctx.add_class(class.name.clone(), class.clone());
                }
            }
        }

        match type_check_program_with_context(&program, &mut ctx) {
            Ok(_) => {
                let warnings = WarningCollector::collect_warnings(&program, &ctx);
                for warning in warnings {
                    let (message, span) = match warning {
                        pain_compiler::Warning::UnusedVariable { name, span } => {
                            (format!("unused variable `{}`", name), span)
                        }
                        pain_compiler::Warning::UnusedFunction { name, span } => {
                            (format!("unused function `{}`", name), span)
                        }
                        pain_compiler::Warning::DeadCode { span, reason } => {
                            (format!("dead code: {}", reason), span)
                        }
                        pain_compiler::Warning::UnreachableCode { span } => {
                            ("unreachable code".to_string(), span)
                        }
                    };

                    diagnostics.push(Diagnostic {
                        range: Range {
                            start: Position {
                                line: (span.line().saturating_sub(1)) as u32,
                                character: (span.column().saturating_sub(1)) as u32,
                            },
                            end: Position {
                                line: (span.line().saturating_sub(1)) as u32,
                                character: (span.column().saturating_sub(1) + 1) as u32,
                            },
                        },
                        severity: Some(DiagnosticSeverity::WARNING),
                        code: None,
                        code_description: None,
                        source: Some("pain".to_string()),
                        message,
                        related_information: None,
                        tags: None,
                        data: None,
                    });
                }
            }
            Err(err) => {
                let formatter = ErrorFormatter::new(text).with_context(&ctx);
                let error_msg = formatter.format_error(&err);
                let span = match &err {
                    pain_compiler::TypeError::UndefinedVariable { span, .. } => *span,
                    pain_compiler::TypeError::TypeMismatch { span, .. } => *span,
                    pain_compiler::TypeError::CannotInferType { span, .. } => *span,
                    pain_compiler::TypeError::InvalidOperation { span, .. } => *span,
                };

                diagnostics.push(Diagnostic {
                    range: Range {
                        start: Position {
                            line: (span.line().saturating_sub(1)) as u32,
                            character: (span.column().saturating_sub(1)) as u32,
                        },
                        end: Position {
                            line: (span.line().saturating_sub(1)) as u32,
                            character: (span.column().saturating_sub(1) + 1) as u32,
                        },
                    },
                    severity: Some(DiagnosticSeverity::ERROR),
                    code: None,
                    code_description: None,
                    source: Some("pain".to_string()),
                    message: error_msg
                        .lines()
                        .next()
                        .unwrap_or(&error_msg)
                        .to_string(),
                    related_information: None,
                    tags: None,
                    data: None,
                });
            }
        }
    }

    diagnostics
}

#[test]
fn test_valid_code_no_diagnostics() {
    let code = r#"
fn main():
    print("Hello, Pain!")
"#;

    let diagnostics = check_document_direct(code);
    let errors: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.severity == Some(DiagnosticSeverity::ERROR))
        .collect();
    assert_eq!(errors.len(), 0, "Valid code should have no errors");
}

#[test]
fn test_undefined_variable_error() {
    let code = r#"
fn main():
    let x = undefined_variable
"#;

    let diagnostics = check_document_direct(code);
    let errors: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.severity == Some(DiagnosticSeverity::ERROR))
        .collect();
    assert!(!errors.is_empty(), "Should detect undefined variable error");
    assert!(
        errors[0].message.contains("undefined") || errors[0].message.contains("Undefined"),
        "Error message should mention undefined variable"
    );
}

#[test]
fn test_type_mismatch_error() {
    let code = r#"
fn main():
    let x: int = "string"
"#;

    let diagnostics = check_document_direct(code);
    let errors: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.severity == Some(DiagnosticSeverity::ERROR))
        .collect();
    assert!(!errors.is_empty(), "Should detect type mismatch error");
}

#[test]
fn test_parse_error() {
    let code = r#"
fn main():
    let x =  # Incomplete statement
"#;

    let diagnostics = check_document_direct(code);
    let errors: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.severity == Some(DiagnosticSeverity::ERROR))
        .collect();
    assert!(!errors.is_empty(), "Should detect parse error");
}

#[test]
fn test_unused_variable_warning() {
    let code = r#"
fn main():
    let unused = 10
    print("test")
"#;

    let diagnostics = check_document_direct(code);
    let warnings: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.severity == Some(DiagnosticSeverity::WARNING))
        .collect();
    assert!(!warnings.is_empty(), "Should detect unused variable warning");
}

#[test]
fn test_function_with_parameters() {
    let code = r#"
fn add(a: int, b: int) -> int:
    return a + b

fn main():
    let result = add(1, 2)
"#;

    let diagnostics = check_document_direct(code);
    let errors: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.severity == Some(DiagnosticSeverity::ERROR))
        .collect();
    assert_eq!(errors.len(), 0, "Valid function with parameters should have no errors");
}

#[test]
fn test_classes() {
    let code = r#"
class Point:
    let x: int
    let y: int
    
    fn new(x: int, y: int) -> Point:
        let p = Point()
        p.x = x
        p.y = y
        return p

fn main():
    let p = Point.new(10, 20)
"#;

    let diagnostics = check_document_direct(code);
    let errors: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.severity == Some(DiagnosticSeverity::ERROR))
        .collect();
    assert_eq!(errors.len(), 0, "Valid class code should have no errors");
}

#[test]
fn test_control_flow() {
    let code = r#"
fn max(a: int, b: int) -> int:
    if a > b:
        return a
    else:
        return b

fn sum(n: int) -> int:
    var result = 0
    var i = 0
    while i <= n:
        result = result + i
        i = i + 1
    return result
"#;

    let diagnostics = check_document_direct(code);
    let errors: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.severity == Some(DiagnosticSeverity::ERROR))
        .collect();
    assert_eq!(errors.len(), 0, "Valid control flow code should have no errors");
}

#[test]
fn test_lists_and_arrays() {
    let code = r#"
fn main() -> int:
    let numbers = [1, 2, 3, 4, 5]
    let sum = 0
    let i = 0
    while i < len(numbers):
        sum = sum + numbers[i]
        i = i + 1
    return sum
"#;

    let diagnostics = check_document_direct(code);
    let errors: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.severity == Some(DiagnosticSeverity::ERROR))
        .collect();
    assert_eq!(errors.len(), 0, "Valid list/array code should have no errors");
}

#[test]
fn test_fibonacci_example() {
    let code = r#"
fn fib(n: int) -> int:
    if n <= 1:
        return n
    return fib(n - 1) + fib(n - 2)

fn main() -> int:
    return fib(20)
"#;

    let diagnostics = check_document_direct(code);
    let errors: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.severity == Some(DiagnosticSeverity::ERROR))
        .collect();
    assert_eq!(errors.len(), 0, "Fibonacci example should have no errors");
}

#[test]
fn test_sum_example() {
    let code = r#"
fn sum(n: int) -> int:
    var result = 0
    var i = 0
    var limit = n
    while i <= limit:
        result = result + i
        i = i + 1
    return result

fn main() -> int:
    return sum(10000)
"#;

    let diagnostics = check_document_direct(code);
    let errors: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.severity == Some(DiagnosticSeverity::ERROR))
        .collect();
    assert_eq!(errors.len(), 0, "Sum example should have no errors");
}

