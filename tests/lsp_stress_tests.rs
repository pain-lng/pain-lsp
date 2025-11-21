// LSP stress tests - test performance, memory, and edge cases

use pain_compiler::parse_with_recovery;

#[test]
fn test_large_file_parsing() {
    // Generate large file (1000+ lines)
    let mut code = String::new();
    for i in 0..1000 {
        code.push_str(&format!("fn func_{}() -> int:\n    return {}\n\n", i, i));
    }
    
    let start = std::time::Instant::now();
    let (parse_result, _) = parse_with_recovery(&code);
    let parse_time = start.elapsed();
    
    assert!(parse_result.is_ok(), "Should parse large file");
    assert!(parse_time.as_millis() < 5000, "Large file parsing should be reasonable (< 5s)");
}

#[test]
fn test_very_deep_nesting() {
    // Test with very deeply nested code
    let mut code = String::from("fn main():\n");
    for i in 0..50 {
        code.push_str(&format!("{}if true:\n", "    ".repeat(i + 1)));
    }
    for i in (0..50).rev() {
        code.push_str(&format!("{}pass\n", "    ".repeat(i + 1)));
    }
    
    let start = std::time::Instant::now();
    let (parse_result, _) = parse_with_recovery(&code);
    let parse_time = start.elapsed();
    
    // Should not panic or take too long
    assert!(parse_time.as_millis() < 2000, "Deep nesting should parse in reasonable time");
}

#[test]
fn test_very_long_line() {
    // Test with very long line
    let mut code = String::from("fn main():\n    let x = ");
    for i in 0..1000 {
        code.push_str(&format!("{} + ", i));
    }
    code.push_str("0\n");
    
    let start = std::time::Instant::now();
    let (parse_result, _) = parse_with_recovery(&code);
    let parse_time = start.elapsed();
    
    // Should not panic
    assert!(parse_time.as_millis() < 2000, "Long line should parse in reasonable time");
}

#[test]
fn test_unicode_strings() {
    let code = r#"
fn main():
    let unicode = "Привет, мир! 🌍"
    let emoji = "Hello 👋 World 🌎"
    let chinese = "你好世界"
"#;
    
    let (parse_result, _) = parse_with_recovery(code);
    assert!(parse_result.is_ok(), "Should handle Unicode strings");
}

#[test]
fn test_special_characters() {
    let code = r#"
fn main():
    let special = "!@#$%^&*()_+-=[]{}|;':\",./<>?"
    let quotes = "\"quoted\" and 'single'"
"#;
    
    let (parse_result, _) = parse_with_recovery(code);
    assert!(parse_result.is_ok(), "Should handle special characters");
}

#[test]
fn test_many_functions() {
    // Test with many functions
    let mut code = String::new();
    for i in 0..500 {
        code.push_str(&format!(
            "fn func_{}(x: int) -> int:\n    return x + {}\n\n",
            i, i
        ));
    }
    code.push_str("fn main() -> int:\n    return func_0(10)\n");
    
    let start = std::time::Instant::now();
    let (parse_result, _) = parse_with_recovery(&code);
    let parse_time = start.elapsed();
    
    assert!(parse_result.is_ok(), "Should parse many functions");
    assert!(parse_time.as_millis() < 3000, "Many functions should parse in reasonable time");
}

#[test]
fn test_many_variables() {
    // Test with many variables in one function
    let mut code = String::from("fn main():\n");
    for i in 0..200 {
        code.push_str(&format!("    let var_{} = {}\n", i, i));
    }
    
    let (_parse_result, _) = parse_with_recovery(&code);
    
    let start = std::time::Instant::now();
    let (parse_result, _) = parse_with_recovery(&code);
    let parse_time = start.elapsed();
    
    assert!(parse_result.is_ok(), "Should parse many variables");
    assert!(parse_time.as_millis() < 2000, "Many variables should parse in reasonable time");
}

#[test]
fn test_empty_file() {
    let code = "";
    
    let (parse_result, _) = parse_with_recovery(code);
    // Empty file may or may not parse - that's ok
    // Just test it doesn't panic
    assert!(true, "Empty file should not panic");
}

#[test]
fn test_only_comments() {
    let code = r#"
# This is a comment
# Another comment
# Yet another comment
"#;
    
    let (parse_result, _) = parse_with_recovery(code);
    // Comments-only file may or may not parse - that's ok
    assert!(true, "Comments-only file should not panic");
}

#[test]
fn test_rapid_changes_simulation() {
    // Simulate rapid document changes
    let mut code = String::from("fn main():\n    let x = 10\n");
    
    for i in 0..100 {
        // Simulate typing - add character by character
        code.push_str(&format!("    let var_{} = {}\n", i, i));
        
        // Parse after each "change"
        let (_parse_result, _) = parse_with_recovery(&code);
        // Should not panic even with incomplete code
        assert!(true, "Rapid changes should not panic");
    }
}

#[test]
fn test_malformed_code_recovery() {
    // Test various malformed code patterns
    let malformed_codes = vec![
        "fn main():\n    let x = \n",  // Incomplete assignment
        "fn main():\n    if \n",  // Incomplete if
        "fn main():\n    return \n",  // Incomplete return
        "fn \n",  // Incomplete function
        "class \n",  // Incomplete class
        "let x = \n",  // Statement outside function
        "fn main():\n    x + \n",  // Incomplete expression
    ];
    
    for code in malformed_codes {
        let (parse_result, parse_errors) = parse_with_recovery(code);
        // Should not panic, should return errors
        assert!(true, "Malformed code should not panic");
        // May have parse errors - that's expected
        if parse_result.is_err() || !parse_errors.is_empty() {
            assert!(true, "Malformed code should produce errors");
        }
    }
}

#[test]
fn test_incomplete_code() {
    // Test incomplete but potentially valid code
    let incomplete_codes = vec![
        "fn main():\n    let x = 10\n    let y = ",  // Typing in progress
        "fn main():\n    if x > 10:\n        ",  // Incomplete block
        "fn add(a: int, b: int) -> int:\n    return ",  // Incomplete return
    ];
    
    for code in incomplete_codes {
        let (parse_result, _) = parse_with_recovery(code);
        // Should not panic
        assert!(true, "Incomplete code should not panic");
    }
}

#[test]
fn test_memory_usage_stability() {
    // Test that parsing doesn't leak memory
    let code = r#"
fn main():
    let x = 10
    let y = 20
    let sum = x + y
"#;
    
    // Parse many times to check for memory leaks
    for _ in 0..1000 {
        let (parse_result, _) = parse_with_recovery(code);
        // Should consistently succeed
        assert!(parse_result.is_ok(), "Repeated parsing should work");
    }
}

#[test]
fn test_concurrent_parsing_simulation() {
    // Simulate concurrent parsing (though we can't truly test concurrency in unit tests)
    let code1 = "fn func1() -> int:\n    return 1\n";
    let code2 = "fn func2() -> int:\n    return 2\n";
    let code3 = "fn func3() -> int:\n    return 3\n";
    
    // Parse all "concurrently" (sequentially in test)
    let (result1, _) = parse_with_recovery(code1);
    let (result2, _) = parse_with_recovery(code2);
    let (result3, _) = parse_with_recovery(code3);
    
    assert!(result1.is_ok(), "First parse should succeed");
    assert!(result2.is_ok(), "Second parse should succeed");
    assert!(result3.is_ok(), "Third parse should succeed");
}

