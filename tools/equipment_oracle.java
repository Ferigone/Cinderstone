import java.io.*;import java.lang.reflect.*;import java.util.*;
import com.google.gson.*;import io.netty.buffer.*;
/** Exact LivingEntity equipment comparison and packet handlers, without attribute/enchantment callbacks. */
class equipment_oracle {
 static final Gson JSON=new Gson();static final List<Object> PACKETS=new ArrayList<>();static ka registry;
 static class Cache extends aqs {
  Cache(){super(null,null,null,null,null,null,0,0,false,null,null,null);}
  public void b(bsr entity,zg<?> packet){PACKETS.add(packet);}
 }
 static class Player extends cmx {
  Player(){super(null,jd.c,0,null);}
  public boolean R_(){return false;}public boolean f(){return false;}public int an(){return 17;}
 }
 static void set(Object o,Class<?> c,String name,Object value)throws Exception{Field f=c.getDeclaredField(name);f.setAccessible(true);f.set(o,value);}
 static Object get(Object o,Class<?> c,String name)throws Exception{Field f=c.getDeclaredField(name);f.setAccessible(true);return f.get(o);}
 static cuq stack(String encoded){ByteBuf bytes=Unpooled.wrappedBuffer(Base64.getDecoder().decode(encoded));cuq stack=cuq.h.decode(new wk(bytes,registry));if(bytes.readableBytes()!=0)throw new IllegalStateException("Trailing Slot data");bytes.release();return stack;}
 static JsonObject encode(Object packet)throws Exception{
  ByteBuf bytes=Unpooled.buffer();int id;Method m;Object buffer;
  if(packet instanceof afc){id=0x5b;m=afc.class.getDeclaredMethod("a",wk.class);buffer=new wk(bytes,registry);}else{id=0x1f;m=acy.class.getDeclaredMethod("a",vw.class);buffer=new vw(bytes);}
  m.setAccessible(true);m.invoke(packet,buffer);byte[] data=new byte[bytes.readableBytes()];bytes.readBytes(data);bytes.release();JsonObject out=new JsonObject();out.addProperty("id",id);out.addProperty("payload",Base64.getEncoder().encodeToString(data));return out;
 }
 public static void main(String[]args)throws Exception{
  ab.a();akt.a();registry=ka.a(lt.aA);Field field=sun.misc.Unsafe.class.getDeclaredField("theUnsafe");field.setAccessible(true);sun.misc.Unsafe unsafe=(sun.misc.Unsafe)field.get(null);
  Player player=(Player)unsafe.allocateInstance(Player.class);aqu world=(aqu)unsafe.allocateInstance(aqu.class);Cache cache=(Cache)unsafe.allocateInstance(Cache.class);set(world,aqu.class,"I",cache);set(player,bsr.class,"r",world);
  Method hand=btn.class.getDeclaredMethod("f",bsy.class);hand.setAccessible(true);Method armor=btn.class.getDeclaredMethod("e",bsy.class);armor.setAccessible(true);
  Method swap=btn.class.getDeclaredMethod("a",Map.class);swap.setAccessible(true);Method equipment=btn.class.getDeclaredMethod("b",Map.class);equipment.setAccessible(true);
  BufferedReader input=new BufferedReader(new InputStreamReader(System.in));for(String line;(line=input.readLine())!=null;){
   JsonObject q=JsonParser.parseString(line).getAsJsonObject();PACKETS.clear();JsonArray changed=new JsonArray();
   if(q.get("op").getAsString().equals("new")){set(player,btn.class,"bX",jv.a(2,cuq.l));set(player,btn.class,"bY",jv.a(4,cuq.l));set(player,btn.class,"bZ",cuq.l);}
   else{
    EnumMap<bsy,cuq> changes=new EnumMap<>(bsy.class);JsonArray slots=q.getAsJsonArray("slots");
    for(bsy slot:bsy.values()){cuq previous=switch(slot.a()){case a->(cuq)hand.invoke(player,slot);case b->(cuq)armor.invoke(player,slot);case c->(cuq)get(player,btn.class,"bZ");};cuq current=stack(slots.get(slot.ordinal()).getAsString());if(player.a(previous,current)){changes.put(slot,current);changed.add(slot.ordinal());}}
    if(!changes.isEmpty()){swap.invoke(player,changes);if(!changes.isEmpty())equipment.invoke(player,changes);}
   }
   JsonObject out=new JsonObject();out.add("changes",changed);JsonArray packets=new JsonArray();for(Object p:PACKETS)packets.add(encode(p));out.add("packets",packets);System.out.println("ORACLE:"+JSON.toJson(out));
  }
 }
}
