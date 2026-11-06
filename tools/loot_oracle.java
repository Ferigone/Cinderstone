import com.google.gson.*;
import com.mojang.serialization.*;
import io.netty.buffer.*;
import java.io.*;
import java.lang.reflect.*;
import java.nio.charset.StandardCharsets;
import java.nio.file.*;
import java.util.*;
import java.util.concurrent.*;
import jdk.internal.org.objectweb.asm.*;
import jdk.internal.org.objectweb.asm.Type;
import net.minecraft.*;
import net.minecraft.commands.Commands;
import net.minecraft.core.*;
import net.minecraft.core.registries.*;
import net.minecraft.nbt.*;
import net.minecraft.network.RegistryFriendlyByteBuf;
import net.minecraft.resources.*;
import net.minecraft.tags.TagKey;
import net.minecraft.server.*;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.server.packs.*;
import net.minecraft.server.packs.repository.*;
import net.minecraft.server.packs.resources.*;
import net.minecraft.util.RandomSource;
import net.minecraft.world.scores.Objective;
import net.minecraft.world.scores.ScoreHolder;
import net.minecraft.world.scores.criteria.ObjectiveCriteria;
import net.minecraft.world.damagesource.*;
import net.minecraft.world.entity.*;
import net.minecraft.world.flag.FeatureFlags;
import net.minecraft.world.item.*;
import net.minecraft.world.item.enchantment.*;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.block.state.properties.Property;
import net.minecraft.world.level.storage.loot.*;
import net.minecraft.world.level.storage.loot.parameters.*;
import net.minecraft.world.phys.Vec3;

/**
 * Local differential oracle for the Cinderstone loot port.
 *
 * Runs the UNCHANGED original Minecraft Java 1.21 loot classes (from the pinned server jar
 * remapped with the official mappings, kept outside Git) over scenarios that describe a
 * synthetic Level, loot context parameters and an RNG seed.  The only synthetic parts are the
 * Level (a generated subclass whose queries are answered from the scenario) and entity state
 * that has no loot semantics of its own.
 */
public class loot_oracle {
  static final Gson GSON = new GsonBuilder().serializeNulls().create();
  static ReloadableServerResources res;
  static RegistryAccess access;
  static HolderGetter.Provider lookup;
  static RegistryOps<JsonElement> jsonOps;
  static RegistryOps<Tag> nbtOps;
  static ServerLevel level;
  static sun.misc.Unsafe unsafe;
  static JsonObject scenarioLevel = new JsonObject();

  // ---------------------------------------------------------------- bootstrap
  @SuppressWarnings("unchecked")
  static void boot() throws Throwable {
    SharedConstants.tryDetectVersion();
    Bootstrap.bootStrap();
    PackRepository repo = ServerPacksSource.createVanillaTrustedRepository();
    repo.reload();
    List<PackResources> packs = repo.getAvailablePacks().stream().filter(p -> p.getId().equals("vanilla")).map(Pack::open).toList();
    MultiPackResourceManager rm = new MultiPackResourceManager(PackType.SERVER_DATA, packs);
    LayeredRegistryAccess<RegistryLayer> layered = RegistryLayer.createRegistryAccess();
    Method m = WorldLoader.class.getDeclaredMethod("loadAndReplaceLayer", ResourceManager.class, LayeredRegistryAccess.class, RegistryLayer.class, List.class);
    m.setAccessible(true);
    layered = (LayeredRegistryAccess<RegistryLayer>) m.invoke(null, rm, layered, RegistryLayer.WORLDGEN, RegistryDataLoader.WORLDGEN_REGISTRIES);
    Executor ex = Runnable::run;
    res = ReloadableServerResources.loadResources(rm, layered, FeatureFlags.VANILLA_SET, Commands.CommandSelection.DEDICATED, 4, ex, ex).get();
    res.updateRegistryTags();
    access = res.fullRegistries().get();
    lookup = res.fullRegistries().lookup();
    jsonOps = RegistryOps.create(JsonOps.INSTANCE, access);
    nbtOps = RegistryOps.create(NbtOps.INSTANCE, access);
    Field f = sun.misc.Unsafe.class.getDeclaredField("theUnsafe");
    f.setAccessible(true);
    unsafe = (sun.misc.Unsafe) f.get(null);
    stubServer = makeServer();
    level = makeLevel();
  }

  // ---------------------------------------------------------------- stub level
  static class Loader extends ClassLoader {
    Loader(ClassLoader parent) { super(parent); }
    Class<?> define(String name, byte[] b) { return defineClass(name, b, 0, b.length); }
  }

  static void hookMethod(ClassWriter cw, String name, String desc) {
    MethodVisitor mv = cw.visitMethod(Opcodes.ACC_PUBLIC, name, desc, null, null);
    mv.visitCode();
    Type[] args = Type.getArgumentTypes(desc);
    mv.visitLdcInsn(name);
    mv.visitVarInsn(Opcodes.ALOAD, 0);
    mv.visitLdcInsn(args.length);
    mv.visitTypeInsn(Opcodes.ANEWARRAY, "java/lang/Object");
    int slot = 1;
    for (int i = 0; i < args.length; i++) {
      mv.visitInsn(Opcodes.DUP);
      mv.visitLdcInsn(i);
      mv.visitVarInsn(args[i].getOpcode(Opcodes.ILOAD), slot);
      slot += args[i].getSize();
      switch (args[i].getSort()) {
        case Type.BOOLEAN -> mv.visitMethodInsn(Opcodes.INVOKESTATIC, "java/lang/Boolean", "valueOf", "(Z)Ljava/lang/Boolean;", false);
        case Type.INT -> mv.visitMethodInsn(Opcodes.INVOKESTATIC, "java/lang/Integer", "valueOf", "(I)Ljava/lang/Integer;", false);
        case Type.LONG -> mv.visitMethodInsn(Opcodes.INVOKESTATIC, "java/lang/Long", "valueOf", "(J)Ljava/lang/Long;", false);
        case Type.FLOAT -> mv.visitMethodInsn(Opcodes.INVOKESTATIC, "java/lang/Float", "valueOf", "(F)Ljava/lang/Float;", false);
        case Type.DOUBLE -> mv.visitMethodInsn(Opcodes.INVOKESTATIC, "java/lang/Double", "valueOf", "(D)Ljava/lang/Double;", false);
        default -> {}
      }
      mv.visitInsn(Opcodes.AASTORE);
    }
    mv.visitMethodInsn(Opcodes.INVOKESTATIC, "loot_oracle", "hook", "(Ljava/lang/String;Ljava/lang/Object;[Ljava/lang/Object;)Ljava/lang/Object;", false);
    Type ret = Type.getReturnType(desc);
    switch (ret.getSort()) {
      case Type.VOID -> { mv.visitInsn(Opcodes.POP); mv.visitInsn(Opcodes.RETURN); }
      case Type.BOOLEAN -> { mv.visitTypeInsn(Opcodes.CHECKCAST, "java/lang/Boolean"); mv.visitMethodInsn(Opcodes.INVOKEVIRTUAL, "java/lang/Boolean", "booleanValue", "()Z", false); mv.visitInsn(Opcodes.IRETURN); }
      case Type.INT -> { mv.visitTypeInsn(Opcodes.CHECKCAST, "java/lang/Integer"); mv.visitMethodInsn(Opcodes.INVOKEVIRTUAL, "java/lang/Integer", "intValue", "()I", false); mv.visitInsn(Opcodes.IRETURN); }
      case Type.LONG -> { mv.visitTypeInsn(Opcodes.CHECKCAST, "java/lang/Long"); mv.visitMethodInsn(Opcodes.INVOKEVIRTUAL, "java/lang/Long", "longValue", "()J", false); mv.visitInsn(Opcodes.LRETURN); }
      default -> { mv.visitTypeInsn(Opcodes.CHECKCAST, ret.getInternalName()); mv.visitInsn(Opcodes.ARETURN); }
    }
    mv.visitMaxs(0, 0);
    mv.visitEnd();
  }

  static Object stubServer;

  static Object makeServer() throws Throwable {
    ClassWriter cw = new ClassWriter(ClassWriter.COMPUTE_MAXS);
    cw.visit(Opcodes.V21, Opcodes.ACC_PUBLIC, "oracle/StubServer", null, "net/minecraft/server/MinecraftServer", null);
    hookMethod(cw, "reloadableRegistries", "()Lnet/minecraft/server/ReloadableServerRegistries$Holder;");
    cw.visitEnd();
    Class<?> c = new Loader(loot_oracle.class.getClassLoader()).define("oracle.StubServer", cw.toByteArray());
    return unsafe.allocateInstance(c);
  }

  static JsonObject scenarioBlockEntity = null;

  /** A BlockEntity whose implicit components come from the scenario. */
  static Object makeBlockEntity() throws Throwable {
    ClassWriter cw = new ClassWriter(ClassWriter.COMPUTE_MAXS);
    cw.visit(Opcodes.V21, Opcodes.ACC_PUBLIC, "oracle/StubBlockEntity", null, "net/minecraft/world/level/block/entity/BlockEntity", null);
    hookMethod(cw, "collectImplicitComponents", "(Lnet/minecraft/core/component/DataComponentMap$Builder;)V");
    cw.visitEnd();
    Class<?> c = new Loader(loot_oracle.class.getClassLoader()).define("oracle.StubBlockEntity", cw.toByteArray());
    Object be = unsafe.allocateInstance(c);
    Field f = net.minecraft.world.level.block.entity.BlockEntity.class.getDeclaredField("components");
    f.setAccessible(true);
    f.set(be, net.minecraft.core.component.DataComponentMap.EMPTY);
    return be;
  }

