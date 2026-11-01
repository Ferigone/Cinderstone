package oracle;

import com.google.gson.*;
import com.mojang.serialization.*;
import io.netty.buffer.*;
import java.io.*;
import java.lang.reflect.*;
import java.util.*;
import java.util.concurrent.atomic.AtomicLong;
import jdk.internal.org.objectweb.asm.*;
import static jdk.internal.org.objectweb.asm.Opcodes.*;

/** Actual original 1.21 recipe book transitions, settings, NBT and STREAM.
 * Original classes are not patched. A controlled trigger subclass observes the
 * original Predicate capture, then forwards into unchanged dt/dv. Network and
 * PlayerAdvancements.award are outer observation boundaries. */
public final class server_recipe_book_oracle {
 static final PrintStream OUT=System.out;
 static final Gson JSON=new GsonBuilder().serializeNulls().create();
 static final String SHA="8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01";
 static Object manager,lookup,access,book,player,advancements,trigger,externalSettings,world,server,reloadable,random;
 static Map<?,?> holders;static JsonObject bootstrap;static Object[] bookTypes;static int packetId;
 static JsonArray events,packets;static JsonObject callback;static boolean callbackConsumed;
 static List<Object> packetObjects=new ArrayList<>();
 static final String[] SETTING_NAMES={"isGuiOpen","isFilteringCraftable","isFurnaceGuiOpen","isFurnaceFilteringCraftable","isBlastingFurnaceGuiOpen","isBlastingFurnaceFilteringCraftable","isSmokerGuiOpen","isSmokerFilteringCraftable"};

