import com.google.gson.Gson;
import com.mojang.authlib.GameProfile;
import com.mojang.serialization.Lifecycle;
import java.io.*;
import java.net.Proxy;
import java.nio.*;
import java.nio.file.*;
import java.security.MessageDigest;
import java.util.*;
import java.util.concurrent.*;
import java.util.function.BooleanSupplier;
import java.util.regex.*;
import java.util.zip.*;
import net.minecraft.server.MinecraftServer;

/**
 * External original-1.17.1 embedding, not DedicatedServer/Main or GameTestServer.
 * The original base constructs real ServerLevels with actual normal WorldData.
 * No spawn-region warm-up is requested; FULL dependencies use the real cache.
 * This program must not be executed until the integration owner authorizes it.
 */
public final class Legacy117World extends MinecraftServer {
    private static final String SERVER_SHA256 =
        "e8c211b41317a9f5a780c98a89592ecb72eb39a6e475d4ac9657e5bc9ffaf55f";
    private static final Path CONTROLLED =
        Path.of("/workspace/blending-region-harness/runs/controlled-worlds");
    private static final Gson JSON = new Gson();
    private final CompletableFuture<Void> stopped = new CompletableFuture<>();
    private final Path world;
    private final long expectedSeed;
    private final int chunkX, chunkZ;
    private volatile Throwable failure;
    private volatile boolean baseStopReturned;
    private boolean requested;
    private Map<String,Object> requestedObservation;

    private Legacy117World(Thread thread, gx.b registries, dib.a storage,
                           dig data, adi packs, xk resources, Path world,
                           long seed, int chunkX, int chunkZ) {
        super(thread, registries, storage, data, packs, Proxy.NO_PROXY, ahr.a(),
              resources, null, null, null, acc::new);
        this.world = world;
        this.expectedSeed = seed;
        this.chunkX = chunkX;
        this.chunkZ = chunkZ;
    }

    @Override protected boolean e() throws IOException {
        // Same actual PlayerList constructor used by GameTestServer, without
        // its fixed flat-world constructor or test/status machinery.
        a(new aeh(this, l, k, 1) {});
        a(new acc(0)); // original createLevels; skip dedicated spawn warm-up
        check(o.A().a() == expectedSeed, "original world seed changed");
        check(E().k().g() instanceof cpu, "original Overworld is not normal noise");
        check(listenerCount() == 0, "embedded legacy engine opened a network listener");
        return true;
    }

    @Override public void a(BooleanSupplier budget) {
        super.a(budget);
        if (requested) return;
        requested = true;
        try {
            int beforeListeners = listenerCount();
            check(beforeListeners == 0, "legacy listener appeared before FULL request");
            cmm chunk = E().k().a(chunkX, chunkZ, cmq.m, true);
            check(chunk instanceof cmx && chunk.j() == cmq.m,
                  "actual source scheduler did not return FULL LevelChunk");
            requestedObservation = new LinkedHashMap<>();
            requestedObservation.put("chunk", List.of(chunkX, chunkZ));
            requestedObservation.put("requested_status", cmq.m.toString());
            requestedObservation.put("returned_status", chunk.j().toString());
            requestedObservation.put("requested_serialized_status", cmq.m.d());
            requestedObservation.put("returned_serialized_status", chunk.j().d());
            requestedObservation.put("returned_class", chunk.getClass().getName());
            requestedObservation.put("generator_class", E().k().g().getClass().getName());
            requestedObservation.put("network_listeners_before_request", beforeListeners);
            requestedObservation.put("network_listeners_after_request", listenerCount());
            check(listenerCount() == 0, "legacy listener appeared during generation");
        } catch (Throwable error) {
            failure = error;
        } finally {
            a(false); // halt; original runServer finally performs save/close
        }
    }

    // Javac ignores the original synthetic bridges after reading the raw ABI.
    // Retain their original casts/delegation to the unchanged source methods.
    @Override public boolean d(Runnable task) { return super.a((xm)task); }
    @Override public Runnable e(Runnable task) { return super.a(task); }
    @Override public void a(Object task) { super.h((Runnable)task); }