  static ServerLevel makeLevel() throws Throwable {
    ClassWriter cw = new ClassWriter(ClassWriter.COMPUTE_MAXS);
    cw.visit(Opcodes.V21, Opcodes.ACC_PUBLIC, "oracle/StubLevel", null, "net/minecraft/server/level/ServerLevel", null);
    String[][] hooks = {
      {"enabledFeatures", "()Lnet/minecraft/world/flag/FeatureFlagSet;"},
      {"getRecipeManager", "()Lnet/minecraft/world/item/crafting/RecipeManager;"},
      {"getServer", "()Lnet/minecraft/server/MinecraftServer;"},
      {"registryAccess", "()Lnet/minecraft/core/RegistryAccess;"},
      {"getBlockState", "(Lnet/minecraft/core/BlockPos;)Lnet/minecraft/world/level/block/state/BlockState;"},
      {"isLoaded", "(Lnet/minecraft/core/BlockPos;)Z"},
      {"getGameTime", "()J"},
      {"getDayTime", "()J"},
      {"isRaining", "()Z"},
      {"isThundering", "()Z"},
      {"getBiome", "(Lnet/minecraft/core/BlockPos;)Lnet/minecraft/core/Holder;"},
      {"canSeeSky", "(Lnet/minecraft/core/BlockPos;)Z"},
      {"dimension", "()Lnet/minecraft/resources/ResourceKey;"},
    };
    for (String[] h : hooks) hookMethod(cw, h[0], h[1]);
    cw.visitEnd();
    Class<?> c = new Loader(loot_oracle.class.getClassLoader()).define("oracle.StubLevel", cw.toByteArray());
    ServerLevel l = (ServerLevel) unsafe.allocateInstance(c);
    Field random = net.minecraft.world.level.Level.class.getField("random");
    unsafe.putObject(l, unsafe.objectFieldOffset(random), RandomSource.create(1L));
    return l;
  }

  public static Object hook(String name, Object self, Object[] a) {
    switch (name) {
      case "enabledFeatures": return FeatureFlags.VANILLA_SET;
      case "getRecipeManager": return res.getRecipeManager();
      case "collectImplicitComponents": {
        net.minecraft.core.component.DataComponentMap.Builder b = (net.minecraft.core.component.DataComponentMap.Builder) a[0];
        if (scenarioBlockEntity.has("custom_name")) b.set(net.minecraft.core.component.DataComponents.CUSTOM_NAME, net.minecraft.network.chat.Component.literal(scenarioBlockEntity.get("custom_name").getAsString()));
        if (scenarioBlockEntity.has("lock")) b.set(net.minecraft.core.component.DataComponents.LOCK, new net.minecraft.world.LockCode(scenarioBlockEntity.get("lock").getAsString()));
        return null;
      }
      case "getServer": return stubServer;
      case "getPlayerList": return scorePlayers;
      case "broadcastAll": { if (a.length == 1) scoreBroadcasts.add((net.minecraft.network.protocol.Packet<?>) a[0]); return null; }
      case "getPlayers": return new ArrayList<>();
      case "reloadableRegistries": return res.fullRegistries();
      case "registryAccess": return access;
      case "isLoaded": return Boolean.TRUE;
      case "getGameTime": return scenarioLevel.has("game_time") ? scenarioLevel.get("game_time").getAsLong() : 0L;
      case "getDayTime": return scenarioLevel.has("day_time") ? scenarioLevel.get("day_time").getAsLong() : 0L;
      case "isRaining": return scenarioLevel.has("raining") && scenarioLevel.get("raining").getAsBoolean();
      case "isThundering": return scenarioLevel.has("thundering") && scenarioLevel.get("thundering").getAsBoolean();
      case "canSeeSky": return scenarioLevel.has("can_see_sky") && scenarioLevel.get("can_see_sky").getAsBoolean();
      case "dimension": return net.minecraft.world.level.Level.OVERWORLD;
      case "getBlockState": {
        BlockPos p = (BlockPos) a[0];
        if (scenarioLevel.has("blocks")) {
          for (JsonElement e : scenarioLevel.getAsJsonArray("blocks")) {
            JsonObject o = e.getAsJsonObject();
            JsonArray pos = o.getAsJsonArray("pos");
            if (pos.get(0).getAsInt() == p.getX() && pos.get(1).getAsInt() == p.getY() && pos.get(2).getAsInt() == p.getZ()) return blockState(o.getAsJsonObject("state"));
          }
        }
        return net.minecraft.world.level.block.Blocks.AIR.defaultBlockState();
      }
      case "getBiome": {
        String biome = scenarioLevel.has("biome") ? scenarioLevel.get("biome").getAsString() : "minecraft:plains";
        return access.registryOrThrow(Registries.BIOME).getHolderOrThrow(ResourceKey.create(Registries.BIOME, ResourceLocation.parse(biome)));
      }
      default: throw new IllegalStateException("unhooked " + name);
    }
  }


  // ---------------------------------------------------------------- scoreboard
  static Object scorePlayers;
  static final List<net.minecraft.network.protocol.Packet<?>> scoreBroadcasts = new ArrayList<>();

  static net.minecraft.server.ServerScoreboard makeScoreboard() throws Throwable {
    Loader loader = new Loader(loot_oracle.class.getClassLoader());
    if (scorePlayers == null) {
      ClassWriter pw = new ClassWriter(ClassWriter.COMPUTE_MAXS);
      pw.visit(Opcodes.V21, Opcodes.ACC_PUBLIC, "oracle/ScorePlayers", null, "net/minecraft/server/players/PlayerList", null);
      hookMethod(pw, "broadcastAll", "(Lnet/minecraft/network/protocol/Packet;)V");
      hookMethod(pw, "getPlayers", "()Ljava/util/List;");
      pw.visitEnd();
      scorePlayers = unsafe.allocateInstance(loader.define("oracle.ScorePlayers", pw.toByteArray()));
    }
    ClassWriter cw = new ClassWriter(ClassWriter.COMPUTE_MAXS);
    cw.visit(Opcodes.V21, Opcodes.ACC_PUBLIC, "oracle/ScoreServer", null, "net/minecraft/server/MinecraftServer", null);
    hookMethod(cw, "getPlayerList", "()Lnet/minecraft/server/players/PlayerList;");
    cw.visitEnd();
    Class<?> c = new Loader(loot_oracle.class.getClassLoader()).define("oracle.ScoreServer", cw.toByteArray());
    Object server = unsafe.allocateInstance(c);
    return new net.minecraft.server.ServerScoreboard((net.minecraft.server.MinecraftServer) server);
  }

  static String frame(net.minecraft.network.protocol.Packet<?> packet) {
    @SuppressWarnings("unchecked")
    net.minecraft.network.ProtocolInfo<net.minecraft.network.protocol.game.ClientGamePacketListener> info =
        net.minecraft.network.protocol.game.GameProtocols.CLIENTBOUND_TEMPLATE.bind(net.minecraft.network.RegistryFriendlyByteBuf.decorator(access));
    ByteBuf buffer = Unpooled.buffer();
    @SuppressWarnings({"unchecked", "rawtypes"})
    net.minecraft.network.codec.StreamCodec<ByteBuf, net.minecraft.network.protocol.Packet<? super net.minecraft.network.protocol.game.ClientGamePacketListener>> codec = (net.minecraft.network.codec.StreamCodec) info.codec();
    codec.encode(buffer, (net.minecraft.network.protocol.Packet) packet);
    byte[] bytes = new byte[buffer.readableBytes()];
    buffer.readBytes(bytes);
    return hex(bytes);
  }

  static ScoreHolder scoreHolder(JsonObject step) {
    String name = step.get("holder").getAsString();
    net.minecraft.network.chat.Component display = step.has("holder_display") ? parseComponent(step.get("holder_display")) : null;
    return new ScoreHolder() {
      public String getScoreboardName() { return name; }
      public net.minecraft.network.chat.Component getDisplayName() { return display; }
    };
  }

  static net.minecraft.network.chat.Component parseComponent(JsonElement e) {
    return net.minecraft.network.chat.ComponentSerialization.CODEC.parse(jsonOps, e).getOrThrow(m -> new IllegalArgumentException(m));
  }

  static net.minecraft.network.chat.numbers.NumberFormat parseFormat(JsonElement e) {
    if (e == null || e.isJsonNull()) return null;
    return net.minecraft.network.chat.numbers.NumberFormatTypes.CODEC.parse(jsonOps, e).getOrThrow(m -> new IllegalArgumentException(m));
  }

  static String gson(net.minecraft.network.chat.Component c) {
    return net.minecraft.network.chat.Component.Serializer.toJson(c, access);
  }

  static String formatHex(net.minecraft.network.chat.numbers.NumberFormat f) throws IOException {
    if (f == null) return null;
    return tagHex(net.minecraft.network.chat.numbers.NumberFormatTypes.CODEC.encodeStart(nbtOps, f).getOrThrow(m -> new IllegalStateException(m)));
  }

