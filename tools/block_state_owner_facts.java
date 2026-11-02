package oracle;

import com.google.gson.*;
import java.lang.reflect.*;
import java.nio.file.*;
import java.security.MessageDigest;
import java.util.*;

/**
 * Source-data extraction from the unmodified pinned original registry bootstrap.
 * No world, Player, replacement registry, tag binding or copied implementation.
 * Run with JDK21's source launcher and ONLY the original inner JAR/bundled libs.
 * Args: output innerJar mappings inventoryJson placementJson outerJar thisSource.
 */
public final class block_state_owner_facts {
    static final String INNER = "8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01";
    static final String OUTER = "450698d1863ab5180c25d7c804ef0fe6369dd1ba";
    static final String MAPPINGS = "31c77994d96f05ba25a870ada70f47f315330437";
    static final String METHOD = "original_1.21_bootstrap_registry_property_and_all_state_capture_v1";

    static void require(boolean condition, String message) {
        if (!condition) throw new IllegalStateException(message);
    }
    static String digest(Path path, String algorithm) throws Exception {
        return HexFormat.of().formatHex(MessageDigest.getInstance(algorithm).digest(Files.readAllBytes(path)));
    }
    static Class<?> cls(String name) throws Exception { return Class.forName(name); }
    static Object invoke(Object target, String method, Class<?>[] parameters, Object... args) throws Exception {
        Class<?> type = target instanceof Class<?> ? (Class<?>) target : target.getClass();
        Method call = type.getMethod(method, parameters);
        call.setAccessible(true);
        return call.invoke(target instanceof Class<?> ? null : target, args);
    }
    static Object invoke(Object target, String method) throws Exception {
        return invoke(target, method, new Class<?>[0]);
    }
    static JsonObject json(Path path) throws Exception {
        return JsonParser.parseString(Files.readString(path)).getAsJsonObject();
    }
    static List<String> domain(JsonObject property) {
        if (property.get("type").getAsString().equals("bool")) return List.of("true", "false");
        List<String> values = new ArrayList<>();
        for (JsonElement value : property.getAsJsonArray("values")) values.add(value.getAsString());
        return values;
    }
    static Map<String, String> encodedValues(JsonObject block, int id) {
        int offset = id - block.get("min_state").getAsInt();
        JsonArray properties = block.getAsJsonArray("properties");
        Map<String, String> result = new LinkedHashMap<>();
        for (int i = properties.size() - 1; i >= 0; i--) {
            JsonObject property = properties.get(i).getAsJsonObject();
            int count = property.get("num_values").getAsInt();
            result.put(property.get("name").getAsString(), domain(property).get(offset % count));
            offset /= count;
        }
        require(offset == 0, "native state offset exceeds its property domain");
        return result;
    }

