// LSP diagnostics tests - test error and warning detection

use pain_lsp::Backend;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Create a test backend for testing check_document
/// Since check_document doesn't use the client, we can create a minimal backend
fn create_test_backend() -> Backend {
    // Create a minimal backend - we'll use a dummy client since check_document doesn't need it
    // In a real implementation, we'd use tower-lsp's test framework
    let (service, _socket) = tower_lsp::LspService::new(|client| Backend {
        client,
        documents: Arc::new(RwLock::new(HashMap::new())),
    });
    
    // We can't easily extract the backend from the service
    // Instead, let's create a helper that directly tests check_document
    // by creating a minimal backend structure
    // For now, we'll test the check_document logic directly
    todo!("Implement proper test backend - need to refactor Backend to separate check_document logic")
}

#[tokio::test]
async fn test_lsp_valid_code_no_diagnostics() {
    let backend = create_test_backend();
    let code = r#"
fn main():
    print("Hello, Pain!")
"#;

    let diagnostics = backend.check_document(code);
    // Valid code should have no errors
    let errors: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.severity == Some(tower_lsp::lsp_types::DiagnosticSeverity::ERROR))
        .collect();
    assert_eq!(errors.len(), 0, "Valid code should have no errors");
}

#[tokio::test]
async fn test_lsp_undefined_variable_error() {
    let backend = create_test_backend();
    let code = r#"
fn main():
    let x = undefined_variable
"#;

    let diagnostics = backend.check_document(code);
    // Should have at least one error for undefined variable
    let errors: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.severity == Some(tower_lsp::lsp_types::DiagnosticSeverity::ERROR))
        .collect();
    assert!(!errors.is_empty(), "Should detect undefined variable error");
    assert!(errors[0].message.contains("undefined") || errors[0].message.contains("Undefined"));
}

#[tokio::test]
async fn test_lsp_type_mismatch_error() {
    let backend = create_test_backend();
    let code = r#"
fn main():
    let x: int = "string"
"#;

    let diagnostics = backend.check_document(code);
    let errors: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.severity == Some(tower_lsp::lsp_types::DiagnosticSeverity::ERROR))
        .collect();
    assert!(!errors.is_empty(), "Should detect type mismatch error");
}

#[tokio::test]
async fn test_lsp_parse_error() {
    let backend = create_test_backend();
    let code = r#"
fn main():
    let x =  # Incomplete statement
"#;

    let diagnostics = backend.check_document(code);
    let errors: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.severity == Some(tower_lsp::lsp_types::DiagnosticSeverity::ERROR))
        .collect();
    assert!(!errors.is_empty(), "Should detect parse error");
}

#[tokio::test]
async fn test_lsp_unused_variable_warning() {
    let backend = create_test_backend();
    let code = r#"
fn main():
    let unused = 10
    print("test")
"#;

    let diagnostics = backend.check_document(code);
    let warnings: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.severity == Some(tower_lsp::lsp_types::DiagnosticSeverity::WARNING))
        .collect();
    // Should warn about unused variable
    assert!(!warnings.is_empty(), "Should detect unused variable warning");
}

#[tokio::test]
async fn test_lsp_function_with_parameters() {
    let backend = create_test_backend();
    let code = r#"
fn add(a: int, b: int) -> int:
    return a + b

fn main():
    let result = add(1, 2)
"#;

    let diagnostics = backend.check_document(code);
    let errors: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.severity == Some(tower_lsp::lsp_types::DiagnosticSeverity::ERROR))
        .collect();
    assert_eq!(errors.len(), 0, "Valid function with parameters should have no errors");
}

#[tokio::test]
async fn test_lsp_classes() {
    let backend = create_test_backend();
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

    let diagnostics = backend.check_document(code);
    let errors: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.severity == Some(tower_lsp::lsp_types::DiagnosticSeverity::ERROR))
        .collect();
    assert_eq!(errors.len(), 0, "Valid class code should have no errors");
}

#[tokio::test]
async fn test_lsp_control_flow() {
    let backend = create_test_backend();
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

    let diagnostics = backend.check_document(code);
    let errors: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.severity == Some(tower_lsp::lsp_types::DiagnosticSeverity::ERROR))
        .collect();
    assert_eq!(errors.len(), 0, "Valid control flow code should have no errors");
}

#[tokio::test]
async fn test_lsp_lists_and_arrays() {
    let backend = create_test_backend();
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

    let diagnostics = backend.check_document(code);
    let errors: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.severity == Some(tower_lsp::lsp_types::DiagnosticSeverity::ERROR))
        .collect();
    assert_eq!(errors.len(), 0, "Valid list/array code should have no errors");
}

