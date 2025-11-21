// Pain LSP server implementation

use pain_compiler::{
    ast::*, error::ErrorFormatter, parse_with_recovery, stdlib::get_stdlib_functions,
    type_check_program_with_context, type_checker::TypeContext, warnings::WarningCollector,
};
use std::collections::{HashMap, HashSet};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use tokio::sync::RwLock;
// Note: timeout and Duration imports removed - simplified implementation
// Timeout protection is handled at the VS Code extension level
use tower_lsp::lsp_types::*;

#[derive(Debug, Clone)]
pub struct HoverInfo {
    pub signature: String,
    pub doc: Option<String>,
}

#[derive(Debug)]
pub struct Backend {
    pub client: tower_lsp::Client,
    pub documents: Arc<RwLock<HashMap<url::Url, String>>>,
    // Track pending operations to allow cancellation
    pub max_document_size: usize, // Maximum document size in bytes (default: 10MB)
    // Cache for parsed programs to avoid re-parsing on every completion/hover
    // Note: This is a simple cache - in production, consider using LRU cache
    pub parsed_cache: Arc<RwLock<HashMap<url::Url, (String, Program)>>>, // (text_hash, program)
}

impl Backend {
    pub fn new(client: tower_lsp::Client) -> Self {
        Self {
            client,
            documents: Arc::new(RwLock::new(HashMap::new())),
            max_document_size: 10 * 1024 * 1024, // 10MB default
            parsed_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    // Get or parse program with caching
    async fn get_or_parse_program(&self, uri: &url::Url, text: &str) -> Option<Program> {
        // Simple hash-based cache check
        let mut hasher = DefaultHasher::new();
        text.hash(&mut hasher);
        let text_hash = hasher.finish().to_string();
        
        // Check cache
        {
            let cache = self.parsed_cache.read().await;
            if let Some((cached_hash, cached_program)) = cache.get(uri) {
                if cached_hash == &text_hash {
                    return Some(cached_program.clone());
                }
            }
        }
        
        // Parse and cache
        let (parse_result, _) = parse_with_recovery(text);
        if let Ok(program) = parse_result {
            let mut cache = self.parsed_cache.write().await;
            // Limit cache size to prevent memory issues
            if cache.len() > 50 {
                cache.clear(); // Simple eviction - clear all
            }
            cache.insert(uri.clone(), (text_hash, program.clone()));
            Some(program)
        } else {
            None
        }
    }
}

#[tower_lsp::async_trait]
impl tower_lsp::LanguageServer for Backend {
    async fn initialize(
        &self,
        _: InitializeParams,
    ) -> Result<InitializeResult, tower_lsp::jsonrpc::Error> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                completion_provider: Some(CompletionOptions {
                    resolve_provider: Some(false),
                    trigger_characters: Some(vec![".".to_string()]),
                    ..Default::default()
                }),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                ..Default::default()
            },
            ..Default::default()
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        // Log initialization - ignore errors to prevent crashes
        let _ = self.client
            .log_message(MessageType::INFO, "Pain LSP server initialized")
            .await;
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri.clone();
        let text = params.text_document.text.clone();
        
        // Check document size to prevent memory issues
        if text.len() > self.max_document_size {
            let _ = self.client
                .log_message(
                    MessageType::WARNING,
                    format!("Document {} is too large ({} bytes), skipping", uri, text.len()),
                )
                .await;
            return;
        }
        
        // Store document - release lock quickly
        {
            let mut docs = self.documents.write().await;
            docs.insert(uri.clone(), text.clone());
        } // Lock released here
        
        // Invalidate cache for this document
        {
            let mut cache = self.parsed_cache.write().await;
            cache.remove(&uri);
        }
        
        // Call on_change after releasing lock to avoid blocking other operations
        self.on_change(uri, text).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri.clone();
        let text = params
            .content_changes
            .into_iter()
            .next()
            .map(|change| change.text)
            .unwrap_or_default();
        
        // Check document size
        if text.len() > self.max_document_size {
            let _ = self.client
                .log_message(
                    MessageType::WARNING,
                    format!("Document {} is too large ({} bytes), skipping", uri, text.len()),
                )
                .await;
            return;
        }
        
        // Store document - release lock quickly
        {
            let mut docs = self.documents.write().await;
            docs.insert(uri.clone(), text.clone());
        } // Lock released here
        
        // Invalidate cache for this document
        {
            let mut cache = self.parsed_cache.write().await;
            cache.remove(&uri);
        }
        
        // Call on_change after releasing lock
        self.on_change(uri, text).await;
    }

