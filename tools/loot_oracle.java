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
      case "export-facts": {
        Files.writeString(Path.of(a[1]), GSON.toJson(exportFacts()));
        break;
      }
      default: throw new IllegalArgumentException(a[0]);
    }
  }
}