  static JsonObject dumpScoreboard(net.minecraft.server.ServerScoreboard sb) throws Exception {
    JsonObject d = new JsonObject();
    JsonArray objectives = new JsonArray();
    for (Objective o : sb.getObjectives()) {
      JsonObject j = new JsonObject();
      j.addProperty("name", o.getName());
      j.addProperty("criteria", o.getCriteria().getName());
      j.addProperty("display", gson(o.getDisplayName()));
      j.addProperty("render", o.getRenderType().getId());
      j.addProperty("auto", o.displayAutoUpdate());
      j.addProperty("format", formatHex(o.numberFormat()));
      objectives.add(j);
    }
    d.add("objectives", objectives);
    JsonObject slots = new JsonObject();
    for (net.minecraft.world.scores.DisplaySlot slot : net.minecraft.world.scores.DisplaySlot.values()) {
      Objective o = sb.getDisplayObjective(slot);
      if (o != null) slots.addProperty(slot.getSerializedName(), o.getName());
    }
    d.add("slots", slots);
    JsonArray scores = new JsonArray();
    for (Objective o : sb.getObjectives()) {
      for (net.minecraft.world.scores.PlayerScoreEntry e : sb.listPlayerScores(o)) {
        JsonObject j = new JsonObject();
        j.addProperty("objective", o.getName());
        j.addProperty("owner", e.owner());
        j.addProperty("value", e.value());
        j.addProperty("display", e.display() == null ? null : gson(e.display()));
        j.addProperty("format", formatHex(e.numberFormatOverride()));
        j.addProperty("locked", sb.getPlayerScoreInfo(ScoreHolder.forNameOnly(e.owner()), o).isLocked());
        scores.add(j);
      }
    }
    d.add("scores", scores);
    JsonArray tracked = new JsonArray();
    for (ScoreHolder h : sb.getTrackedPlayers()) tracked.add(h.getScoreboardName());
    d.add("tracked", tracked);
    JsonArray teams = new JsonArray();
    for (net.minecraft.world.scores.PlayerTeam t : sb.getPlayerTeams()) {
      JsonObject j = new JsonObject();
      j.addProperty("name", t.getName());
      j.addProperty("display", gson(t.getDisplayName()));
      j.addProperty("prefix", gson(t.getPlayerPrefix()));
      j.addProperty("suffix", gson(t.getPlayerSuffix()));
      j.addProperty("color", t.getColor().getName());
      j.addProperty("friendly", t.isAllowFriendlyFire());
      j.addProperty("invisibles", t.canSeeFriendlyInvisibles());
      j.addProperty("nametag", t.getNameTagVisibility().name);
      j.addProperty("death", t.getDeathMessageVisibility().name);
      j.addProperty("collision", t.getCollisionRule().name);
      JsonArray players = new JsonArray();
      for (String p : t.getPlayers()) players.add(p);
      j.add("players", players);
      teams.add(j);
    }
    d.add("teams", teams);
    return d;
  }

  static JsonObject scoreboardStep(net.minecraft.server.ServerScoreboard sb, JsonObject step, java.util.Map<String, Objective> unused) throws Throwable {
    JsonObject r = new JsonObject();
    String op = step.get("op").getAsString();
    switch (op) {
      case "objective_add": {
        ObjectiveCriteria criteria = ObjectiveCriteria.byName(step.get("criteria").getAsString()).orElseThrow();
        sb.addObjective(step.get("name").getAsString(), criteria, parseComponent(step.get("display")),
            ObjectiveCriteria.RenderType.byId(step.get("render").getAsString()), step.get("auto").getAsBoolean(), parseFormat(step.get("format")));
        break;
      }
      case "objective_remove": sb.removeObjective(sb.getObjective(step.get("name").getAsString())); break;
      case "objective_set": {
        Objective o = sb.getObjective(step.get("name").getAsString());
        if (step.has("display")) o.setDisplayName(parseComponent(step.get("display")));
        if (step.has("render")) o.setRenderType(ObjectiveCriteria.RenderType.byId(step.get("render").getAsString()));
        if (step.has("auto")) o.setDisplayAutoUpdate(step.get("auto").getAsBoolean());
        if (step.has("format")) o.setNumberFormat(parseFormat(step.get("format")));
        break;
      }
      case "display": {
        net.minecraft.world.scores.DisplaySlot slot = net.minecraft.world.scores.DisplaySlot.CODEC.byName(step.get("slot").getAsString());
        Objective o = step.get("objective").isJsonNull() ? null : sb.getObjective(step.get("objective").getAsString());
        sb.setDisplayObjective(slot, o);
        break;
      }
      case "score_set": case "score_add": case "score_reset": case "score_lock": case "score_unlock": case "score_display": case "score_format": case "score_get": {
        Objective o = sb.getObjective(step.get("objective").getAsString());
        net.minecraft.world.scores.ScoreAccess access2 = sb.getOrCreatePlayerScore(scoreHolder(step), o, step.has("force") && step.get("force").getAsBoolean());
        switch (op) {
          case "score_set": access2.set(step.get("value").getAsInt()); break;
          case "score_add": r.addProperty("ret", access2.add(step.get("value").getAsInt())); break;
          case "score_reset": access2.reset(); break;
          case "score_lock": access2.lock(); break;
          case "score_unlock": access2.unlock(); break;
          case "score_display": access2.display(step.get("display").isJsonNull() ? null : parseComponent(step.get("display"))); break;
          case "score_format": access2.numberFormatOverride(parseFormat(step.get("format"))); break;
          default: r.addProperty("ret", access2.get());
        }
        break;
      }
      case "score_remove": sb.resetSinglePlayerScore(scoreHolder(step), sb.getObjective(step.get("objective").getAsString())); break;
      case "score_remove_all": sb.resetAllPlayerScores(scoreHolder(step)); break;
      case "team_add": sb.addPlayerTeam(step.get("name").getAsString()); break;
      case "team_remove": sb.removePlayerTeam(sb.getPlayerTeam(step.get("name").getAsString())); break;
      case "team_set": {
        net.minecraft.world.scores.PlayerTeam t = sb.getPlayerTeam(step.get("name").getAsString());
        JsonElement v = step.get("value");
        switch (step.get("field").getAsString()) {
          case "display": t.setDisplayName(parseComponent(v)); break;
          case "prefix": t.setPlayerPrefix(v.isJsonNull() ? null : parseComponent(v)); break;
          case "suffix": t.setPlayerSuffix(v.isJsonNull() ? null : parseComponent(v)); break;
          case "color": t.setColor(net.minecraft.ChatFormatting.getByName(v.getAsString())); break;
          case "friendly": t.setAllowFriendlyFire(v.getAsBoolean()); break;
          case "invisibles": t.setSeeFriendlyInvisibles(v.getAsBoolean()); break;
          case "nametag": t.setNameTagVisibility(net.minecraft.world.scores.Team.Visibility.byName(v.getAsString())); break;
          case "death": t.setDeathMessageVisibility(net.minecraft.world.scores.Team.Visibility.byName(v.getAsString())); break;
          case "collision": t.setCollisionRule(net.minecraft.world.scores.Team.CollisionRule.byName(v.getAsString())); break;
          case "options": t.unpackOptions(v.getAsInt()); break;
          default: throw new IllegalArgumentException(step.get("field").getAsString());
        }
        break;
      }
      case "team_join": r.addProperty("ret", sb.addPlayerToTeam(step.get("player").getAsString(), sb.getPlayerTeam(step.get("team").getAsString()))); break;
      case "team_leave": r.addProperty("ret", sb.removePlayerFromTeam(step.get("player").getAsString())); break;
      case "team_leave_from": sb.removePlayerFromTeam(step.get("player").getAsString(), sb.getPlayerTeam(step.get("team").getAsString())); break;
      case "track": sb.startTrackingObjective(sb.getObjective(step.get("objective").getAsString())); break;
      case "untrack": sb.stopTrackingObjective(sb.getObjective(step.get("objective").getAsString())); break;
      case "start_packets": case "stop_packets": {
        Objective o = sb.getObjective(step.get("objective").getAsString());
        JsonArray frames = new JsonArray();
        for (net.minecraft.network.protocol.Packet<?> p : (op.equals("start_packets") ? sb.getStartTrackingPackets(o) : sb.getStopTrackingPackets(o))) frames.add(frame(p));
        r.add("packets", frames);
        break;
      }
      case "save": r.addProperty("data", tagHex(new net.minecraft.world.scores.ScoreboardSaveData(sb).save(new CompoundTag(), access))); break;
      case "dump": r.add("dump", dumpScoreboard(sb)); break;
      default: throw new IllegalArgumentException(op);
    }
    return r;
  }

  // ---------------------------------------------------------------- scenario object factories
  static ResourceLocation rl(String s) { return ResourceLocation.parse(s); }

  @SuppressWarnings({"unchecked", "rawtypes"})
  static BlockState blockState(JsonObject o) {
    Block b = BuiltInRegistries.BLOCK.get(rl(o.get("name").getAsString()));
    BlockState s = b.defaultBlockState();
    if (o.has("properties")) {
      for (Map.Entry<String, JsonElement> e : o.getAsJsonObject("properties").entrySet()) {
        Property p = b.getStateDefinition().getProperty(e.getKey());
        if (p == null) throw new IllegalArgumentException("no property " + e.getKey() + " on " + o.get("name"));
        Optional v = p.getValue(e.getValue().getAsString());
        if (v.isEmpty()) throw new IllegalArgumentException("bad value " + e.getValue());
        s = (BlockState) s.setValue(p, (Comparable) v.get());
      }
    }
    return s;
  }

