// Pain LSP server - main entry point

use pain_lsp::Backend;
use tower_lsp::{LspService, Server};
use std::fs::OpenOptions;
use std::io::Write;

// Helper function to log to file (in temp directory for visibility)
fn log_to_file(msg: &str) {
    let log_path = std::env::temp_dir().join("pain_lsp_debug.log");
    if let Ok(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
    {
        let _ = writeln!(file, "[{}] {}", chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f"), msg);
    }
}

#[tokio::main]
async fn main() {
    let log_path = std::env::temp_dir().join("pain_lsp_debug.log");
    eprintln!("=== Pain LSP starting, log file: {:?} ===", log_path);
    
    log_to_file("=== LSP MAIN START ===");
    log_to_file(&format!("Log file location: {:?}", log_path));
    log_to_file(&format!("Current working directory: {:?}", std::env::current_dir()));
    
    // Set panic hook to log panics before they crash the server
    std::panic::set_hook(Box::new(|panic_info| {
        let msg = format!("LSP PANIC: {:?}", panic_info);
        eprintln!("{}", msg);
        log_to_file(&msg);
        
        let loc_msg = format!("LSP PANIC location: {:?}", panic_info.location());
        eprintln!("{}", loc_msg);
        log_to_file(&loc_msg);
        
        if let Some(s) = panic_info.payload().downcast_ref::<&str>() {
            let panic_msg = format!("LSP PANIC message: {}", s);
            eprintln!("{}", panic_msg);
            log_to_file(&panic_msg);
        } else if let Some(s) = panic_info.payload().downcast_ref::<String>() {
            let panic_msg = format!("LSP PANIC message: {}", s);
            eprintln!("{}", panic_msg);
            log_to_file(&panic_msg);
        }
    }));
    log_to_file("Panic hook set");

    log_to_file("Creating stdin/stdout");
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    log_to_file("stdin/stdout created");

    log_to_file("Creating LspService");
    let (service, socket) = LspService::new(|client| {
        log_to_file("Backend::new called");
        Backend::new(client)
    });
    log_to_file("LspService created");
    
    log_to_file("Starting server");
    Server::new(stdin, stdout, socket).serve(service).await;
    log_to_file("Server stopped");
}
