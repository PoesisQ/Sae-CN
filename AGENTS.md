# Sae Reader development constraints

- Native, simple and efficient operation is the permanent first principle. Implement application logic in Rust wherever practical; retain the approved Rust + Slint + native MuPDF route. No browser engine or web runtime.
- Initialize format adapters and optional features only when needed. Bound caches, avoid duplicate buffers and background polling, and measure both speed and memory with real Windows builds.
- Preserve document content, selection, links and typography. Do not trade away EPUB compatibility or layout correctness to report better performance.
- Use the engineering brief as the requirements and acceptance baseline. Report evidence and solution options for major native engine limitations; do not claim untested acceptance gates.
- Keep user books read-only and out of the repository, diagnostics and release archives. Distribute corresponding source and licenses under the owner's AGPL decision.
