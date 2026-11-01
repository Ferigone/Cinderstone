package oracle;

import com.google.gson.*;
import java.nio.file.*;
import java.lang.reflect.*;
import java.util.*;
import java.util.function.BiConsumer;

/** Reflection/proxies call unchanged, signed Minecraft 1.21 light classes. */
public final class lighting_oracle {
    static final Gson GSON=new Gson();
    static Class<?> cls(String n)throws Exception{return Class.forName(n);}
    static final Map<String,Method> METHODS=new HashMap<>();
    static Object call(Object receiver,String name,Class<?>[] types,Object...args)throws Exception{
        Class<?> c=receiver instanceof Class<?>?(Class<?>)receiver:receiver.getClass();String key=c.getName()+name+Arrays.toString(types);
        Method m=METHODS.get(key);if(m==null){m=c.getMethod(name,types);m.setAccessible(true);METHODS.put(key,m);}return m.invoke(receiver instanceof Class<?>?null:receiver,args);
    }
    static final Class<?>[] NONE={};
    static Object pos(int x,int y,int z)throws Exception{return cls("jd").getConstructor(int.class,int.class,int.class).newInstance(x,y,z);}
    static int coord(Object p,String name)throws Exception{return(int)call(p,name,NONE);}
    static Object state(int id)throws Exception{return call(cls("dfy"),"a",new Class<?>[]{int.class},id);}
    static long key(int x,int z){return(x&0xffffffffL)|((long)z<<32);}
    static void extract(Path output)throws Exception{
        Object empty=cls("dcl").getField("a").get(null),zero=pos(0,0,0);Object[] dirs=cls("ji").getEnumConstants();
        List<Object> faces=new ArrayList<>();Map<String,Integer> faceIds=new LinkedHashMap<>();List<int[]> rows=new ArrayList<>();Map<String,Integer> rowIds=new LinkedHashMap<>();List<Integer> states=new ArrayList<>();List<Integer> uncached=new ArrayList<>();Field cacheField=cls("dtb$a").getDeclaredField("a");cacheField.setAccessible(true);
        for(int id=0;id<26684;id++){
            Object s=state(id);if(cacheField.get(s)==null)uncached.add(id);int[] row=new int[9];row[0]=(int)call(s,"h",NONE);row[1]=(int)call(s,"b",new Class<?>[]{cls("dcc"),cls("jd")},empty,zero);
            row[2]=((boolean)call(s,"i",NONE)?1:0)|((boolean)call(s,"d",NONE)?2:0)|(!(boolean)call(call(s,"u",NONE),"c",NONE)?4:0)|(cls("dki").isInstance(call(s,"b",NONE))?8:0);
            for(int d=0;d<6;d++){
                Object face=call(cls("eov"),"a",new Class<?>[]{cls("dcc"),cls("jd"),cls("dtc"),cls("ji")},empty,zero,s,dirs[d]);
                List<double[]> boxes=new ArrayList<>();for(Object b:(List<?>)call(face,"e",NONE)){double[] box=new double[6];for(int i=0;i<6;i++)box[i]=cls("ewx").getField(""+(char)('a'+i)).getDouble(b);boxes.add(box);}
                String k=GSON.toJson(boxes);Integer found=faceIds.get(k);if(found==null){found=faces.size();faceIds.put(k,found);faces.add(face);}row[d+3]=found;
            }
            String k=Arrays.toString(row);Integer r=rowIds.get(k);if(r==null){r=rows.size();rowIds.put(k,r);rows.add(row);}states.add(r);
        }
        int contextChecks=0;
        for(int id:uncached)for(int contextId:new int[]{0,1,80,12566}) {
            Object contextState=state(contextId),location=pos(-33,7,129);
            Object context=Proxy.newProxyInstance(cls("dcc").getClassLoader(),new Class<?>[]{cls("dcc")},(obj,m,args)->{
                switch(m.getName()){case "a_":return contextState;case "b_":return call(contextState,"u",NONE);case "c_":return null;case "I_":return -64;case "J_":return 384;}
                if(m.isDefault())return InvocationHandler.invokeDefault(obj,m,args);throw new UnsupportedOperationException(m.toString());
            });
            Object s=state(id);int[] expected=rows.get(states.get(id));
            if((int)call(s,"b",new Class<?>[]{cls("dcc"),cls("jd")},context,location)!=expected[1])throw new IllegalStateException("context-dependent opacity "+id);
            for(int d=0;d<6;d++){
                Object face=call(cls("eov"),"a",new Class<?>[]{cls("dcc"),cls("jd"),cls("dtc"),cls("ji")},context,location,s,dirs[d]);
                List<double[]> boxes=new ArrayList<>();for(Object b:(List<?>)call(face,"e",NONE)){double[] box=new double[6];for(int i=0;i<6;i++)box[i]=cls("ewx").getField(""+(char)('a'+i)).getDouble(b);boxes.add(box);}
                if(!Objects.equals(faceIds.get(GSON.toJson(boxes)),expected[d+3]))throw new IllegalStateException("context-dependent face "+id);
            }
            contextChecks++;
        }
        byte[] blocked=new byte[(faces.size()*faces.size()+7)/8];for(int a=0;a<faces.size();a++)for(int b=0;b<faces.size();b++)if((boolean)call(cls("exs"),"b",new Class<?>[]{cls("exv"),cls("exv")},faces.get(a),faces.get(b))){int bit=a*faces.size()+b;blocked[bit/8]|=(byte)(1<<(bit%8));}
        JsonObject out=new JsonObject();out.addProperty("minecraft","1.21");out.addProperty("oracle_inner_sha256","8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01");out.addProperty("description","rows: emission,opacity,heightmap flags(isAir=1,blocksMotion=2,fluidNonempty=4,LeavesBlock=8),six light-occlusion face IDs DOWN UP NORTH SOUTH WEST EAST; face union occlusion matrix LSB-first");out.addProperty("uncached_world_context_checks",contextChecks);out.add("uncached_states",GSON.toJsonTree(uncached));out.addProperty("face_count",faces.size());out.add("rows",GSON.toJsonTree(rows));out.add("states",GSON.toJsonTree(states));out.addProperty("occlusion",Base64.getEncoder().encodeToString(blocked));Files.writeString(output,GSON.toJson(out)+"\n");System.out.println("ORACLE:"+states.size()+" states, "+rows.size()+" facts, "+faces.size()+" shapes");
    }
    static final class World {
        final Map<String,Object> blocks=new LinkedHashMap<>();final Map<Long,Chunk> chunks=new LinkedHashMap<>();final int minY,height;final Object proxy,source,engine;
        Object block(Object p)throws Exception{return blocks.getOrDefault(coord(p,"u")+","+coord(p,"v")+","+coord(p,"w"),state(0));}
        Object invoke(Object obj,Method m,Object[] args)throws Throwable{
            String n=m.getName();switch(n){case "I_":return minY;case "J_":return height;case "a_":return block(args[0]);case "b_":return call(block(args[0]),"u",NONE);case "c_":return null;case "q":return proxy;case "c":{Chunk c=chunks.get(key((int)args[0],(int)args[1]));return c==null?null:c.proxy;}case "toString":return "lighting_oracle.World";case "hashCode":return System.identityHashCode(obj);case "equals":return obj==args[0];}
            if(m.isDefault())return InvocationHandler.invokeDefault(obj,m,args);if(n.equals("a")&&m.getReturnType()==void.class)return null;throw new UnsupportedOperationException(m.toString());
        }
        World(JsonObject q)throws Exception{
            minY=q.get("min_y").getAsInt();height=q.get("height").getAsInt();
            proxy=Proxy.newProxyInstance(cls("dcc").getClassLoader(),new Class<?>[]{cls("dcc")},this::invoke);source=Proxy.newProxyInstance(cls("dvl").getClassLoader(),new Class<?>[]{cls("dvl")},this::invoke);
            for(JsonElement e:q.getAsJsonArray("chunks")){JsonArray a=e.getAsJsonArray();int x=a.get(0).getAsInt(),z=a.get(1).getAsInt();chunks.put(key(x,z),new Chunk(this,x,z));}
            for(JsonElement e:q.getAsJsonArray("blocks"))put(e.getAsJsonArray());
            for(Chunk c:chunks.values())for(int x=c.x*16;x<c.x*16+16;x++)for(int z=c.z*16;z<c.z*16+16;z++)for(int y=minY+height-1;y>=minY;y--)call(c.sources,"a",new Class<?>[]{cls("dcc"),int.class,int.class,int.class},c.proxy,x&15,y,z&15);
            engine=cls("eot").getConstructor(cls("dvl"),boolean.class,boolean.class).newInstance(source,true,!q.has("sky_enabled")||q.get("sky_enabled").getAsBoolean());
            for(Chunk c:chunks.values())for(int y=minY>>4;y<(minY+height)>>4;y++)call(engine,"a",new Class<?>[]{cls("kf"),boolean.class},call(cls("kf"),"a",new Class<?>[]{int.class,int.class,int.class},c.x,y,c.z),false);
            for(Chunk c:chunks.values())call(engine,"a",new Class<?>[]{cls("dcd"),boolean.class},cls("dcd").getConstructor(int.class,int.class).newInstance(c.x,c.z),true);
            for(Chunk c:chunks.values())call(engine,"b",new Class<?>[]{cls("dcd")},cls("dcd").getConstructor(int.class,int.class).newInstance(c.x,c.z));settle();
        }
        void put(JsonArray a)throws Exception{blocks.put(a.get(0)+","+a.get(1)+","+a.get(2),state(a.get(3).getAsInt()));}
        void edit(JsonArray a)throws Exception{put(a);int x=a.get(0).getAsInt(),y=a.get(1).getAsInt(),z=a.get(2).getAsInt();Chunk c=chunks.get(key(x>>4,z>>4));call(c.sources,"a",new Class<?>[]{cls("dcc"),int.class,int.class,int.class},c.proxy,x&15,y,z&15);call(engine,"a",new Class<?>[]{cls("jd")},pos(x,y,z));}
        void settle()throws Exception{int limit=0;while((boolean)call(engine,"K_",NONE)){call(engine,"a",NONE);if(++limit>100)throw new IllegalStateException("unsettled");}}
    }
    static final class Chunk {
        final World world;final int x,z;final Object proxy,sources;
        Chunk(World w,int x,int z)throws Exception{world=w;this.x=x;this.z=z;sources=cls("eoo").getConstructor(cls("dcy")).newInstance(w.proxy);proxy=Proxy.newProxyInstance(cls("dvk").getClassLoader(),new Class<?>[]{cls("dvk")},this::invoke);}
        Object invoke(Object obj,Method m,Object[] args)throws Throwable{
            if(m.getName().equals("B"))return sources;
            if(m.getName().equals("a_")||m.getName().equals("b_")){Object p=args[0];Object s=world.block(pos(x*16+(coord(p,"u")&15),coord(p,"v"),z*16+(coord(p,"w")&15)));return m.getName().equals("a_")?s:call(s,"u",NONE);}
            if(m.getName().equals("a")&&args.length==1&&args[0] instanceof BiConsumer){@SuppressWarnings("unchecked") BiConsumer<Object,Object> f=(BiConsumer<Object,Object>)args[0];for(var e:world.blocks.entrySet()){String[] p=e.getKey().split(",");int px=Integer.parseInt(p[0]),py=Integer.parseInt(p[1]),pz=Integer.parseInt(p[2]);if(px>>4==x&&pz>>4==z&&(int)call(e.getValue(),"h",NONE)>0)f.accept(pos(px,py,pz),e.getValue());}return null;}
            return world.invoke(obj,m,args);
        }
    }
    static void scene(JsonObject q)throws Exception{World w=new World(q);JsonArray out=new JsonArray();for(JsonElement batch:q.getAsJsonArray("updates")){for(JsonElement e:batch.getAsJsonArray())w.edit(e.getAsJsonArray());w.settle();JsonArray values=new JsonArray();for(JsonElement e:q.getAsJsonArray("queries")){JsonArray p=e.getAsJsonArray();Object pos=pos(p.get(0).getAsInt(),p.get(1).getAsInt(),p.get(2).getAsInt());JsonArray pair=new JsonArray();for(String layer:new String[]{"a","b"})pair.add((int)call(call(w.engine,"a",new Class<?>[]{cls("ddf")},cls("ddf").getField(layer).get(null)),"b",new Class<?>[]{cls("jd")},pos));values.add(pair);}out.add(values);}System.out.println("ORACLE:"+GSON.toJson(out));}
    public static void main(String[] args)throws Exception{call(cls("ab"),"a",NONE);call(cls("akt"),"a",NONE);if(args[0].equals("extract")){extract(Path.of(args[1]));return;}for(String line:Files.readAllLines(Path.of(args[1])))scene(JsonParser.parseString(line).getAsJsonObject());}
}
