# Native target source packaging

PR116 merged as `5f959b999b1fce2a2f3ef2e29eb4214a64a22fe3`. Its actual
full-checkout CI41388277220 passed every job and step, but automatic release
`0f814478-1f80-4067-b020-2aa8706073b0` failed with Rust E0583 at
`worldgen/mod.rs`: `.dockerignore` admits `src/**` and subsequently excludes
`**/target/**`, removing the newly introduced source directory. The previously
successful release stayed live. A compiler check on an unfiltered Git checkout
did not verify this packaging boundary.

The correction moves only the four owned source files from
`src/worldgen/target/` to `src/worldgen/chunk_target/` and selects that module
with `#[path = "chunk_target/mod.rs"] pub mod target;`. Public
`worldgen::target` paths, child modules, fixture include depth and APIs remain
unchanged. Docker, deployment and reserved central files are untouched.

| Historical file under `target/` | Current file under `chunk_target/` | Identical content SHA256 |
| --- | --- | --- |
| facts.rs | facts.rs | edb13e3c576f538a10c985e418f50378a5850cd049d38d1e4b462ebfcc43478a |
| host.rs | host.rs | eb58ab7654c597592f94f8d657fe2d516b55407e74f890dba21f99e65557c331 |
| mod.rs | mod.rs | e61066d51705f2a5925293591953dd8b6723424a7cb28bcb01700613a3ab4566 |
| native.rs | native.rs | 5cbc33d97299c406534b62fc261bd4eb1fa1e3c09775d03aa2df4b5cc1ad68c7 |

Both existing-target runtime/validation fixtures intentionally describe their
actual native source commit `a28bfc2ef8b11581b9a09f76a9d54f25e8afc890`.
Their `production_sha256` keys include the historical `target/` paths and the
old parent-module hash. They are historical paired-run evidence, not a claim
that those paths or the old parent hash exist in the corrected checkout.
They are not rewritten to imply a new numeric replay. The four identical
file hashes above establish the relocation mapping; current compiler and
Docker-context validation are separate checks.

Six subordinate read-only reviews check public API, aquifer/NoiseChunk
lifecycle, fixture includes, checker discovery and packaging/provenance.
The first local Docker attempt stopped during apt for disk space; the second
reached COPY but again exhausted space. Task-owned old Cargo caches were
cleaned. The third reached the actual Cargo command but failed HTTPS registry
access because the inner image lacks this executor's proxy CA. TLS was not
disabled and deployment configuration was not changed.

A temporary external Dockerfile retained the production builder instructions
through every COPY, before its networked Cargo RUN. With the repository's
actual `.dockerignore`, it produced image
`sha256:c14f6d51a0b66a9117fefa8f13446003a4e7c54d4693bf9bf6a6373eb5aed846`.
Inside that image, checks confirmed the new module exists, the old excluded
directory does not, and then `cargo check --locked --lib --bin mc121-rust`
passed in34.32s, process exit0. It used unchanged Rust1.99.1/OpenSSL builder
inputs, a read-only mount of already cached registry dependencies,
`CARGO_NET_OFFLINE=true`, and task-owned `/tmp` compiler output. No network,
credential or TLS fallback was used. This proves filtered-context module and
compiler integration; it is not a successful local release-image build or a
new original/native numeric replay. Hosted release is recorded separately.