    async fn completion(
        &self,
        params: CompletionParams,
    ) -> Result<Option<CompletionResponse>, tower_lsp::jsonrpc::Error> {
        let uri = params.text_document_position.text_document.uri.clone();
        let position = params.text_document_position.position;

        // Get document text - clone quickly and release lock
        let text = {
            let docs = self.documents.read().await;
            docs.get(&uri).cloned()
        }; // Lock released here
        
        if let Some(text) = text {
            // Use cached parsing for better performance
            let program = self.get_or_parse_program(&uri, &text).await;
            if let Some(program) = program {
                // Wrap get_completions in catch_unwind to prevent panics
                // Note: Timeout protection is handled at the VS Code extension level
                let items = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    self.get_completions(&program, &text, position)
                })).unwrap_or_else(|_| {
                    // If get_completions panics, return basic completions
                    self.get_basic_completions()
                });
                
                return Ok(Some(CompletionResponse::Array(items)));
            }
        }

        // Fallback to basic completions if parsing fails
        Ok(Some(CompletionResponse::Array(
            self.get_basic_completions(),
        )))
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>, tower_lsp::jsonrpc::Error> {
        let uri = params.text_document_position_params.text_document.uri.clone();
        let position = params.text_document_position_params.position;

        // Get document text from cache - clone quickly and release lock
        let text = {
            let docs = self.documents.read().await;
            docs.get(&uri).cloned()
        }; // Lock released here
        
        if let Some(text) = text {
            // Use parse_with_recovery instead of parse to avoid panics
            let (parse_result, _) = parse_with_recovery(&text);
            if let Ok(program) = parse_result {
                // Wrap find_function_at_position in catch_unwind to prevent panics
                let hover_info = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    find_function_at_position(
                        &program,
                        position.line as usize + 1,
                        position.character as usize + 1,
                    )
                }));

                if let Ok(Some(hover_info)) = hover_info {
                    let mut contents = Vec::new();

                    // Add function signature
                    contents.push(MarkedString::String(hover_info.signature));

                    // Add doc comment if present
                    if let Some(doc) = hover_info.doc {
                        contents.push(MarkedString::String(format!("---\n{}", doc)));
                    }

                    return Ok(Some(Hover {
                        contents: HoverContents::Array(contents),
                        range: None,
                    }));
                }
            }
        }

        Ok(None)
    }

    async fn shutdown(&self) -> Result<(), tower_lsp::jsonrpc::Error> {
        // Clear documents and cache on shutdown to free memory
        {
            let mut docs = self.documents.write().await;
            docs.clear();
        }
        {
            let mut cache = self.parsed_cache.write().await;
            cache.clear();
        }
        Ok(())
    }
}

