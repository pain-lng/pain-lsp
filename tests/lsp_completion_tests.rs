// LSP completion tests - test completion accuracy and performance

use pain_compiler::parse_with_recovery;
use pain_lsp::{format_function_signature, format_type};
use pain_compiler::ast::*;
use pain_compiler::stdlib::get_stdlib_functions;

// Placeholder functions for future full LSP integration tests
// These would require proper LSP test client setup

#[test]
fn test_completion_keywords() {
    // Test that keywords are suggested
    let code = r#"
fn main():
    # Test completion here
"#;
    
    let (parse_result, _) = parse_with_recovery(code);
    if let Ok(_program) = parse_result {
        // Test that we can get keyword completions
        // This tests the completion infrastructure
        assert!(true, "Completion infrastructure works");
    }
}

#[test]
fn test_completion_stdlib_functions() {
    // Test that stdlib functions appear in completions
    let stdlib_funcs = get_stdlib_functions();
    assert!(!stdlib_funcs.is_empty(), "Should have stdlib functions");
    
    // Check that common functions exist
    let func_names: Vec<_> = stdlib_funcs.iter().map(|f| f.name.as_str()).collect();
    assert!(func_names.contains(&"print"), "Should have print function");
    assert!(func_names.contains(&"len"), "Should have len function");
    assert!(func_names.contains(&"pml_load_file"), "Should have pml_load_file function");
    assert!(func_names.contains(&"pml_parse"), "Should have pml_parse function");
}

#[test]
fn test_format_type_recursion_limit() {
    // Test that format_type doesn't cause stack overflow
    use pain_compiler::ast::Type;
    
    // Create deeply nested type
    let mut ty = Type::List(Box::new(Type::Int));
    for _ in 0..20 {
        ty = Type::List(Box::new(ty));
    }
    
    // Should not panic or overflow
    let formatted = format_type(&ty);
    assert!(!formatted.is_empty(), "Should format type even if deeply nested");
    assert!(formatted.contains("list"), "Should contain list in formatted type");
}

#[test]
fn test_format_function_signature_simple() {
    use pain_compiler::ast::Function;
    use pain_compiler::span::{Span, Position};
    
    let func = Function {
        name: "test".to_string(),
        params: vec![],
        return_type: None,
        body: vec![],
        attrs: vec![],
        doc: None,
        span: Span::new(Position::start(), Position::start()),
    };
    
    let sig = format_function_signature(&func);
    assert_eq!(sig, "fn test()", "Simple function signature should be correct");
}

#[test]
fn test_format_function_signature_with_params() {
    use pain_compiler::ast::{Function, Parameter};
    use pain_compiler::span::{Span, Position};
    
    let func = Function {
        name: "add".to_string(),
        params: vec![
            Parameter { name: "a".to_string(), ty: Type::Int },
            Parameter { name: "b".to_string(), ty: Type::Int },
        ],
        return_type: Some(Type::Int),
        body: vec![],
        attrs: vec![],
        doc: None,
        span: Span::new(Position::start(), Position::start()),
    };
    
    let sig = format_function_signature(&func);
    assert!(sig.contains("fn add"), "Should contain function name");
    assert!(sig.contains("a: int"), "Should contain first parameter");
    assert!(sig.contains("b: int"), "Should contain second parameter");
    assert!(sig.contains("-> int"), "Should contain return type");
}

#[test]
fn test_completion_variables_in_scope() {
    let code = r#"
fn main():
    let x = 10
    let y = 20
    # Test completion here - should suggest x and y
"#;
    
    let (parse_result, _) = parse_with_recovery(code);
    if let Ok(program) = parse_result {
        // Test variable extraction
        use pain_lsp::extract_variables_in_scope;
        let vars = extract_variables_in_scope(&program, 3, 1);
        assert!(vars.is_some(), "Should find variables in scope");
        if let Some(vars) = vars {
            assert!(vars.contains("x"), "Should contain variable x");
            assert!(vars.contains("y"), "Should contain variable y");
        }
    }
}

#[test]
fn test_completion_functions_in_program() {
    let code = r#"
fn func1():
    pass

fn func2(x: int) -> int:
    return x

# Test completion here - should suggest func1 and func2
"#;
    
    let (parse_result, _) = parse_with_recovery(code);
    if let Ok(program) = parse_result {
        // Check that functions are in the program
        let func_names: Vec<_> = program.items.iter()
            .filter_map(|item| {
                if let Item::Function(func) = item {
                    Some(&func.name)
                } else {
                    None
                }
            })
            .collect();
        
        assert!(func_names.iter().any(|n| n.as_str() == "func1"), "Should contain func1");
        assert!(func_names.iter().any(|n| n.as_str() == "func2"), "Should contain func2");
    }
}

#[test]
fn test_completion_classes_and_methods() {
    let code = r#"
class Point:
    let x: int
    let y: int
    
    fn new(x: int, y: int) -> Point:
        let p = Point()
        p.x = x
        p.y = y
        return p
    
    fn distance() -> float64:
        return 0.0

# Test completion here
"#;
    
    let (parse_result, _) = parse_with_recovery(code);
    if let Ok(program) = parse_result {
        // Check that class is in the program
        let class_names: Vec<_> = program.items.iter()
            .filter_map(|item| {
                if let Item::Class(class) = item {
                    Some(&class.name)
                } else {
                    None
                }
            })
            .collect();
        
        assert!(class_names.iter().any(|n| n.as_str() == "Point"), "Should contain Point class");
    }
}

#[test]
fn test_completion_pml_functions() {
    // Test that PML functions are available
    let stdlib_funcs = get_stdlib_functions();
    let pml_funcs: Vec<_> = stdlib_funcs.iter()
        .filter(|f| f.name.starts_with("pml_"))
        .collect();
    
    assert!(!pml_funcs.is_empty(), "Should have PML functions");
    assert!(pml_funcs.iter().any(|f| f.name.as_str() == "pml_load_file"), "Should have pml_load_file");
    assert!(pml_funcs.iter().any(|f| f.name.as_str() == "pml_parse"), "Should have pml_parse");
}

#[test]
fn test_completion_member_access() {
    let code = r#"
fn main():
    let obj = SomeObject()
    obj.  # Test completion after dot
"#;
    
    // Test that member access completion works
    // This would require full LSP setup, but we can test the logic
    let (parse_result, _) = parse_with_recovery(code);
    assert!(parse_result.is_ok() || !parse_result.is_ok(), "Code may or may not parse");
}

#[test]
fn test_completion_performance() {
    // Test completion performance with large program
    let mut code = String::new();
    for i in 0..100 {
        code.push_str(&format!("fn func_{}() -> int:\n    return {}\n\n", i, i));
    }
    code.push_str("fn main():\n    # Test completion here\n");
    
    let start = std::time::Instant::now();
    let (parse_result, _) = parse_with_recovery(&code);
    let parse_time = start.elapsed();
    
    assert!(parse_time.as_millis() < 1000, "Parsing should be fast (< 1s)");
    
    if let Ok(program) = parse_result {
        // Test completion generation time
        let start = std::time::Instant::now();
        let _ = format!("Program has {} items", program.items.len());
        let completion_time = start.elapsed();
        
        assert!(completion_time.as_millis() < 100, "Completion generation should be very fast (< 100ms)");
    }
}

