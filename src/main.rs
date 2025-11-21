// Pain LSP server - main entry point

use pain_lsp::Backend;
use tower_lsp::{LspService, Server};


#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(|client| Backend {
        client,
        documents: std::sync::Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
    });
    Server::new(stdin, stdout, socket).serve(service).await;
}
