// LSP test helpers for comprehensive testing
// Shared by other test crates, so not every helper is used in each of them
#![allow(dead_code)]

use tower_lsp::lsp_types::*;
use tower_lsp::LspService;
use url::Url;

// Import Backend from library
use pain_lsp::Backend;

/// Test LSP client wrapper for testing
/// This provides a simplified interface for testing LSP functionality
pub struct TestLspClient {
    // The service is never driven; it only owns the Backend (see `backend()`)
    service: LspService<Backend>,
}

impl TestLspClient {
    /// Create a new test LSP client
    pub fn new() -> Self {
        let (service, _socket) = LspService::new(Backend::new);
        Self { service }
    }

    fn backend(&self) -> &Backend {
        self.service.inner()
    }

    /// Open a document in the LSP
    pub async fn open_document(&self, uri: Url, text: String) {
        self.backend().documents.write().await.insert(uri, text);
    }

    /// Change document content
    pub async fn change_document(&self, uri: Url, text: String) {
        self.backend().documents.write().await.insert(uri, text);
    }

    /// Get diagnostics for a document by checking it
    pub fn get_diagnostics(&self, text: &str) -> Vec<Diagnostic> {
        self.backend().check_document(text)
    }
}

impl Default for TestLspClient {
    fn default() -> Self {
        Self::new()
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

#[tokio::test]
async fn test_client_tracks_documents_and_reports_diagnostics() {
    let client = TestLspClient::new();
    let uri = test_uri("main.pain");
    client
        .open_document(
            uri.clone(),
            "fn main():
    let x = 1
"
            .to_string(),
        )
        .await;
    client
        .change_document(
            uri.clone(),
            "fn main(:
"
            .to_string(),
        )
        .await;
    let text = client
        .backend()
        .documents
        .read()
        .await
        .get(&uri)
        .cloned()
        .unwrap();
    assert_eq!(
        text,
        "fn main(:
"
    );
    assert!(!client.get_diagnostics(&text).is_empty());
    let _ = range(position(0, 0), position(0, 1));
}
