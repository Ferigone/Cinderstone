import java.io.*;import java.lang.reflect.*;import java.util.*;
import com.google.gson.*;import io.netty.buffer.*;
/** Unchanged 1.21 Player.defineSynchedData, SynchedEntityData and metadata serializers. */
class metadata_oracle {
 static final Gson JSON=new Gson();static final ajw<?>[] KEYS=new ajw<?>[21];static ka registry;static aka data;static Object previous;
 static class Holder implements ajz {public void a(ajw<?> key){}public void a(List<aka.c<?>> values){}}
 static Object field(Class<?> c,String name,Object owner)throws Exception{Field f=c.getDeclaredField(name);f.setAccessible(true);return f.get(owner);}
 static void set(Object o,Class<?> c,String name,Object value)throws Exception{Field f=c.getDeclaredField(name);f.setAccessible(true);f.set(o,value);}
 static void keys(Class<?> c,String...names)throws Exception{for(String name:names){ajw<?> key=(ajw<?>)field(c,name,null);KEYS[key.a()]=key;}}
 @SuppressWarnings({"rawtypes","unchecked"})static void assign(int id,Object value,boolean force){data.a((ajw)KEYS[id],value,force);}
 static String packet(List<aka.c<?>> entries)throws Exception{
  if(entries==null)return null;ByteBuf bytes=Unpooled.buffer();wk buffer=new wk(bytes,registry);aez p=new aez(17,entries);
  Method m=aez.class.getDeclaredMethod("b",wk.class);m.setAccessible(true);m.invoke(p,buffer);byte[] encoded=new byte[bytes.readableBytes()];bytes.readBytes(encoded);bytes.release();return Base64.getEncoder().encodeToString(encoded);
 }
 @SuppressWarnings({"rawtypes","unchecked"})public static void main(String[]args)throws Exception{
  ab.a();akt.a();registry=ka.a(lt.aA);
  Class<?> entity=bsr.class,living=btn.class,player=cmx.class;
  keys(entity,"ap","aP","aQ","aR","aS","aT","at","aU");keys(living,"aE","bM","bN","bO","bP","bQ","bR");keys(player,"e","f","bX","bY","bZ","ca");
  Field uf=sun.misc.Unsafe.class.getDeclaredField("theUnsafe");uf.setAccessible(true);sun.misc.Unsafe unsafe=(sun.misc.Unsafe)uf.get(null);
  aqv instance=(aqv)unsafe.allocateInstance(aqv.class);BufferedReader reader=new BufferedReader(new InputStreamReader(System.in));
  for(String line;(line=reader.readLine())!=null;){
   JsonObject q=JsonParser.parseString(line).getAsJsonObject();String op=q.get("op").getAsString();boolean changed=false;
   if(op.equals("new")){
    aka.a builder=new aka.a(instance);
    Object[] base={Byte.valueOf((byte)0),Integer.valueOf(300),Optional.empty(),Boolean.FALSE,Boolean.FALSE,Boolean.FALSE,bua.a,Integer.valueOf(0)};
    for(int id=0;id<8;id++)builder.a((ajw)KEYS[id],base[id]);
    Method define=cmx.class.getDeclaredMethod("a",aka.a.class);define.setAccessible(true);define.invoke(instance,builder);data=builder.a();set(data,aka.class,"d",new Holder());
   }else if(op.equals("set")||op.equals("reuse")){
    int id=q.get("id").getAsInt();Object value=previous;
    if(op.equals("set")){ByteBuf bytes=Unpooled.wrappedBuffer(Base64.getDecoder().decode(q.get("payload").getAsString()));wk buffer=new wk(bytes,registry);value=KEYS[id].b().codec().decode(buffer);if(bytes.readableBytes()!=0)throw new IllegalStateException("Trailing metadata input");bytes.release();previous=value;}
    Object old=data.a((ajw)KEYS[id]);changed=q.get("force").getAsBoolean()||!Objects.equals(old,value);assign(id,value,q.get("force").getAsBoolean());
   }
   JsonObject out=new JsonObject();out.addProperty("changed",changed);out.addProperty("is_dirty",data.a());out.addProperty("snapshot",packet(data.c()));
   if(op.equals("drain"))out.addProperty("dirty",packet(data.b()));
   JsonArray schema=new JsonArray();if(op.equals("new"))for(int id=0;id<21;id++){JsonObject f=new JsonObject();f.addProperty("id",id);f.addProperty("serializer",ajy.b(KEYS[id].b()));schema.add(f);}out.add("schema",schema);
   System.out.println("ORACLE:"+JSON.toJson(out));
  }
 }
}
