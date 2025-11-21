// LSP test helpers for comprehensive testing

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LspService};
use url::Url;

// Import Backend from library
use pain_lsp::Backend;

/// Test LSP client wrapper for testing
/// This provides a simplified interface for testing LSP functionality
pub struct TestLspClient {
    backend: Arc<Backend>,
}

impl TestLspClient {
    /// Create a new test LSP client with a mock client
    pub async fn new() -> Self {
        // Create a mock client for testing
        // In real implementation, we'd use tower-lsp's test utilities
        let (service, _) = LspService::new(|client| {
            Backend {
                client,
                documents: Arc::new(RwLock::new(HashMap::new())),
            }
        });
        
        // For now, we'll need to create Backend directly
        // This is a simplified version - full implementation would use proper test client
        todo!("Implement proper LSP test client setup with mock client")
    }

    /// Open a document in the LSP
    pub async fn open_document(&self, uri: Url, text: String) {
        self.backend.documents.write().await.insert(uri, text);
    }

    /// Change document content
    pub async fn change_document(&self, uri: Url, text: String) {
        self.backend.documents.write().await.insert(uri, text);
    }

    /// Get diagnostics for a document by checking it
    pub async fn get_diagnostics(&self, uri: &Url, text: &str) -> Vec<Diagnostic> {
        // Use Backend's check_document method
        // Note: This requires making check_document public or creating a test method
        self.backend.check_document(text)
    }

    /// Request completion at position
    pub async fn request_completion(
        &self,
        uri: Url,
        position: Position,
    ) -> Option<CompletionResponse> {
        // Use Backend's completion method
        // Note: This requires proper async handling
        todo!("Implement completion request")
    }

    /// Request hover information at position
    pub async fn request_hover(&self, uri: Url, position: Position) -> Option<Hover> {
        // Use Backend's hover method
        todo!("Implement hover request")
    }
}

/// Helper to create a test document URI
pub fn test_uri(path: &str) -> Url {
    Url::parse(&format!("file:///{}", path)).unwrap()
}

/// Helper to create a position
pub fn position(line: u32, character: u32) -> Position {
    Position { line, character }
}

/// Helper to create a range
pub fn range(start: Position, end: Position) -> Range {
    Range { start, end }
}

