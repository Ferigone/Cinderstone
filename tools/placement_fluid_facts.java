package oracle;

import com.google.gson.GsonBuilder;
import java.lang.reflect.Method;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.IdentityHashMap;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

/** Minimal original-class placement fluid extraction; no block-entity creation. */
public final class placement_fluid_facts {
    private static final String INNER_SHA256 =
        "8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01";
    private static final String MAPPINGS_SHA1 =
        "31c77994d96f05ba25a870ada70f47f315330437";

    private static List<List<Integer>> ranges(List<Integer> types, int wanted) {
        List<List<Integer>> output = new ArrayList<>();
        for (int at = 0; at < types.size(); at++) {
            if (types.get(at) != wanted) continue;
            int start = at;
            while (at + 1 < types.size() && types.get(at + 1) == wanted) at++;
            output.add(List.of(start, at));
        }
        return output;
    }

    public static void main(String[] args) throws Exception {
        if (args.length != 1) throw new IllegalArgumentException("output JSON path required");
        Class.forName("ab").getMethod("a").invoke(null);
        Class.forName("akt").getMethod("a").invoke(null);

        Class<?> block = Class.forName("dfy");
        Class<?> blockState = Class.forName("dtc");
        Class<?> fluidState = Class.forName("epe");
        Class<?> fluids = Class.forName("epf");
        Object mapper = block.getField("q").get(null);
        Method size = mapper.getClass().getMethod("c");
        Method byId = mapper.getClass().getMethod("a", int.class);
        Method getFluidState = blockState.getMethod("u");
        Method getType = fluidState.getMethod("a");
        Method getAmount = fluidState.getMethod("e");
        int count = (Integer) size.invoke(mapper);
        if (count != 26_684) throw new IllegalStateException("foreign original state domain");

        // These are field identities from the official 1.21 mappings, not
        // class-name/suffix guesses or raw Fluid registry ID ordinals.
        String[] fields = {"a", "c", "b", "e", "d"};
        String[] names = {"EMPTY", "WATER", "FLOWING_WATER", "LAVA", "FLOWING_LAVA"};
        Object[] actualTypes = new Object[fields.length];
        Map<Object, Integer> typeCodes = new IdentityHashMap<>();
        Map<String, String> mappedFields = new LinkedHashMap<>();
        for (int index = 0; index < fields.length; index++) {
            actualTypes[index] = fluids.getField(fields[index]).get(null);
            if (typeCodes.put(actualTypes[index], index) != null)
                throw new IllegalStateException("fluid fields unexpectedly alias");
            mappedFields.put(fields[index], names[index]);
        }

        List<Integer> flags = new ArrayList<>(count);
        List<Integer> observedTypes = new ArrayList<>(count);
        List<Integer> amounts = new ArrayList<>(count);
        for (int id = 0; id < count; id++) {
            Object state = byId.invoke(mapper, id);
            if (state == null) throw new IllegalStateException("original state registry hole");
            Object fluid = getFluidState.invoke(state);
            Object type = getType.invoke(fluid);
            Integer typeCode = typeCodes.get(type);
            if (typeCode == null) throw new IllegalStateException("unknown actual vanilla fluid type");
            int amount = (Integer) getAmount.invoke(fluid);
            boolean sourceWater = type == actualTypes[1];
            boolean water = sourceWater || type == actualTypes[2];
            int value = (sourceWater ? 1 : 0) | (water ? 2 : 0)
                | (water && amount == 8 ? 4 : 0) | (type != actualTypes[0] ? 8 : 0);
            flags.add(value);
            observedTypes.add(typeCode);
            amounts.add(amount);
        }
        Map<String, Object> output = new LinkedHashMap<>();
        output.put("minecraft", "1.21");
        output.put("protocol", 767);
        output.put("inner_server_sha256", INNER_SHA256);
        output.put("official_mappings_sha1", MAPPINGS_SHA1);
        output.put("state_count", count);
        output.put("queried_nonnull_states", flags.size());
        output.put("method", "unchanged original IdMapper -> BlockState.getFluidState -> getType/getAmount");
        output.put("epf_fields", mappedFields);
        output.put("type_code_names", List.of(names));
        output.put("type_codes_are_observation_indexes_not_registry_ids", true);
        output.put("observed_type_codes", observedTypes);
        output.put("observed_amounts", amounts);
        output.put("fluids", flags);
        output.put("flowing_water_state_ranges", ranges(observedTypes, 2));
        output.put("flowing_lava_state_ranges", ranges(observedTypes, 4));
        output.put("block_entity_factory_invocations", 0);
        Files.writeString(Path.of(args[0]), new GsonBuilder().setPrettyPrinting().create().toJson(output) + "\n");
        System.out.println("PLACEMENT_FLUID_FACTS states=" + count
            + " flowing_water=" + ranges(observedTypes, 2)
            + " flowing_lava=" + ranges(observedTypes, 4));
    }
}