  static ItemStack stack(JsonObject o) {
    if (o.get("id").getAsString().equals("minecraft:air")) return ItemStack.EMPTY;
    DataResult<ItemStack> r = ItemStack.CODEC.parse(jsonOps, o);
    return r.getOrThrow(msg -> new IllegalArgumentException("stack: " + msg + " in " + o));
  }

  static Entity entity(JsonObject o) {
    EntityType<?> t = BuiltInRegistries.ENTITY_TYPE.get(rl(o.get("type").getAsString()));
    Entity e = t.create(level);
    if (e == null) throw new IllegalStateException("cannot create " + o.get("type"));
    if (o.has("on_fire") && o.get("on_fire").getAsBoolean()) e.setRemainingFireTicks(100);
    if (o.has("mainhand") && e instanceof LivingEntity le) le.setItemSlot(EquipmentSlot.MAINHAND, stack(o.getAsJsonObject("mainhand")));
    if (o.has("size") && e instanceof net.minecraft.world.entity.monster.Slime s) s.setSize(o.get("size").getAsInt(), false);
    if (o.has("open_water") && e instanceof net.minecraft.world.entity.projectile.FishingHook h) {
      try {
        Field f = net.minecraft.world.entity.projectile.FishingHook.class.getDeclaredField("openWater");
        f.setAccessible(true);
        f.setBoolean(h, o.get("open_water").getAsBoolean());
      } catch (ReflectiveOperationException ex) { throw new IllegalStateException(ex); }
    }
    if (o.has("variant") && e instanceof net.minecraft.world.entity.animal.frog.Frog fr) {
      fr.setVariant(access.registryOrThrow(Registries.FROG_VARIANT).getHolderOrThrow(ResourceKey.create(Registries.FROG_VARIANT, rl(o.get("variant").getAsString()))));
    }
    return e;
  }

  static Entity presenceOnly(Class<?> c) throws Exception { return (Entity) unsafe.allocateInstance(c); }

  static String tagHex(Tag tag) throws IOException {
    ByteArrayOutputStream bytes = new ByteArrayOutputStream();
    NbtIo.writeAnyTag(tag, new DataOutputStream(bytes));
    return hex(bytes.toByteArray());
  }

  /** SNBT cannot spell an empty key, so the generator writes WRAP and it becomes "" here. */
  static Tag renameWrap(Tag tag) {
    if (tag instanceof CompoundTag compound) {
      CompoundTag out = new CompoundTag();
      for (String key : compound.getAllKeys()) out.put(key.equals("WRAP") ? "" : key, renameWrap(compound.get(key)));
      return out;
    }
    if (tag instanceof ListTag list) {
      ListTag out = new ListTag();
      for (Tag element : list) out.add(renameWrap(element));
      return out;
    }
    return tag;
  }

  static Tag[] wrongTags() {
    ListTag ints = new ListTag(); ints.add(IntTag.valueOf(1));
    ListTag compounds = new ListTag(); compounds.add(new CompoundTag());
    CompoundTag wrapper = new CompoundTag(); wrapper.put("", StringTag.valueOf("x"));
    ListTag wrapped = new ListTag(); wrapped.add(wrapper);
    return new Tag[] {
      ByteTag.valueOf((byte) 0), ByteTag.valueOf((byte) 1), ShortTag.valueOf((short) 2), IntTag.valueOf(256), LongTag.valueOf(4L),
      FloatTag.valueOf(2.5f), FloatTag.valueOf(256.0f), DoubleTag.valueOf(0.4), StringTag.valueOf("x"), StringTag.valueOf(""),
      new ListTag(), ints, compounds, wrapped, new CompoundTag(), wrapper,
      new ByteArrayTag(new byte[] {1}), new IntArrayTag(new int[] {1}), new LongArrayTag(new long[] {1}),
    };
  }

  /** One-step mutants of an NBT tree: removals, wrong-typed replacements, foreign `type` members. */
  static List<Tag> nbtMutants(Tag root) {
    List<Tag> out = new ArrayList<>();
    collectMutants(root, root, new ArrayList<>(), out);
    return out;
  }

  static void collectMutants(Tag root, Tag node, List<Object> path, List<Tag> out) {
    Tag[] wrong = wrongTags();
    if (!path.isEmpty()) {
      Tag removed = editAt(root, path, null);
      if (removed != null) out.add(removed);
      for (Tag w : wrong) {
        Tag replaced = editAt(root, path, w);
        if (replaced != null) out.add(replaced);
      }
    } else {
      for (Tag w : wrong) out.add(w);
    }
    if (node instanceof CompoundTag compound) {
      if (!compound.contains("type")) {
        for (Tag kind : new Tag[] {StringTag.valueOf("text"), StringTag.valueOf("translatable"), StringTag.valueOf("keybind"),
            StringTag.valueOf("score"), StringTag.valueOf("selector"), StringTag.valueOf("nbt"), StringTag.valueOf("bogus"), ByteTag.valueOf((byte) 1)}) {
          List<Object> typePath = new ArrayList<>(path);
          typePath.add("type");
          Tag added = editAt(root, typePath, kind);
          if (added != null) out.add(added);
        }
      }
      List<String> keys = new ArrayList<>(compound.getAllKeys());
      Collections.sort(keys);
      for (String key : keys) {
        List<Object> child = new ArrayList<>(path);
        child.add(key);
        collectMutants(root, compound.get(key), child, out);
      }
    } else if (node instanceof ListTag list) {
      for (int i = 0; i < list.size(); i++) {
        List<Object> child = new ArrayList<>(path);
        child.add(i);
        collectMutants(root, list.get(i), child, out);
      }
    }
  }

  /** Copy of root with the node at path replaced by value (removed when value is null); null when illegal. */
  static Tag editAt(Tag root, List<Object> path, Tag value) {
    try {
      Tag copy = root.copy();
      Tag parent = copy;
      for (int i = 0; i < path.size() - 1; i++) {
        Object step = path.get(i);
        parent = step instanceof String key ? ((CompoundTag) parent).get(key) : ((ListTag) parent).get((Integer) step);
      }
      Object last = path.get(path.size() - 1);
      if (last instanceof String key) {
        CompoundTag compound = (CompoundTag) parent;
        if (value == null) compound.remove(key); else compound.put(key, value.copy());
      } else {
        ListTag list = (ListTag) parent;
        if (value == null) list.remove((int) (Integer) last); else list.set((Integer) last, value.copy());
      }
      return copy;
    } catch (RuntimeException e) {
      return null;
    }
  }

  static byte[] unhex(String text) {
    byte[] out = new byte[text.length() / 2];
    for (int i = 0; i < out.length; i++) out[i] = (byte) Integer.parseInt(text.substring(2 * i, 2 * i + 2), 16);
    return out;
  }

  static String hex(byte[] b) {
    StringBuilder sb = new StringBuilder();
    for (byte x : b) sb.append(String.format("%02x", x));
    return sb.toString();
  }

  static String wire(ItemStack s) {
    RegistryFriendlyByteBuf buf = new RegistryFriendlyByteBuf(Unpooled.buffer(), access);
    ItemStack.OPTIONAL_STREAM_CODEC.encode(buf, s);
    byte[] b = new byte[buf.readableBytes()];
    buf.readBytes(b);
    return hex(b);
  }

