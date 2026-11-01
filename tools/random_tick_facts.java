package oracle;
import com.google.gson.*;
import java.lang.reflect.*;
import java.nio.file.*;
public final class random_tick_facts {
  static String declaring(Class<?> c,String name,Class<?>... p)throws Exception {while(c!=null){try{return c.getDeclaredMethod(name,p).getDeclaringClass().getName();}catch(NoSuchMethodException ignored){c=c.getSuperclass();}}throw new NoSuchMethodException(name);}
  public static void main(String[] args)throws Exception {
    Class.forName("ab").getMethod("a").invoke(null);Class.forName("akt").getMethod("a").invoke(null);
    Class<?> block=Class.forName("dfy"),state=Class.forName("dtc"),level=Class.forName("dcw"),server=Class.forName("aqu"),pos=Class.forName("jd"),random=Class.forName("ayw"),fluidState=Class.forName("epe");
    JsonObject root=new JsonObject();root.addProperty("minecraft","1.21");root.addProperty("jar_sha256","8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01");
    JsonArray flags=new JsonArray(),fluidClasses=new JsonArray();
    for(int i=0;i<26684;i++){Object s=block.getMethod("a",int.class).invoke(null,i),fs=state.getMethod("u").invoke(s),ft=fluidState.getMethod("a").invoke(fs);int bits=((boolean)state.getMethod("v").invoke(s)?1:0)|((boolean)fluidState.getMethod("f").invoke(fs)?2:0)|(!(boolean)fluidState.getMethod("c").invoke(fs)?4:0)|(!(boolean)state.getMethod("i").invoke(s)?8:0);flags.add(bits);fluidClasses.add((bits&2)==0?"":declaring(ft.getClass(),"b",level,pos,fluidState,random));}
    root.add("state_flags",flags);root.add("fluid_random_classes",fluidClasses);JsonArray blocks=new JsonArray();
    JsonObject catalogue=JsonParser.parseString(Files.readString(Path.of(args[0]))).getAsJsonObject();
    for(JsonElement value:catalogue.getAsJsonArray("blocks")){JsonObject b=value.getAsJsonObject();Object s=block.getMethod("a",int.class).invoke(null,b.get("default").getAsInt()),instance=state.getMethod("b").invoke(s);JsonObject row=new JsonObject();for(String key:new String[]{"name","min","max","default"})row.add(key,b.get(key));row.addProperty("random_tick_class",declaring(instance.getClass(),"b",state,server,pos,random));row.addProperty("precipitation_class",declaring(instance.getClass(),"a",state,level,pos,Class.forName("ddw$c")));blocks.add(row);}
    root.add("blocks",blocks);Files.writeString(Path.of(args[1]),new Gson().toJson(root)+"\n");
  }
}