    public static void main(String[] args) throws Exception {
        require(args.length == 7, "expected output inner mappings inventory placement outer extractor-source");
        Path output = Path.of(args[0]), inner = Path.of(args[1]), mappings = Path.of(args[2]);
        Path inventoryPath = Path.of(args[3]), placementPath = Path.of(args[4]);
        require(digest(inner, "SHA-256").equals(INNER), "foreign original inner server");
        require(digest(Path.of(args[5]), "SHA-1").equals(OUTER), "foreign original outer server");
        require(digest(mappings, "SHA-1").equals(MAPPINGS), "foreign official mappings");
        for (String name : List.of("ab", "akt", "lt", "dfy", "dtc", "dtd", "dte", "duf")) {
            Class<?> original = Class.forName(name, false, block_state_owner_facts.class.getClassLoader());
            Path loaded = Path.of(original.getProtectionDomain().getCodeSource().getLocation().toURI());
            require(Files.isSameFile(loaded, inner), "original class did not come from pinned inner JAR: " + name);
        }
        String inventoryHash = digest(inventoryPath, "SHA-256");
        String placementHash = digest(placementPath, "SHA-256");
        String extractorHash = digest(Path.of(args[6]), "SHA-256");
        JsonObject inventory = json(inventoryPath), placement = json(placementPath);
        require(inventory.get("version").getAsString().equals("1.21")
            && inventory.get("protocol").getAsInt() == 767, "foreign native state catalog");
        require(placement.get("minecraft").getAsString().equals("1.21")
            && placement.get("jar_sha256").getAsString().equals(INNER), "foreign placement catalog");
        JsonObject nativeBlocks = inventory.getAsJsonObject("blocks");
        Map<String, JsonObject> placementBlocks = new HashMap<>();
        for (JsonElement entry : placement.getAsJsonArray("blocks")) {
            JsonObject row = entry.getAsJsonObject();
            require(placementBlocks.put(row.get("name").getAsString(), row) == null, "duplicate placement key");
        }

        invoke(cls("ab"), "a");
        invoke(cls("akt"), "a");
        Object version = invoke(cls("ab"), "b");
        require(invoke(version, "c").equals("1.21") && (int) invoke(cls("ab"), "c") == 767,
            "actual bootstrapped version/protocol differs");
        Object registry = cls("lt").getField("e").get(null);
        Class<?> stateClass = cls("dtc"), propertyClass = cls("duf"), blockClass = cls("dfy");
        Method rawId = cls("jz").getMethod("a", Object.class);
        Method keyOf = cls("jz").getMethod("b", Object.class);
        Method stateId = blockClass.getMethod("i", stateClass);
        Method propertyName = propertyClass.getMethod("f");
        Method valueName = propertyClass.getMethod("a", Comparable.class);
        Method possibleValues = propertyClass.getMethod("a");
        Method valueClass = propertyClass.getMethod("g");
        Set<Integer> allBlockIds = new HashSet<>(), allStateIds = new HashSet<>();
        Set<String> allKeys = new HashSet<>();
        JsonArray blocks = new JsonArray();
        for (Object block : (Iterable<?>) registry) {
            int blockId = (int) rawId.invoke(registry, block);
            String key = keyOf.invoke(registry, block).toString();
            require(allBlockIds.add(blockId) && allKeys.add(key), "duplicate actual Block identity");
            require(key.startsWith("minecraft:"), "nonvanilla original registry key");
            JsonObject nativeBlock = nativeBlocks.getAsJsonObject(key.substring("minecraft:".length()));
            JsonObject placementBlock = placementBlocks.get(key);
            require(nativeBlock != null && placementBlock != null, "actual Block absent from existing catalogs: " + key);
            require(nativeBlock.get("id").getAsInt() == blockId, "native raw Block ID differs: " + key);
            require(placementBlock.get("class").getAsString().equals(block.getClass().getName()), "placement class differs: " + key);
            Object definition = invoke(block, "l");
            Collection<?> definitions = (Collection<?>) invoke(definition, "d");
            JsonArray encoded = nativeBlock.getAsJsonArray("properties"), properties = new JsonArray();
            require(encoded.size() == definitions.size(), "property count differs: " + key);
            Set<Object> propertyOwners = Collections.newSetFromMap(new IdentityHashMap<>());
            int propertyIndex = 0;
            for (Object property : definitions) {
                require(propertyOwners.add(property), "duplicate actual Property singleton");
                String name = (String) propertyName.invoke(property);
                String kind = cls("dtt").isInstance(property) ? "bool"
                    : cls("duc").isInstance(property) ? "int"
                    : cls("dua").isInstance(property) ? "enum" : null;
                require(kind != null, "unsupported original Property family: " + property.getClass().getName());
                List<String> names = new ArrayList<>();
                for (Object value : (Collection<?>) possibleValues.invoke(property))
                    names.add((String) valueName.invoke(property, (Comparable<?>) value));
                JsonObject nativeProperty = encoded.get(propertyIndex++).getAsJsonObject();
                require(nativeProperty.get("name").getAsString().equals(name)
                    && nativeProperty.get("type").getAsString().equals(kind)
                    && nativeProperty.get("num_values").getAsInt() == names.size()
                    && domain(nativeProperty).equals(names), "property owner/domain/order differs: " + key + "/" + name);
                JsonObject fact = new JsonObject();
                fact.addProperty("name", name);
                fact.addProperty("kind", kind);
                fact.addProperty("original_class", property.getClass().getName());
                fact.addProperty("original_value_class", ((Class<?>) valueClass.invoke(property)).getName());
                fact.add("values", new Gson().toJsonTree(names));
                properties.add(fact);
            }
            int minimum = Integer.MAX_VALUE, maximum = Integer.MIN_VALUE, stateCount = 0;
            List<String> entryOrder = null;
            for (Object state : (Collection<?>) invoke(definition, "a")) {
                int id = (int) stateId.invoke(null, state);
                require(allStateIds.add(id), "duplicate actual raw state ID");
                require(invoke(state, "b") == block, "state belongs to another Block");
                Map<?, ?> values = (Map<?, ?>) invoke(state, "C");
                require(values.size() == propertyOwners.size(), "state Property identity set differs");
                List<String> order = new ArrayList<>();
                Map<String, String> serialized = new LinkedHashMap<>();
                for (Map.Entry<?, ?> entry : values.entrySet()) {
                    require(propertyOwners.contains(entry.getKey()), "state uses foreign Property singleton");
                    String name = (String) propertyName.invoke(entry.getKey());
                    order.add(name);
                    require(serialized.put(name, (String) valueName.invoke(entry.getKey(), (Comparable<?>) entry.getValue())) == null,
                        "duplicate state property name");
                }
                if (entryOrder == null) entryOrder = List.copyOf(order);
                require(entryOrder.equals(order), "StateHolder order varies; per-state capture is required: " + key);
                require(serialized.equals(encodedValues(nativeBlock, id)), "actual raw-state values differ: " + id);
                minimum = Math.min(minimum, id); maximum = Math.max(maximum, id); stateCount++;
            }
            int defaultId = (int) stateId.invoke(null, invoke(block, "o"));
            require(stateCount > 0 && maximum - minimum + 1 == stateCount, "noncontiguous actual state range");
            require(nativeBlock.get("min_state").getAsInt() == minimum
                && nativeBlock.get("max_state").getAsInt() == maximum
                && nativeBlock.get("default_state").getAsInt() == defaultId
                && placementBlock.get("min").getAsInt() == minimum
                && placementBlock.get("max").getAsInt() == maximum
                && placementBlock.get("default").getAsInt() == defaultId, "state range/default differs: " + key);
            JsonObject row = new JsonObject();
            row.addProperty("key", key); row.addProperty("registry_id", blockId);
            row.addProperty("original_block_class", block.getClass().getName());
            row.addProperty("minimum_state", minimum); row.addProperty("maximum_state", maximum);
            row.addProperty("default_state", defaultId);
            row.add("properties", properties);
            row.add("state_holder_property_entry_order", new Gson().toJsonTree(entryOrder));
            row.addProperty("verified_states", stateCount);
            blocks.add(row);
        }
        require(blocks.size() == 1060 && nativeBlocks.size() == 1060 && placementBlocks.size() == 1060,
            "original Block domain is not complete");
        require(allStateIds.size() == 26684, "original state domain is not complete");
        for (int id = 0; id < 1060; id++) require(allBlockIds.contains(id), "missing actual Block ID");
        for (int id = 0; id < 26684; id++) require(allStateIds.contains(id), "missing actual state ID");
        JsonObject out = new JsonObject();
        out.addProperty("minecraft", "1.21"); out.addProperty("protocol", 767);
        out.addProperty("method", METHOD); out.addProperty("original_server_sha1", OUTER);
        out.addProperty("original_inner_sha256", INNER); out.addProperty("official_mappings_sha1", MAPPINGS);
        require(inventoryHash.equals(digest(inventoryPath, "SHA-256"))
            && placementHash.equals(digest(placementPath, "SHA-256"))
            && extractorHash.equals(digest(Path.of(args[6]), "SHA-256")), "source inputs changed during capture");
        out.addProperty("inventory_catalog_sha256", inventoryHash);
        out.addProperty("placement_catalog_sha256", placementHash);
        out.addProperty("extractor_source_sha256", extractorHash);
        out.addProperty("java_runtime", System.getProperty("java.runtime.version"));
        out.addProperty("block_count", blocks.size()); out.addProperty("state_count", allStateIds.size());
        out.add("blocks", blocks);
        Files.writeString(output, new Gson().toJson(out) + "\n");
        System.out.println("BLOCK_STATE_OWNER_FACTS original blocks=" + blocks.size()
            + " states=" + allStateIds.size() + " sha256=" + digest(output, "SHA-256"));
    }
}
