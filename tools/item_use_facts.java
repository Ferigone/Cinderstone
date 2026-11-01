package oracle;
import com.google.gson.Gson;
import java.lang.reflect.*;
import java.nio.file.*;
import java.util.*;
/** Exact original dispatch ownership; data extraction, not a behavior oracle. */
public final class item_use_facts {
 static Class<?> c(String n)throws Exception{return Class.forName(n);}
 static Object call(Object o,String n,Class<?>[] p,Object...a)throws Exception{Class<?> k=o instanceof Class<?>?(Class<?>)o:o.getClass();Method m=k.getMethod(n,p);m.setAccessible(true);return m.invoke(o instanceof Class<?>?null:o,a);}
 static String owner(Object o,String n,Class<?>...p)throws Exception{Class<?> k=o.getClass();while(k!=null){try{return k.getDeclaredMethod(n,p).getDeclaringClass().getName();}catch(NoSuchMethodException e){k=k.getSuperclass();}}throw new NoSuchMethodException(n);}
 public static void main(String[] args)throws Exception {
  call(c("ab"),"a",new Class<?>[0]);call(c("akt"),"a",new Class<?>[0]);
  Object sounds=c("lt").getField("b").get(null);
  List<Object> rows=new ArrayList<>();
  for(int id=0;id<1333;id++){
   Object item=call(c("cul"),"b",new Class<?>[]{int.class},id);
   rows.add(Arrays.asList(owner(item,"a",c("dcw"),c("cmx"),c("bqq")),owner(item,"a",c("cuq"),c("dcw"),c("btn")),owner(item,"a",c("dcw"),c("btn"),c("cuq"),int.class),owner(item,"a",c("cuq"),c("dcw"),c("btn"),int.class),(int)call(sounds,"a",new Class<?>[]{Object.class},call(item,"ap_",new Class<?>[0])),(int)call(sounds,"a",new Class<?>[]{Object.class},call(item,"aq_",new Class<?>[0]))));
  }
  Map<String,Integer> soundIds=new TreeMap<>();for(String n:new String[]{"uj","mD","kc","kd"})soundIds.put(n,(int)call(sounds,"a",new Class<?>[]{Object.class},c("avp").getField(n).get(null)));
  Map<String,Integer> events=new TreeMap<>();for(String[] row:new String[][]{{"D","minecraft:item_interact_start"},{"C","minecraft:item_interact_finish"},{"m","minecraft:eat"}})events.put(row[1],(int)call(call(c("dxz").getField(row[0]).get(null),"a",new Class<?>[0]),"a",new Class<?>[0]));
  Map<String,Object> out=new LinkedHashMap<>();out.put("minecraft","1.21");out.put("jar_sha256","8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01");out.put("row_contract","useOwner,finishOwner,onUseTickOwner,releaseOwner,drinkSound,eatSound");out.put("items",rows);out.put("sounds",soundIds);out.put("game_event_radii",events);Files.writeString(Path.of(args[0]),new Gson().toJson(out)+"\n");System.out.println("ITEM_USE_FACTS items="+rows.size()+" sounds="+soundIds);
 }
}
