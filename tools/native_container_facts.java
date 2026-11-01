package oracle;
import com.google.gson.*;
import java.lang.reflect.*;
import java.nio.file.*;
public final class native_container_facts {
 public static void main(String[] a)throws Exception{
  Class.forName("ab").getMethod("a").invoke(null);Class.forName("akt").getMethod("a").invoke(null);
  Object registry=Class.forName("lt").getField("b").get(null);Method id=Class.forName("jz").getMethod("a",Object.class),key=Class.forName("jz").getMethod("b",Object.class);JsonObject sounds=new JsonObject();
  for(String field:new String[]{"eQ","eO","bz","by"}){Object sound=Class.forName("avp").getField(field).get(null);sounds.addProperty(key.invoke(registry,sound).toString(),(int)id.invoke(registry,sound));}
  JsonObject blocks=new JsonObject();Object br=Class.forName("lt").getField("e").get(null);for(int state:new int[]{2955,18409}){Object st=Class.forName("dfy").getMethod("a",int.class).invoke(null,state);Object block=Class.forName("dtc").getMethod("b").invoke(st);blocks.addProperty(key.invoke(br,block).toString(),(int)id.invoke(br,block));}JsonObject root=new JsonObject();root.add("block_ids",blocks);root.addProperty("minecraft","1.21");root.addProperty("oracle_inner_sha256","8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01");root.add("sounds",sounds);root.addProperty("chest_type_id",1);root.addProperty("barrel_type_id",26);root.addProperty("opener_recheck_delay",5);root.addProperty("update_tag_owner","dqh:emptyCompound");Files.writeString(Path.of(a[0]),new GsonBuilder().setPrettyPrinting().create().toJson(root)+"\n");
 }
}
