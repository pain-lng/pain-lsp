// Pain LSP server - main entry point

use pain_lsp::Backend;
use tower_lsp::{LspService, Server};

#[tokio::main]
async fn main() {
    // Set panic hook to log panics before they crash the server
    std::panic::set_hook(Box::new(|panic_info| {
        eprintln!("LSP PANIC: {:?}", panic_info);
        eprintln!("LSP PANIC location: {:?}", panic_info.location());
        if let Some(s) = panic_info.payload().downcast_ref::<&str>() {
            eprintln!("LSP PANIC message: {}", s);
        } else if let Some(s) = panic_info.payload().downcast_ref::<String>() {
            eprintln!("LSP PANIC message: {}", s);
        }
    }));

    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(|client| Backend::new(client));
    
    // Wrap serve in catch_unwind to prevent crashes
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        Server::new(stdin, stdout, socket).serve(service)
    }));
    
    match result {
        Ok(serve_future) => {
            serve_future.await;
        }
        Err(_) => {
            eprintln!("LSP: Server initialization panicked");
            std::process::exit(1);
        }
    }
}