    @Override public void u() {
        super.u();
        baseStopReturned = true;
    }
    @Override public void f() {
        super.f();
        stopped.complete(null);
    }
    @Override public void a(q crash) {
        if (crash != null) failure = new IllegalStateException("Original server crash: " + crash);
        super.a(crash);
    }
    @Override public File B() { return world.getParent().toFile(); }
    @Override public int h() { return 4; }
    @Override public int i() { return 2; }
    @Override public boolean j() { return false; }
    @Override public boolean k() { return false; }
    @Override public int l() { return 0; }
    @Override public boolean m() { return false; }
    @Override public boolean n() { return false; }
    @Override public boolean o() { return false; }
    @Override public boolean E_() { return false; }
    @Override public boolean a(GameProfile profile) { return false; }
    @Override public Optional<String> p() { return Optional.empty(); }
    @Override public ac a(ac report) {
        report.a("Type", "Original 1.17.1 normal-world embedded harness");
        return report;
    }

    private int listenerCount() {
        try {
            java.lang.reflect.Field field = aci.class.getDeclaredField("f");
            field.setAccessible(true);
            List<?> listeners = (List<?>)field.get(ae());
            synchronized (listeners) { return listeners.size(); }
        } catch (ReflectiveOperationException error) {
            throw new IllegalStateException("cannot observe original listener owner", error);
        }
    }

    static void run(String[] args, String verifiedSuperclassHash) throws Exception {
        check(args.length == 2 || args.length == 5,
              "usage: Legacy117Bootstrap <official-server.jar> <new-world-dir> [seed chunkX chunkZ]");
        Path artifact = Path.of(args[0]).toRealPath();
        Path loaded = Path.of(MinecraftServer.class.getProtectionDomain().getCodeSource()
                              .getLocation().toURI()).toRealPath();
        check(artifact.equals(loaded) && sha(Files.readAllBytes(loaded)).equals(SERVER_SHA256),
              "wrong actually loaded original server JAR");
        Path world = Path.of(args[1]).toAbsolutePath().normalize();
        check(world.startsWith(CONTROLLED) && world.getNameCount() > CONTROLLED.getNameCount(),
              "world must be an explicitly controlled harness directory");
        check(!Files.exists(world), "refusing to replace or reuse an existing world");
        Path workingEula = Path.of("eula.txt").toAbsolutePath().normalize();
        boolean eulaAbsentBefore = !Files.exists(workingEula) && !Files.exists(world.resolve("eula.txt"));
        check(eulaAbsentBefore, "EULA file present before controlled embedded run");
        long seed = args.length == 5 ? Long.parseLong(args[2]) : 123456789L;
        int cx = args.length == 5 ? Integer.parseInt(args[3]) : 0;
        int cz = args.length == 5 ? Integer.parseInt(args[4]) : 0;
        Files.createDirectories(world.getParent());

        // The separate non-server entry bootstraps before this subclass can
        // initialize MinecraftServer's static fields.
        gx.b registries = gx.a();
        adi packs = new adi(acw.b, new adk[] { new adl() });
        xk resources = null;
        dib.a storage = null;
        Legacy117World server = null;
        try {
            packs.a();
            packs.a(List.of("vanilla"));
            dm.a commandSelection = (dm.a)dm.a.class.getField("a").get(null);
            resources = xk.a(packs.f(), registries, commandSelection, 2, ad.f(), Runnable::run).get();
            resources.j();
            gw<cnv> dimensions = registries.d(gw.P);
            gw<bxp> biomes = registries.d(gw.aO);
            gw<cpv> noise = registries.d(gw.aH);
            gr<cnw> stems = cnv.a(dimensions, biomes, noise, seed);
            cpu normal = cql.a(biomes, noise, seed);
            cql generatorSettings = new cql(seed, true, false,
                                            cql.a(dimensions, stems, normal));
            bwu levelSettings = new bwu(world.getFileName().toString(), bwn.a, false,
                                       ary.c, false, new bwm(), bwd.a);
            dig data = new dig(levelSettings, generatorSettings, Lifecycle.stable());
            storage = dib.a(world.getParent()).c(world.getFileName().toString());
            final dib.a actualStorage = storage;
            final xk actualResources = resources;
            server = MinecraftServer.a(thread -> new Legacy117World(thread, registries,
                actualStorage, data, packs, actualResources, world, seed, cx, cz));
            server.stopped.get(100, TimeUnit.SECONDS);
            server.a(true); // join the original server thread after save/close
            check(server.baseStopReturned, "original stopServer did not return");
            if (server.failure != null) throw new IllegalStateException("source generation failed", server.failure);
            check(server.requestedObservation != null, "no actual FULL request completed");
            // The provenance artifact is written only after source save/close.
            Map<String,Object> report = provenance(world, seed, server.requestedObservation,
                                                  verifiedSuperclassHash);
            report.put("network_listeners_after_stop", server.listenerCount());
            check(server.listenerCount() == 0, "legacy listener remained after stop");
            report.put("eula_file_absent_before", eulaAbsentBefore);
            boolean eulaAbsentAfter = !Files.exists(workingEula) && !Files.exists(world.resolve("eula.txt"));
            check(eulaAbsentAfter, "EULA file appeared during controlled embedded run");
            report.put("eula_file_absent_after", eulaAbsentAfter);
            report.put("eula_observation_scope", "process working directory and actual new world root; read only");
            Files.writeString(world.resolve("original-generation-provenance.json"),
                              JSON.toJson(report) + "\n", StandardOpenOption.CREATE_NEW);
            System.out.println(JSON.toJson(report));
        } finally {
            if (server != null) {
                server.a(false);
            } else {
                if (resources != null) resources.close();
                if (storage != null) storage.close();
            }
            packs.close();
            ad.h();
        }
    }

