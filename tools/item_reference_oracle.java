package oracle;
import com.google.gson.*;
import java.io.*;
import java.lang.reflect.*;
import java.util.*;

/** Original SynchedEntityData/ItemEntity setters and merges; reference identity
 * and in-place mutation are exercised on actual Java ItemStack objects. */
public final class item_reference_oracle {
 static final PrintStream OUT=System.out;
 static String lastWire;
 static Class<?> c(String n)throws Exception{return Class.forName(n);}
 static Object call(Object o,String name,Class<?>[]types,Object...args)throws Exception{return runtime_collision_oracle.call(o,name,types,args);}
 static int dirty(Object item)throws Exception{
  Object data=call(item,"ar",new Class<?>[]{}),values=call(data,"b",new Class<?>[]{});int mask=0;
  lastWire=null;
  if(values!=null){for(Object value:(List<?>)values)mask|=1<<(Integer)call(value,"a",new Class<?>[]{});
   Object packet=c("aez").getConstructor(int.class,List.class).newInstance((Integer)call(item,"an",new Class<?>[]{}),values);
   lastWire=item_entity_oracle.packetBytes(packet,"b");
  }return mask;
 }
 static Object make(JsonObject stack,int id)throws Exception{
  JsonObject q=new JsonObject();q.add("stack",stack);q.addProperty("age",0);q.addProperty("pickup_delay",0);
  return item_entity_oracle.plainItem(q,id);
 }
 static JsonObject state(Object item)throws Exception{
  JsonObject out=new JsonObject();Object stack=call(item,"p",new Class<?>[]{});
  out.addProperty("count",(Integer)call(stack,"H",new Class<?>[]{}));
  out.addProperty("removed",(Boolean)call(item,"dJ",new Class<?>[]{}));out.addProperty("dirty_mask",dirty(item));
  if(lastWire!=null)out.addProperty("dirty_wire",lastWire);return out;
 }
 public static void main(String[]args)throws Exception{
  item_entity_oracle.main(new String[]{"tracking"});Gson gson=new Gson();BufferedReader reader=new BufferedReader(new InputStreamReader(System.in));
  for(String line;(line=reader.readLine())!=null;){JsonObject q=JsonParser.parseString(line).getAsJsonObject();Object left=make(q.getAsJsonObject("left"),1);JsonObject out=new JsonObject();out.addProperty("constructor_dirty",dirty(left));String op=q.get("op").getAsString();
   if(op.equals("merge")){
    Object right=make(q.getAsJsonObject("right"),2);dirty(right);
    Method mergeable=c("cjh").getDeclaredMethod("H");mergeable.setAccessible(true);
    if((Boolean)mergeable.invoke(left)&&(Boolean)mergeable.invoke(right)){
     Method merge=c("cjh").getDeclaredMethod("a",c("cjh"));merge.setAccessible(true);merge.invoke(left,right);
    }
    out.add("right",state(right));
   }else{
    Object stack=call(left,"p",new Class<?>[]{});
    for(JsonElement e:q.getAsJsonArray("actions")){String action=e.getAsString();
     if(action.equals("same_reference"))call(left,"a",new Class<?>[]{c("cuq")},stack);
     else if(action.equals("equal_copy")){stack=call(stack,"s",new Class<?>[]{});call(left,"a",new Class<?>[]{c("cuq")},stack);}
     else if(action.equals("count_in_place"))call(stack,"e",new Class<?>[]{int.class},q.get("count").getAsInt());
     else if(action.equals("gravity_on"))call(left,"f",new Class<?>[]{boolean.class},true);
     else if(action.equals("gravity_off"))call(left,"f",new Class<?>[]{boolean.class},false);
     else throw new IllegalArgumentException(action);
    }
   }
   out.add("left",state(left));OUT.println("ORACLE:"+gson.toJson(out));
  }
 }
}
