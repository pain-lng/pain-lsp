// Pain LSP server

use pain_compiler::{ast::*, parse, stdlib::get_stdlib_functions, type_check_program};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::RwLock;
use tower_lsp::lsp_types::*;
use tower_lsp::{LspService, Server};

#[derive(Debug, Clone)]
struct HoverInfo {
    signature: String,
    doc: Option<String>,
}

#[derive(Debug)]
struct Backend {
    client: tower_lsp::Client,
    documents: Arc<RwLock<HashMap<Url, String>>>,
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
        self.client
            .log_message(MessageType::INFO, "Pain LSP server initialized")
            .await;
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri.clone();
        let text = params.text_document.text.clone();
        self.documents
            .write()
            .await
            .insert(uri.clone(), text.clone());
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
        self.documents
            .write()
            .await
            .insert(uri.clone(), text.clone());
        self.on_change(uri, text).await;
    }

    async fn completion(
        &self,
        params: CompletionParams,
    ) -> Result<Option<CompletionResponse>, tower_lsp::jsonrpc::Error> {
        let uri = params.text_document_position.text_document.uri.clone();
        let position = params.text_document_position.position;

        // Get document text
        let text = self.documents.read().await.get(&uri).cloned();
        if let Some(text) = text {
            // Parse document to extract context
            if let Ok(program) = parse(&text) {
                let items = self.get_completions(&program, &text, position);
                return Ok(Some(CompletionResponse::Array(items)));
            }
        }

        // Fallback to basic completions if parsing fails
        Ok(Some(CompletionResponse::Array(
            self.get_basic_completions(),
        )))
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>, tower_lsp::jsonrpc::Error> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;

        // Get document text from cache
        let text = self.documents.read().await.get(&uri).cloned();
        if let Some(text) = text {
            if let Ok(program) = parse(&text) {
                // Find function at position (LSP uses 0-based line numbers)
                if let Some(hover_info) = find_function_at_position(
                    &program,
                    position.line as usize + 1,
                    position.character as usize + 1,
                ) {
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
        Ok(())
    }
}

impl Backend {
    /// Get context-aware completions
    fn get_completions(
        &self,
        program: &Program,
        text: &str,
        position: Position,
    ) -> Vec<CompletionItem> {
        let mut items = Vec::new();
        let line = position.line as usize;
        let column = position.character as usize;

        // Get text before cursor on current line
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

        // Extract functions from program
        let mut function_names = HashSet::new();
        for item in &program.items {
            match item {
                Item::Function(func) => {
                    function_names.insert(func.name.clone());
                    items.push(CompletionItem {
                        label: func.name.clone(),
                        kind: Some(CompletionItemKind::FUNCTION),
                        detail: Some(format_function_signature(func)),
                        documentation: func.doc.clone().map(|d| Documentation::String(d)),
                        ..Default::default()
                    });
                }
                Item::Class(class) => {
                    // Add class name
                    items.push(CompletionItem {
                        label: class.name.clone(),
                        kind: Some(CompletionItemKind::CLASS),
                        detail: Some(format!("class {}", class.name)),
                        documentation: class.doc.clone().map(|d| Documentation::String(d)),
                        ..Default::default()
                    });

                    // Add class methods
                    for method in &class.methods {
                        function_names.insert(method.name.clone());
                        items.push(CompletionItem {
                            label: format!("{}.{}", class.name, method.name),
                            kind: Some(CompletionItemKind::METHOD),
                            detail: Some(format_function_signature(method)),
                            documentation: method.doc.clone().map(|d| Documentation::String(d)),
                            ..Default::default()
                        });
                    }
                }
            }
        }

        // Extract variables from current scope (simplified - just from current function)
        if let Some(vars) = extract_variables_in_scope(program, line + 1, column + 1) {
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

        // Add stdlib functions
        for stdlib_func in get_stdlib_functions() {
            // Avoid duplicates
            if !function_names.contains(&stdlib_func.name) {
                let params_str: Vec<String> = stdlib_func
                    .params
                    .iter()
                    .map(|(name, ty)| format!("{}: {}", name, format_type(ty)))
                    .collect();
                let signature = format!(
                    "{}({}) -> {}",
                    stdlib_func.name,
                    params_str.join(", "),
                    format_type(&stdlib_func.return_type)
                );

                items.push(CompletionItem {
                    label: stdlib_func.name.clone(),
                    kind: Some(CompletionItemKind::FUNCTION),
                    detail: Some(signature.clone()),
                    documentation: Some(Documentation::String(stdlib_func.description)),
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
    fn get_keyword_completions(&self) -> Vec<CompletionItem> {
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
    fn get_basic_completions(&self) -> Vec<CompletionItem> {
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

    async fn on_change(&self, uri: Url, text: String) {
        let diagnostics = self.check_document(&text);
        self.client
            .publish_diagnostics(uri, diagnostics, None)
            .await;
    }

    fn check_document(&self, text: &str) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        // Parse
        match parse(text) {
            Ok(program) => {
                // Type check
                match type_check_program(&program) {
                    Ok(_) => {
                        // No errors
                    }
                    Err(err) => {
                        diagnostics.push(Diagnostic {
                            range: Range {
                                start: Position {
                                    line: 0,
                                    character: 0,
                                },
                                end: Position {
                                    line: 0,
                                    character: 0,
                                },
                            },
                            severity: Some(DiagnosticSeverity::ERROR),
                            code: None,
                            code_description: None,
                            source: Some("pain".to_string()),
                            message: format!("Type error: {:?}", err),
                            related_information: None,
                            tags: None,
                            data: None,
                        });
                    }
                }
            }
            Err(err) => {
                diagnostics.push(Diagnostic {
                    range: Range {
                        start: Position {
                            line: 0,
                            character: 0,
                        },
                        end: Position {
                            line: 0,
                            character: 0,
                        },
                    },
                    severity: Some(DiagnosticSeverity::ERROR),
                    code: None,
                    code_description: None,
                    source: Some("pain".to_string()),
                    message: format!("Parse error: {}", err),
                    related_information: None,
                    tags: None,
                    data: None,
                });
            }
        }

        diagnostics
    }
}

// Find function at given line and column position
fn find_function_at_position(program: &Program, line: usize, _column: usize) -> Option<HoverInfo> {
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
fn format_function_signature(func: &Function) -> String {
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

// Format type for display
fn format_type(ty: &Type) -> String {
    match ty {
        Type::Int => "int".to_string(),
        Type::Str => "str".to_string(),
        Type::Float32 => "float32".to_string(),
        Type::Float64 => "float64".to_string(),
        Type::Bool => "bool".to_string(),
        Type::Dynamic => "dynamic".to_string(),
        Type::List(inner) => format!("list[{}]", format_type(inner)),
        Type::Array(inner) => format!("array[{}]", format_type(inner)),
        Type::Map(k, v) => format!("map[{}, {}]", format_type(k), format_type(v)),
        Type::Tensor(inner, dims) => format!("Tensor[{}, {:?}]", format_type(inner), dims),
        Type::Named(name) => name.clone(),
    }
}

// Extract variables visible at given position (simplified implementation)
fn extract_variables_in_scope(
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
fn extract_variables_from_statements(statements: &[Statement], variables: &mut HashSet<String>) {
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

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(|client| Backend {
        client,
        documents: Arc::new(RwLock::new(HashMap::new())),
    });
    Server::new(stdin, stdout, socket).serve(service).await;
}
