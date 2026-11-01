package oracle;
import com.google.gson.*;
import java.lang.reflect.*;
import java.nio.file.*;
import java.util.*;
/** Read exact Player.createAttributes defaults without constructing a player. */
public final class player_nbt_attributes {
 public static void main(String[] args)throws Exception{
  Class.forName("ab").getMethod("a").invoke(null);Class.forName("akt").getMethod("a").invoke(null);
  Object builder=Class.forName("cmx").getMethod("fM").invoke(null);
  Object supplier=Class.forName("buv$a").getMethod("a").invoke(builder);
  Field f=Class.forName("buv").getDeclaredField("a");f.setAccessible(true);
  Object registry=Class.forName("lt").getField("s").get(null);
  JsonObject values=new JsonObject();
  for(Map.Entry<?,?> e:((Map<?,?>)f.get(supplier)).entrySet()){
   Object attribute=Class.forName("jm").getMethod("a").invoke(e.getKey());
   String name=Class.forName("jz").getMethod("b",Object.class).invoke(registry,attribute).toString();
   values.addProperty(name,(double)Class.forName("bus").getMethod("b").invoke(e.getValue()));
  }
  JsonObject out=new JsonObject();out.addProperty("minecraft","1.21");out.addProperty("data_version",3953);
  out.addProperty("oracle_inner_sha256","8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01");out.add("attributes",values);
  Files.writeString(Path.of(args[0]),new GsonBuilder().setPrettyPrinting().create().toJson(out)+"\n");
 }
}
