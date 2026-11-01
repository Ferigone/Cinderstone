package oracle;
import com.google.gson.*;
import java.io.*;
import java.lang.reflect.*;
import java.nio.file.*;
import java.util.*;
import java.util.stream.Stream;
import jdk.internal.org.objectweb.asm.*;
import static jdk.internal.org.objectweb.asm.Opcodes.*;

/** Calls the original signed Minecraft1.21 methods; adapters supply world shapes.
 * No Minecraft class is rewritten. ASM creates only oracle.* callback subclasses.
 */
public final class player_physics_oracle {
    static final Gson JSON=new GsonBuilder().serializeNulls().create();static final PrintStream OUT=System.out;
    static Class<?> cls(String name)throws Exception{return Class.forName(name);}
    static Class<?>[] types(Class<?>...types){return types;}
    static Object call(Object target,String name,Class<?>[] types,Object...args)throws Exception{return survival_oracle.call(target,name,types,args);}
    static Object unsafe;static Method allocate;static Object player,world,border,listener;static Class<?> PLAYER,WORLD,BORDER;
    static Object alloc(Class<?> type)throws Exception{return allocate.invoke(unsafe,type);}
    static String fbits(float v){return String.format("%08x",Float.floatToIntBits(v));}
    static String dbits(double v){return String.format("%016x",Double.doubleToLongBits(v));}
    static float f(JsonElement v){return Float.intBitsToFloat(Integer.parseUnsignedInt(v.getAsString(),16));}
    static double d(JsonElement v){return Double.longBitsToDouble(Long.parseUnsignedLong(v.getAsString(),16));}
    static Object vec(JsonArray a)throws Exception{return cls("exc").getConstructor(double.class,double.class,double.class).newInstance(d(a.get(0)),d(a.get(1)),d(a.get(2)));}
    static Object box(JsonArray a)throws Exception{return cls("ewx").getConstructor(double.class,double.class,double.class,double.class,double.class,double.class).newInstance(d(a.get(0)),d(a.get(1)),d(a.get(2)),d(a.get(3)),d(a.get(4)),d(a.get(5)));}
    static Object shape(JsonArray a)throws Exception{return call(cls("exs"),"a",types(cls("ewx")),box(a));}
    static JsonArray vecOut(Object v)throws Exception {JsonArray out=new JsonArray();for(String n:new String[]{"c","d","e"})out.add(dbits(cls("exc").getField(n).getDouble(v)));return out;}
    static JsonArray boxOut(Object v)throws Exception {JsonArray out=new JsonArray();for(String n:new String[]{"a","b","c","d","e","f"})out.add(dbits(cls("ewx").getField(n).getDouble(v)));return out;}
    static void set(Object target,String owner,String name,Object value)throws Exception {survival_oracle.set(target,owner,name,value);}
    public static List<Object> filter(List<Object> all,Object bounds)throws Exception {List<Object> out=new ArrayList<>();Object query=call(cls("exs"),"a",types(cls("ewx")),bounds);Object and=cls("exg").getField("i").get(null);for(Object s:all)if((boolean)call(cls("exs"),"c",types(cls("exv"),cls("exv"),cls("exg")),s,query,and))out.add(s);return out;}
    public static Stream<Object> blockStates(Object w,Object bounds)throws Exception {WORLD.getField("lastBounds").set(w,bounds);return Stream.of(call(cls("dfy"),"a",types(int.class),WORLD.getField("allAir").getBoolean(w)?0:1));}
    static void buildAdapters()throws Exception {
        survival_oracle loader=new survival_oracle();String w="oracle/PhysicsWorld";ClassWriter c=survival_oracle.begin(w,"dcw");for(String n:new String[]{"blocks","entities"})survival_oracle.field(c,n,"Ljava/util/List;");survival_oracle.field(c,"border","Ldut;");survival_oracle.field(c,"lastBounds","Lewx;");survival_oracle.field(c,"allAir","Z");survival_oracle.getter(c,w,"C_","()Ldut;","border","Ldut;",ARETURN);
        for(String[] v:new String[][]{{"e","blocks","Ljava/lang/Iterable;"},{"c","entities","Ljava/util/List;"}}){MethodVisitor m=survival_oracle.method(c,v[0],"(Lbsr;Lewx;)"+v[2]);m.visitVarInsn(ALOAD,0);m.visitFieldInsn(GETFIELD,w,v[1],"Ljava/util/List;");m.visitVarInsn(ALOAD,2);m.visitMethodInsn(INVOKESTATIC,"oracle/player_physics_oracle","filter","(Ljava/util/List;Ljava/lang/Object;)Ljava/util/List;",false);m.visitInsn(ARETURN);survival_oracle.end(m);}
        MethodVisitor m=survival_oracle.method(c,"a","(Lewx;)Ljava/util/stream/Stream;");m.visitVarInsn(ALOAD,0);m.visitVarInsn(ALOAD,1);m.visitMethodInsn(INVOKESTATIC,"oracle/player_physics_oracle","blockStates","(Ljava/lang/Object;Ljava/lang/Object;)Ljava/util/stream/Stream;",false);m.visitInsn(ARETURN);survival_oracle.end(m);WORLD=survival_oracle.define(loader,w,c);
        String b="oracle/PhysicsBorder";c=survival_oracle.begin(b,"dut");survival_oracle.field(c,"active","Z");survival_oracle.field(c,"shape","Lexv;");survival_oracle.getter(c,b,"a","(Lbsr;Lewx;)Z","active","Z",IRETURN);survival_oracle.getter(c,b,"c","()Lexv;","shape","Lexv;",ARETURN);BORDER=survival_oracle.define(loader,b,c);
        String p="oracle/PhysicsPlayer";c=survival_oracle.begin(p,"aqv");survival_oracle.field(c,"world","Ldcw;");survival_oracle.field(c,"step","F");survival_oracle.field(c,"scale","F");survival_oracle.field(c,"ground","Z");survival_oracle.getter(c,p,"dO","()Ldcw;","world","Ldcw;",ARETURN);survival_oracle.getter(c,p,"dI","()F","step","F",FRETURN);survival_oracle.getter(c,p,"eb","()F","scale","F",FRETURN);survival_oracle.getter(c,p,"aF","()Z","ground","Z",IRETURN);PLAYER=survival_oracle.define(loader,p,c);
    }
    static void init()throws Exception {call(cls("ab"),"a",types());call(cls("akt"),"a",types());Class<?> u=cls("sun.misc.Unsafe");Field field=u.getDeclaredField("theUnsafe");field.setAccessible(true);unsafe=field.get(null);allocate=u.getMethod("allocateInstance",Class.class);buildAdapters();player=alloc(PLAYER);world=alloc(WORLD);border=alloc(BORDER);listener=alloc(cls("aru"));PLAYER.getField("world").set(player,world);WORLD.getField("border").set(world,border);set(player,"bsr","r",world);set(listener,"aru","f",player);OUT.println("SOURCE:"+cls("aru").getProtectionDomain().getCodeSource().getLocation());}
    static JsonArray dimensions(Object value)throws Exception {JsonArray row=new JsonArray();for(String method:new String[]{"a","b","c"})row.add(fbits((float)call(value,method,types())));row.add((boolean)call(value,"e",types()));return row;}
    static void extract(Path output)throws Exception {
        JsonObject result=new JsonObject();result.addProperty("minecraft","1.21");result.addProperty("oracle_inner_sha256","8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01");JsonArray poses=new JsonArray();PLAYER.getField("scale").setFloat(player,1f);
        for(Object pose:cls("bua").getEnumConstants()){JsonObject p=new JsonObject();p.addProperty("id",(int)call(pose,"a",types()));p.addProperty("name",pose.toString());p.add("dimensions",dimensions(call(player,"a",types(cls("bua")),pose)));p.add("default_dimensions",dimensions(call(player,"e",types(cls("bua")),pose)));poses.add(p);}result.add("poses",poses);
        JsonArray flags=new JsonArray();for(int i=0;i<26684;i++){Object state=call(cls("dfy"),"a",types(int.class),i);int bits=((boolean)call(state,"f",types())?1:0)|(cls("dsu").isInstance(call(state,"b",types()))?2:0)|((boolean)call(state,"i",types())?4:0);flags.add(bits);}result.add("block_flags",flags);Files.writeString(output,JSON.toJson(result)+"\n");OUT.println("EXTRACT:"+poses.size()+" poses, "+flags.size()+" states");
    }
    static List<Object> shapes(JsonArray rows)throws Exception {List<Object> out=new ArrayList<>();for(JsonElement e:rows)out.add(shape(e.getAsJsonArray()));return out;}
    static void scene(JsonObject q)throws Exception {WORLD.getField("blocks").set(world,shapes(q.getAsJsonArray("blocks")));WORLD.getField("entities").set(world,shapes(q.getAsJsonArray("entities")));boolean active=q.has("border")&&!q.get("border").isJsonNull();BORDER.getField("active").setBoolean(border,active);BORDER.getField("shape").set(border,active?shape(q.getAsJsonArray("border")):call(cls("exs"),"a",types()));if(q.has("bounds"))set(player,"bsr","aF",box(q.getAsJsonArray("bounds")));if(q.has("position"))set(player,"bsr","s",vec(q.getAsJsonArray("position")));PLAYER.getField("scale").setFloat(player,q.has("scale")?f(q.get("scale")):1f);}
    static List<Object> stepShapes(JsonObject q)throws Exception {
        List<Object> out=shapes(q.getAsJsonArray("shapes"));
        if(q.has("custom_empty"))for(JsonElement e:q.getAsJsonArray("custom_empty")){
            JsonArray row=e.getAsJsonArray();double[][] coords=new double[3][];
            for(int axis=0;axis<3;axis++){JsonArray a=row.get(axis).getAsJsonArray();coords[axis]=new double[a.size()];for(int i=0;i<a.size();i++)coords[axis][i]=d(a.get(i));}
            Object grid=cls("exf").getConstructor(int.class,int.class,int.class).newInstance(coords[0].length-1,coords[1].length-1,coords[2].length-1);
            Constructor<?> ctor=cls("exe").getDeclaredConstructor(cls("exl"),double[].class,double[].class,double[].class);ctor.setAccessible(true);out.add(ctor.newInstance(grid,coords[0],coords[1],coords[2]));
        }
        return out;
    }
    static JsonElement run(JsonObject q)throws Exception {
        switch(q.get("op").getAsString()){
            case "primitive":{JsonArray p=q.getAsJsonArray("position"),r=q.getAsJsonArray("rotation");Method invalid=cls("aru").getDeclaredMethod("b",double.class,double.class,double.class,float.class,float.class);invalid.setAccessible(true);JsonObject out=new JsonObject();out.addProperty("invalid",(boolean)invalid.invoke(null,d(p.get(0)),d(p.get(1)),d(p.get(2)),f(r.get(0)),f(r.get(1))));JsonArray position=new JsonArray();for(int i=0;i<3;i++){Method clamp=cls("aru").getDeclaredMethod(i==1?"b":"a",double.class);clamp.setAccessible(true);position.add(dbits((double)clamp.invoke(null,d(p.get(i)))));}out.add("position",position);JsonArray rotation=new JsonArray();rotation.add(fbits((float)call(cls("ayo"),"g",types(float.class),f(r.get(0)))));rotation.add(fbits((float)call(cls("ayo"),"g",types(float.class),f(r.get(1)))));out.add("rotation",rotation);return out;}
            case "dimensions":{PLAYER.getField("scale").setFloat(player,f(q.get("scale")));return dimensions(call(player,"a",types(cls("bua")),cls("bua").getEnumConstants()[q.get("pose").getAsInt()]));}
            case "step_heights":{Object b=box(q.getAsJsonArray("bounds"));Method m=cls("bsr").getDeclaredMethod("a",cls("ewx"),List.class,float.class,float.class);m.setAccessible(true);float[] heights=(float[])m.invoke(null,b,stepShapes(q),f(q.get("step")),f(q.get("baseline")));JsonArray out=new JsonArray();for(float v:heights)out.add(fbits(v));return out;}
            case "collide":{scene(q);PLAYER.getField("step").setFloat(player,f(q.get("step")));PLAYER.getField("ground").setBoolean(player,q.get("ground").getAsBoolean());Method m=cls("bsr").getDeclaredMethod("a",cls("exc"));m.setAccessible(true);return vecOut(m.invoke(player,vec(q.getAsJsonArray("movement"))));}
            case "pose_fits":{scene(q);return new JsonPrimitive((boolean)call(player,"h",types(cls("bua")),cls("bua").getEnumConstants()[q.get("pose").getAsInt()]));}
            case "new_collision":{scene(q);Method m=cls("aru").getDeclaredMethod("a",cls("dcz"),cls("ewx"),double.class,double.class,double.class);m.setAccessible(true);JsonArray t=q.getAsJsonArray("target");return new JsonPrimitive((boolean)m.invoke(listener,world,box(q.getAsJsonArray("old_bounds")),d(t.get(0)),d(t.get(1)),d(t.get(2))));}
            case "floating_bounds":{set(player,"bsr","aF",box(q.getAsJsonArray("bounds")));WORLD.getField("allAir").setBoolean(world,q.get("air").getAsBoolean());Method m=cls("aru").getDeclaredMethod("b",cls("bsr"));m.setAccessible(true);JsonObject out=new JsonObject();out.addProperty("all_air",(boolean)m.invoke(listener,player));out.add("bounds",boxOut(WORLD.getField("lastBounds").get(world)));return out;}
            default:throw new IllegalArgumentException(q.toString());
        }
    }
    public static void main(String[]args)throws Exception {init();if(args.length>0&&args[0].equals("extract")){extract(Path.of(args[1]));return;}BufferedReader in=new BufferedReader(new InputStreamReader(System.in));for(String line;(line=in.readLine())!=null;)try{OUT.println("ORACLE:"+JSON.toJson(run(JsonParser.parseString(line).getAsJsonObject())));}catch(Throwable e){OUT.println("QUERY:"+line);e.printStackTrace(OUT);throw e;}}
}
