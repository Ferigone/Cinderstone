package oracle;
import com.google.gson.*;
import java.lang.reflect.*;
import java.nio.file.*;
public final class world_neighbor_facts {
  static String declared(Class<?> c,Class<?>[] p)throws Exception {while(c!=null){try{return c.getDeclaredMethod("a",p).getDeclaringClass().getName();}catch(NoSuchMethodException ignored){c=c.getSuperclass();}}throw new NoSuchMethodException();}
  public static void main(String[] args)throws Exception {
    Class.forName("ab").getMethod("a").invoke(null);Class.forName("akt").getMethod("a").invoke(null);
    Class<?> state=Class.forName("dtc"),level=Class.forName("dcw"),pos=Class.forName("jd"),block=Class.forName("dfy");
    Class<?>[] parameters={state,level,pos,block,pos,boolean.class};JsonObject root=new JsonObject();JsonObject callbacks=new JsonObject();
    JsonObject catalogue=JsonParser.parseString(Files.readString(Path.of(args[0]))).getAsJsonObject();
    for(JsonElement value:catalogue.getAsJsonArray("blocks")){JsonObject b=value.getAsJsonObject();Object s=block.getMethod("a",int.class).invoke(null,b.get("default").getAsInt()),instance=state.getMethod("b").invoke(s);callbacks.addProperty(b.get("name").getAsString(),declared(instance.getClass(),parameters));}
    root.add("neighbor_changed",callbacks);JsonObject events=new JsonObject();Object registry=Class.forName("lt").getField("a").get(null);
    for(Object event:(Iterable<?>)registry){String name=Class.forName("jz").getMethod("b",Object.class).invoke(registry,event).toString();events.addProperty(name,(int)event.getClass().getMethod("a").invoke(event));}root.add("game_event_radius",events);
    Object[] order=(Object[])Class.forName("eqi").getField("a").get(null);JsonArray directions=new JsonArray();for(Object direction:order)directions.add(direction.toString());root.add("update_order",directions);
    Files.writeString(Path.of(args[1]),new GsonBuilder().setPrettyPrinting().create().toJson(root)+"\n");
  }
}
