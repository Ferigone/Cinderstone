package oracle;
import com.google.gson.*;
import java.io.*;import java.nio.file.*;
/** Calls unchanged1.21 Inventory.add; only the enclosing player's environmental
 * creative/level/equipment hooks use the existing container oracle stubs. */
public final class player_inventory_oracle {
 static Class<?> cls(String name)throws Exception{return Class.forName(name);}
 public static void main(String[] args)throws Exception {
  Path bootstrap=Files.createTempFile("mc121-pickup-bootstrap",".json");
  try{container_oracle.main(new String[]{"equipment",bootstrap.toString()});}finally{Files.delete(bootstrap);}
  BufferedReader input=new BufferedReader(new InputStreamReader(System.in));
  for(String line;(line=input.readLine())!=null;){
   JsonObject q=JsonParser.parseString(line).getAsJsonObject(),out=new JsonObject();out.add("name",q.get("name"));
   try{
    Object player=container_oracle.newPlayer(q.get("creative").getAsBoolean());
    Object inventory=cls("cmx").getMethod("fY").invoke(player);
    Object menu=container_oracle.menuClass.getConstructor(cls("cmw"),boolean.class,cls("cmx")).newInstance(inventory,true,player);
    container_oracle.set(player,cls("cmx"),"cc",menu);container_oracle.set(player,cls("cmx"),"cd",menu);
    cls("cmw").getField("k").setInt(inventory,q.get("selected").getAsInt());
    var slots=(java.util.List<?>)cls("cpu").getField("i").get(menu);
    var initial=q.getAsJsonArray("slots");for(int i=0;i<46;i++)cls("crq").getMethod("f",cls("cuq")).invoke(slots.get(i),container_oracle.parse(initial.get(i)));
    Object pickup=container_oracle.parse(q.get("pickup"));
    out.addProperty("accepted",(boolean)cls("cmw").getMethod("f",cls("cuq")).invoke(inventory,pickup));
    out.add("remaining",container_oracle.encode(pickup));
    out.add("state",container_oracle.snapshot(menu));
   }catch(Throwable error){Throwable cause=error instanceof java.lang.reflect.InvocationTargetException?error.getCause():error;out.addProperty("error",cause.toString());}
   System.out.println("ORACLE:"+container_oracle.GSON.toJson(out));
  }
 }
}
