package oracle;
import com.google.gson.*;import java.io.*;import java.lang.reflect.*;import java.util.*;import java.util.function.Consumer;
/** Executes actual ServerEntity.sendPairingData on actual ItemEntity. No packet
 * constructors/order are substituted; environmental level is from item oracle. */
public final class item_pairing_oracle {
 static Class<?> cls(String name)throws Exception{return Class.forName(name);}
 static JsonObject packet(Object packet)throws Exception {
  String name=packet.getClass().getName(),method,bufferType;int id;
  switch(name){case "abv":id=1;method="a";bufferType="wk";break;case "aez":id=0x58;method="b";bufferType="wk";break;case "afb":id=0x5a;method="a";bufferType="vw";break;default:throw new IllegalStateException(name);}
  Object bytes=cls("io.netty.buffer.Unpooled").getMethod("buffer").invoke(null);
  Object registry=cls("ka").getMethod("a",cls("jz")).invoke(null,cls("lt").getField("aA").get(null));
  Object buffer=cls("wk").getConstructor(cls("io.netty.buffer.ByteBuf"),cls("ka")).newInstance(bytes,registry);
  Method write=packet.getClass().getDeclaredMethod(method,cls(bufferType));write.setAccessible(true);write.invoke(packet,buffer);
  byte[] encoded=new byte[(Integer)cls("io.netty.buffer.ByteBuf").getMethod("readableBytes").invoke(bytes)];
  cls("io.netty.buffer.ByteBuf").getMethod("readBytes",byte[].class).invoke(bytes,(Object)encoded);cls("io.netty.buffer.ByteBuf").getMethod("release").invoke(bytes);
  JsonObject out=new JsonObject();out.addProperty("id",id);out.addProperty("payload",Base64.getEncoder().encodeToString(encoded));return out;
 }
 static void position(Object item,JsonObject pos)throws Exception {
  cls("bsr").getMethod("a_",double.class,double.class,double.class).invoke(item,pos.get("x").getAsDouble(),pos.get("y").getAsDouble(),pos.get("z").getAsDouble());
  cls("bsr").getMethod("t",float.class).invoke(item,pos.get("yaw").getAsFloat());cls("bsr").getMethod("u",float.class).invoke(item,pos.get("pitch").getAsFloat());
 }
 static void velocity(Object item,JsonArray v)throws Exception {cls("bsr").getMethod("n",double.class,double.class,double.class).invoke(item,v.get(0).getAsDouble(),v.get(1).getAsDouble(),v.get(2).getAsDouble());}
 public static void main(String[] args)throws Exception {
  item_entity_oracle.main(new String[]{"tracking"});
  BufferedReader input=new BufferedReader(new InputStreamReader(System.in));
  for(String line;(line=input.readLine())!=null;){JsonObject q=JsonParser.parseString(line).getAsJsonObject(),out=q.deepCopy();
   try{
    JsonObject itemQuery=new JsonObject();itemQuery.add("stack",q.get("stack"));itemQuery.addProperty("age",0);itemQuery.addProperty("pickup_delay",0);
    Object item=item_entity_oracle.plainItem(itemQuery,17);cls("bsr").getMethod("a_",UUID.class).invoke(item,UUID.fromString("00000000-0000-4000-8000-000000000001"));position(item,q.getAsJsonObject("position"));velocity(item,q.getAsJsonArray("velocity"));cls("bsr").getMethod("f",boolean.class).invoke(item,q.get("no_gravity").getAsBoolean());
    List<Object> packets=new ArrayList<>();Object tracker=cls("aqt").getConstructor(cls("aqu"),cls("bsr"),int.class,boolean.class,Consumer.class).newInstance(null,item,20,true,(Consumer<Object>)(packets::add));
    if(q.has("physical_position"))position(item,q.getAsJsonObject("physical_position"));if(q.has("physical_velocity"))velocity(item,q.getAsJsonArray("physical_velocity"));
    if(q.has("unsent_stack"))cls("cjh").getMethod("a",cls("cuq")).invoke(item,item_entity_oracle.codec.parse(item_entity_oracle.ops,q.get("unsent_stack")).getOrThrow());
    cls("aqt").getMethod("a",cls("aqv"),Consumer.class).invoke(tracker,null,(Consumer<Object>)(packets::add));
    JsonArray result=new JsonArray();for(Object value:packets)result.add(packet(value));out.add("packets",result);
   }catch(Throwable error){Throwable cause=error;while(cause instanceof InvocationTargetException&&cause.getCause()!=null)cause=cause.getCause();out.addProperty("error",cause.toString());}
   System.out.println("ORACLE:"+item_entity_oracle.G.toJson(out));
  }
 }
}
