package oracle;

import com.google.gson.*;
import java.io.*;
import java.lang.reflect.*;
import java.util.*;
import jdk.internal.org.objectweb.asm.*;
import static jdk.internal.org.objectweb.asm.Opcodes.*;

/** Actual cjh instance: no Entity or ItemEntity overrides, bytecode edits or
 * physics substitutes. The generated world supplies blocks/availability and
 * an empty neighbouring-entity/listener environment. Original baseTick, move,
 * collision, support, inside-block and fluid routines all execute normally.
 */
public final class item_full_oracle {
    static final Gson JSON=new GsonBuilder().serializeNulls().create();static final PrintStream OUT=System.out;
    static Class<?> C(String n)throws Exception{return Class.forName(n);}
    static Class<?>[] T(Class<?>...t){return t;}
    static Object call(Object o,String n,Class<?>[]t,Object...a)throws Exception{
        Class<?> c=o instanceof Class<?>?(Class<?>)o:o.getClass();
        try{Method m=c.getMethod(n,t);m.setAccessible(true);return m.invoke(o instanceof Class<?>?null:o,a);}catch(NoSuchMethodException e){return survival_oracle.call(o,n,t,a);}
    }
    static void set(Object o,String c,String f,Object v)throws Exception{survival_oracle.set(o,c,f,v);}
    static Object get(Object o,String c,String n)throws Exception{Field f=C(c).getDeclaredField(n);f.setAccessible(true);return f.get(o);}
    static String fb(float v){return String.format("%08x",Float.floatToRawIntBits(v));}
    static String db(double v){return String.format("%016x",Double.doubleToRawLongBits(v));}
    static float f(JsonElement v){return Float.intBitsToFloat(Integer.parseUnsignedInt(v.getAsString(),16));}
    static double d(JsonElement v){return Double.longBitsToDouble(Long.parseUnsignedLong(v.getAsString(),16));}
    static Class<?> WORLD;static Object unsafe;static Method allocate;static Object world,item,water,lava;
    static Object[] states;static Map<String,Integer> blocks;static long gameTime;static JsonArray events;
    static Object alloc(Class<?> c)throws Exception{return allocate.invoke(unsafe,c);}
    static int coordinate(Object pos,String name)throws Exception{return (int)call(pos,name,T());}
    public static Object block(Object pos)throws Exception{
        int x=coordinate(pos,"u"),y=coordinate(pos,"v"),z=coordinate(pos,"w");
        Integer id=blocks.get(x+","+y+","+z);
        if(id==null)id=y < -64||y>319?0:y<=60?1:y<=62?10:y==63?9:0;
        return states[id];
    }
    public static Object fluid(Object pos)throws Exception{return call(block(pos),"u",T());}
    public static long time(){return gameTime;}
    public static void event(Object holder)throws Exception{events.add("game_event/"+call(holder,"g",T()));}
    static void listMethod(ClassWriter c,String name,String desc){MethodVisitor m=survival_oracle.method(c,name,desc);m.visitMethodInsn(INVOKESTATIC,"java/util/List","of","()Ljava/util/List;",true);m.visitInsn(ARETURN);survival_oracle.end(m);}
    static void constant(ClassWriter c,String n,int v){MethodVisitor m=survival_oracle.method(c,n,"()I");m.visitLdcInsn(v);m.visitInsn(IRETURN);survival_oracle.end(m);}
    static void trueMethod(ClassWriter c,String n,String desc){MethodVisitor m=survival_oracle.method(c,n,desc);m.visitInsn(ICONST_1);m.visitInsn(IRETURN);survival_oracle.end(m);}
    static void bindFluidTags()throws Exception{
        water=C("awk").getField("a").get(null);lava=C("awk").getField("b").get(null);
        Map<Object,List<Object>> tags=new HashMap<>();
        for(String[] row:new String[][]{{"water","c","b"},{"lava","e","d"}}){
            try(InputStream in=C("bsr").getResourceAsStream("/data/minecraft/tags/fluid/"+row[0]+".json")){
                if(in==null)throw new IllegalStateException("missingofficialfluidtag");Set<String> ids=new HashSet<>();for(JsonElement e:JsonParser.parseReader(new InputStreamReader(in)).getAsJsonObject().getAsJsonArray("values"))ids.add(e.getAsString());if(!ids.equals(Set.of("minecraft:"+row[0],"minecraft:flowing_"+row[0])))throw new IllegalStateException("fluidtagchanged");
            }
            List<Object> holders=new ArrayList<>();for(int i=1;i<3;i++)holders.add(call(C("epf").getField(row[i]).get(null),"k",T()));tags.put(row[0].equals("water")?water:lava,holders);
        }
        call(C("lt").getField("c").get(null),"a",T(Map.class),tags);
    }
    static Object dimension()throws Exception{return C("dwy").getConstructor(OptionalLong.class,boolean.class,boolean.class,boolean.class,boolean.class,double.class,boolean.class,boolean.class,int.class,int.class,int.class,C("awu"),C("akr"),float.class,C("dwy$a")).newInstance(OptionalLong.empty(),true,false,false,true,1.0,true,false,-64,384,384,null,null,0f,null);}
    static void init(String tags)throws Exception{
        call(C("ab"),"a",T());call(C("akt"),"a",T());placement_tags.bind(tags);bindFluidTags();
        Class<?> u=C("sun.misc.Unsafe");Field f=u.getDeclaredField("theUnsafe");f.setAccessible(true);unsafe=f.get(null);allocate=u.getMethod("allocateInstance",Class.class);
        states=new Object[26684];for(int id=0;id<states.length;id++)states[id]=call(C("dfy"),"a",T(int.class),id);
        String name="oracle/FullItemWorld";ClassWriter c=survival_oracle.begin(name,"dcw");
        MethodVisitor m=survival_oracle.method(c,"a_","(Ljd;)Ldtc;");m.visitVarInsn(ALOAD,1);m.visitMethodInsn(INVOKESTATIC,"oracle/item_full_oracle","block","(Ljava/lang/Object;)Ljava/lang/Object;",false);m.visitTypeInsn(CHECKCAST,"dtc");m.visitInsn(ARETURN);survival_oracle.end(m);
        m=survival_oracle.method(c,"b_","(Ljd;)Lepe;");m.visitVarInsn(ALOAD,1);m.visitMethodInsn(INVOKESTATIC,"oracle/item_full_oracle","fluid","(Ljava/lang/Object;)Ljava/lang/Object;",false);m.visitTypeInsn(CHECKCAST,"epe");m.visitInsn(ARETURN);survival_oracle.end(m);
        m=survival_oracle.method(c,"c","(II)Ldcc;");m.visitVarInsn(ALOAD,0);m.visitInsn(ARETURN);survival_oracle.end(m);
        m=survival_oracle.method(c,"c_","(Ljd;)Ldqh;");m.visitInsn(ACONST_NULL);m.visitInsn(ARETURN);survival_oracle.end(m);
        trueMethod(c,"b","(IIII)Z");trueMethod(c,"b","(II)Z");
        constant(c,"I_",-64);constant(c,"J_",384);
        m=survival_oracle.method(c,"ag","()Lbnf;");m.visitFieldInsn(GETSTATIC,"bnc","a","Lbnc;");m.visitInsn(ARETURN);survival_oracle.end(m);
        for(String[] row:new String[][]{{"border","C_","Ldut;"},{"dimension","D_","Ldwy;"},{"damage","aj","Lbrl;"},{"rules","ab","Ldcs;"}}){survival_oracle.field(c,row[0],row[2]);survival_oracle.getter(c,name,row[1],"()"+row[2],row[0],row[2],ARETURN);}
        m=survival_oracle.method(c,"Z","()J");m.visitMethodInsn(INVOKESTATIC,"oracle/item_full_oracle","time","()J",false);m.visitInsn(LRETURN);survival_oracle.end(m);
        listMethod(c,"a","(Lbsr;Lewx;Ljava/util/function/Predicate;)Ljava/util/List;");listMethod(c,"a","(Ljava/lang/Class;Lewx;Ljava/util/function/Predicate;)Ljava/util/List;");listMethod(c,"a","(Ldxn;Lewx;Ljava/util/function/Predicate;)Ljava/util/List;");listMethod(c,"x","()Ljava/util/List;");
        m=survival_oracle.method(c,"a","(Ljm;Lexc;Ldxz$a;)V");m.visitVarInsn(ALOAD,1);m.visitMethodInsn(INVOKESTATIC,"oracle/item_full_oracle","event","(Ljava/lang/Object;)V",false);m.visitInsn(RETURN);survival_oracle.end(m);
        m=survival_oracle.method(c,"r","(Ljd;)Z");m.visitInsn(ICONST_0);m.visitInsn(IRETURN);survival_oracle.end(m);
        WORLD=survival_oracle.define(new survival_oracle(),name,c);
        OUT.println("SOURCE:"+C("cjh").getProtectionDomain().getCodeSource().getLocation());
    }
    static JsonArray vector(Object v)throws Exception{JsonArray a=new JsonArray();for(String name:new String[]{"c","d","e"})a.add(db(C("exc").getField(name).getDouble(v)));return a;}
    static int dirty()throws Exception{int mask=0;Object values=call(call(item,"ar",T()),"b",T());if(values!=null)for(Object value:(List<?>)values)mask|=1<<(int)call(value,"a",T());return mask;}
    static JsonObject snapshot()throws Exception{
        JsonObject s=new JsonObject();s.add("position",vector(call(item,"dm",T())));s.add("velocity",vector(call(item,"dr",T())));Object box=call(item,"cK",T());JsonArray bounds=new JsonArray();for(String n:new String[]{"a","b","c","d","e","f"})bounds.add(db(C("ewx").getField(n).getDouble(box)));s.add("bounds",bounds);
        for(String[] row:new String[][]{{"on_ground","aG"},{"horizontal_collision","Q"},{"vertical_collision","R"},{"vertical_collision_below","S"},{"minor_horizontal_collision","T"},{"no_physics","ag"},{"has_impulse","av"},{"first_tick","an"},{"touching_water","aj"},{"was_eye_in_water","al"},{"on_ground_no_blocks","bf"}})s.addProperty(row[0],(boolean)get(item,"bsr",row[1]));
        for(String[] row:new String[][]{{"fire_ticks","aJ"},{"tick_count","ai"}})s.addProperty(row[0],(int)get(item,"bsr",row[1]));
        for(String[] row:new String[][]{{"age","i"},{"pickup_delay","j"},{"health","k"}})s.addProperty(row[0],(int)get(item,"cjh",row[1]));
        s.addProperty("fall_distance",fb((float)get(item,"bsr","ac")));s.addProperty("removed",(boolean)call(item,"dJ",T()));
        Optional<?> support=(Optional<?>)get(item,"bsr","aC");if(support.isEmpty())s.add("support",JsonNull.INSTANCE);else{JsonArray pos=new JsonArray();for(String n:new String[]{"u","v","w"})pos.add(coordinate(support.get(),n));s.add("support",pos);}
        JsonArray depths=new JsonArray();for(Object tag:new Object[]{water,lava})depths.add(db((double)call(item,"b",T(C("awu")),tag)));s.add("fluid_depths",depths);
        Set<?> eyes=(Set<?>)get(item,"bsr","aK");int eyeTags=(eyes.contains(water)?1:0)|(eyes.contains(lava)?2:0);s.addProperty("fluid_on_eyes",eyeTags);
        Field key=C("bsr").getDeclaredField("ap");key.setAccessible(true);s.addProperty("flags",((Byte)call(call(item,"ar",T()),"a",T(C("ajw")),key.get(null))).byteValue()&255);s.addProperty("dirty_mask",dirty());s.add("events",events);return s;
    }
    static JsonObject run(JsonObject q)throws Exception{
        blocks=new HashMap<>();for(JsonElement e:q.getAsJsonArray("blocks")){JsonArray a=e.getAsJsonArray();blocks.put(a.get(0).getAsInt()+","+a.get(1).getAsInt()+","+a.get(2).getAsInt(),a.get(3).getAsInt());}
        world=alloc(WORLD);WORLD.getField("border").set(world,C("dut").getConstructor().newInstance());WORLD.getField("dimension").set(world,dimension());WORLD.getField("damage").set(world,alloc(C("brl")));WORLD.getField("rules").set(world,C("dcs").getConstructor().newInstance());set(world,"dcw","z",C("dyz").getConstructor(long.class).newInstance(12345L));
        Object registry=C("lt").getField("g").get(null),stone=call(registry,"a",T(C("akr")),call(C("akr"),"a",T(String.class),"minecraft:stone"));Object stack=C("cuq").getConstructor(C("dcv"),int.class).newInstance(stone,1);
        JsonArray p=q.getAsJsonArray("position"),v=q.getAsJsonArray("velocity");item=C("cjh").getConstructor(C("dcw"),double.class,double.class,double.class,C("cuq"),double.class,double.class,double.class).newInstance(world,d(p.get(0)),d(p.get(1)),d(p.get(2)),stack,d(v.get(0)),d(v.get(1)),d(v.get(2)));
        Object random=C("dyz").getConstructor(long.class).newInstance(q.get("seed").getAsLong());set(item,"bsr","ah",random);set(item,"bsr","o",q.get("entity_id").getAsInt());set(item,"bsr","ai",q.get("tick_count").getAsInt());set(item,"bsr","aG",q.get("on_ground").getAsBoolean());set(item,"bsr","ac",f(q.get("fall_distance")));set(item,"cjh","i",q.get("age").getAsInt());set(item,"cjh","j",q.get("pickup_delay").getAsInt());call(item,"a",T(float.class,float.class),0f,0f);call(item,"f",T(boolean.class),q.get("no_gravity").getAsBoolean());
        if(!q.has("drain_initial_metadata")||q.get("drain_initial_metadata").getAsBoolean())dirty();
        JsonArray values=new JsonArray();for(int tick=0;tick<q.get("ticks").getAsInt();tick++){
            if((boolean)call(item,"dJ",T()))break;events=new JsonArray();gameTime=tick+1;
            if(!q.has("clear_has_impulse")||q.get("clear_has_impulse").getAsBoolean())set(item,"bsr","av",false);
            call(item,"bv",T());set(item,"bsr","ai",((int)get(item,"bsr","ai"))+1);call(item,"l",T());values.add(snapshot());
        }
        JsonObject result=new JsonObject();result.add("states",values);result.addProperty("rng_next_int",(int)call(random,"f",T()));return result;
    }
    public static void main(String[]args)throws Exception{init(args[0]);BufferedReader in=new BufferedReader(new InputStreamReader(System.in));for(String line;(line=in.readLine())!=null;)try{OUT.println("ORACLE:"+JSON.toJson(run(JsonParser.parseString(line).getAsJsonObject())));}catch(Throwable e){OUT.println("QUERY:"+line);e.printStackTrace(OUT);throw e;}}
}
