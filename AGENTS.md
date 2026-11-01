# Cinderstone development workflow

- Work on small, coherent porting and integration stages. The integration coordinator publishes completed stages to `master`. Independent teams use assigned feature branches and issue-linked pull requests; they do not merge `master` or deploy.
- Keep the README and roadmap in English. Describe implemented, partial and missing behavior honestly; never claim complete vanilla parity or a performance advantage without evidence.
- The fixed100-item delivery checklist is a planning indicator, not behavioral compatibility, effort or remaining time. Preserve `scope_version` and its denominator; document any deliberate scope change.
- Prioritize native implementation and integration. Use a focused compiler check and a small runtime check appropriate to the change. Defer comprehensive conformance suites until the porting stage calls for them.
- Never commit credentials, player/world saves, runtime/build caches, proprietary Java binaries, captured private data, or raw JVM disassembly. Public exports must sanitize private checkpoint metadata.
- Preserve GPL-2.0-only and third-party notices. Do not relabel the HotSpot-derived math component as permissively licensed original code.
- Railway follows the deployment branch automatically. Keep the world on its existing `/data` volume, use one replica and allow graceful shutdown. Never replace or delete the world volume as part of a code update.
- Verify deployment status and the Minecraft TCP endpoint after publishing a deployable stage. A queued build is not a successful deployment.

- Publish normal source files and directories. Never commit a source archive; attach archives only to GitHub Releases when requested.

- Independent sessions follow `docs/coordination/README.md`: distinct assigned team issues, unique instance IDs, separate checkouts and explicit file ownership. GitHub comments record coordination; they are not atomic locks or automatically expiring leases.
- The integration coordinator owns central Main adapters, shared runtime/export/manifest seams, status/checklist documents, the persistent progress journal and Railway. Request narrow integration contracts before touching reserved files.
- Report whole checklist deliveries, partial implementation and blocked integration separately. Preserve the existing scope and denominator. A completed substage does not by itself complete an aggregate delivery.
- Record pauses and handoffs truthfully. Cross-chat messages, issue polling and coding do not continue automatically after a session stops.
