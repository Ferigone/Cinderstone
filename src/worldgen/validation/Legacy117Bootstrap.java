import java.io.File;
import java.nio.file.*;
import java.security.MessageDigest;
import java.util.*;
import java.util.regex.Pattern;
import net.minecraft.server.MinecraftServer;

/** Plain entry: verify runtime provenance, then initialize original registries. */
public final class Legacy117Bootstrap {
    private static final String HASH =
        "e8c211b41317a9f5a780c98a89592ecb72eb39a6e475d4ac9657e5bc9ffaf55f";
    private static final String SERVER_CLASS_HASH =
        "4846a20513148027eaf124a51427df69b34560924e6471c8913613648e741f71";

    public static void main(String[] args) throws Exception {
        if (args.length != 2 && args.length != 5)
            throw new IllegalArgumentException("expected official JAR and new controlled world path, optionally seed/chunkX/chunkZ");
        Path artifact = Path.of(args[0]).toRealPath();
        StringBuilder hash = new StringBuilder();
        for (byte b : MessageDigest.getInstance("SHA-256").digest(Files.readAllBytes(artifact)))
            hash.append(String.format("%02x", b & 255));
        if (!HASH.equals(hash.toString())) throw new IllegalStateException("wrong original artifact");
        Path own = Path.of(Legacy117Bootstrap.class.getProtectionDomain().getCodeSource()
                           .getLocation().toURI()).toRealPath();
        Set<Path> runtime = new HashSet<>();
        for (String entry : System.getProperty("java.class.path").split(Pattern.quote(File.pathSeparator)))
            runtime.add(Path.of(entry).toRealPath());
        if (!runtime.equals(Set.of(own, artifact)) || !Files.isDirectory(own)
            || Files.exists(own.resolve("net/minecraft/server/MinecraftServer.class")))
            throw new IllegalStateException("compile-only headers must be absent from the exact runtime classpath");
        // Class literals do not initialize the target. Check owners before any
        // bootstrap call or subclass initialization can use their code.
        for (Class<?> type : new Class<?>[] { MinecraftServer.class, cpu.class, nk.class, ab.class, wy.class, aci.class }) {
            Path loaded = Path.of(type.getProtectionDomain().getCodeSource().getLocation().toURI()).toRealPath();
            if (!loaded.equals(artifact)) throw new IllegalStateException("non-original runtime owner " + type.getName());
        }
        StringBuilder superclassHash = new StringBuilder();
        try (var input = MinecraftServer.class.getResourceAsStream("MinecraftServer.class")) {
            if (input == null) throw new IllegalStateException("original superclass resource missing");
            for (byte b : MessageDigest.getInstance("SHA-256").digest(input.readAllBytes()))
                superclassHash.append(String.format("%02x", b & 255));
        }
        if (!SERVER_CLASS_HASH.equals(superclassHash.toString()))
            throw new IllegalStateException("runtime superclass bytes changed");
        ab.a();
        wy.a();
        wy.c();
        Legacy117World.run(args, superclassHash.toString());
    }
}