impl Backend {
    /// Get context-aware completions
    pub fn get_completions(
        &self,
        program: &Program,
        text: &str,
        position: Position,
    ) -> Vec<CompletionItem> {
        // Wrap in catch_unwind to prevent panics
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.get_completions_internal(program, text, position)
        })).unwrap_or_else(|_| {
            // If anything panics, return basic completions
            eprintln!("LSP: get_completions panicked, returning basic completions");
            self.get_basic_completions()
        })
    }

    fn get_completions_internal(
        &self,
        program: &Program,
        text: &str,
        position: Position,
    ) -> Vec<CompletionItem> {
        let mut items = Vec::new();
        let line = position.line as usize;
        let column = position.character as usize;

        // Get text before cursor on current line - safe indexing
        let lines: Vec<&str> = text.lines().collect();
        let current_line = if line < lines.len() {
            lines[line]
        } else {
            return self.get_basic_completions();
        };

        let text_before_cursor = if column <= current_line.len() {
            &current_line[..column]
        } else {
            current_line
        };

        // Check if we're after a dot (member access)
        let is_member_access = text_before_cursor.trim_end().ends_with('.');

        // Extract functions from program - optimize by limiting detail formatting
        // Format full signatures only for first N items to improve performance
        let mut function_names = HashSet::new();
        let max_detailed_items = 50; // Limit detailed formatting for performance
        let mut detailed_count = 0;
        
        for item in &program.items {
            match item {
                Item::Function(func) => {
                    function_names.insert(func.name.clone());
                    // Only format full signature for first N items
                    let detail = if detailed_count < max_detailed_items {
                        detailed_count += 1;
                        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                            format_function_signature(func)
                        })).unwrap_or_else(|_| format!("fn {}", func.name))
                    } else {
                        format!("fn {}", func.name)
                    };
                    
                    items.push(CompletionItem {
                        label: func.name.clone(),
                        kind: Some(CompletionItemKind::FUNCTION),
                        detail: Some(detail),
                        documentation: func.doc.clone().map(Documentation::String),
                        ..Default::default()
                    });
                }
                Item::Class(class) => {
                    // Add class name
                    items.push(CompletionItem {
                        label: class.name.clone(),
                        kind: Some(CompletionItemKind::CLASS),
                        detail: Some(format!("class {}", class.name)),
                        documentation: class.doc.clone().map(Documentation::String),
                        ..Default::default()
                    });

                    // Add class methods - optimize formatting
                    for method in &class.methods {
                        function_names.insert(method.name.clone());
                        let detail = if detailed_count < max_detailed_items {
                            detailed_count += 1;
                            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                                format_function_signature(method)
                            })).unwrap_or_else(|_| format!("fn {}", method.name))
                        } else {
                            format!("fn {}", method.name)
                        };
                        
                        items.push(CompletionItem {
                            label: format!("{}.{}", class.name, method.name),
                            kind: Some(CompletionItemKind::METHOD),
                            detail: Some(detail),
                            documentation: method.doc.clone().map(Documentation::String),
                            ..Default::default()
                        });
                    }
                }
            }
        }

        // Extract variables from current scope - wrap in catch_unwind
        let vars = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            extract_variables_in_scope(program, line + 1, column + 1)
        })).unwrap_or(None);
        
        if let Some(vars) = vars {
            for var_name in vars {
                if !function_names.contains(&var_name) {
                    items.push(CompletionItem {
                        label: var_name.clone(),
                        kind: Some(CompletionItemKind::VARIABLE),
                        detail: Some("Variable".to_string()),
                        ..Default::default()
                    });
                }
            }
        }

        // Add stdlib functions - optimize by caching formatted signatures
        // Only format signatures if we're actually going to use them
        // This avoids expensive formatting for functions that won't be shown
        let stdlib_funcs = get_stdlib_functions();
        let max_stdlib_items = 100; // Limit stdlib completions to prevent UI lag
        
        for stdlib_func in stdlib_funcs.iter().take(max_stdlib_items) {
            // Avoid duplicates
            if !function_names.contains(&stdlib_func.name) {
                // Only format signature if we have space (performance optimization)
                let signature = if items.len() < 200 {
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        let params_str: Vec<String> = stdlib_func
                            .params
                            .iter()
                            .map(|(name, ty)| format!("{}: {}", name, format_type(ty)))
                            .collect();
                        format!(
                            "{}({}) -> {}",
                            stdlib_func.name,
                            params_str.join(", "),
                            format_type(&stdlib_func.return_type)
                        )
                    })).unwrap_or_else(|_| format!("{}()", stdlib_func.name))
                } else {
                    // For large lists, use simple format to save time
                    format!("{}()", stdlib_func.name)
                };

                items.push(CompletionItem {
                    label: stdlib_func.name.clone(),
                    kind: Some(CompletionItemKind::FUNCTION),
                    detail: Some(signature),
                    documentation: Some(Documentation::String(stdlib_func.description.clone())),
                    ..Default::default()
                });
            }
        }

        // Add keywords (only if not in member access context)
        if !is_member_access {
            items.extend(self.get_keyword_completions());
        }

        items
    }

    /// Get basic keyword completions
    pub fn get_keyword_completions(&self) -> Vec<CompletionItem> {
        vec![
            CompletionItem {
                label: "fn".to_string(),
                kind: Some(CompletionItemKind::KEYWORD),
                detail: Some("Function definition".to_string()),
                ..Default::default()
            },
            CompletionItem {
                label: "let".to_string(),
                kind: Some(CompletionItemKind::KEYWORD),
                detail: Some("Immutable variable".to_string()),
                ..Default::default()
            },
            CompletionItem {
                label: "var".to_string(),
                kind: Some(CompletionItemKind::KEYWORD),
                detail: Some("Mutable variable".to_string()),
                ..Default::default()
            },
            CompletionItem {
                label: "if".to_string(),
                kind: Some(CompletionItemKind::KEYWORD),
                detail: Some("Conditional statement".to_string()),
                ..Default::default()
            },
            CompletionItem {
                label: "else".to_string(),
                kind: Some(CompletionItemKind::KEYWORD),
                detail: Some("Else branch".to_string()),
                ..Default::default()
            },
            CompletionItem {
                label: "for".to_string(),
                kind: Some(CompletionItemKind::KEYWORD),
                detail: Some("For loop".to_string()),
                ..Default::default()
            },
            CompletionItem {
                label: "while".to_string(),
                kind: Some(CompletionItemKind::KEYWORD),
                detail: Some("While loop".to_string()),
                ..Default::default()
            },
            CompletionItem {
                label: "break".to_string(),
                kind: Some(CompletionItemKind::KEYWORD),
                detail: Some("Break out of loop".to_string()),
                ..Default::default()
            },
            CompletionItem {
                label: "continue".to_string(),
                kind: Some(CompletionItemKind::KEYWORD),
                detail: Some("Continue to next loop iteration".to_string()),
                ..Default::default()
            },
            CompletionItem {
                label: "return".to_string(),
                kind: Some(CompletionItemKind::KEYWORD),
                detail: Some("Return from function".to_string()),
                ..Default::default()
            },
        ]
    }

    /// Get basic completions (fallback)
    pub fn get_basic_completions(&self) -> Vec<CompletionItem> {
        let mut items = self.get_keyword_completions();

        // Add basic stdlib functions
        items.push(CompletionItem {
            label: "print".to_string(),
            kind: Some(CompletionItemKind::FUNCTION),
            detail: Some("print(value: dynamic) -> void".to_string()),
            ..Default::default()
        });

        items
    }

    async fn on_change(&self, uri: url::Url, text: String) {
        // Wrap check_document in catch_unwind to prevent panics from crashing LSP
        // Note: We compute diagnostics synchronously here, but the lock is already released
        // so this won't block other operations. For very large files, this could still be slow,
        // but it's better than blocking the document cache.
        let diagnostics = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.check_document(&text)
        })).unwrap_or_else(|_| {
            // If check_document panics, return empty diagnostics
            // Log the panic for debugging
            eprintln!("LSP: check_document panicked, returning empty diagnostics");
            vec![]
        });
        
        // Publish diagnostics - this is fire-and-forget, returns ()
        // If this panics, it will be caught by the LSP framework
        self.client.publish_diagnostics(uri, diagnostics, None).await;
    }

    pub fn check_document(&self, text: &str) -> Vec<Diagnostic> {
        // Wrap entire function in catch_unwind to prevent any panics
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.check_document_internal(text)
        })).unwrap_or_else(|_| {
            // If anything panics, return empty diagnostics
            vec![]
        })
    }

    fn check_document_internal(&self, text: &str) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        // Parse with error recovery for better IDE experience
        let (parse_result, parse_errors) = parse_with_recovery(text);

        // Add parse errors as diagnostics
        for parse_err in &parse_errors {
            diagnostics.push(self.parse_error_to_diagnostic(parse_err));
        }

        // If parsing succeeded (even partially), try type checking
        if let Ok(program) = parse_result {
            // Build type context for better error messages
            let mut ctx = TypeContext::new();
            for item in &program.items {
                match item {
                    Item::Function(func) => {
                        ctx.add_function(func.name.clone(), func.clone());
                    }
                    Item::Class(class) => {
                        ctx.add_class(class.name.clone(), class.clone());
                    }
                }
            }

            // Type check - wrap in catch_unwind to prevent panics
            let type_check_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                type_check_program_with_context(&program, &mut ctx)
            }));

            match type_check_result {
                Ok(Ok(_)) => {
                    // Collect warnings - wrap in catch_unwind
                    let warnings_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        WarningCollector::collect_warnings(&program, &ctx)
                    }));
                    
                    if let Ok(warnings) = warnings_result {
                        for warning in warnings {
                            diagnostics.push(self.warning_to_diagnostic(&warning, text));
                        }
                    }
                }
                Ok(Err(err)) => {
                    // Type error - format safely
                    let error_msg = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        let formatter = ErrorFormatter::new(text).with_context(&ctx);
                        formatter.format_error(&err)
                    })).unwrap_or_else(|_| format!("Type error: {:?}", err));
                    
                    diagnostics.push(self.type_error_to_diagnostic(&err, &error_msg));
                }
                Err(_) => {
                    // Type checking panicked - skip type checking diagnostics
                }
            }
        }

        diagnostics
    }

    pub fn parse_error_to_diagnostic(&self, err: &pain_compiler::error::ParseError) -> Diagnostic {
        Diagnostic {
            range: Range {
                start: Position {
                    line: (err.span.line().saturating_sub(1)) as u32,
                    character: (err.span.column().saturating_sub(1)) as u32,
                },
                end: Position {
                    line: (err.span.line().saturating_sub(1)) as u32,
                    character: (err.span.column().saturating_sub(1) + 1) as u32,
                },
            },
            severity: Some(DiagnosticSeverity::ERROR),
            code: None,
            code_description: None,
            source: Some("pain".to_string()),
            message: err.message.clone(),
            related_information: None,
            tags: None,
            data: None,
        }
    }

    pub fn type_error_to_diagnostic(
        &self,
        err: &pain_compiler::TypeError,
        formatted_msg: &str,
    ) -> Diagnostic {
        let span = match err {
            pain_compiler::TypeError::UndefinedVariable { span, .. } => *span,
            pain_compiler::TypeError::TypeMismatch { span, .. } => *span,
            pain_compiler::TypeError::CannotInferType { span, .. } => *span,
            pain_compiler::TypeError::InvalidOperation { span, .. } => *span,
        };

        Diagnostic {
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
            message: formatted_msg
                .lines()
                .next()
                .unwrap_or(formatted_msg)
                .to_string(),
            related_information: None,
            tags: None,
            data: None,
        }
    }

    pub fn warning_to_diagnostic(&self, warning: &pain_compiler::Warning, _text: &str) -> Diagnostic {
        let (message, span) = match warning {
            pain_compiler::Warning::UnusedVariable { name, span } => {
                (format!("unused variable `{}`", name), *span)
            }
            pain_compiler::Warning::UnusedFunction { name, span } => {
                (format!("unused function `{}`", name), *span)
            }
            pain_compiler::Warning::DeadCode { span, reason } => {
                (format!("dead code: {}", reason), *span)
            }
            pain_compiler::Warning::UnreachableCode { span } => {
                ("unreachable code".to_string(), *span)
            }
        };

        Diagnostic {
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
        }
    }
}

