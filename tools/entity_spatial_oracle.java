package oracle;
import com.google.gson.*;
import java.io.*;
import java.lang.reflect.*;
import java.util.*;
import java.util.stream.*;

/** Execute the original signed1.21 EntitySectionStorage/EntitySection and
 * ClassInstanceMultiMap. Entities are genuine ItemEntity/ServerPlayer objects;
 * only their ID and AABB fields are injected. No Minecraft class is rewritten. */
public final class entity_spatial_oracle {
 static final Gson JSON=new GsonBuilder().serializeNulls().create();static final PrintStream OUT=System.out;
 static Class<?> ENTITY,ACCESS,SECTION,STORAGE,TYPE,CONSUMER,CONTINUATION,VISIBILITY,BOX,POS,CHUNK;
 static Class<?>[] CLASSES;static Object[] STATUS;static Object CONTINUE,ABORT,unsafe;static Method allocate;
 static Field ID,BOUNDS,SECTIONS,MULTIMAP,BYCLASS;
 static Class<?> C(String n)throws Exception{return Class.forName(n);}
 static Object call(Object o,String name,Class<?>[]types,Object...args)throws Exception {
  Class<?> c=o instanceof Class<?>?(Class<?>)o:o.getClass();Method m=null;
  try{m=c.getMethod(name,types);}catch(NoSuchMethodException e){for(Class<?> q=c;q!=null;q=q.getSuperclass())try{m=q.getDeclaredMethod(name,types);break;}catch(NoSuchMethodException ignored){}}
  if(m==null)throw new NoSuchMethodException(c+"."+name+Arrays.toString(types));m.setAccessible(true);return m.invoke(o instanceof Class<?>?null:o,args);
 }
 static Class<?>[] T(Class<?>...types){return types;}
 static Field field(Class<?> c,String n)throws Exception {Field f=c.getDeclaredField(n);f.setAccessible(true);return f;}
 static double d(JsonElement e){return Double.longBitsToDouble(Long.parseUnsignedLong(e.getAsString(),16));}
 static Object box(JsonArray a)throws Exception{return BOX.getConstructor(double.class,double.class,double.class,double.class,double.class,double.class).newInstance(d(a.get(0)),d(a.get(1)),d(a.get(2)),d(a.get(3)),d(a.get(4)),d(a.get(5)));}
 static long key(JsonArray p)throws Exception{return (long)call(POS,"b",T(int.class,int.class,int.class),p.get(0).getAsInt(),p.get(1).getAsInt(),p.get(2).getAsInt());}
 static long chunk(int x,int z)throws Exception{return (long)call(CHUNK,"c",T(int.class,int.class),x,z);}
 static Object alloc(Class<?> c)throws Exception{return allocate.invoke(unsafe,c);}
 static void init()throws Exception {
  call(C("ab"),"a",T());call(C("akt"),"a",T());ENTITY=C("bsr");ACCESS=C("dxg");SECTION=C("dxk");STORAGE=C("dxl");TYPE=C("dxn");CONSUMER=C("axa");CONTINUATION=C("axa$a");VISIBILITY=C("dxt");BOX=C("ewx");POS=C("kf");CHUNK=C("dcd");
  CLASSES=new Class<?>[]{ENTITY,C("cjh"),C("aqv"),C("cmx"),C("btn")};STATUS=VISIBILITY.getEnumConstants();CONTINUE=CONTINUATION.getEnumConstants()[0];ABORT=CONTINUATION.getEnumConstants()[1];
  Class<?> u=C("sun.misc.Unsafe");Field f=field(u,"theUnsafe");unsafe=f.get(null);allocate=u.getMethod("allocateInstance",Class.class);
  ID=field(ENTITY,"o");BOUNDS=field(ENTITY,"aF");SECTIONS=field(STORAGE,"c");MULTIMAP=field(SECTION,"b");BYCLASS=field(C("axf"),"a");
 }
 static final class Scene {
  final Map<Integer,Object> entities=new LinkedHashMap<>();final IdentityHashMap<Object,Integer> handles=new IdentityHashMap<>();final Object storage;
  Scene(JsonObject q)throws Exception {
   for(JsonElement v:q.getAsJsonArray("entities")){JsonObject e=v.getAsJsonObject();int handle=e.get("handle").getAsInt(),kind=e.get("kind").getAsInt();Object entity=alloc(CLASSES[kind]);ID.setInt(entity,e.get("id").getAsInt());BOUNDS.set(entity,box(e.getAsJsonArray("bounds")));entities.put(handle,entity);handles.put(entity,handle);}
   int initial=q.get("initial_visibility").getAsInt();Map<Long,Object> statuses=new HashMap<>();if(q.has("chunk_visibility"))for(JsonElement v:q.getAsJsonArray("chunk_visibility")){JsonArray p=v.getAsJsonArray();statuses.put(chunk(p.get(0).getAsInt(),p.get(1).getAsInt()),STATUS[p.get(2).getAsInt()]);}
   Class<?> function=C("it.unimi.dsi.fastutil.longs.Long2ObjectFunction");Object provider=Proxy.newProxyInstance(function.getClassLoader(),new Class<?>[]{function},(proxy,m,args)->{
    if(m.getName().equals("get")||m.getName().equals("apply"))return statuses.getOrDefault(((Number)args[0]).longValue(),STATUS[initial]);
    if(m.getName().equals("toString"))return "OriginalSpatialVisibilityProvider";if(m.getName().equals("hashCode"))return System.identityHashCode(proxy);if(m.getName().equals("equals"))return proxy==args[0];throw new UnsupportedOperationException(m.toString());
   });storage=STORAGE.getConstructor(Class.class,function).newInstance(ENTITY,provider);
  }
  Object section(long key,boolean create)throws Exception{return call(storage,create?"c":"d",T(long.class),key);}
  Object entity(JsonObject op){return Objects.requireNonNull(entities.get(op.get("entity").getAsInt()),"unknown entity handle");}
  List<Integer> list(Iterable<?> values){List<Integer> out=new ArrayList<>();for(Object entity:values)out.add(Objects.requireNonNull(handles.get(entity)));return out;}
  @SuppressWarnings("unchecked") Map<Long,Object> sections()throws Exception{return (Map<Long,Object>)SECTIONS.get(storage);}
  JsonArray snapshot()throws Exception {
   JsonArray out=new JsonArray();for(long key:new TreeSet<Long>(sections().keySet())){
    Object section=section(key,false),multi=MULTIMAP.get(section);Map<?,?> caches=(Map<?,?>)BYCLASS.get(multi);JsonObject row=new JsonObject();row.addProperty("key",Long.toString(key));JsonArray coords=new JsonArray();for(String n:new String[]{"b","c","d"})coords.add((int)call(POS,n,T(long.class),key));row.add("section",coords);row.addProperty("visibility",((Enum<?>)call(section,"c",T())).ordinal());row.add("all",JSON.toJsonTree(list((Iterable<?>)multi)));JsonObject cached=new JsonObject();for(int i=0;i<CLASSES.length;i++)if(caches.containsKey(CLASSES[i]))cached.add(Integer.toString(i),JSON.toJsonTree(list((Iterable<?>)caches.get(CLASSES[i]))));row.add("cached",cached);out.add(row);
   }return out;
  }
  Object query(JsonObject op)throws Exception {
   Object bounds=box(op.getAsJsonArray("bounds"));Integer cls=op.has("class_key")&&!op.get("class_key").isJsonNull()?op.get("class_key").getAsInt():null;boolean exact=op.has("exact")&&op.get("exact").getAsBoolean();int limit=op.has("abort_after")?op.get("abort_after").getAsInt():0;List<Integer> results=new ArrayList<>();List<String> visited=new ArrayList<>();boolean[] aborted={false};Object test=cls==null?null:call(TYPE,exact?"b":"a",T(Class.class),CLASSES[cls]);IdentityHashMap<Object,Long> keys=new IdentityHashMap<>();for(Map.Entry<Long,Object> e:sections().entrySet())keys.put(e.getValue(),e.getKey());
   Object entityConsumer=Proxy.newProxyInstance(CONSUMER.getClassLoader(),new Class<?>[]{CONSUMER},(proxy,m,args)->{if(!m.getName().equals("accept"))throw new UnsupportedOperationException(m.toString());results.add(Objects.requireNonNull(handles.get(args[0])));if(limit>0&&results.size()>=limit){aborted[0]=true;return ABORT;}return CONTINUE;});
   Object sectionConsumer=Proxy.newProxyInstance(CONSUMER.getClassLoader(),new Class<?>[]{CONSUMER},(proxy,m,args)->{if(!m.getName().equals("accept"))throw new UnsupportedOperationException(m.toString());Object section=args[0];visited.add(Long.toString(Objects.requireNonNull(keys.get(section))));return cls==null?call(section,"a",T(BOX,CONSUMER),bounds,entityConsumer):call(section,"a",T(TYPE,BOX,CONSUMER),test,bounds,entityConsumer);});
   // This is the original getEntities composition, retaining original methods
   // while recording which accessible nonempty sections its callback visits.
   call(storage,"a",T(BOX,CONSUMER),bounds,sectionConsumer);JsonObject out=new JsonObject();out.add("handles",JSON.toJsonTree(results));out.add("sections",JSON.toJsonTree(visited));out.addProperty("aborted",aborted[0]);return out;
  }
  Object operation(JsonObject op)throws Exception {
   switch(op.get("op").getAsString()){
    case "position_key":{JsonArray p=op.getAsJsonArray("position");int[] xyz=new int[3];for(int i=0;i<3;i++)xyz[i]=(int)call(POS,"a",T(double.class),d(p.get(i)));long k=(long)call(POS,"b",T(int.class,int.class,int.class),xyz[0],xyz[1],xyz[2]);JsonObject out=new JsonObject();out.addProperty("key",Long.toString(k));JsonArray coords=new JsonArray();for(String n:new String[]{"b","c","d"})coords.add((int)call(POS,n,T(long.class),k));out.add("section",coords);return out;}
    case "create_section":{long k=key(op.getAsJsonArray("section"));boolean existed=section(k,false)!=null;section(k,true);return !existed;}
    case "add":call(section(key(op.getAsJsonArray("section")),true),"a",T(ACCESS),entity(op));return null;
    case "remove":{Object s=section(key(op.getAsJsonArray("section")),false);return s!=null&&(boolean)call(s,"b",T(ACCESS),entity(op));}
    case "remove_section":{long k=key(op.getAsJsonArray("section"));boolean existed=section(k,false)!=null;call(storage,"e",T(long.class),k);return existed;}
    case "visibility":{Object s=section(key(op.getAsJsonArray("section")),true);return ((Enum<?>)call(s,"a",T(VISIBILITY),STATUS[op.get("visibility").getAsInt()])).ordinal();}
    case "bounds":BOUNDS.set(entity(op),box(op.getAsJsonArray("bounds")));return null;
    case "id":ID.setInt(entity(op),op.get("id").getAsInt());return null;
    case "move":{long from=key(op.getAsJsonArray("from")),to=key(op.getAsJsonArray("to"));Object value=entity(op),old=section(from,false);boolean removed=old!=null&&(boolean)call(old,"b",T(ACCESS),value);call(section(to,true),"a",T(ACCESS),value);if(old!=null&&op.has("remove_empty")&&op.get("remove_empty").getAsBoolean()&&(boolean)call(old,"a",T()))call(storage,"e",T(long.class),from);return removed;}
    case "query":return query(op);
    case "chunk_sections":{JsonArray p=op.getAsJsonArray("chunk");LongStream keys=(LongStream)call(storage,"a",T(long.class),chunk(p.get(0).getAsInt(),p.get(1).getAsInt()));return keys.mapToObj(Long::toString).toList();}
    default:throw new IllegalArgumentException(op.toString());
   }
  }
 }
 static JsonArray run(JsonObject q)throws Exception {Scene s=new Scene(q);JsonArray out=new JsonArray();for(JsonElement v:q.getAsJsonArray("operations")){JsonObject result=new JsonObject();result.add("result",JSON.toJsonTree(s.operation(v.getAsJsonObject())));result.add("sections",s.snapshot());out.add(result);}return out;}
 public static void main(String[]args)throws Exception {init();OUT.println("SOURCE:"+STORAGE.getProtectionDomain().getCodeSource().getLocation());BufferedReader input=new BufferedReader(new InputStreamReader(System.in));for(String line;(line=input.readLine())!=null;)try{OUT.println("ORACLE:"+JSON.toJson(run(JsonParser.parseString(line).getAsJsonObject())));}catch(Throwable e){OUT.println("QUERY:"+line);e.printStackTrace(OUT);throw e;}}
}
