# pain-lsp — guide for AI agents

Language server for the Pain language, built on `tower-lsp`. It is a member of the
workspace in the parent repository (`pain-lng/pain`) and depends on
`../pain-compiler` by path, so it only builds from inside that workspace.
General rules for the project are in the parent repository's `AGENTS.md`.

## Layout

| File | Responsibility |
| --- | --- |
| `src/lsp.rs` | `Backend`: document store, diagnostics (`check_document`), completion, hover |
| `src/lib.rs` | Re-exports used by the tests and by `main.rs` |
| `src/main.rs` | Binary `pain-lsp`: stdio transport, debug log file |
| `tests/` | Integration tests, one file per feature area |
| `build.rs`, `resources/` | Windows icon embedding |

## Commands

```bash
cd pain-lsp && cargo test --tests        # the form CI uses
cargo clippy -p pain-lsp --all-targets -- -D warnings
cargo build --release -p pain-lsp        # the binary the VS Code extension launches
```

## Conventions

- Diagnostics are whatever the compiler reports: `check_document` runs
  `parse_with_recovery`, the type checker and the warning collector from
  `pain-compiler`. Fix wrong diagnostics in the compiler, not by filtering here.
- Tests build a real backend without a client connection:

  ```rust
  let (service, _socket) = tower_lsp::LspService::new(Backend::new);
  let diagnostics = service.inner().check_document(code);
  ```

  Tests that only need parsing call `pain_compiler` directly.
- Pain code in tests must be valid Pain unless the test is about errors. Check a
  snippet with `pain check` when in doubt.

## Things that are easy to get wrong

- **stdout is the protocol channel.** Never `println!` in the server; log with
  `eprintln!` or to the debug log (`pain_lsp_debug.log` in the system temp directory).
- **Handlers must not panic or block.** Compiler calls are wrapped in
  `catch_unwind`; keep locks on the document map and the parse cache short and do
  not hold them across `.await` or a compiler call.
- A stricter type checker shows up here immediately as new editor diagnostics.
  When changing checks in `pain-compiler`, run the LSP tests too.
- CI runs clippy with `-D warnings` on the test targets as well: no unused
  variables, no `assert!(true)` placeholders.
