package oracle;
import java.nio.file.*;
import java.util.*;
import com.google.gson.*;

/** Unchanged original compressed NbtIo edits/inspection on an isolated world. */
public final class main_world_nbt_oracle {
 static Class<?> C(String name)throws Exception{return Class.forName(name);}
 static Object call(Object target,String name,Class<?>[]types,Object...args)throws Exception {
  Class<?> type=target instanceof Class<?>?(Class<?>)target:target.getClass();
  return type.getMethod(name,types).invoke(target instanceof Class<?>?null:target,args);
 }
 public static void main(String[]args)throws Exception {
  Path file=Path.of(args[1]).resolve("level.dat");
  Object root=call(C("uo"),"a",new Class[]{Path.class,C("uk")},file,call(C("uk"),"a",new Class[]{}));
  Object data=call(root,"p",new Class[]{String.class},"Data");
  if(args[0].equals("prepare")) {
   call(data,"a",new Class[]{String.class,boolean.class},"raining",true);
   call(data,"a",new Class[]{String.class,boolean.class},"thundering",true);
   for(String field:new String[]{"rainTime","thunderTime"})call(data,"a",new Class[]{String.class,int.class},field,1000);
   call(data,"a",new Class[]{String.class,int.class},"clearWeatherTime",0);
   Object rules=call(data,"p",new Class[]{String.class},"GameRules");
   call(rules,"a",new Class[]{String.class,String.class},"doWeatherCycle","false");
   call(data,"a",new Class[]{String.class,String.class},"Phase5UnknownUTF16",new String(new char[]{65,0,0xD800,90}));
   call(data,"a",new Class[]{String.class,long.class},"Phase5UnknownLong",Long.MIN_VALUE+23);
   call(C("uo"),"a",new Class[]{C("ub"),Path.class},root,file);
  }
  Map<String,Object> out=new LinkedHashMap<>();
  for(String key:new String[]{"raining","thundering"})out.put(key,call(data,"q",new Class[]{String.class},key));
  for(String key:new String[]{"rainTime","thunderTime","clearWeatherTime"})out.put(key,call(data,"h",new Class[]{String.class},key));
  Object rules=call(data,"p",new Class[]{String.class},"GameRules");
  out.put("doWeatherCycle",call(rules,"l",new Class[]{String.class},"doWeatherCycle"));
  String text=(String)call(data,"l",new Class[]{String.class},"Phase5UnknownUTF16");
  out.put("unknown_utf16",text.chars().toArray());
  out.put("unknown_long",call(data,"i",new Class[]{String.class},"Phase5UnknownLong"));
  for(String key:new String[]{"raining","rainTime","Phase5UnknownUTF16","Phase5UnknownLong"})
   out.put(key+"_tag_type",call(data,"d",new Class[]{String.class},key));
  System.out.println("WORLD_NBT:"+new Gson().toJson(out));
 }
}
