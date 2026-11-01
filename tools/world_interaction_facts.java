package oracle;
import com.google.gson.*;
import java.lang.reflect.*;
import java.nio.file.*;
public final class world_interaction_facts {
  static Object field(Object object,String name)throws Exception {Class<?> c=object.getClass();while(c!=null){try{Field f=c.getDeclaredField(name);f.setAccessible(true);return f.get(object);}catch(NoSuchFieldException ignored){c=c.getSuperclass();}}throw new NoSuchFieldException(name);}
  public static void main(String[] args)throws Exception {
    Class.forName("ab").getMethod("a").invoke(null);Class.forName("akt").getMethod("a").invoke(null);
    JsonArray facts=new JsonArray();Object registry=Class.forName("lt").getField("b").get(null);
    Method soundId=Class.forName("jz").getMethod("a",Object.class),soundName=Class.forName("jz").getMethod("b",Object.class);
    JsonObject catalogue=JsonParser.parseString(Files.readString(Path.of(args[0]))).getAsJsonObject();
    for(JsonElement value:catalogue.getAsJsonArray("blocks")) {JsonObject block=value.getAsJsonObject();String cls=block.get("placement_class").getAsString();if(!java.util.Set.of("die","dom","dix").contains(cls))continue;
      int state=block.get("default").getAsInt();Object s=Class.forName("dfy").getMethod("a",int.class).invoke(null,state),b=Class.forName("dtc").getMethod("b").invoke(s);Field tf=Class.forName(cls).getDeclaredField(cls.equals("die")?"l":cls.equals("dom")?"m":"G");tf.setAccessible(true);Object type=tf.get(b);
      String close=cls.equals("die")?"h":cls.equals("dom")?"j":"f",open=cls.equals("die")?"i":cls.equals("dom")?"k":"g";Object cs=type.getClass().getMethod(close).invoke(type),os=type.getClass().getMethod(open).invoke(type);
      JsonObject row=new JsonObject();row.addProperty("name",block.get("name").getAsString());row.addProperty("can_open_by_hand",cls.equals("dix")||(boolean)type.getClass().getMethod("c").invoke(type));row.addProperty("open_sound_id",(int)soundId.invoke(registry,os));row.addProperty("close_sound_id",(int)soundId.invoke(registry,cs));row.addProperty("open_sound",soundName.invoke(registry,os).toString());row.addProperty("close_sound",soundName.invoke(registry,cs).toString());facts.add(row);
    }
    Files.writeString(Path.of(args[1]),new GsonBuilder().setPrettyPrinting().create().toJson(facts)+"\n");
    JsonArray placements=new JsonArray();for(int i=0;i<26684;i++){Object st=Class.forName("dfy").getMethod("a",int.class).invoke(null,i);Object ty=Class.forName("dtc").getMethod("w").invoke(st);JsonArray row=new JsonArray();row.add((int)soundId.invoke(registry,ty.getClass().getMethod("e").invoke(ty)));row.add((float)ty.getClass().getMethod("a").invoke(ty));row.add((float)ty.getClass().getMethod("b").invoke(ty));placements.add(row);}Files.writeString(Path.of(args[2]),new Gson().toJson(placements)+"\n");
  }
}
