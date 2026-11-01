package oracle;
import com.google.gson.*;
import com.mojang.serialization.*;
import java.io.*;
import java.lang.reflect.*;
import java.util.*;

/** Actual original GameRules(DynamicLike), private loadFromTag and createTag.
 * Every input is parsed by original NbtIo; no Minecraft classes are changed. */
public final class world_rules_oracle {
 static final Gson JSON=new GsonBuilder().serializeNulls().create();static final PrintStream OUT=System.out;
 static Class<?> RULES,IO,TAG,ACC;static DynamicOps<Object> OPS;static Method LOAD,READ;static Constructor<?> FROM_TAG;static final TreeMap<String,String> TYPES=new TreeMap<>();
 static Class<?> C(String n)throws Exception{return Class.forName(n);}static Class<?>[] T(Class<?>...a){return a;}
 static Object call(Object o,String n,Class<?>[]t,Object...a)throws Exception{Class<?>c=o instanceof Class<?>?(Class<?>)o:o.getClass();Method m=c.getMethod(n,t);m.setAccessible(true);return m.invoke(o instanceof Class<?>?null:o,a);}
 static Object field(Object o,String owner,String n)throws Exception{Field f=C(owner).getDeclaredField(n);f.setAccessible(true);return f.get(o);}
 static Object tag(String input)throws Exception{byte[]raw=Base64.getDecoder().decode(input);DataInputStream in=new DataInputStream(new ByteArrayInputStream(raw));Object value=READ.invoke(null,in,call(ACC,"a",T()));if(in.available()!=0)throw new AssertionError("unconsumed input");return value;}
 static Dynamic<Object> dynamic(String input)throws Exception{return new Dynamic<>(OPS,tag(input));}
 static JsonObject snapshot(Object rules)throws Exception {
  Object written=call(rules,"a",T());JsonObject values=new JsonObject(),serialized=new JsonObject();TreeMap<String,Object> rows=new TreeMap<>();for(var e:((Map<?,?>)field(rules,"dcs","ac")).entrySet())rows.put((String)call(e.getKey(),"a",T()),e.getValue());
  for(var e:rows.entrySet()){Object v=e.getValue();if(v.getClass()==C("dcs$a"))values.addProperty(e.getKey(),(boolean)call(v,"a",T()));else if(v.getClass()==C("dcs$d"))values.addProperty(e.getKey(),(int)call(v,"a",T()));else throw new AssertionError("unknown rule type");serialized.addProperty(e.getKey(),(String)call(written,"l",T(String.class),e.getKey()));}
  JsonObject out=new JsonObject();out.add("values",values);out.add("written",serialized);return out;
 }
 @SuppressWarnings("unchecked") static void init()throws Exception {
  call(C("ab"),"a",T());call(C("akt"),"a",T());RULES=C("dcs");IO=C("uo");TAG=C("uy");ACC=C("uk");OPS=(DynamicOps<Object>)C("up").getField("a").get(null);READ=IO.getMethod("b",DataInput.class,ACC);FROM_TAG=RULES.getConstructor(DynamicLike.class);LOAD=RULES.getDeclaredMethod("a",DynamicLike.class);LOAD.setAccessible(true);
  Object defaults=RULES.getConstructor().newInstance();for(var e:((Map<?,?>)field(defaults,"dcs","ac")).entrySet())TYPES.put((String)call(e.getKey(),"a",T()),e.getValue().getClass()==C("dcs$a")?"boolean":"integer");
  JsonArray ranges=new JsonArray();for(int cp=0;cp<=65535;cp++)if(Character.digit((char)cp,10)==0){for(int d=0;d<10;d++)if(Character.digit((char)(cp+d),10)!=d)throw new AssertionError("non-contiguous digits");ranges.add(cp);}
  // Assert that the range representation describes Character.digit for every
  // possible UTF-16 unit, including surrogates and non-decimal characters.
  for(int cp=0;cp<=65535;cp++){int represented=-1;for(JsonElement z:ranges){int d=cp-z.getAsInt();if(d>=0&&d<10)represented=d;}if(represented!=Character.digit((char)cp,10))throw new AssertionError("digit range loss");}
  JsonObject aliases=new JsonObject();for(char target:"true".toCharArray()){JsonArray row=new JsonArray();for(int cp=0;cp<=65535;cp++)if(String.valueOf((char)cp).equalsIgnoreCase(String.valueOf(target)))row.add(cp);aliases.add(String.valueOf(target),row);}
  JsonArray ctors=new JsonArray();for(Constructor<?>c:RULES.getConstructors())ctors.add(Arrays.toString(c.getParameterTypes()));JsonObject header=new JsonObject();header.addProperty("source",RULES.getProtectionDomain().getCodeSource().getLocation().toString());header.addProperty("jdk",System.getProperty("java.version"));header.add("defaults",snapshot(defaults));header.add("types",JSON.toJsonTree(TYPES));header.add("digit_zeros",ranges);header.add("boolean_char_aliases",aliases);header.add("public_constructors",ctors);OUT.println("HEADER:"+JSON.toJson(header));
 }
 static JsonArray query(JsonObject q)throws Exception {Object rules=q.has("initial")&&!q.get("initial").isJsonNull()?FROM_TAG.newInstance(dynamic(q.get("initial").getAsString())):RULES.getConstructor().newInstance();JsonArray states=new JsonArray();states.add(snapshot(rules));for(JsonElement input:q.getAsJsonArray("loads")){LOAD.invoke(rules,dynamic(input.getAsString()));states.add(snapshot(rules));}return states;}
 public static void main(String[]args)throws Exception{init();BufferedReader in=new BufferedReader(new InputStreamReader(System.in));for(String line;(line=in.readLine())!=null;)OUT.println("ORACLE:"+JSON.toJson(query(JsonParser.parseString(line).getAsJsonObject())));}
}
