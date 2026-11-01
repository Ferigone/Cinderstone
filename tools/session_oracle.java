import java.io.*;
import java.lang.reflect.*;
import java.util.*;
import com.google.gson.*;
import com.mojang.authlib.GameProfile;
/** Controlled clock, unchanged vanilla common-listener tick/receive methods. */
class session_oracle {
 static final Gson JSON=new Gson();static long now;
 static class Listener extends arr {
  boolean owner,disconnected;List<Long> sent=new ArrayList<>();
  Listener(){super(null,null,null);}
  public vu b(){return null;}
  public boolean c(){return true;}
  public void a(aaa p){}
  protected GameProfile i(){return null;}
  protected boolean h(){return owner;}
  public void a(wz reason){disconnected=true;}
  public void b(zg<?> packet){sent.add(((zq)packet).b());}
  void tick(){e();}
 }
 static void set(Object object,Class<?> type,String name,Object value)throws Exception{Field f=type.getDeclaredField(name);f.setAccessible(true);f.set(object,value);}
 public static void main(String[]args)throws Exception{
  ab.a();akt.a();ad.c=()->now*1_000_000;
  Field field=sun.misc.Unsafe.class.getDeclaredField("theUnsafe");field.setAccessible(true);sun.misc.Unsafe unsafe=(sun.misc.Unsafe)field.get(null);
  net.minecraft.server.MinecraftServer server=(net.minecraft.server.MinecraftServer)unsafe.allocateInstance(apn.class);
  set(server,net.minecraft.server.MinecraftServer.class,"A",bnc.a);
  Listener listener=null;BufferedReader reader=new BufferedReader(new InputStreamReader(System.in));
  for(String line;(line=reader.readLine())!=null;){
   JsonObject q=JsonParser.parseString(line).getAsJsonObject();now=q.get("now").getAsLong();String op=q.get("op").getAsString();
   if(op.equals("new")) {listener=(Listener)unsafe.allocateInstance(Listener.class);listener.sent=new ArrayList<>();listener.owner=q.get("owner").getAsBoolean();set(listener,arr.class,"d",server);set(listener,arr.class,"j",now);set(listener,arr.class,"o",q.get("latency").getAsInt());}
   listener.disconnected=false;listener.sent.clear();
   if(op.equals("tick"))listener.tick();
   if(op.equals("receive"))listener.a(new aac(q.get("id").getAsLong()));
   if(op.equals("close")){Method m=arr.class.getDeclaredMethod("l");m.setAccessible(true);m.invoke(listener);}
   JsonObject out=new JsonObject();out.addProperty("latency",listener.k());out.addProperty("timeout",listener.disconnected);JsonArray sent=new JsonArray();for(long id:listener.sent)sent.add(id);out.add("sent",sent);System.out.println("ORACLE:"+JSON.toJson(out));
  }
 }
}
