# Cinderstone development workflow

- Work on small, coherent porting and integration stages. Commit and publish each completed stage to the public `Ferigone/Cinderstone` repository on `master` instead of keeping completed work only in scratch storage.
- Keep the README and roadmap in English. Describe implemented, partial and missing behavior honestly; never claim complete vanilla parity or a performance advantage without evidence.
- The fixed100-item delivery checklist is a planning indicator, not behavioral compatibility, effort or remaining time. Preserve `scope_version` and its denominator; document any deliberate scope change.
- Prioritize native implementation and integration. Use a focused compiler check and a small runtime check appropriate to the change. Defer comprehensive conformance suites until the porting stage calls for them.
- Never commit credentials, player/world saves, runtime/build caches, proprietary Java binaries, captured private data, or raw JVM disassembly. Public exports must sanitize private checkpoint metadata.
- Preserve GPL-2.0-only and third-party notices. Do not relabel the HotSpot-derived math component as permissively licensed original code.
- Railway follows the deployment branch automatically. Keep the world on its existing `/data` volume, use one replica and allow graceful shutdown. Never replace or delete the world volume as part of a code update.
- Verify deployment status and the Minecraft TCP endpoint after publishing a deployable stage. A queued build is not a successful deployment.

- Publish normal source files and directories. Never commit a source archive; attach archives only to GitHub Releases when requested.
