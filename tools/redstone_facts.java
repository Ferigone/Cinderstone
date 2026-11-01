package oracle;
import com.google.gson.*;import java.lang.reflect.*;import java.nio.file.*;
public class redstone_facts {
 static Class<?> c(String s)throws Exception{return Class.forName(s);}
 static Method m(Class<?> type,String n,Class<?>...p)throws Exception{for(Class<?> t=type;t!=null;t=t.getSuperclass()){try{Method m=t.getDeclaredMethod(n,p);m.setAccessible(true);return m;}catch(NoSuchMethodException e){}}throw new NoSuchMethodException(n);}
 static Object f(Object v,String n)throws Exception{Field f=v.getClass().getDeclaredField(n);f.setAccessible(true);return f.get(v);}
 static int sound(Object v)throws Exception{return (int)c("jz").getMethod("a",Object.class).invoke(c("lt").getField("b").get(null),v);}
 public static void main(String[] a)throws Exception{
 c("ab").getMethod("a").invoke(null);c("akt").getMethod("a").invoke(null);
 JsonObject root=new JsonObject();root.addProperty("minecraft","1.21");root.addProperty("jar_sha256","8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01");
 root.addProperty("lever_sound",sound(c("avp").getField("nU").get(null)));JsonArray rows=new JsonArray();
 for(JsonElement e:JsonParser.parseString(Files.readString(Path.of(a[0]))).getAsJsonObject().getAsJsonArray("blocks")){
 JsonObject in=e.getAsJsonObject(),o=new JsonObject();for(String k:new String[]{"name","min","max","class"})o.add(k,in.get(k));Object state=c("dfy").getMethod("a",int.class).invoke(null,in.get("default").getAsInt());Object block=c("dtc").getMethod("b").invoke(state);Class<?> bc=block.getClass();
 for(String n:new String[]{"a","b"})o.addProperty(n.equals("a")?"signal":"direct",m(bc,n,c("dtc"),c("dcc"),c("jd"),c("ji")).getDeclaringClass().getName());
 o.addProperty("neighbor",m(bc,"a",c("dtc"),c("dcw"),c("jd"),c("dfy"),c("jd"),boolean.class).getDeclaringClass().getName());
 o.addProperty("on_place",m(bc,"b",c("dtc"),c("dcw"),c("jd"),c("dtc"),boolean.class).getDeclaringClass().getName());
 o.addProperty("on_remove",m(bc,"a",c("dtc"),c("dcw"),c("jd"),c("dtc"),boolean.class).getDeclaringClass().getName());
 if(c("dgi").isInstance(block)){Object type=f(block,"N");o.addProperty("pressed_time",(int)f(block,"O"));o.addProperty("arrows",(boolean)c("dtr").getMethod("e").invoke(type));o.addProperty("on_sound",sound(c("dtr").getMethod("o").invoke(type)));o.addProperty("off_sound",sound(c("dtr").getMethod("n").invoke(type)));}
 if(c("dfm").isInstance(block)){Field d=c("dfm").getDeclaredField("d");d.setAccessible(true);Object type=d.get(block);o.addProperty("pressed_time",(int)m(bc,"b").invoke(block));o.addProperty("mobs_only",c("dtr").getMethod("f").invoke(type)==c("dtr$a").getField("b").get(null));o.addProperty("on_sound",sound(c("dtr").getMethod("m").invoke(type)));o.addProperty("off_sound",sound(c("dtr").getMethod("l").invoke(type)));if(c("dpp").isInstance(block))o.addProperty("max_weight",(int)f(block,"g"));}
 rows.add(o);
 }
 root.add("blocks",rows);Files.writeString(Path.of(a[1]),new Gson().toJson(root)+"\n");System.out.println("REDSTONE_FACTS "+rows.size());
 }
}
