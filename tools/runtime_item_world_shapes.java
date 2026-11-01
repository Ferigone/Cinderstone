package oracle;
import com.google.gson.*;
import java.nio.file.*;

/** Original BlockState.isCollisionShapeFullBlock values for escape queries. */
public final class runtime_item_world_shapes {
 public static void main(String[]args)throws Exception {
  fluid_physics_facts.call(Class.forName("ab"),"a",new Class<?>[]{});
  fluid_physics_facts.call(Class.forName("akt"),"a",new Class<?>[]{});
  Object world=Class.forName("dcl").getField("a").get(null),pos=Class.forName("jd").getConstructor(int.class,int.class,int.class).newInstance(0,0,0);JsonArray full=new JsonArray();
  for(int id=0;id<26684;id++){Object state=fluid_physics_facts.call(Class.forName("dfy"),"a",new Class<?>[]{int.class},id);full.add((boolean)fluid_physics_facts.call(state,"r",new Class<?>[]{Class.forName("dcc"),Class.forName("jd")},world,pos));}
  JsonObject out=new JsonObject();out.addProperty("minecraft","1.21");out.addProperty("oracle_inner_sha256","8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01");out.add("full",full);Files.writeString(Path.of(args[0]),new Gson().toJson(out)+"\n");System.out.println("ORACLE:"+full.size()+" full-collision values; dynamic entries require live context");
 }
}
