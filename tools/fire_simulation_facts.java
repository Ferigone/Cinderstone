package oracle;

import com.google.gson.*;
import java.lang.reflect.*;
import java.nio.file.*;
import java.util.*;

/** Actual FireBlock odds after vanilla bootstrap, including wet-state zeroes. */
public final class fire_simulation_facts {
    static Class<?> c(String n)throws Exception{return Class.forName(n);}
    static Object call(Object target,String name,Class<?>[] types,Object...args)throws Exception{
        Class<?> type=target instanceof Class<?>?(Class<?>)target:target.getClass();
        Method m=type.getMethod(name,types);m.setAccessible(true);return m.invoke(target instanceof Class<?>?null:target,args);
    }
    public static void main(String[] args)throws Exception{
        call(c("ab"),"a",new Class<?>[0]);call(c("akt"),"a",new Class<?>[0]);
        Object normal=c("dga").getField("cr").get(null),soul=c("dga").getField("cs").get(null);
        Method burn=c("diy").getDeclaredMethod("n",c("dtc")),ignite=c("diy").getDeclaredMethod("o",c("dtc"));burn.setAccessible(true);ignite.setAccessible(true);
        List<Integer> states=new ArrayList<>();List<List<Object>> rows=new ArrayList<>();Map<String,Integer> unique=new LinkedHashMap<>();
        for(int id=0;id<26684;id++){
            Object state=call(c("dfy"),"a",new Class<?>[]{int.class},id),block=call(state,"b",new Class<?>[0]);
            List<Object> row=Arrays.asList((int)burn.invoke(normal,state),(int)ignite.invoke(normal,state),(boolean)call(state,"i",new Class<?>[0]),c("doi").isInstance(block),block==normal?1:block==soul?2:0,(boolean)call(state,"v",new Class<?>[0]),(boolean)call(state,"t",new Class<?>[0]),call(block,"v",new Class<?>[0])==c("ern").getField("a").get(null));
            String key=new Gson().toJson(row);Integer index=unique.get(key);if(index==null){index=rows.size();unique.put(key,index);rows.add(row);}states.add(index);
        }
        Map<String,Integer> ids=new TreeMap<>();for(String name:new String[]{"a","cr","cs","co","ed"}){
            try{Object block=c("dga").getField(name).get(null);ids.put(name,(int)call(c("dfy"),"i",new Class<?>[]{c("dtc")},call(block,"o",new Class<?>[0])));}catch(NoSuchFieldException ignored){}
        }
        JsonObject biome=JsonParser.parseString(new String(c("diy").getResourceAsStream("/data/minecraft/tags/worldgen/biome/increased_fire_burnout.json").readAllBytes(),java.nio.charset.StandardCharsets.UTF_8)).getAsJsonObject();
        JsonObject dimension=JsonParser.parseString(new String(c("diy").getResourceAsStream("/data/minecraft/dimension_type/overworld.json").readAllBytes(),java.nio.charset.StandardCharsets.UTF_8)).getAsJsonObject();
        Map<String,Object> out=new LinkedHashMap<>();out.put("minecraft","1.21");out.put("jar_sha256","8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01");out.put("row_contract","burnOdds,igniteOdds,isAir,isTnt,fireKind0none1normal2soul,isRandomlyTicking,hasBlockEntity,lootTableIsBuiltinEmpty");out.put("states",states);out.put("rows",rows);out.put("block_ids",ids);out.put("increased_burnout_biomes",biome.getAsJsonArray("values"));out.put("overworld_infiniburn",dimension.get("infiniburn").getAsString());Files.writeString(Path.of(args[0]),new Gson().toJson(out)+"\n");System.out.println("FIRE_FACTS states="+states.size()+" rows="+rows.size()+" ids="+ids);
    }
}
