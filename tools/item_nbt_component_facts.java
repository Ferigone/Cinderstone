package oracle;
import com.google.gson.*;
import java.nio.file.*;
/** Read-only bootstrap facts from the unmodified official 1.21 registry. */
public final class item_nbt_component_facts {
  public static void main(String[] args) throws Exception {
    Class.forName("ab").getMethod("a").invoke(null);
    Class.forName("akt").getMethod("a").invoke(null);
    Object registry=Class.forName("lt").getField("aq").get(null);
    Class<?> type=Class.forName("kp"), registryType=Class.forName("jz");
    JsonArray entries=new JsonArray();
    for(Object value:(Iterable<?>)registry){
      JsonObject item=new JsonObject();
      item.addProperty("id",(int)registryType.getMethod("a",Object.class).invoke(registry,value));
      item.addProperty("name",registryType.getMethod("b",Object.class).invoke(registry,value).toString());
      item.addProperty("persistent",type.getMethod("b").invoke(value)!=null);
      entries.add(item);
    }
    JsonObject result=new JsonObject();result.addProperty("minecraft","1.21");
    result.addProperty("oracle_inner_sha256","8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01");
    result.add("components",entries);
    Files.writeString(Path.of(args[0]),new GsonBuilder().setPrettyPrinting().create().toJson(result)+"\n");
  }
}
