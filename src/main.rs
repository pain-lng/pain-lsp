// Pain LSP server

use tower_lsp::{LspService, Server};
use tower_lsp::lsp_types::*;
use pain_compiler::{parse, type_check_program, ast::*};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

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
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult, tower_lsp::jsonrpc::Error> {
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
        self.client.log_message(MessageType::INFO, "Pain LSP server initialized").await;
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri.clone();
        let text = params.text_document.text.clone();
        self.documents.write().await.insert(uri.clone(), text.clone());
        self.on_change(uri, text).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri.clone();
        let text = params.content_changes.into_iter().next()
            .map(|change| change.text)
            .unwrap_or_default();
        self.documents.write().await.insert(uri.clone(), text.clone());
        self.on_change(uri, text).await;
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>, tower_lsp::jsonrpc::Error> {
        let _uri = params.text_document_position.text_document.uri;
        let _position = params.text_document_position.position;
        
        // TODO: Parse document and provide context-aware completions
        let items = vec![
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
                ..Default::default()
            },
            CompletionItem {
                label: "for".to_string(),
                kind: Some(CompletionItemKind::KEYWORD),
                ..Default::default()
            },
            CompletionItem {
                label: "while".to_string(),
                kind: Some(CompletionItemKind::KEYWORD),
                ..Default::default()
            },
            CompletionItem {
                label: "print".to_string(),
                kind: Some(CompletionItemKind::FUNCTION),
                detail: Some("Built-in print function".to_string()),
                ..Default::default()
            },
        ];
        
        Ok(Some(CompletionResponse::Array(items)))
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>, tower_lsp::jsonrpc::Error> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;
        
        // Get document text from cache
        let text = self.documents.read().await.get(&uri).cloned();
        if let Some(text) = text {
            if let Ok(program) = parse(&text) {
                // Find function at position (LSP uses 0-based line numbers)
                if let Some(hover_info) = find_function_at_position(&program, position.line as usize + 1, position.character as usize + 1) {
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
                                start: Position { line: 0, character: 0 },
                                end: Position { line: 0, character: 0 },
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
                        start: Position { line: 0, character: 0 },
                        end: Position { line: 0, character: 0 },
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
        let Item::Function(func) = item;
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
            sig.push_str(&format!("@{}{} ", attr.name, 
                if attr.args.is_empty() { String::new() } 
                else { format!("({})", attr.args.len()) }));
        }
    }
    
    // Function name and parameters
    sig.push_str("fn ");
    sig.push_str(&func.name);
    sig.push('(');
    
    let params: Vec<String> = func.params.iter()
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