// Find function at given line and column position
pub fn find_function_at_position(program: &Program, line: usize, _column: usize) -> Option<HoverInfo> {
    for item in &program.items {
        let Item::Function(func) = item else { continue };
        // Simple heuristic: check if position is on function name line
        // In a real implementation, we'd use proper span information
        let func_line = func.span.start.line;

        // Check if we're on the function definition line
        if line == func_line || line == func_line + 1 {
            let signature = format_function_signature(func);
            return Some(HoverInfo {
                signature,
                doc: func.doc.clone(),
            });
        }
    }
    None
}

// Format function signature for hover display
pub fn format_function_signature(func: &Function) -> String {
    // Wrap in catch_unwind to prevent panics from format_type recursion
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        format_function_signature_internal(func)
    })).unwrap_or_else(|_| {
        // Fallback to simple signature if formatting panics
        format!("fn {}()", func.name)
    })
}

fn format_function_signature_internal(func: &Function) -> String {
    let mut sig = String::new();

    // Attributes
    if !func.attrs.is_empty() {
        for attr in &func.attrs {
            sig.push_str(&format!(
                "@{}{} ",
                attr.name,
                if attr.args.is_empty() {
                    String::new()
                } else {
                    format!("({})", attr.args.len())
                }
            ));
        }
    }

    // Function name and parameters
    sig.push_str("fn ");
    sig.push_str(&func.name);
    sig.push('(');

    let params: Vec<String> = func
        .params
        .iter()
        .map(|p| format!("{}: {}", p.name, format_type(&p.ty)))
        .collect();
    sig.push_str(&params.join(", "));
    sig.push(')');

    // Return type
    if let Some(ref ret_ty) = func.return_type {
        sig.push_str(" -> ");
        sig.push_str(&format_type(ret_ty));
    }

    sig
}

