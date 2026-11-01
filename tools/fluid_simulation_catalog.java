package oracle;

import com.google.gson.Gson;
import java.lang.reflect.*;
import java.nio.file.*;
import java.util.*;

/** Static block contracts used by the native FlowingFluid implementation. */
public final class fluid_simulation_catalog {
    static Class<?> c(String name) throws Exception { return Class.forName(name); }
    static Object call(Object target, String name, Class<?>[] types, Object... args) throws Exception {
        Class<?> type = target instanceof Class<?> ? (Class<?>) target : target.getClass();
        Method method = type.getMethod(name, types); method.setAccessible(true);
        return method.invoke(target instanceof Class<?> ? null : target, args);
    }
    public static void main(String[] args) throws Exception {
        call(c("ab"), "a", new Class<?>[0]); call(c("akt"), "a", new Class<?>[0]);
        Object empty = c("dcl").getField("a").get(null);
        Object origin = c("jd").getConstructor(int.class,int.class,int.class).newInstance(0,0,0);
        Object[] fluids = {c("epf").getField("c").get(null),c("epf").getField("b").get(null),c("epf").getField("e").get(null),c("epf").getField("d").get(null)};
        Class<?>[] canTypes = {c("cmx"),c("dcc"),c("jd"),c("dtc"),c("epd")};
        Class<?>[] placeTypes = {c("dcx"),c("jd"),c("dtc"),c("epe")};
        List<List<Object>> rows = new ArrayList<>(); List<Integer> states = new ArrayList<>();
        List<Integer> ignitedByLava = new ArrayList<>();
        List<Integer> dynamicShape = new ArrayList<>();
        Map<String,Integer> unique = new LinkedHashMap<>(); Map<String,String> owners = new TreeMap<>();
        for (int id=0; id<26684; id++) {
            Object state=call(c("dfy"),"a",new Class<?>[]{int.class},id);
            Object block=call(state,"b",new Class<?>[0]);
            if ((boolean)call(block,"p",new Class<?>[0])) dynamicShape.add(id);
            boolean solid=(boolean)call(state,"e",new Class<?>[0]);
            if ((boolean)call(state,"j",new Class<?>[0])) ignitedByLava.add(id);
            boolean liquid=c("dko").isInstance(block), container=c("dkp").isInstance(block);
            int mode=0, mask=0;
            if(container){
                String owner=block.getClass().getMethod("a",placeTypes).getDeclaringClass().getName();
                owners.put(block.getClass().getName(),owner);
                // SlabBlock's override only rejects DOUBLE, then delegates to
                // SimpleWaterloggedBlock. Its captured admission mask already
                // encodes that DOUBLE rejection.
                mode=(owner.equals("dna") || owner.equals("dnc"))?1:2;
                for(int f=0; f<4; f++) {
                    try { if((boolean)call(block,"a",canTypes,null,empty,origin,state,fluids[f])) mask|=1<<f; }
                    catch(InvocationTargetException e) { mask=-1; break; }
                }
            }
            List<Object> row=Arrays.asList(solid,liquid,mode,mask); String key=new Gson().toJson(row);
            Integer index=unique.get(key); if(index==null){index=rows.size();unique.put(key,index);rows.add(row);}states.add(index);
        }
        Map<String,Object> out=new LinkedHashMap<>();out.put("minecraft","1.21");out.put("oracle_inner_sha256","8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01");
        out.put("description","rows=[BlockState.isSolid,LiquidBlock,container0none1SimpleWaterlogged-or-Slab-delegate2custom,canPlaceLiquid bit0sourceWater1flowingWater2sourceLava3flowingLava; -1 means world-dependent and unsupported by static table]");
        out.put("states",states);out.put("rows",rows);out.put("container_place_owners",owners);out.put("ignited_by_lava",ignitedByLava);out.put("dynamic_shape",dynamicShape);
        Files.writeString(Path.of(args[0]),new Gson().toJson(out)+"\n");System.out.println("FLUID_CATALOG "+states.size()+" states "+rows.size()+" rows "+owners);
    }
}
