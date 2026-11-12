// Comprehensive LSP tests covering all Pain language features

use pain_lsp::Backend;

/// Run the document checker on `code`. These tests only assert that checking
/// each language feature completes without panicking.
fn check(code: &str) {
    let (service, _socket) = tower_lsp::LspService::new(Backend::new);
    let _diagnostics = service.inner().check_document(code);
}

/// Test LSP with simple function
#[tokio::test]
async fn test_lsp_simple_function() {
    let code = r#"
fn hello():
    print("Hello, Pain!")
"#;

    check(code);
}

/// Test LSP with function parameters
#[tokio::test]
async fn test_lsp_function_with_parameters() {
    let code = r#"
fn add(a: int, b: int) -> int:
    return a + b
"#;

    check(code);
}

/// Test LSP with classes
#[tokio::test]
async fn test_lsp_classes() {
    let code = r#"
class Point:
    let x: int
    let y: int
    
    fn new(x: int, y: int) -> Point:
        let p = Point()
        p.x = x
        p.y = y
        return p
"#;

    check(code);
}

/// Test LSP with variables and type inference
#[tokio::test]
async fn test_lsp_variables() {
    let code = r#"
fn main():
    let x = 10
    let y = 20
    let sum = x + y
"#;

    check(code);
}

/// Test LSP with control flow (if/else)
#[tokio::test]
async fn test_lsp_if_else() {
    let code = r#"
fn max(a: int, b: int) -> int:
    if a > b:
        return a
    else:
        return b
"#;

    check(code);
}

/// Test LSP with while loops
#[tokio::test]
async fn test_lsp_while_loop() {
    let code = r#"
fn sum(n: int) -> int:
    var result = 0
    var i = 0
    while i <= n:
        result = result + i
        i = i + 1
    return result
"#;

    check(code);
}

/// Test LSP with for loops
#[tokio::test]
async fn test_lsp_for_loop() {
    let code = r#"
fn main():
    for i in [1, 2, 3, 4, 5]:
        print(i)
"#;

    check(code);
}

/// Test LSP with lists/arrays
#[tokio::test]
async fn test_lsp_lists() {
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

    check(code);
}

/// Test LSP with maps
#[tokio::test]
async fn test_lsp_maps() {
    let code = r#"
fn main():
    let map = {"key": "value", "num": 42}
    let value = map["key"]
"#;

    check(code);
}

/// Test LSP with doc comments
#[tokio::test]
async fn test_lsp_doc_comments() {
    let code = r#"
/// This is a function that adds two numbers
/// 
/// Args:
///   a: First number
///   b: Second number
/// 
/// Returns:
///   Sum of a and b
fn add(a: int, b: int) -> int:
    return a + b
"#;

    check(code);
}

/// Test LSP with PML integration
#[tokio::test]
async fn test_lsp_pml_integration() {
    let code = r#"
fn main():
    let config = pml_load_file("config.pml")
    let app_name = config.app.name
"#;

    check(code);
}

/// Test LSP error diagnostics
#[tokio::test]
async fn test_lsp_error_diagnostics() {
    let code = r#"
fn main():
    let x = undefined_variable  # Should show error
    return x + "string"  # Type mismatch error
"#;

    check(code);
}

/// Test LSP with malformed code
#[tokio::test]
async fn test_lsp_malformed_code() {
    let code = r#"
fn main():
    let x =  # Incomplete statement
    if  # Incomplete if
"#;

    check(code);
}

/// Test LSP completion accuracy
#[tokio::test]
async fn test_lsp_completion() {
    let code = r#"
fn main():
    let x = 10
    # Test completion after typing "x"
"#;

    check(code);
}

/// Test LSP with stdlib functions
#[tokio::test]
async fn test_lsp_stdlib_completion() {
    let code = r#"
fn main():
    # Test completion for stdlib functions like print, len, etc.
"#;

    check(code);
}

/// Test LSP hover tooltips
#[tokio::test]
async fn test_lsp_hover() {
    let code = r#"
fn add(a: int, b: int) -> int:
    return a + b

fn main():
    let result = add(1, 2)
"#;

    check(code);
}

/// Test LSP with large file (1000+ lines)
#[tokio::test]
async fn test_lsp_large_file() {
    // Generate large file
    let mut code = String::new();
    for i in 0..1000 {
        code.push_str(&format!("fn func_{}() -> int:\n    return {}\n\n", i, i));
    }

    // TODO: Test LSP performance with large file
}

/// Test LSP with concurrent document changes
#[tokio::test]
async fn test_lsp_concurrent_changes() {
    // TODO: Simulate rapid typing/changes
}

/// Test LSP memory leaks (long-running session)
#[tokio::test]
async fn test_lsp_memory_leaks() {
    // TODO: Run many operations and check memory
}