 static Class<?> C(String name)throws Exception{return Class.forName(name);}
 static Object call(Object target,String name,Class<?>[]types,Object...args)throws Exception {
  Class<?> owner=target instanceof Class<?>?(Class<?>)target:target.getClass();Method method=null;
  try{method=owner.getMethod(name,types);}catch(NoSuchMethodException ignored){for(Class<?> c=owner;c!=null;c=c.getSuperclass())try{method=c.getDeclaredMethod(name,types);break;}catch(NoSuchMethodException e){}}
  if(method==null)throw new NoSuchMethodException(owner+"."+name);method.setAccessible(true);return method.invoke(target instanceof Class<?>?null:target,args);
 }
 static Object get(Object target,String owner,String field)throws Exception{Field f=C(owner).getDeclaredField(field);f.setAccessible(true);return f.get(target);}
 static void set(Object target,String owner,String field,Object value)throws Exception{Field f=C(owner).getDeclaredField(field);f.setAccessible(true);f.set(target,value);}
 static Object unsafe()throws Exception{Field f=C("sun.misc.Unsafe").getDeclaredField("theUnsafe");f.setAccessible(true);return f.get(null);}
 static Object alloc(Class<?> type)throws Exception{return C("sun.misc.Unsafe").getMethod("allocateInstance",Class.class).invoke(unsafe(),type);}
 static String text(JsonElement value){
  if(value.isJsonObject()&&value.getAsJsonObject().has("utf16")){JsonArray a=value.getAsJsonObject().getAsJsonArray("utf16");char[]s=new char[a.size()];for(int i=0;i<s.length;i++)s[i]=(char)a.get(i).getAsInt();return new String(s);}
  return value.getAsString();
 }
 static Object id(JsonElement value)throws Exception{return call(C("akr"),"a",new Class<?>[]{String.class},text(value));}
 static List<String> strings(Collection<?> values){List<String> result=new ArrayList<>();for(Object value:values)result.add(value.toString());return result;}
 static JsonArray array(Collection<?> values){JsonArray out=new JsonArray();for(Object value:values)out.add(value.toString());return out;}
 @SuppressWarnings("unchecked")static JsonArray known()throws Exception{return array(new TreeSet<>(strings((Set<?>)get(book,"avs","a"))));}
 static JsonArray highlight()throws Exception{return array(new TreeSet<>(strings((Set<?>)get(book,"avs","b"))));}
 static Object settings()throws Exception{return get(book,"avs","c");}
 static JsonArray settingValues(Object settings)throws Exception {
  Map<?,?> states=(Map<?,?>)get(settings,"avt","b");JsonArray out=new JsonArray();
  for(Object type:bookTypes){Object value=states.get(type);if(value==null){out.add(JsonNull.INSTANCE);out.add(JsonNull.INSTANCE);}else{out.add((boolean)get(value,"avt$a","a"));out.add((boolean)get(value,"avt$a","b"));}}
  return out;
 }
 static void setValues(Object target,JsonArray values)throws Exception{for(int i=0;i<4;i++){call(target,"a",new Class<?>[]{C("crj"),boolean.class},bookTypes[i],values.get(2*i).getAsBoolean());call(target,"b",new Class<?>[]{C("crj"),boolean.class},bookTypes[i],values.get(2*i+1).getAsBoolean());}}
 static JsonObject state()throws Exception {JsonObject out=new JsonObject();out.add("known",known());out.add("highlight",highlight());out.add("settings",settingValues(settings()));return out;}
 static Throwable root(Throwable error){while(error instanceof InvocationTargetException&&error.getCause()!=null)error=error.getCause();return error;}
 static JsonObject error(Throwable error){error=root(error);JsonObject out=new JsonObject();out.addProperty("class",error.getClass().getName());out.addProperty("message",String.valueOf(error.getMessage()));return out;}
 static String wire(Object packet)throws Exception{ByteBuf raw=Unpooled.buffer();try{Object buf=C("vw").getConstructor(ByteBuf.class).newInstance(raw);call(C("aee").getField("a").get(null),"encode",new Class<?>[]{Object.class,Object.class},buf,packet);byte[]bytes=new byte[raw.readableBytes()];raw.getBytes(raw.readerIndex(),bytes);return Base64.getEncoder().encodeToString(bytes);}finally{raw.release();}}
 static JsonObject packet(Object value)throws Exception {
  JsonObject out=new JsonObject();out.addProperty("packet_id",packetId);out.addProperty("state",((Enum<?>)call(value,"g",new Class<?>[]{})).ordinal());
  out.add("recipes",array((List<?>)call(value,"b",new Class<?>[]{})));out.add("highlights",array((List<?>)call(value,"e",new Class<?>[]{})));out.add("settings",settingValues(call(value,"f",new Class<?>[]{})));out.addProperty("wire",wire(value));return out;
 }
 public static void sent(Object value)throws Exception{JsonObject e=state();e.addProperty("callback","send");e.addProperty("packet_index",packetObjects.size());events.add(e);packetObjects.add(value);packets.add(packet(value));}
 static void mutateCallback(String phase)throws Exception {
  if(callback==null||callbackConsumed||!callback.get("phase").getAsString().equals(phase))return;callbackConsumed=true;
  JsonObject e=state();e.addProperty("callback","mutation_begin");e.addProperty("phase",phase);e.addProperty("mutation",callback.get("mutation").getAsString());events.add(e);
  switch(callback.get("mutation").getAsString()){
   case "remove" -> {for(JsonElement rid:callback.getAsJsonArray("recipes"))call(book,"c",new Class<?>[]{C("akr")},id(rid));}
   case "highlight" -> {for(JsonElement rid:callback.getAsJsonArray("recipes"))call(book,"d",new Class<?>[]{C("akr")},id(rid));}
   case "settings" -> setValues(settings(),callback.getAsJsonArray("settings"));
   case "reentrant_add" -> call(book,"a",new Class<?>[]{Collection.class,C("aqv")},resolve(callback.getAsJsonArray("recipes")),player);
   case "throw" -> throw new IllegalStateException("controlled recipe callback failure");
   default -> throw new IllegalArgumentException("unsupported controlled mutation");
  }
  e=state();e.addProperty("callback","mutation_end");e.addProperty("phase",phase);events.add(e);
 }
 public static void criterion(Object predicate)throws Exception {
  Object captured=null;for(Field f:predicate.getClass().getDeclaredFields())if(f.getType()==C("czb")){f.setAccessible(true);captured=f.get(predicate);break;}
  if(captured==null)throw new IllegalStateException("original recipe predicate capture missing");
  JsonObject e=state();e.addProperty("callback","recipe_unlocked");e.addProperty("recipe",call(captured,"a",new Class<?>[]{}).toString());events.add(e);mutateCallback("criterion");
 }
 public static Object environment(String name)throws Exception{JsonObject e=state();e.addProperty("callback",name);events.add(e);return switch(name){case "advancements"->advancements;case "level_context"->world;case "world_server"->server;case "world_random"->random;case "server_lookup"->reloadable;default->throw new IllegalArgumentException(name);};}
 public static boolean awarded(Object holder,String criterion)throws Exception{JsonObject e=state();e.addProperty("callback","criterion_award");e.addProperty("criterion",criterion);events.add(e);mutateCallback("award");return true;}
 static ClassWriter clazz(String name,String parent){ClassWriter w=new ClassWriter(0);w.visit(V21,ACC_PUBLIC,name,null,parent,null);return w;}
 static Class<?> define(String name,ClassWriter w){w.visitEnd();byte[]raw=w.toByteArray();return new ClassLoader(server_recipe_book_oracle.class.getClassLoader()){Class<?> build(){return defineClass(name,raw,0,raw.length);}}.build();}
 static void envMethod(ClassWriter w,String name,String type,String kind){MethodVisitor m=w.visitMethod(ACC_PUBLIC,name,"()L"+type+";",null,null);m.visitCode();m.visitLdcInsn(kind);m.visitMethodInsn(INVOKESTATIC,"oracle/server_recipe_book_oracle","environment","(Ljava/lang/String;)Ljava/lang/Object;",false);m.visitTypeInsn(CHECKCAST,type);m.visitInsn(ARETURN);m.visitMaxs(1,1);m.visitEnd();}
 static Class<?> PLAYER,CONNECTION,ADVANCEMENTS,WORLD,SERVER;
 static void adapters()throws Exception {
  ClassWriter w=clazz("OracleRecipeConnection","aru");MethodVisitor m=w.visitMethod(ACC_PUBLIC,"b","(Lzg;)V",null,null);m.visitCode();m.visitVarInsn(ALOAD,1);m.visitMethodInsn(INVOKESTATIC,"oracle/server_recipe_book_oracle","sent","(Ljava/lang/Object;)V",false);m.visitInsn(RETURN);m.visitMaxs(1,2);m.visitEnd();CONNECTION=define("OracleRecipeConnection",w);
  w=clazz("OracleRecipePlayer","aqv");envMethod(w,"R","akz","advancements");envMethod(w,"A","aqu","level_context");PLAYER=define("OracleRecipePlayer",w);
  w=clazz("OracleRecipeWorld","aqu");envMethod(w,"o","net/minecraft/server/MinecraftServer","world_server");envMethod(w,"E_","ayw","world_random");WORLD=define("OracleRecipeWorld",w);
  w=clazz("OracleRecipeServer","net/minecraft/server/MinecraftServer");envMethod(w,"be","alb$b","server_lookup");SERVER=define("OracleRecipeServer",w);
  w=clazz("OracleRecipeAdvancements","akz");m=w.visitMethod(ACC_PUBLIC,"a","(Lag;Ljava/lang/String;)Z",null,null);m.visitCode();m.visitVarInsn(ALOAD,1);m.visitVarInsn(ALOAD,2);m.visitMethodInsn(INVOKESTATIC,"oracle/server_recipe_book_oracle","awarded","(Ljava/lang/Object;Ljava/lang/String;)Z",false);m.visitInsn(IRETURN);m.visitMaxs(2,3);m.visitEnd();ADVANCEMENTS=define("OracleRecipeAdvancements",w);
  w=clazz("OracleRecipeUnlockedTrigger","dt");m=w.visitMethod(ACC_PUBLIC,"<init>","()V",null,null);m.visitCode();m.visitVarInsn(ALOAD,0);m.visitMethodInsn(INVOKESPECIAL,"dt","<init>","()V",false);m.visitInsn(RETURN);m.visitMaxs(1,1);m.visitEnd();
  m=w.visitMethod(ACC_PROTECTED,"a","(Laqv;Ljava/util/function/Predicate;)V",null,null);m.visitCode();m.visitVarInsn(ALOAD,2);m.visitMethodInsn(INVOKESTATIC,"oracle/server_recipe_book_oracle","criterion","(Ljava/lang/Object;)V",false);m.visitVarInsn(ALOAD,0);m.visitVarInsn(ALOAD,1);m.visitVarInsn(ALOAD,2);m.visitMethodInsn(INVOKESPECIAL,"dt","a","(Laqv;Ljava/util/function/Predicate;)V",false);m.visitInsn(RETURN);m.visitMaxs(3,3);m.visitEnd();
  trigger=define("OracleRecipeUnlockedTrigger",w).getConstructor().newInstance();Field f=C("an").getField("g");Object u=unsafe();Object base=C("sun.misc.Unsafe").getMethod("staticFieldBase",Field.class).invoke(u,f);long offset=(long)C("sun.misc.Unsafe").getMethod("staticFieldOffset",Field.class).invoke(u,f);C("sun.misc.Unsafe").getMethod("putObjectVolatile",Object.class,long.class,Object.class).invoke(u,base,offset,trigger);
 }
 @SuppressWarnings("unchecked")static void init(String file)throws Exception {
  call(C("ab"),"a",new Class<?>[]{});call(C("akt"),"a",new Class<?>[]{});bootstrap=JsonParser.parseReader(new FileReader(file)).getAsJsonObject();
  Object itemRegistry=C("lt").getField("g").get(null),itemKey=C("lu").getField("K").get(null);Map<Object,List<Object>> tags=new HashMap<>();
  for(Map.Entry<String,JsonElement> entry:bootstrap.getAsJsonObject("tags").entrySet()){Object tag=call(C("awu"),"a",new Class<?>[]{C("akq"),C("akr")},itemKey,id(new JsonPrimitive(entry.getKey().contains(":")?entry.getKey():"minecraft:"+entry.getKey())));List<Object> values=new ArrayList<>();for(JsonElement i:entry.getValue().getAsJsonArray())values.add(((Optional<?>)call(itemRegistry,"c",new Class<?>[]{int.class},i.getAsInt())).orElseThrow());tags.put(tag,values);}call(itemRegistry,"a",new Class<?>[]{Map.class},tags);
  lookup=call(C("pd"),"a",new Class<?>[]{});manager=C("czd").getConstructor(C("jo$a")).newInstance(lookup);Map<Object,JsonElement> recipes=new LinkedHashMap<>();for(Map.Entry<String,JsonElement> entry:bootstrap.getAsJsonObject("recipes").entrySet())recipes.put(id(new JsonPrimitive(entry.getKey())),entry.getValue());call(manager,"a",new Class<?>[]{Map.class,C("aue"),C("bnf")},recipes,null,null);holders=(Map<?,?>)get(manager,"czd","e");
  if(holders.size()!=bootstrap.getAsJsonObject("recipes").size())throw new IllegalStateException("actual Recipe.CODEC omitted recipes: "+holders.size());bookTypes=C("crj").getEnumConstants();
  Object packetType=C("agg").getField("ai").get(null);Class<?> visitor=C("wh$a$a");Object proxy=Proxy.newProxyInstance(visitor.getClassLoader(),new Class<?>[]{visitor},(p,m,a)->{if(m.getName().equals("accept")&&a[0].equals(packetType))packetId=(int)a[1];return null;});call(C("agh").getField("b").get(null),"a",new Class<?>[]{visitor},proxy);
  adapters();access=call(C("ka"),"a",new Class<?>[]{C("jz")},C("lt").getField("aA").get(null));reloadable=C("alb$b").getConstructor(C("ka$b")).newInstance(access);
  OUT.println("SOURCE:"+C("avu").getProtectionDomain().getCodeSource().getLocation());OUT.println("SOURCE:"+C("aee").getProtectionDomain().getCodeSource().getLocation());
 }
 static Object holder(JsonElement query)throws Exception {
  if(query.isJsonObject()){JsonObject h=query.getAsJsonObject();Object template=holders.get(id(h.get("template")));if(template==null)throw new IllegalArgumentException("unknown template recipe");return C("czb").getConstructor(C("akr"),C("cyz")).newInstance(id(h.get("id")),call(template,"b",new Class<?>[]{}));}
  Object value=holders.get(id(query));if(value==null)throw new IllegalArgumentException("unknown default recipe: "+query);return value;
 }
 static List<Object> resolve(JsonArray queries)throws Exception {List<Object> out=new ArrayList<>();for(JsonElement q:queries)out.add(holder(q));return out;}
 static Object tag(JsonObject q)throws Exception {
  int type=q.get("type").getAsInt();JsonElement value=q.get("value");return switch(type){
   case 0->C("ud").getField("b").get(null);
   case 1->call(C("tz"),"a",new Class<?>[]{byte.class},(byte)value.getAsInt());
   case 2->call(C("ut"),"a",new Class<?>[]{short.class},(short)value.getAsInt());
   case 3->call(C("ug"),"a",new Class<?>[]{int.class},value.getAsInt());
   case 4->call(C("uj"),"a",new Class<?>[]{long.class},value.getAsLong());
   case 5->call(C("ue"),"a",new Class<?>[]{float.class},q.has("bits")?Float.intBitsToFloat((int)Long.parseUnsignedLong(q.get("bits").getAsString(),16)):value.getAsFloat());
   case 6->call(C("uc"),"a",new Class<?>[]{double.class},q.has("bits")?Double.longBitsToDouble(Long.parseUnsignedLong(q.get("bits").getAsString(),16)):value.getAsDouble());
   case 8->call(C("uw"),"a",new Class<?>[]{String.class},text(q.has("utf16")?q:value));
   case 9->{Object list=C("uh").getConstructor().newInstance();for(JsonElement v:value.getAsJsonArray())((List<Object>)list).add(tag(v.getAsJsonObject()));yield list;}
   case 10->{Object compound=C("ub").getConstructor().newInstance();for(Map.Entry<String,JsonElement> e:value.getAsJsonObject().entrySet())call(compound,"a",new Class<?>[]{String.class,C("uy")},e.getKey(),tag(e.getValue().getAsJsonObject()));yield compound;}
   case 7->{byte[]a=new byte[value.getAsJsonArray().size()];for(int i=0;i<a.length;i++)a[i]=(byte)value.getAsJsonArray().get(i).getAsInt();yield C("ty").getConstructor(byte[].class).newInstance((Object)a);}
   case 11->{int[]a=new int[value.getAsJsonArray().size()];for(int i=0;i<a.length;i++)a[i]=value.getAsJsonArray().get(i).getAsInt();yield C("uf").getConstructor(int[].class).newInstance((Object)a);}
   case 12->{long[]a=new long[value.getAsJsonArray().size()];for(int i=0;i<a.length;i++)a[i]=value.getAsJsonArray().get(i).getAsLong();yield C("ui").getConstructor(long[].class).newInstance((Object)a);}
   default->throw new IllegalArgumentException("NBT type "+type);
  };
 }
 static JsonObject nbtLogical(Object nbt)throws Exception {JsonObject out=new JsonObject(),flags=new JsonObject();for(String key:SETTING_NAMES)flags.addProperty(key,(boolean)call(nbt,"q",new Class<?>[]{String.class},key));out.add("settings",flags);for(String key:new String[]{"recipes","toBeDisplayed"}){Object list=call(nbt,"c",new Class<?>[]{String.class,int.class},key,8);List<String>values=new ArrayList<>();for(int i=0;i<(int)call(list,"size",new Class<?>[]{});i++)values.add((String)call(list,"j",new Class<?>[]{int.class},i));Collections.sort(values);out.add(key,array(values));}return out;}
 static JsonObject snapshot()throws Exception {JsonObject out=state();try{Object nbt=call(book,"b",new Class<?>[]{});out.add("nbt",nbtLogical(nbt));out.add("nbt_key_order",array((Set<?>)call(nbt,"e",new Class<?>[]{})));ByteArrayOutputStream bytes=new ByteArrayOutputStream();call(C("uo"),"a",new Class<?>[]{C("ub"),DataOutput.class},nbt,new DataOutputStream(bytes));out.addProperty("nbt_wire",Base64.getEncoder().encodeToString(bytes.toByteArray()));}catch(Throwable e){out.add("nbt_error",error(e));}out.add("events",events.deepCopy());out.add("packets",packets.deepCopy());return out;}
 static JsonObject facts()throws Exception {
  book=C("avu").getConstructor().newInstance();JsonObject out=new JsonObject();out.addProperty("minecraft","1.21");out.addProperty("oracle_inner_sha256",SHA);out.addProperty("packet_id",packetId);out.add("setting_names",JSON.toJsonTree(SETTING_NAMES));
  JsonArray types=new JsonArray();String[]names={"CRAFTING","FURNACE","BLAST_FURNACE","SMOKER"};for(int i=0;i<4;i++){JsonObject type=new JsonObject();type.addProperty("ordinal",i);type.addProperty("name",names[i]);type.addProperty("open_tag",SETTING_NAMES[i*2]);type.addProperty("filter_tag",SETTING_NAMES[i*2+1]);types.add(type);}out.add("book_types",types);
  JsonArray recipes=new JsonArray();for(String key:new TreeSet<>(bootstrap.getAsJsonObject("recipes").keySet())){Object value=holders.get(id(new JsonPrimitive(key))),recipe=call(value,"b",new Class<?>[]{});JsonObject row=new JsonObject();row.addProperty("id",call(value,"a",new Class<?>[]{}).toString());row.addProperty("is_special",(boolean)call(recipe,"as_",new Class<?>[]{}));row.addProperty("value_class",recipe.getClass().getName());row.addProperty("raw_type",bootstrap.getAsJsonObject("recipes").getAsJsonObject(key).get("type").getAsString());recipes.add(row);}out.add("recipes",recipes);
  out.add("to_nbt_key_order",array((Set<?>)call(call(book,"b",new Class<?>[]{}),"e",new Class<?>[]{})));out.addProperty("default_catalogue_count",holders.size());out.addProperty("tag_binding_scope","Actual Recipe.CODEC uses explicit expanded tag bindings sorted by item ID from the prior crafting fixture. Natural Vanilla TagLoader order is not claimed.");out.addProperty("criteria_environment","Controlled forwarding observer subclass installed in an.g; actual dt/dv execute, registered outer award only observed. Default advancement registration/dynamic datapack trigger serialization not represented.");return out;
 }
 static JsonObject declarations()throws Exception {
  List<Object> sorted=new ArrayList<>();for(String key:new TreeSet<>(bootstrap.getAsJsonObject("recipes").keySet()))sorted.add(holders.get(id(new JsonPrimitive(key))));
  Object packet=C("agc").getConstructor(Collection.class).newInstance(sorted),type=call(packet,"a",new Class<?>[]{});int[]number={-1};Class<?>visitor=C("wh$a$a");Object proxy=Proxy.newProxyInstance(visitor.getClassLoader(),new Class<?>[]{visitor},(p,m,a)->{if(m.getName().equals("accept")&&a[0].equals(type))number[0]=(int)a[1];return null;});call(C("agh").getField("b").get(null),"a",new Class<?>[]{visitor},proxy);
  ByteBuf raw=Unpooled.buffer();byte[]bytes;Object decoded;try{Object buf=C("wk").getConstructor(ByteBuf.class,C("ka")).newInstance(raw,access);Object codec=C("agc").getField("a").get(null);call(codec,"encode",new Class<?>[]{Object.class,Object.class},buf,packet);bytes=new byte[raw.readableBytes()];raw.getBytes(raw.readerIndex(),bytes);decoded=call(codec,"decode",new Class<?>[]{Object.class},buf);if(raw.readableBytes()!=0)throw new IllegalStateException("declaration bytes not fully consumed");}finally{raw.release();}
  JsonArray ids=new JsonArray();for(Object holder:(Collection<?>)call(decoded,"b",new Class<?>[]{}))ids.add(call(holder,"a",new Class<?>[]{}).toString());JsonObject out=new JsonObject();out.addProperty("minecraft","1.21");out.addProperty("oracle_inner_sha256",SHA);out.addProperty("packet_id",number[0]);out.addProperty("payload",Base64.getEncoder().encodeToString(bytes));out.add("decoded_ids",ids);out.addProperty("decoded_count",ids.size());out.addProperty("tag_binding_scope","Actual STREAM with explicit recipe-ID-sorted holder input and prior crafting fixture's expanded item-ID-sorted ingredient tag bindings; not natural Vanilla TagLoader ordering or a native serializer implementation.");return out;
 }
 static long rngState()throws Exception{return ((AtomicLong)get(random,"dyz","h")).get();}
 static JsonObject run(JsonObject q)throws Exception {
  book=C("avu").getConstructor().newInstance();externalSettings=C("avt").getConstructor().newInstance();player=alloc(PLAYER);advancements=alloc(ADVANCEMENTS);world=alloc(WORLD);server=alloc(SERVER);random=C("dyz").getConstructor(long.class).newInstance(123456L);set(player,"bsr","s",C("exc").getConstructor(double.class,double.class,double.class).newInstance(0.,64.,0.));set(player,"aqv","c",alloc(CONNECTION));set(player,"aqv","cY",book);set(world,"dcw","z",random);call(trigger,"a",new Class<?>[]{C("akz")},advancements);((Map<?,?>)get(trigger,"dv","a")).clear();packetObjects.clear();JsonArray states=new JsonArray();
  for(JsonElement operation:q.getAsJsonArray("operations")){JsonObject op=operation.getAsJsonObject();events=new JsonArray();packets=new JsonArray();callback=op.has("callback")?op.getAsJsonObject("callback"):null;callbackConsumed=false;JsonElement result=JsonNull.INSTANCE;JsonObject failure=null;long before=rngState();try{String name=op.get("op").getAsString();switch(name){
   case "add_recipes"->result=JSON.toJsonTree(call(book,"a",new Class<?>[]{Collection.class,C("aqv")},resolve(op.getAsJsonArray("recipes")),player));
   case "remove_recipes"->result=JSON.toJsonTree(call(book,"b",new Class<?>[]{Collection.class,C("aqv")},resolve(op.getAsJsonArray("recipes")),player));
   case "direct_add"->call(book,"a",new Class<?>[]{C("czb")},holder(op.get("recipe")));
   case "direct_remove"->call(book,"c",new Class<?>[]{C("czb")},holder(op.get("recipe")));
   case "add_highlight"->call(book,"f",new Class<?>[]{C("czb")},holder(op.get("recipe")));
   case "remove_highlight"->call(book,"e",new Class<?>[]{C("czb")},holder(op.get("recipe")));
   case "contains"->result=JSON.toJsonTree(call(book,"b",new Class<?>[]{C("czb")},op.get("recipe").isJsonNull()?null:holder(op.get("recipe"))));
   case "contains_id"->result=JSON.toJsonTree(call(book,"b",new Class<?>[]{C("akr")},id(op.get("id"))));
   case "will_highlight"->result=JSON.toJsonTree(call(book,"d",new Class<?>[]{C("czb")},holder(op.get("recipe"))));
   case "parse_id"->result=new JsonPrimitive(id(op.get("id")).toString());
   case "set_open"->call(book,"a",new Class<?>[]{C("crj"),boolean.class},bookTypes[op.get("type").getAsInt()],op.get("value").getAsBoolean());
   case "set_filter"->call(book,"b",new Class<?>[]{C("crj"),boolean.class},bookTypes[op.get("type").getAsInt()],op.get("value").getAsBoolean());
   case "set_settings"->setValues(settings(),op.getAsJsonArray("settings"));
   case "get_settings_copy"->externalSettings=call(book,"a",new Class<?>[]{});
   case "external_set"->setValues(externalSettings,op.getAsJsonArray("settings"));
   case "set_external"->call(book,"a",new Class<?>[]{C("avt")},externalSettings);
   case "settings_wire_decode"->{byte[]bytes=Base64.getDecoder().decode(op.get("wire").getAsString());ByteBuf raw=Unpooled.wrappedBuffer(bytes);try{Object buf=C("vw").getConstructor(ByteBuf.class).newInstance(raw);Object s=call(C("avt"),"a",new Class<?>[]{C("vw")},buf);call(book,"a",new Class<?>[]{C("avt")},s);result=new JsonPrimitive(raw.readableBytes());}finally{raw.release();}}
   case "load_nbt"->call(book,"a",new Class<?>[]{C("ub"),C("czd")},tag(op.getAsJsonObject("nbt")),manager);
   case "copy_from"->{Object source=C("avu").getConstructor().newInstance();for(JsonElement recipe:op.getAsJsonArray("known"))call(source,"a",new Class<?>[]{C("czb")},holder(recipe));for(JsonElement recipe:op.getAsJsonArray("highlight"))call(source,"f",new Class<?>[]{C("czb")},holder(recipe));setValues(get(source,"avs","c"),op.getAsJsonArray("settings"));call(book,"a",new Class<?>[]{C("avs")},source);}
   case "self_copy"->call(book,"a",new Class<?>[]{C("avs")},book);
   case "init"->call(book,"a",new Class<?>[]{C("aqv")},player);
   case "register_criterion"->{Object instance=C("dt$a").getConstructor(Optional.class,C("akr")).newInstance(Optional.empty(),id(op.get("recipe")));Object listener=C("aq$a").getConstructor(C("ar"),C("ag"),String.class).newInstance(instance,null,op.get("name").getAsString());call(trigger,"a",new Class<?>[]{C("akz"),C("aq$a")},advancements,listener);}
   case "clear_criteria"->call(trigger,"a",new Class<?>[]{C("akz")},advancements);
   case "packet_ordered"->{List<Object> recipes=new ArrayList<>(),highlights=new ArrayList<>();for(JsonElement rid:op.getAsJsonArray("recipes"))recipes.add(id(rid));for(JsonElement rid:op.getAsJsonArray("highlights"))highlights.add(id(rid));Object p=C("aee").getConstructor(C("aee$a"),Collection.class,Collection.class,C("avt")).newInstance(C("aee$a").getEnumConstants()[op.get("state").getAsInt()],recipes,highlights,call(book,"a",new Class<?>[]{}));sent(p);}
   case "packet_decode"->{ByteBuf raw=Unpooled.wrappedBuffer(Base64.getDecoder().decode(op.get("wire").getAsString()));try{Object buf=C("vw").getConstructor(ByteBuf.class).newInstance(raw);Object p=call(C("aee").getField("a").get(null),"decode",new Class<?>[]{Object.class},buf);packets.add(packet(p));result=new JsonPrimitive(raw.readableBytes());}finally{raw.release();}}
   case "saved_packet"->{int index=op.get("index").getAsInt();packets.add(packet(packetObjects.get(index)));}
   default->throw new IllegalArgumentException("unknown oracle op "+name);
  }}catch(Throwable e){failure=error(e);}JsonObject state=snapshot();state.add("result",result);state.add("error",failure==null?JsonNull.INSTANCE:failure);state.add("external_settings",settingValues(externalSettings));state.addProperty("world_rng_before",before);state.addProperty("world_rng_after",rngState());states.add(state);}
  JsonObject out=new JsonObject();out.add("states",states);return out;
 }
 public static void main(String[]args)throws Exception{init(args[0]);if(args.length>1){OUT.println("ORACLE:"+JSON.toJson(args[1].equals("facts")?facts():declarations()));return;}BufferedReader input=new BufferedReader(new InputStreamReader(System.in));for(String line;(line=input.readLine())!=null;)OUT.println("ORACLE:"+JSON.toJson(run(JsonParser.parseString(line).getAsJsonObject())));}
}
