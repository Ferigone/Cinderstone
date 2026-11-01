# Third-party notices and reference data

The Cinderstone combined Rust implementation is distributed under GPL-2.0-only. Existing third-party copyright and license notices remain applicable to their respective files. This license statement does not relicense third-party data or dependencies.

- `src/hotspot_math.rs` derives from Intel/Oracle OpenJDK HotSpot math code under GPL-2.0-only. Preserve its source header and `licenses/hotspot-math-GPL2.txt`.
- Preserve `licenses/fdlibm-notice.txt` for the corresponding mathematical routines.
- Rust dependency license texts and the dependency inventory are retained under `third_party/`. Dependencies retain their own license terms. The deployment links the operating system OpenSSL library.
- The `minecraft-data` npm wrapper (`PrismarineJS/node-minecraft-data`) declares MIT in its package metadata. Embedded protocol/game reference data retains its upstream provenance; project GPL terms do not relicense that data.
- Minecraft identifiers, registry/protocol captures, recipe definitions and extracted behavioral facts are reference data derived from Minecraft Java 1.21. Their provenance is recorded in the relevant fixture metadata and `fixtures/source-lock.json`; the project license does not grant ownership of Mojang/Microsoft material or Minecraft trademarks.

The repository does not distribute the Minecraft server/client JARs, world/player saves, session credentials, or raw JVM disassembly. Minecraft is a trademark of Mojang/Microsoft. Cinderstone is an independent project and is not affiliated with or endorsed by Mojang or Microsoft.