  // ---------------------------------------------------------------- run one scenario
  @SuppressWarnings({"unchecked", "rawtypes"})
  static JsonObject run(JsonObject sc) throws Throwable {
    scenarioLevel = sc.has("level") ? sc.getAsJsonObject("level") : new JsonObject();
    JsonObject p = sc.getAsJsonObject("params");
    ResourceKey<LootTable> key = ResourceKey.create(Registries.LOOT_TABLE, rl(sc.get("table").getAsString()));
    LootTable table = sc.has("table_json")
      ? LootTable.DIRECT_CODEC.parse(jsonOps, sc.get("table_json")).getOrThrow(msg -> new IllegalArgumentException("table: " + msg))
      : res.fullRegistries().getLootTable(key);
    LootParams.Builder b = new LootParams.Builder(level);
    if (p.has("origin")) { JsonArray o = p.getAsJsonArray("origin"); b.withParameter(LootContextParams.ORIGIN, new Vec3(o.get(0).getAsDouble(), o.get(1).getAsDouble(), o.get(2).getAsDouble())); }
    if (p.has("block_state")) b.withParameter(LootContextParams.BLOCK_STATE, blockState(p.getAsJsonObject("block_state")));
    if (p.has("tool")) b.withParameter(LootContextParams.TOOL, stack(p.getAsJsonObject("tool")));
    if (p.has("block_entity")) { scenarioBlockEntity = p.getAsJsonObject("block_entity"); b.withParameter(LootContextParams.BLOCK_ENTITY, (net.minecraft.world.level.block.entity.BlockEntity) makeBlockEntity()); }
    if (p.has("explosion_radius")) b.withParameter(LootContextParams.EXPLOSION_RADIUS, p.get("explosion_radius").getAsFloat());
    if (p.has("this_entity")) b.withParameter(LootContextParams.THIS_ENTITY, entity(p.getAsJsonObject("this_entity")));
    if (p.has("attacking_entity")) b.withParameter(LootContextParams.ATTACKING_ENTITY, entity(p.getAsJsonObject("attacking_entity")));
    if (p.has("direct_attacking_entity")) b.withParameter(LootContextParams.DIRECT_ATTACKING_ENTITY, entity(p.getAsJsonObject("direct_attacking_entity")));
    if (p.has("last_damage_player") && p.get("last_damage_player").getAsBoolean()) b.withParameter(LootContextParams.LAST_DAMAGE_PLAYER, (net.minecraft.world.entity.player.Player) presenceOnly(net.minecraft.server.level.ServerPlayer.class));
    if (p.has("damage_source")) {
      JsonObject d = p.getAsJsonObject("damage_source");
      Holder<DamageType> type = access.registryOrThrow(Registries.DAMAGE_TYPE).getHolderOrThrow(ResourceKey.create(Registries.DAMAGE_TYPE, rl(d.get("type").getAsString())));
      Entity direct = d.has("direct_entity") ? entity(d.getAsJsonObject("direct_entity")) : null;
      Entity source = d.has("source_entity") ? entity(d.getAsJsonObject("source_entity")) : null;
      b.withParameter(LootContextParams.DAMAGE_SOURCE, new DamageSource(type, direct, source));
    }
    if (sc.has("luck")) b.withLuck(sc.get("luck").getAsFloat());
    LootContextParamSet set = table.getParamSet();
    LootParams params = b.create(set);

    long seed = sc.get("seed").getAsLong();
    if (sc.has("fill")) {
      net.minecraft.world.SimpleContainer container = new net.minecraft.world.SimpleContainer(sc.get("fill").getAsInt());
      if (sc.has("fill_existing")) {
        for (Map.Entry<String, JsonElement> e : sc.getAsJsonObject("fill_existing").entrySet()) container.setItem(Integer.parseInt(e.getKey()), stack(e.getValue().getAsJsonObject()));
      }
      table.fill(container, params, seed);
      JsonObject r = new JsonObject();
      r.addProperty("name", sc.get("name").getAsString());
      JsonArray slots = new JsonArray();
      for (int i = 0; i < container.getContainerSize(); i++) slots.add(wire(container.getItem(i)));
      r.add("stacks", slots);
      r.addProperty("rng_next", 0);
      return r;
    }
    if (sc.has("sequence")) {
      JsonObject q = sc.getAsJsonObject("sequence");
      net.minecraft.world.RandomSequences sequences = new net.minecraft.world.RandomSequences(q.get("world_seed").getAsLong());
      if (q.has("salt")) sequences.setSeedDefaults(q.get("salt").getAsInt(), q.get("include_world_seed").getAsBoolean(), q.get("include_sequence_id").getAsBoolean());
      ResourceLocation sequenceId = rl(q.get("id").getAsString());
      Constructor<LootContext> sctor = LootContext.class.getDeclaredConstructor(LootParams.class, RandomSource.class, HolderGetter.Provider.class);
      sctor.setAccessible(true);
      JsonArray perRoll = new JsonArray();
      for (int i = 0; i < q.get("rolls").getAsInt(); i++) {
        LootContext sctx = sctor.newInstance(params, sequences.get(sequenceId), lookup);
        List<ItemStack> produced = new ArrayList<>();
        table.getRandomItems(sctx, produced::add);
        StringBuilder sb = new StringBuilder();
        for (ItemStack st : produced) { if (sb.length() > 0) sb.append(','); sb.append(wire(st)); }
        perRoll.add(sb.toString());
      }
      net.minecraft.nbt.CompoundTag saved = sequences.save(new net.minecraft.nbt.CompoundTag(), access);
      long[] state = saved.getCompound("sequences").getCompound(sequenceId.toString()).getLongArray("source");
      JsonObject r = new JsonObject();
      r.addProperty("name", sc.get("name").getAsString());
      r.add("stacks", perRoll);
      r.addProperty("rng_next", state[0] + "," + state[1]);
      return r;
    }
    RandomSource rnd = RandomSource.create(seed);
    Constructor<LootContext> ctor = LootContext.class.getDeclaredConstructor(LootParams.class, RandomSource.class, HolderGetter.Provider.class);
    ctor.setAccessible(true);
    LootContext ctx = ctor.newInstance(params, rnd, lookup);
    List<ItemStack> out = new ArrayList<>();
    table.getRandomItems(ctx, out::add);

    JsonObject r = new JsonObject();
    r.addProperty("name", sc.get("name").getAsString());
    JsonArray stacks = new JsonArray();
    for (ItemStack s : out) stacks.add(wire(s));
    r.add("stacks", stacks);
    r.addProperty("rng_next", rnd.nextInt());
    return r;
  }


  // ---------------------------------------------------------------- registry facts export
  static String id(Holder<?> h) { return h.unwrapKey().orElseThrow().location().toString(); }

  static JsonArray ids(Iterable<? extends Holder<?>> hs) {
    JsonArray a = new JsonArray();
    for (Holder<?> h : hs) a.add(id(h));
    return a;
  }

  @SuppressWarnings({"unchecked", "rawtypes"})
  static JsonObject tagsOf(net.minecraft.core.Registry<?> reg) {
    JsonObject o = new JsonObject();
    java.util.TreeMap<String, JsonArray> sorted = new java.util.TreeMap<>();
    reg.getTags().forEach(pair -> {
      HolderSet.Named named = (HolderSet.Named) pair.getSecond();
      JsonArray arr = new JsonArray();
      named.stream().forEach(h -> arr.add(id((Holder<?>) h)));
      sorted.put(((TagKey<?>) pair.getFirst()).location().toString(), arr);
    });
    sorted.forEach(o::add);
    return o;
  }

  @SuppressWarnings({"unchecked", "rawtypes"})
  static JsonArray registryOrder(net.minecraft.core.Registry reg) {
    JsonArray a = new JsonArray();
    for (int i = 0; i < reg.size(); i++) {
      Object v = reg.byId(i);
      a.add(reg.getKey(v).toString());
    }
    return a;
  }

  static JsonObject cost(Enchantment.Cost c) {
    JsonObject o = new JsonObject();
    o.addProperty("base", c.base());
    o.addProperty("per_level_above_first", c.perLevelAboveFirst());
    return o;
  }

  static JsonObject exportFacts() {
    JsonObject root = new JsonObject();
    root.addProperty("format", "cinderstone-loot-facts-v1");
    root.addProperty("minecraft", "1.21");
    net.minecraft.core.Registry<Enchantment> er = access.registryOrThrow(Registries.ENCHANTMENT);
    JsonArray enchs = new JsonArray();
    for (int i = 0; i < er.size(); i++) {
      Enchantment e = er.byId(i);
      Holder<Enchantment> h = er.getHolder(i).orElseThrow();
      JsonObject o = new JsonObject();
      o.addProperty("id", id(h));
      o.addProperty("weight", e.getWeight());
      o.addProperty("max_level", e.getMaxLevel());
      o.add("min_cost", cost(e.definition().minCost()));
      o.add("max_cost", cost(e.definition().maxCost()));
      o.addProperty("anvil_cost", e.getAnvilCost());
      JsonArray slots = new JsonArray();
      for (EquipmentSlotGroup g : e.definition().slots()) slots.add(g.getSerializedName());
      o.add("slots", slots);
      o.add("supported_items", ids(e.definition().supportedItems()));
      if (e.definition().primaryItems().isPresent()) o.add("primary_items", ids(e.definition().primaryItems().get())); else o.add("primary_items", JsonNull.INSTANCE);
      o.add("exclusive_set", ids(e.exclusiveSet()));
      enchs.add(o);
    }
    root.add("enchantments", enchs);
    JsonObject value = new JsonObject();
    JsonArray disabled = new JsonArray();
    for (Item item : BuiltInRegistries.ITEM) {
      String name = BuiltInRegistries.ITEM.getKey(item).toString();
      int v = item.getEnchantmentValue();
      if (v != 0) value.addProperty(name, v);
      if (!item.isEnabled(FeatureFlags.VANILLA_SET)) disabled.add(name);
    }
    JsonArray instant = new JsonArray();
    for (net.minecraft.world.effect.MobEffect effect : BuiltInRegistries.MOB_EFFECT) if (effect.isInstantenous()) instant.add(BuiltInRegistries.MOB_EFFECT.getKey(effect).toString());
    root.add("instant_mob_effects", instant);
    root.add("item_enchantment_value", value);
    root.add("disabled_items", disabled);
    JsonObject regs = new JsonObject();
    regs.add("item", registryOrder(BuiltInRegistries.ITEM));
    regs.add("block", registryOrder(BuiltInRegistries.BLOCK));
    regs.add("potion", registryOrder(BuiltInRegistries.POTION));
    regs.add("mob_effect", registryOrder(BuiltInRegistries.MOB_EFFECT));
    regs.add("entity_type", registryOrder(BuiltInRegistries.ENTITY_TYPE));
    regs.add("enchantment", registryOrder(er));
    regs.add("instrument", registryOrder(access.registryOrThrow(Registries.INSTRUMENT)));
    regs.add("trim_material", registryOrder(access.registryOrThrow(Registries.TRIM_MATERIAL)));
    regs.add("trim_pattern", registryOrder(access.registryOrThrow(Registries.TRIM_PATTERN)));
    regs.add("damage_type", registryOrder(access.registryOrThrow(Registries.DAMAGE_TYPE)));
    regs.add("biome", registryOrder(access.registryOrThrow(Registries.BIOME)));
    regs.add("map_decoration_type", registryOrder(BuiltInRegistries.MAP_DECORATION_TYPE));
    root.add("registries", regs);
    JsonObject tags = new JsonObject();
    tags.add("item", tagsOf(BuiltInRegistries.ITEM));
    tags.add("block", tagsOf(BuiltInRegistries.BLOCK));
    tags.add("enchantment", tagsOf(er));
    tags.add("damage_type", tagsOf(access.registryOrThrow(Registries.DAMAGE_TYPE)));
    tags.add("entity_type", tagsOf(BuiltInRegistries.ENTITY_TYPE));
    tags.add("biome", tagsOf(access.registryOrThrow(Registries.BIOME)));
    tags.add("instrument", tagsOf(access.registryOrThrow(Registries.INSTRUMENT)));
    root.add("tags", tags);
    return root;
  }