    private static Map<String,Object> provenance(Path world, long seed,
                                                 Map<String,Object> request,
                                                 String superclassHash) throws Exception {
        Path levelFile = world.resolve("level.dat");
        na levelRoot = nk.a(levelFile.toFile());
        na data = compound(levelRoot, "Data");
        check(integer(data, "DataVersion") == 2730, "saved level is not actual 1.17.1 DataVersion");
        na settings = compound(data, "WorldGenSettings");
        check(settings.b("seed", 4) && settings.i("seed") == seed, "saved original seed differs");
        na generator = compound(compound(compound(settings, "dimensions"),
                                         "minecraft:overworld"), "generator");
        check(string(generator, "type").equals("minecraft:noise"), "saved original generator is not noise");
        List<Object> chunks = new ArrayList<>();
        List<Object> regions = new ArrayList<>();
        Map<String,Integer> statuses = new TreeMap<>();
        Pattern name = Pattern.compile("r\\.(-?\\d+)\\.(-?\\d+)\\.mca");
        try (var files = Files.list(world.resolve("region"))) {
            for (Path file : files.sorted().toList()) {
                Matcher match = name.matcher(file.getFileName().toString());
                if (!match.matches()) continue;
                int rx = Integer.parseInt(match.group(1)), rz = Integer.parseInt(match.group(2));
                byte[] bytes = Files.readAllBytes(file);
                regions.add(Map.of("file", world.relativize(file).toString(),
                                   "size", bytes.length, "sha256", sha(bytes)));
                // Original reads may create an empty RegionFile placeholder.
                // It has no populated slots, but remains in the raw inventory.
                if (bytes.length == 0) continue;
                check(bytes.length >= 8192, "short nonempty original region file");
                ByteBuffer buffer = ByteBuffer.wrap(bytes).order(ByteOrder.BIG_ENDIAN);
                for (int slot = 0; slot < 1024; slot++) {
                    int location = buffer.getInt(slot * 4);
                    if (location == 0) continue;
                    int x = rx * 32 + (slot & 31), z = rz * 32 + (slot >> 5);
                    int start = (location >>> 8) * 4096, sectors = location & 255;
                    check(sectors > 0 && start >= 8192 && start + 5 <= bytes.length,
                          "invalid original region location");
                    int length = buffer.getInt(start), marker = bytes[start + 4] & 255;
                    check(length >= 1 && (long)start + 4 + length <= bytes.length
                          && length + 4 <= sectors * 4096, "invalid original chunk record");
                    byte[] compressed = (marker & 128) == 0
                        ? Arrays.copyOfRange(bytes, start + 5, start + 4 + length)
                        : Files.readAllBytes(file.resolveSibling("c." + x + "." + z + ".mcc"));
                    byte[] nbt;
                    try (InputStream raw = new ByteArrayInputStream(compressed);
                         InputStream input = (marker & 127) == 1 ? new GZIPInputStream(raw)
                            : (marker & 127) == 2 ? new InflaterInputStream(raw)
                            : (marker & 127) == 3 ? raw
                            : unsupportedCompression(marker)) {
                        nbt = input.readAllBytes();
                    }
                    na chunk = nk.a((DataInput)new DataInputStream(new ByteArrayInputStream(nbt)));
                    int version = integer(chunk, "DataVersion");
                    na contents = compound(chunk, "Level");
                    check(version == 2730 && integer(contents, "xPos") == x
                          && integer(contents, "zPos") == z, "saved original chunk identity changed");
                    String status = string(contents, "Status");
                    statuses.merge(status, 1, Integer::sum);
                    chunks.add(Map.of("chunk", List.of(x, z), "DataVersion", version,
                                      "status", status, "nbt_sha256", sha(nbt),
                                      "compressed_sha256", sha(compressed),
                                      "region", world.relativize(file).toString()));
                }
            }
        }
        boolean center = false;
        for (Object entry : chunks) {
            Map<?,?> chunk = (Map<?,?>)entry;
            if (chunk.get("chunk").equals(request.get("chunk"))) {
                // ChunkSerializer writes getName(), while toString() returns
                // the registry-qualified key. Preserve both observed names.
                check(chunk.get("status").equals(request.get("returned_serialized_status")),
                      "saved requested chunk lost actual FULL status");
                center = true;
            }
        }
        check(center, "actual FULL target was not saved");
        Map<String,Object> report = new LinkedHashMap<>();
        report.put("official_version", "1.17.1");
        report.put("official_server_sha256", SERVER_SHA256);
        report.put("runtime_original_superclass_sha256", superclassHash);
        report.put("seed", Long.toString(seed));
        report.put("world", world.toString());
        report.put("profile", "original normal NoiseBasedChunkGenerator and original registry settings");
        report.put("entry", "embedded original MinecraftServer.createLevels, then real ServerChunkCache FULL request");
        report.put("dedicated_spawn_warmup", false);
        report.put("base_stop_server_returned_before_observation", true);
        report.put("requested", request);
        report.put("level_dat_sha256", sha(Files.readAllBytes(levelFile)));
        report.put("saved_DataVersion", integer(data, "DataVersion"));
        report.put("saved_generator_type", string(generator, "type"));
        report.put("regions", regions);
        report.put("observed_saved_chunks", chunks);
        report.put("observed_status_counts", statuses);
        List<Object> rawFiles = new ArrayList<>();
        try (var paths = Files.walk(world)) {
            for (Path path : paths.filter(Files::isRegularFile).sorted().toList())
                rawFiles.add(Map.of("file", world.relativize(path).toString(),
                                    "size", Files.size(path), "sha256", sha(Files.readAllBytes(path))));
        }
        report.put("raw_world_files_before_harness_metadata", rawFiles);
        return report;
    }

    private static InputStream unsupportedCompression(int marker) throws IOException {
        throw new IOException("unsupported actual region compression " + marker);
    }
    private static na compound(na tag, String key) {
        check(tag.b(key, 10), "missing original compound " + key);
        return tag.p(key);
    }
    private static int integer(na tag, String key) {
        check(tag.b(key, 3), "missing original integer " + key);
        return tag.h(key);
    }
    private static String string(na tag, String key) {
        check(tag.b(key, 8), "missing original string " + key);
        return tag.l(key);
    }
    private static String sha(byte[] bytes) throws Exception {
        StringBuilder out = new StringBuilder();
        for (byte b : MessageDigest.getInstance("SHA-256").digest(bytes))
            out.append(String.format("%02x", b & 255));
        return out.toString();
    }
    private static void check(boolean condition, String message) {
        if (!condition) throw new IllegalStateException(message);
    }
}
