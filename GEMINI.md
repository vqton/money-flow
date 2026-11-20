Read and follow `AGENTS.md` at the repository root. It is the source of truth.

## Gemini / Antigravity Operational Guidelines

* **Instruction Hierarchy**: Follow `AGENTS.md` for all codebase invariants, financial safety rules, architecture layers, and verification commands.
* **Context Loading & Reading**:
  * For targeted code inspection, use `read_file` with explicit line ranges (`StartLine`/`EndLine`) or `Select-String` via PowerShell. Avoid reading entire 1,500+ line files at once.
  * For codebase understanding, prefer CodeGraph tools (`codegraph_explore`) when `.codegraph/` is present.
* **Execution Environment**:
  * Execute commands using PowerShell on Windows. Always specify `--manifest-path backend/Cargo.toml` for cargo operations.
  * Toolchain binaries reside in `D:\rust\cargo\bin`.
* **Output Standards**:
  * Keep responses concise and structured in GitHub-flavored Markdown.
  * Cite specific file paths and line ranges (`file:///path/to/file#L10-L25`) for all evidence.
  * Reserve LaTeX math (`$...$`, `$$...$$`) for multi-term financial formulas.