  public static void main(String[] a) throws Throwable {
    boot();
    switch (a[0]) {
      case "run": {
        JsonArray in = JsonParser.parseString(Files.readString(Path.of(a[1]))).getAsJsonArray();
        JsonArray outArr = new JsonArray();
        int failed = 0;
        for (JsonElement e : in) {
          JsonObject sc = e.getAsJsonObject();
          try { outArr.add(run(sc)); }
          catch (Throwable t) {
            failed++;
            JsonObject r = new JsonObject();
            r.addProperty("name", sc.get("name").getAsString());
            r.addProperty("error", t.getClass().getSimpleName() + ": " + String.valueOf(t.getMessage()));
            outArr.add(r);
          }
        }
        Files.writeString(Path.of(a[2]), GSON.toJson(outArr));
        System.out.println("scenarios=" + in.size() + " failed=" + failed);
        break;
      }
      case "parse": {
        // Codec accept/reject parity: parse each table JSON with LootTable.DIRECT_CODEC.
        JsonArray in = JsonParser.parseString(Files.readString(Path.of(a[1]))).getAsJsonArray();
        JsonArray outArr = new JsonArray();
        for (JsonElement e : in) {
          JsonObject r = new JsonObject();
          try {
            DataResult<LootTable> result = LootTable.DIRECT_CODEC.parse(jsonOps, e);
            r.addProperty("ok", result.result().isPresent());
            if (result.error().isPresent()) r.addProperty("error", result.error().get().message());
          } catch (Throwable t) {
            r.addProperty("ok", false);
            r.addProperty("error", t.getClass().getSimpleName() + ": " + t.getMessage());
          }
          outArr.add(r);
        }
        Files.writeString(Path.of(a[2]), GSON.toJson(outArr));
        System.out.println("parsed=" + in.size());
        break;
      }
      case "component": {
        // ComponentSerialization.CODEC: accept/reject, the NBT network form and the JSON re-encoding.
        JsonArray in = JsonParser.parseString(Files.readString(Path.of(a[1]))).getAsJsonArray();
        JsonArray outArr = new JsonArray();
        for (JsonElement e : in) {
          JsonObject r = new JsonObject();
          try {
            DataResult<net.minecraft.network.chat.Component> result = net.minecraft.network.chat.ComponentSerialization.CODEC.parse(jsonOps, e);
            r.addProperty("ok", result.result().isPresent());
            if (result.error().isPresent()) r.addProperty("error", result.error().get().message());
            if (result.result().isPresent()) {
              net.minecraft.network.chat.Component c = result.result().get();
              Tag nbt = net.minecraft.network.chat.ComponentSerialization.CODEC.encodeStart(nbtOps, c).getOrThrow(m -> new IllegalStateException(m));
              ByteArrayOutputStream bytes = new ByteArrayOutputStream();
              NbtIo.writeAnyTag(nbt, new DataOutputStream(bytes));
              r.addProperty("nbt", hex(bytes.toByteArray()));
              r.addProperty("snbt", nbt.toString());
              JsonElement json = net.minecraft.network.chat.ComponentSerialization.CODEC.encodeStart(jsonOps, c).getOrThrow(m -> new IllegalStateException(m));
              r.addProperty("json", net.minecraft.util.GsonHelper.toStableString(json));
              r.addProperty("string", c.getString());
              r.addProperty("json_gson", net.minecraft.network.chat.Component.Serializer.toJson(c, access));
            }
          } catch (Throwable t) {
            r.addProperty("ok", false);
            r.addProperty("error", t.getClass().getSimpleName() + ": " + t.getMessage());
          }
          outArr.add(r);
        }
        Files.writeString(Path.of(a[2]), GSON.toJson(outArr));
        System.out.println("components=" + in.size());
        break;
      }
      case "component-nbt": {
        // ComponentSerialization.CODEC over NbtOps. Input: JSON array of SNBT strings. With a
        // fourth argument "mutate" every input is also mutated one step at a time (each member or
        // element removed, each node replaced by a wrong-typed tag, foreign `type` members added)
        // and the mutants are decided too. Output rows: in (hex of the tag), ok, out (hex of the
        // codec's NbtOps re-encoding of the decoded component).
        JsonArray in = JsonParser.parseString(Files.readString(Path.of(a[1]))).getAsJsonArray();
        boolean mutate = a.length > 3 && a[3].equals("mutate");
        LinkedHashMap<String, Tag> inputs = new LinkedHashMap<>();
        for (JsonElement e : in) {
          Tag base;
          try {
            base = new TagParser(new com.mojang.brigadier.StringReader(e.getAsString())).readValue();
          } catch (com.mojang.brigadier.exceptions.CommandSyntaxException bad) {
            System.err.println("skipped SNBT " + e.getAsString() + ": " + bad.getMessage());
            continue;
          }
          base = renameWrap(base);
          inputs.putIfAbsent(tagHex(base), base);
        }
        if (mutate) {
          for (Tag base : new ArrayList<>(inputs.values())) {
            for (Tag mutant : nbtMutants(base)) inputs.putIfAbsent(tagHex(mutant), mutant);
          }
        }
        JsonArray outArr = new JsonArray();
        for (Map.Entry<String, Tag> entry : inputs.entrySet()) {
          JsonObject r = new JsonObject();
          r.addProperty("in", entry.getKey());
          try {
            DataResult<net.minecraft.network.chat.Component> result = net.minecraft.network.chat.ComponentSerialization.CODEC.parse(nbtOps, entry.getValue());
            r.addProperty("ok", result.result().isPresent());
            if (result.error().isPresent()) r.addProperty("error", result.error().get().message());
            if (result.result().isPresent()) {
              Tag out = net.minecraft.network.chat.ComponentSerialization.CODEC.encodeStart(nbtOps, result.result().get()).getOrThrow(m -> new IllegalStateException(m));
              r.addProperty("out", tagHex(out));
            }
          } catch (Throwable t) {
            r.addProperty("ok", false);
            r.addProperty("error", t.getClass().getSimpleName() + ": " + t.getMessage());
          }
          outArr.add(r);
        }
        Files.writeString(Path.of(a[2]), GSON.toJson(outArr));
        System.out.println("component-nbt=" + inputs.size());
        break;
      }
      case "saved-data": {
        // The original DimensionDataStorage / SavedData / RandomSequences / MapIndex over a real
        // temporary directory. Input: JSON array of scenarios {world_seed, steps:[{op, ...}]};
        // output: per scenario, one result object per step. Ops: put_snbt, put_hex, delete, get,
        // compute, free_map_id, draw, reset, clear, set_dirty, save, read_tag.
        JsonArray in = JsonParser.parseString(Files.readString(Path.of(a[1]))).getAsJsonArray();
        JsonArray outArr = new JsonArray();
        for (JsonElement e : in) {
          JsonObject scenario = e.getAsJsonObject();
          long worldSeed = scenario.get("world_seed").getAsLong();
          Path dir = Files.createTempDirectory("saved-data-oracle");
          Map<String, byte[]> put = new HashMap<>();
          try {
            net.minecraft.world.level.storage.DimensionDataStorage storage =
                new net.minecraft.world.level.storage.DimensionDataStorage(dir.toFile(), net.minecraft.util.datafix.DataFixers.getDataFixer(), access);
            JsonArray results = new JsonArray();
            for (JsonElement stepElement : scenario.getAsJsonArray("steps")) {
              JsonObject step = stepElement.getAsJsonObject();
              JsonObject r = new JsonObject();
              try {
                String op = step.get("op").getAsString();
                switch (op) {
                  case "put_snbt": {
                    String name = step.get("name").getAsString();
                    CompoundTag root = new CompoundTag();
                    root.put("data", renameWrap(new TagParser(new com.mojang.brigadier.StringReader(step.get("snbt").getAsString())).readValue()));
                    if (!step.has("data_version")) { NbtUtils.addCurrentDataVersion(root); }
                    else if (!step.get("data_version").isJsonNull()) { root.putInt("DataVersion", step.get("data_version").getAsInt()); }
                    byte[] bytes;
                    ByteArrayOutputStream buffer = new ByteArrayOutputStream();
                    if (step.has("gzip") && !step.get("gzip").getAsBoolean()) {
                      NbtIo.write(root, new DataOutputStream(buffer));
                    } else {
                      NbtIo.writeCompressed(root, buffer);
                    }
                    bytes = buffer.toByteArray();
                    Files.write(dir.resolve(name + ".dat"), bytes);
                    put.put(name, bytes);
                    r.addProperty("hex", hex(bytes));
                    break;
                  }
                  case "put_hex": {
                    String name = step.get("name").getAsString();
                    byte[] bytes = unhex(step.get("hex").getAsString());
                    Files.write(dir.resolve(name + ".dat"), bytes);
                    put.put(name, bytes);
                    break;
                  }
                  case "delete":
                    Files.deleteIfExists(dir.resolve(step.get("name").getAsString() + ".dat"));
                    break;
                  case "get":
                  case "compute":
                  case "set_dirty": {
                    String name = step.get("name").getAsString();
                    net.minecraft.world.level.saveddata.SavedData data;
                    if (name.equals("random_sequences")) {
                      var factory = net.minecraft.world.RandomSequences.factory(worldSeed);
                      data = op.equals("get") ? storage.get(factory, name) : storage.computeIfAbsent(factory, name);
                    } else {
                      var factory = net.minecraft.world.level.saveddata.maps.MapIndex.factory();
                      data = op.equals("get") ? storage.get(factory, name) : storage.computeIfAbsent(factory, name);
                    }
                    r.addProperty("present", data != null);
                    if (data != null) {
                      if (op.equals("set_dirty")) data.setDirty();
                      r.addProperty("dirty", data.isDirty());
                      r.addProperty("data", tagHex(data.save(new CompoundTag(), access)));
                    }
                    break;
                  }
                  case "free_map_id": {
                    var index = storage.computeIfAbsent(net.minecraft.world.level.saveddata.maps.MapIndex.factory(), "idcounts");
                    r.addProperty("id", index.getFreeAuxValueForMap().id());
                    r.addProperty("dirty", index.isDirty());
                    break;
                  }
                  case "draw": {
                    var sequences = storage.computeIfAbsent(net.minecraft.world.RandomSequences.factory(worldSeed), "random_sequences");
                    RandomSource random = sequences.get(ResourceLocation.parse(step.get("id").getAsString()));
                    JsonArray draws = new JsonArray();
                    for (int i = 0; i < step.get("n").getAsInt(); i++) draws.add(random.nextLong());
                    r.add("draws", draws);
                    r.addProperty("dirty", sequences.isDirty());
                    break;
                  }
                  case "reset": {
                    var sequences = storage.computeIfAbsent(net.minecraft.world.RandomSequences.factory(worldSeed), "random_sequences");
                    ResourceLocation id = ResourceLocation.parse(step.get("id").getAsString());
                    if (step.has("salt")) sequences.reset(id, step.get("salt").getAsInt(), step.get("include_world_seed").getAsBoolean(), step.get("include_sequence_id").getAsBoolean());
                    else sequences.reset(id);
                    r.addProperty("dirty", sequences.isDirty());
                    break;
                  }
                  case "clear": {
                    var sequences = storage.computeIfAbsent(net.minecraft.world.RandomSequences.factory(worldSeed), "random_sequences");
                    r.addProperty("cleared", sequences.clear());
                    break;
                  }
                  case "defaults": {
                    var sequences = storage.computeIfAbsent(net.minecraft.world.RandomSequences.factory(worldSeed), "random_sequences");
                    sequences.setSeedDefaults(step.get("salt").getAsInt(), step.get("include_world_seed").getAsBoolean(), step.get("include_sequence_id").getAsBoolean());
                    break;
                  }
                  case "save": {
                    storage.save();
                    JsonArray files = new JsonArray();
                    List<Path> listed;
                    try (var stream = Files.list(dir)) { listed = stream.sorted().toList(); }
                    for (Path file : listed) {
                      byte[] bytes = Files.readAllBytes(file);
                      String name = file.getFileName().toString();
                      name = name.substring(0, name.length() - 4);
                      JsonObject f = new JsonObject();
                      f.addProperty("name", name);
                      f.addProperty("rewritten", !Arrays.equals(bytes, put.get(name)));
                      f.addProperty("gzip", bytes.length >= 2 && (bytes[0] & 255) == 0x1f && (bytes[1] & 255) == 0x8b);
                      try {
                        CompoundTag tag;
                        if (f.get("gzip").getAsBoolean()) tag = NbtIo.readCompressed(new ByteArrayInputStream(bytes), NbtAccounter.unlimitedHeap());
                        else tag = NbtIo.read(new DataInputStream(new ByteArrayInputStream(bytes)));
                        f.addProperty("nbt", tagHex(tag));
                      } catch (Exception bad) {
                        f.addProperty("unreadable", true);
                      }
                      files.add(f);
                    }
                    r.add("files", files);
                    break;
                  }
                  case "read_tag": {
                    String name = step.get("name").getAsString();
                    var type = name.equals("random_sequences") ? net.minecraft.util.datafix.DataFixTypes.SAVED_DATA_RANDOM_SEQUENCES : net.minecraft.util.datafix.DataFixTypes.SAVED_DATA_MAP_INDEX;
                    int version = step.has("version") ? step.get("version").getAsInt() : SharedConstants.getCurrentVersion().getDataVersion().getVersion();
                    r.addProperty("nbt", tagHex(storage.readTagFromDisk(name, type, version)));
                    break;
                  }
                  default: throw new IllegalArgumentException(op);
                }
              } catch (Throwable t) {
                r.addProperty("error", t.getClass().getSimpleName() + ": " + t.getMessage());
              }
              results.add(r);
            }
            outArr.add(results);
          } finally {
            try (var walk = Files.walk(dir)) { walk.sorted(Comparator.reverseOrder()).forEach(p -> p.toFile().delete()); }
          }
        }
        Files.writeString(Path.of(a[2]), GSON.toJson(outArr));
        System.out.println("saved-data=" + in.size());
        break;
      }
      case "saved-data-mutants": {
        // One-step mutants of saved-data payloads, loaded by the original factory through a real
        // DimensionDataStorage. Input: [{name, world_seed, bases:[snbt]}]. Output rows:
        // {name, world_seed, payload (hex of the `data` compound), present, data (hex of save())}.
        JsonArray in = JsonParser.parseString(Files.readString(Path.of(a[1]))).getAsJsonArray();
        JsonArray outArr = new JsonArray();
        Path dir = Files.createTempDirectory("saved-data-mutants");
        try {
          for (JsonElement e : in) {
            JsonObject spec = e.getAsJsonObject();
            String name = spec.get("name").getAsString();
            long worldSeed = spec.get("world_seed").getAsLong();
            LinkedHashMap<String, Tag> payloads = new LinkedHashMap<>();
            for (JsonElement base : spec.getAsJsonArray("bases")) {
              Tag tag = renameWrap(new TagParser(new com.mojang.brigadier.StringReader(base.getAsString())).readValue());
              payloads.putIfAbsent(tagHex(tag), tag);
              for (Tag mutant : nbtMutants(tag)) payloads.putIfAbsent(tagHex(mutant), mutant);
            }
            for (Map.Entry<String, Tag> payload : payloads.entrySet()) {
              JsonObject r = new JsonObject();
              r.addProperty("name", name);
              r.addProperty("world_seed", worldSeed);
              r.addProperty("payload", payload.getKey());
              CompoundTag root = new CompoundTag();
              root.put("data", payload.getValue());
              NbtUtils.addCurrentDataVersion(root);
              Files.deleteIfExists(dir.resolve(name + ".dat"));
              NbtIo.writeCompressed(root, dir.resolve(name + ".dat"));
              net.minecraft.world.level.storage.DimensionDataStorage storage =
                  new net.minecraft.world.level.storage.DimensionDataStorage(dir.toFile(), net.minecraft.util.datafix.DataFixers.getDataFixer(), access);
              net.minecraft.world.level.saveddata.SavedData data;
              if (name.equals("random_sequences")) data = storage.get(net.minecraft.world.RandomSequences.factory(worldSeed), name);
              else data = storage.get(net.minecraft.world.level.saveddata.maps.MapIndex.factory(), name);
              r.addProperty("present", data != null);
              if (data != null) r.addProperty("data", tagHex(data.save(new CompoundTag(), access)));
              outArr.add(r);
            }
          }
        } finally {
          try (var walk = Files.walk(dir)) { walk.sorted(Comparator.reverseOrder()).forEach(p -> p.toFile().delete()); }
        }
        Files.writeString(Path.of(a[2]), GSON.toJson(outArr));
        System.out.println("saved-data-mutants=" + outArr.size());
        break;
      }
      case "scoreboard": {
        // The original Scoreboard/ServerScoreboard driven by scripted operations. Input: JSON array
        // of scenarios {steps:[{op,...}]} or {load: <hex of a scoreboard.dat payload>, steps:[...]};
        // output per step: error, return value, broadcast packet frames (id + payload) and dumps.
        JsonArray in = JsonParser.parseString(Files.readString(Path.of(a[1]))).getAsJsonArray();
        JsonArray outArr = new JsonArray();
        for (JsonElement e : in) {
          JsonObject scenario = e.getAsJsonObject();
          scoreBroadcasts.clear();
          net.minecraft.server.ServerScoreboard sb = makeScoreboard();
          JsonArray results = new JsonArray();
          if (scenario.has("load")) {
            JsonObject r = new JsonObject();
            try {
              CompoundTag payload = (CompoundTag) NbtIo.readAnyTag(new DataInputStream(new ByteArrayInputStream(unhex(scenario.get("load").getAsString()))), NbtAccounter.unlimitedHeap());
              new net.minecraft.world.scores.ScoreboardSaveData(sb).load(payload, access);
            } catch (Throwable t) {
              r.addProperty("error", t.getClass().getSimpleName() + ": " + t.getMessage());
            }
            results.add(r);
          }
          for (JsonElement stepElement : scenario.getAsJsonArray("steps")) {
            JsonObject r;
            scoreBroadcasts.clear();
            try {
              r = scoreboardStep(sb, stepElement.getAsJsonObject(), null);
            } catch (Throwable t) {
              r = new JsonObject();
              r.addProperty("error", t.getClass().getSimpleName() + ": " + t.getMessage());
            }
            JsonArray frames = new JsonArray();
            for (net.minecraft.network.protocol.Packet<?> p : scoreBroadcasts) {
              try { frames.add(frame(p)); } catch (Throwable t) { frames.add("ERROR"); }
            }
            if (frames.size() > 0) r.add("broadcast", frames);
            results.add(r);
          }
          outArr.add(results);
        }
        Files.writeString(Path.of(a[2]), GSON.toJson(outArr));
        System.out.println("scoreboard=" + in.size());
        break;
      }
      case "scoreboard-facts": {
        // Registries behind stat criteria (`minecraft.mined:minecraft.stone`).
        JsonObject facts = new JsonObject();
        JsonArray types = new JsonArray();
        for (net.minecraft.stats.StatType<?> type : net.minecraft.core.registries.BuiltInRegistries.STAT_TYPE) {
          JsonObject t = new JsonObject();
          t.addProperty("id", net.minecraft.core.registries.BuiltInRegistries.STAT_TYPE.getKey(type).toString());
          t.addProperty("registry", type.getRegistry().key().location().toString());
          types.add(t);
        }
        facts.add("stat_types", types);
        JsonArray custom = new JsonArray();
        for (ResourceLocation id : net.minecraft.core.registries.BuiltInRegistries.CUSTOM_STAT.keySet()) custom.add(id.toString());
        facts.add("custom_stats", custom);
        JsonArray names = new JsonArray();
        for (String n : ObjectiveCriteria.getCustomCriteriaNames()) names.add(n);
        facts.add("custom_criteria", names);
        Files.writeString(Path.of(a[1]), GSON.toJson(facts));
        break;
      }
      case "scoreboard-load-mutants": {
        // One-step mutants of scoreboard.dat payloads loaded by the original ScoreboardSaveData.
        // Input: JSON array of payload hex strings. Output rows: {payload, error?, dump?}.
        JsonArray in = JsonParser.parseString(Files.readString(Path.of(a[1]))).getAsJsonArray();
        LinkedHashMap<String, Tag> payloads = new LinkedHashMap<>();
        for (JsonElement e : in) {
          Tag base = NbtIo.readAnyTag(new DataInputStream(new ByteArrayInputStream(unhex(e.getAsString()))), NbtAccounter.unlimitedHeap());
          payloads.putIfAbsent(tagHex(base), base);
          for (Tag mutant : nbtMutants(base)) payloads.putIfAbsent(tagHex(mutant), mutant);
        }
        JsonArray outArr = new JsonArray();
        for (Map.Entry<String, Tag> payload : payloads.entrySet()) {
          JsonObject r = new JsonObject();
          r.addProperty("payload", payload.getKey());
          try {
            net.minecraft.server.ServerScoreboard sb = makeScoreboard();
            if (!(payload.getValue() instanceof CompoundTag compound)) throw new IllegalArgumentException("not a compound");
            new net.minecraft.world.scores.ScoreboardSaveData(sb).load(compound, access);
            r.add("dump", dumpScoreboard(sb));
            r.addProperty("data", tagHex(new net.minecraft.world.scores.ScoreboardSaveData(sb).save(new CompoundTag(), access)));
          } catch (Throwable t) {
            r.addProperty("error", t.getClass().getSimpleName() + ": " + t.getMessage());
          }
          outArr.add(r);
        }
        Files.writeString(Path.of(a[2]), GSON.toJson(outArr));
        System.out.println("scoreboard-load-mutants=" + outArr.size());
        break;
      }
      case "tags": {
        // The original TagLoader over folder packs on disk. Input: JSON array of scenarios
        // {known:[ids], packs:[{id, files:{"ns:path":text}}]}; output per scenario:
        // {tags:{id:[values in collection order]}} (ids sorted).
        JsonArray in = JsonParser.parseString(Files.readString(Path.of(a[1]))).getAsJsonArray();
        JsonArray outArr = new JsonArray();
        for (JsonElement e : in) {
          JsonObject scenario = e.getAsJsonObject();
          Path root = Files.createTempDirectory("tags-oracle");
          try {
            List<PackResources> packs = new ArrayList<>();
            for (JsonElement packElement : scenario.getAsJsonArray("packs")) {
              JsonObject pack = packElement.getAsJsonObject();
              Path dir = root.resolve(pack.get("id").getAsString());
              Files.createDirectories(dir);
              for (Map.Entry<String, JsonElement> file : pack.getAsJsonObject("files").entrySet()) {
                String[] parts = file.getKey().split(":", 2);
                Path target = dir.resolve("data").resolve(parts[0]).resolve("tags/test").resolve(parts[1] + ".json");
                Files.createDirectories(target.getParent());
                Files.writeString(target, file.getValue().getAsString());
              }
              packs.add(new PathPackResources(new PackLocationInfo(pack.get("id").getAsString(), net.minecraft.network.chat.Component.literal(pack.get("id").getAsString()), PackSource.WORLD, Optional.empty()), dir));
            }
            Set<String> known = new HashSet<>();
            for (JsonElement k : scenario.getAsJsonArray("known")) known.add(k.getAsString());
            MultiPackResourceManager manager = new MultiPackResourceManager(PackType.SERVER_DATA, packs);
            net.minecraft.tags.TagLoader<String> loader = new net.minecraft.tags.TagLoader<>(
                id -> known.contains(id.toString()) ? Optional.of(id.toString()) : Optional.empty(), "tags/test");
            Map<ResourceLocation, Collection<String>> built;
            try {
              built = loader.loadAndBuild(manager);
            } catch (StackOverflowError overflow) {
              // DependencySorter.isCyclic recurses forever once a tag depends on itself.
              JsonObject failed = new JsonObject();
              failed.addProperty("error", "StackOverflowError");
              outArr.add(failed);
              manager.close();
              continue;
            }
            TreeMap<String, JsonArray> sorted = new TreeMap<>();
            for (Map.Entry<ResourceLocation, Collection<String>> entry : built.entrySet()) {
              JsonArray values = new JsonArray();
              for (String v : entry.getValue()) values.add(v);
              sorted.put(entry.getKey().toString(), values);
            }
            JsonObject r = new JsonObject();
            JsonObject tags = new JsonObject();
            for (Map.Entry<String, JsonArray> entry : sorted.entrySet()) tags.add(entry.getKey(), entry.getValue());
            r.add("tags", tags);
            outArr.add(r);
            manager.close();
          } finally {
            try (var walk = Files.walk(root)) { walk.sorted(Comparator.reverseOrder()).forEach(path -> path.toFile().delete()); }
          }
        }
        Files.writeString(Path.of(a[2]), GSON.toJson(outArr));
        System.out.println("tags=" + in.size());
        break;
      }
      case "export-tags": {
        // Every shipped `tags/**` JSON of the vanilla data pack, by directory: {dir:{"ns:path":text}}.
        PackRepository repo = ServerPacksSource.createVanillaTrustedRepository();
        repo.reload();
        List<PackResources> packs = repo.getAvailablePacks().stream().filter(p -> p.getId().equals("vanilla")).map(Pack::open).toList();
        MultiPackResourceManager manager = new MultiPackResourceManager(PackType.SERVER_DATA, packs);
        JsonObject all = new JsonObject();
        net.minecraft.resources.FileToIdConverter converter = net.minecraft.resources.FileToIdConverter.json("tags");
        TreeMap<String, TreeMap<String, String>> byDirectory = new TreeMap<>();
        for (Map.Entry<ResourceLocation, Resource> entry : converter.listMatchingResources(manager).entrySet()) {
          String id = converter.fileToId(entry.getKey()).getPath();
          int slash = id.indexOf('/');
          if (slash < 0) continue;
          String text;
          try (java.io.InputStream stream = entry.getValue().open()) { text = new String(stream.readAllBytes(), StandardCharsets.UTF_8); }
          byDirectory.computeIfAbsent(id.substring(0, slash), k -> new TreeMap<>()).put(entry.getKey().getNamespace() + ":" + id.substring(slash + 1), text);
        }
        for (Map.Entry<String, TreeMap<String, String>> directory : byDirectory.entrySet()) {
          JsonObject files = new JsonObject();
          for (Map.Entry<String, String> file : directory.getValue().entrySet()) files.addProperty(file.getKey(), file.getValue());
          all.add(directory.getKey(), files);
        }
        Files.writeString(Path.of(a[1]), GSON.toJson(all));
        break;
      }
      case "export-facts": {
        Files.writeString(Path.of(a[1]), GSON.toJson(exportFacts()));
        break;
      }
      default: throw new IllegalArgumentException(a[0]);
    }
  }
}
