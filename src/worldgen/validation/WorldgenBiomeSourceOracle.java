package oracle;

import com.google.gson.*;
import com.mojang.datafixers.util.Pair;
import java.io.*;
import java.lang.reflect.*;
import java.nio.file.*;
import java.security.*;
import java.util.*;

/** Original registry parameter order and actual unchanged biome-source searches. */
public class WorldgenBiomeSourceOracle {
    static final PrintStream OUT = System.out;
    static final String INNER = "8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01";
    static final String MAPPINGS = "31c77994d96f05ba25a870ada70f47f315330437";
    static final Gson JSON = new GsonBuilder().serializeNulls().create();
    static Class<?> KEY, LOCATION, HOLDER, PARAMETER, POINT, TARGET, LIST, STATE, SAMPLER, FUNCTION, CONTEXT;
    static Object getters, presetRegistry, settingsRegistry, biomeRegistry;
    static final Map<String,Object> presets = new LinkedHashMap<>();
    static final Map<String,List<?>> rows = new LinkedHashMap<>();

    static Object call(Object owner, Class<?> type, String name, Class<?>[] args, Object... values) throws Exception {
        return type.getMethod(name,args).invoke(owner,values);
    }
    static Object key(Object registry, String name) throws Exception {
        return KEY.getMethod("a",KEY,LOCATION).invoke(null,registry,LOCATION.getMethod("a",String.class).invoke(null,name));
    }
    static Object holder(Object registry, String name) throws Exception {
        Object getter=call(getters,Class.forName("jn$a"),"b",new Class<?>[]{KEY},registry);
        return call(getter,Class.forName("jn"),"b",new Class<?>[]{KEY},key(registry,name));
    }
    static Object value(Object holder) throws Exception { return call(holder,HOLDER,"a",new Class<?>[]{}); }
    static String biome(Object holder) throws Exception {
        Object key=((Optional<?>)call(holder,HOLDER,"e",new Class<?>[]{})).orElseThrow();
        return call(key,KEY,"a",new Class<?>[]{}).toString();
    }
    static long[][] parameters(Object point) throws Exception {
        Method method=POINT.getDeclaredMethod("a"); method.setAccessible(true);
        List<?> space=(List<?>)method.invoke(point); long[][] result=new long[space.size()][2];
        for(int i=0;i<space.size();i++) {
            result[i][0]=(long)call(space.get(i),PARAMETER,"a",new Class<?>[]{});
            result[i][1]=(long)call(space.get(i),PARAMETER,"b",new Class<?>[]{});
        }
        return result;
    }
    static Object first(Object row) { return ((Pair<?,?>)row).getFirst(); }
    static Object second(Object row) { return ((Pair<?,?>)row).getSecond(); }
    static List<Map<String,Object>> entries(List<?> rows) throws Exception {
        List<Map<String,Object>> entries=new ArrayList<>();
        for(Object row:rows) entries.add(Map.of("parameters",parameters(first(row)),"biome",biome(second(row))));
        return entries;
    }
    static Object target(long[] q) throws Exception {
        return TARGET.getConstructor(long.class,long.class,long.class,long.class,long.class,long.class)
            .newInstance(q[0],q[1],q[2],q[3],q[4],q[5]);
    }
    static long[] targetArray(Object target) throws Exception {
        Method method=TARGET.getDeclaredMethod("a");method.setAccessible(true);
        return Arrays.copyOf((long[])method.invoke(target),6);
    }
    static Object indexList(List<?> original) throws Exception {
        List<Object> indexed=new ArrayList<>();
        for(int i=0;i<original.size();i++) indexed.add(Pair.of(first(original.get(i)),i));
        return LIST.getConstructor(List.class).newInstance(indexed);
    }
    static Map<String,Object> query(Object actual,Object indexed,List<?> original,long[] query) throws Exception {
        Object point=target(query);
        String selected=biome(call(actual,LIST,"a",new Class<?>[]{TARGET},point));
        int index=(Integer)call(indexed,LIST,"a",new Class<?>[]{TARGET},point);
        if(!selected.equals(biome(second(original.get(index)))))throw new AssertionError("holder/index tree choice mismatch");
        return Map.of("target",query,"index",index,"biome",selected);
    }
    static List<Map<String,Object>> searchSequences() throws Exception {
        List<Map<String,Object>> result=new ArrayList<>();
        for(String name:presets.keySet()) {
            List<?> original=rows.get(name);
            // A fresh original list owner models a new worker's empty search history.
            Object actual=LIST.getConstructor(List.class).newInstance(original),indexed=indexList(original);
            List<long[]> queries=new ArrayList<>();
            queries.add(new long[6]);queries.add(new long[]{10000,10000,10000,10000,10000,10000});
            queries.add(new long[]{-10000,-10000,-10000,-10000,-10000,-10000});
            queries.add(new long[]{0,0,0,0,10000,0});queries.add(new long[]{0,0,0,0,-10000,0});
            for(int i=0;i<19;i++) {
                long[][] space=parameters(first(original.get((original.size()-1)*i/18)));
                long[] center=new long[6];for(int d=0;d<6;d++)center[d]=space[d][0]+(space[d][1]-space[d][0])/2;
                queries.add(center);
            }
            // Repeating the first target retains any legitimate original last-leaf history.
            queries.add(new long[6]);
            List<Map<String,Object>> observed=new ArrayList<>();
            for(long[] q:queries)observed.add(query(actual,indexed,original,q));
            result.add(Map.of("preset",name,"queries",observed));
        }
        return result;
    }
    static List<Map<String,Object>> ties() throws Exception {
        List<Object> original=new ArrayList<>();List<Map<String,Object>> entries=new ArrayList<>();
        for(int i=0;i<2;i++) {
            Object[] arguments=new Object[7];long[][] space=new long[7][2];
            for(int d=0;d<6;d++) {
                long v=d==0?(i==0?-10:10):0;space[d][0]=v;space[d][1]=v;
                arguments[d]=PARAMETER.getConstructor(long.class,long.class).newInstance(v,v);
            }
            arguments[6]=0L;
            Object point=POINT.getConstructor(PARAMETER,PARAMETER,PARAMETER,PARAMETER,PARAMETER,PARAMETER,long.class).newInstance(arguments);
            original.add(Pair.of(point,i));entries.add(Map.of("parameters",space,"value",i==0?"controlled:left":"controlled:right"));
        }
        List<Map<String,Object>> sequences=new ArrayList<>();
        for(int[] temperatures:new int[][]{{0,10,0,-10,0},{10,0,-10,0},{-10,0,10,0}}) {
            Object list=LIST.getConstructor(List.class).newInstance(original);List<Map<String,Object>> observed=new ArrayList<>();
            for(int t:temperatures) {
                long[] q={t,0,0,0,0,0};int index=(Integer)call(list,LIST,"a",new Class<?>[]{TARGET},target(q));
                observed.add(Map.of("target",q,"index",index,"value",index==0?"controlled:left":"controlled:right"));
            }
            sequences.add(Map.of("entries",entries,"queries",observed));
        }
        return sequences;
    }
    static Object wrappedSampler(String settings,long seed,int cx,int cz) throws Exception {
        Object set=value(holder(settingsRegistry,settings));
        Object state=STATE.getMethod("a",Class.forName("jn$a"),KEY,long.class).invoke(null,getters,key(settingsRegistry,settings),seed);
        Class<?> settingsClass=Class.forName("dzd"),chunk=Class.forName("dzc"),noise=Class.forName("dzg");
        Object grid=call(set,settingsClass,"f",new Class<?>[]{});int width=(int)call(grid,noise,"b",new Class<?>[]{});
        Method picker=Class.forName("dzb").getDeclaredMethod("a",settingsClass);picker.setAccessible(true);
        Field beard=Class.forName("dys$b").getField("a");beard.setAccessible(true);
        Object wrapped=chunk.getConstructor(int.class,STATE,int.class,int.class,noise,Class.forName("dys$c"),settingsClass,Class.forName("dyk$a"),Class.forName("eaa"))
            .newInstance(16/width,state,cx*16,cz*16,grid,beard.get(null),set,picker.invoke(null,set),Class.forName("eaa").getMethod("a").invoke(null));
        Method cached=chunk.getDeclaredMethod("a",Class.forName("dze"),List.class);cached.setAccessible(true);
        return cached.invoke(wrapped,STATE.getMethod("a").invoke(state),settingsClass.getMethod("k").invoke(set));
    }
    static List<Map<String,Object>> sampleSequences() throws Exception {
        List<Map<String,Object>> sequences=new ArrayList<>();
        for(String setting:new String[]{"overworld","large_biomes","amplified","nether"}) {
            String preset=setting.equals("nether")?"minecraft:nether":"minecraft:overworld";
            List<?> original=rows.get(preset);Object actual=LIST.getConstructor(List.class).newInstance(original),indexed=indexList(original);
            Object source=Class.forName("dej").getMethod("a",LIST).invoke(null,actual);
            Object sampler=wrappedSampler("minecraft:"+setting,123456789L,0,0);
            List<Map<String,Object>> observed=new ArrayList<>();
            for(int[] quart:new int[][]{{0,0,0},{3,16,3},{1,-16,2},{2,32,1},{0,80,3}}) {
                Object target=call(sampler,SAMPLER,"a",new Class<?>[]{int.class,int.class,int.class},quart[0],quart[1],quart[2]);
                String selected=biome(call(source,Class.forName("dej"),"getNoiseBiome",new Class<?>[]{int.class,int.class,int.class,SAMPLER},quart[0],quart[1],quart[2],sampler));
                int index=(Integer)call(indexed,LIST,"a",new Class<?>[]{TARGET},target);
                if(!selected.equals(biome(second(original.get(index)))))throw new AssertionError("sample holder/index mismatch");
                observed.add(Map.of("quart",quart,"target",targetArray(target),"index",index,"biome",selected));
            }
            sequences.add(Map.of("settings","minecraft:"+setting,"preset",preset,"seed","123456789","chunk",new int[]{0,0},"queries",observed));
        }
        return sequences;
    }
    static List<Map<String,Object>> endSequences() throws Exception {
        Object getter=call(getters,Class.forName("jn$a"),"b",new Class<?>[]{KEY},biomeRegistry);
        Object source=Class.forName("den").getMethod("a",Class.forName("jn")).invoke(null,getter);
        List<Map<String,Object>> result=new ArrayList<>();
        for(long seed:new long[]{0,123456789,-987654321}) {
            Object sampler=wrappedSampler("minecraft:end",seed,0,0);
            Object erosion=SAMPLER.getMethod("e").invoke(sampler);
            List<Map<String,Object>> observed=new ArrayList<>();
            for(int[] quart:new int[][]{{0,0,0},{256,0,0},{260,16,0},{-260,-1,0},{512,32,512},{1024,0,-1024},{-4096,64,2048},{8192,-16,-8192},{32768,16,16384}}) {
                String selected=biome(call(source,Class.forName("den"),"getNoiseBiome",new Class<?>[]{int.class,int.class,int.class,SAMPLER},quart[0],quart[1],quart[2],sampler));
                int bx=quart[0]<<2,by=quart[1]<<2,bz=quart[2]<<2,sx=bx>>4,sz=bz>>4;
                Map<String,Object> q=new LinkedHashMap<>();q.put("quart",quart);q.put("biome",selected);
                if((long)sx*sx+(long)sz*sz>4096) {
                    int[] p={(sx*2+1)*8,by,(sz*2+1)*8};Object point=Class.forName("dyr$e").getConstructor(int.class,int.class,int.class).newInstance(p[0],p[1],p[2]);
                    double sample=(double)call(erosion,FUNCTION,"a",new Class<?>[]{CONTEXT},point);
                    q.put("erosion_point",p);q.put("erosion_bits",Long.toUnsignedString(Double.doubleToRawLongBits(sample)));
                } else {q.put("erosion_point",null);q.put("erosion_bits",null);}
                observed.add(q);
            }
            result.add(Map.of("settings","minecraft:end","seed",Long.toString(seed),"chunk",new int[]{0,0},"queries",observed));
        }
        return result;
    }
    static List<Map<String,Object>> endThresholds() throws Exception {
        Object getter=call(getters,Class.forName("jn$a"),"b",new Class<?>[]{KEY},biomeRegistry);
        Object source=Class.forName("den").getMethod("a",Class.forName("jn")).invoke(null,getter);
        Method constant=Class.forName("dys").getMethod("a",double.class);
        Object zero=constant.invoke(null,0.0);
        List<Map<String,Object>> observed=new ArrayList<>();
        // These are controlled original density inputs to the actual original
        // source predicate, not claims that natural terrain produces NaN/inf.
        for(double erosion:new double[]{Double.NaN,Double.NEGATIVE_INFINITY,Double.POSITIVE_INFINITY,
            Math.nextDown(0.25),0.25,Math.nextUp(0.25),
            Math.nextDown(-0.0625),-0.0625,Math.nextUp(-0.0625),
            Math.nextDown(-0.21875),-0.21875,Math.nextUp(-0.21875)}) {
            Object function=constant.invoke(null,erosion);
            Object sampler=SAMPLER.getConstructor(FUNCTION,FUNCTION,FUNCTION,FUNCTION,FUNCTION,FUNCTION,List.class)
                .newInstance(zero,zero,zero,function,zero,zero,List.of());
            String selected=biome(call(source,Class.forName("den"),"getNoiseBiome",new Class<?>[]{int.class,int.class,int.class,SAMPLER},260,0,0,sampler));
            observed.add(Map.of("quart",new int[]{260,0,0},"erosion_point",new int[]{1048,0,8},"erosion_bits",Long.toUnsignedString(Double.doubleToRawLongBits(erosion)),"biome",selected));
        }
        return observed;
    }
    static String sha(Path file) throws Exception {return HexFormat.of().formatHex(MessageDigest.getInstance("SHA-256").digest(Files.readAllBytes(file)));}
    public static void main(String[] args) throws Exception {
        if(args.length!=2)throw new IllegalArgumentException("usage: WorldgenBiomeSourceOracle <output-directory> <oracle-source-file>");
        Path out=Path.of(args[0]);Files.createDirectories(out);
        Class<?> originalType=Class.forName("dzm",false,ClassLoader.getSystemClassLoader());
        if(!sha(Path.of(originalType.getProtectionDomain().getCodeSource().getLocation().toURI())).equals(INNER))throw new IllegalStateException("wrong original inner server");
        Class.forName("ab").getMethod("a").invoke(null);Class.forName("akt").getMethod("a").invoke(null);
        KEY=Class.forName("akq");LOCATION=Class.forName("akr");HOLDER=Class.forName("jm");PARAMETER=Class.forName("def$b");POINT=Class.forName("def$d");TARGET=Class.forName("def$h");LIST=Class.forName("def$c");STATE=Class.forName("dzm");SAMPLER=Class.forName("def$f");FUNCTION=Class.forName("dyr");CONTEXT=Class.forName("dyr$b");
        Object lookup=Class.forName("pd").getMethod("a").invoke(null);getters=Class.forName("jo$a").getMethod("b").invoke(lookup);
        presetRegistry=Class.forName("lu").getField("aZ").get(null);settingsRegistry=Class.forName("lu").getField("aO").get(null);biomeRegistry=Class.forName("lu").getField("aF").get(null);
        Map<String,Object> extracted=new LinkedHashMap<>();
        for(String name:new String[]{"minecraft:overworld","minecraft:nether"}) {
            Object list=Class.forName("dek").getMethod("a").invoke(value(holder(presetRegistry,name)));
            List<?> original=(List<?>)LIST.getMethod("a").invoke(list);presets.put(name,list);rows.put(name,original);
            extracted.put(name,Map.of("entries",entries(original)));
        }
        Map<String,Object> parameters=new LinkedHashMap<>();parameters.put("minecraft","1.21");parameters.put("protocol",767);parameters.put("source_inner_sha256",INNER);parameters.put("mappings_sha1",MAPPINGS);parameters.put("oracle_source_sha256",sha(Path.of(args[1])));parameters.put("observation","Actual original built-in MULTI_NOISE_BIOME_SOURCE_PARAMETER_LIST registry holders; ordered Climate.ParameterList.values and exact quantized-long parameterSpace endpoints, including point offset interval.");parameters.put("presets",extracted);
        Files.writeString(out.resolve("world-generation-multinoise-parameters.json"),JSON.toJson(parameters)+"\n");
        Map<String,Object> vectors=new LinkedHashMap<>();vectors.put("minecraft","1.21");vectors.put("source_inner_sha256",INNER);vectors.put("mappings_sha1",MAPPINGS);vectors.put("oracle_source_sha256",sha(Path.of(args[1])));vectors.put("observation","Unchanged original ParameterList/RTree and MultiNoiseBiomeSource searches. Index tokens use the same ordered original parameter objects in a parallel original ParameterList, checked against actual Holder biome results. Every sequence starts with a fresh original list owner to model a new worker search history.");vectors.put("sampler_scope","Actual original NoiseChunk.cachedClimateSampler with explicit empty Blender/beardifier, supplied to original sources. No natural BIOMES storage/status, blending retrogen or spawn-search result is claimed.");vectors.put("search_sequences",searchSequences());vectors.put("tie_sequences",ties());vectors.put("sample_sequences",sampleSequences());vectors.put("end_sequences",endSequences());vectors.put("end_thresholds",endThresholds());
        Files.writeString(out.resolve("world-generation-biome-source-vectors.json"),JSON.toJson(vectors)+"\n");
        OUT.println(JSON.toJson(Map.of("parameters",out.resolve("world-generation-multinoise-parameters.json").toString(),"vectors",out.resolve("world-generation-biome-source-vectors.json").toString(),"rows",Map.of("minecraft:overworld",rows.get("minecraft:overworld").size(),"minecraft:nether",rows.get("minecraft:nether").size()))));
    }
}