// Format type for display with recursion limit to prevent stack overflow
pub fn format_type(ty: &Type) -> String {
    format_type_with_depth(ty, 0)
}

fn format_type_with_depth(ty: &Type, depth: usize) -> String {
    // Limit recursion depth to prevent stack overflow
    if depth > 10 {
        return "...".to_string();
    }

    match ty {
        Type::Int => "int".to_string(),
        Type::Str => "str".to_string(),
        Type::Float32 => "float32".to_string(),
        Type::Float64 => "float64".to_string(),
        Type::Bool => "bool".to_string(),
        Type::Dynamic => "dynamic".to_string(),
        Type::List(inner) => format!("list[{}]", format_type_with_depth(inner, depth + 1)),
        Type::Array(inner) => format!("array[{}]", format_type_with_depth(inner, depth + 1)),
        Type::Map(k, v) => format!("map[{}, {}]", format_type_with_depth(k, depth + 1), format_type_with_depth(v, depth + 1)),
        Type::Tensor(inner, dims) => format!("Tensor[{}, {:?}]", format_type_with_depth(inner, depth + 1), dims),
        Type::Named(name) => name.clone(),
    }
}

// Extract variables visible at given position (simplified implementation)
pub fn extract_variables_in_scope(
    program: &Program,
    line: usize,
    _column: usize,
) -> Option<HashSet<String>> {
    let mut variables = HashSet::new();

    // Find function containing this line
    for item in &program.items {
        if let Item::Function(func) = item {
            let func_start = func.span.start.line;
            let func_end = func.span.end.line;

            // Check if position is within this function
            if line >= func_start && line <= func_end {
                // Add function parameters
                for param in &func.params {
                    variables.insert(param.name.clone());
                }

                // Extract variables from function body (simplified - just from let/var statements)
                extract_variables_from_statements(&func.body, &mut variables);

                return Some(variables);
            }
        }
    }

    None
}

// Extract variable names from statements
pub fn extract_variables_from_statements(statements: &[Statement], variables: &mut HashSet<String>) {
    for stmt in statements {
        match stmt {
            Statement::Let { name, .. } => {
                variables.insert(name.clone());
            }
            Statement::For { var, body, .. } => {
                variables.insert(var.clone());
                extract_variables_from_statements(body, variables);
            }
            Statement::If { then, else_, .. } => {
                extract_variables_from_statements(then, variables);
                if let Some(else_stmts) = else_ {
                    extract_variables_from_statements(else_stmts, variables);
                }
            }
            Statement::While { body, .. } => {
                extract_variables_from_statements(body, variables);
            }
            _ => {}
        }
    }
}
