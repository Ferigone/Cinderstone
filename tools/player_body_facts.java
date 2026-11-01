package oracle;
import com.google.gson.Gson;
import java.lang.reflect.*;
import java.nio.file.*;
import java.util.*;
public final class player_body_facts {
 static Class<?> c(String n)throws Exception{return Class.forName(n);}
 static Object call(Object t,String n,Class<?>[] a,Object...v)throws Exception{Class<?> k=t instanceof Class<?>?(Class<?>)t:t.getClass();Method m=k.getMethod(n,a);m.setAccessible(true);return m.invoke(t instanceof Class<?>?null:t,v);}
 public static void main(String[]args)throws Exception{
  call(c("ab"),"a",new Class<?>[0]);call(c("akt"),"a",new Class<?>[0]);
  List<Boolean> render=new ArrayList<>();List<Integer> friction=new ArrayList<>();for(int id=0;id<26684;id++){Object s=call(c("dfy"),"a",new Class<?>[]{int.class},id),b=call(s,"b",new Class<?>[0]);friction.add(Float.floatToRawIntBits((float)call(b,"h",new Class<?>[0])));render.add(call(s,"l",new Class<?>[0])!=c("dmf").getField("a").get(null));}
  List<Boolean> inventory=new ArrayList<>();Object items=c("lt").getField("g").get(null);for(Object item:(Iterable<?>)items){Method m=item.getClass().getMethod("a",c("cuq"),c("dcw"),c("bsr"),int.class,boolean.class);inventory.add(m.getDeclaringClass()==c("cul"));}
  Object registry=c("lt").getField("b").get(null);Map<String,Integer>sounds=new TreeMap<>();for(String name:new String[]{"iU","us","ut","uu"})sounds.put(name,(int)call(registry,"a",new Class<?>[]{Object.class},c("avp").getField(name).get(null)));
  Files.writeString(Path.of(args[0]),new Gson().toJson(Map.of("minecraft","1.21","inner_sha256","8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01","friction_bits",friction,"sprint_render_visible",render,"inventory_tick_default",inventory,"sounds",sounds)));
 }
}
