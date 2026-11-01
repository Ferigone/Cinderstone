package oracle;

import com.google.gson.*;
import java.io.*;
import java.lang.reflect.*;
import java.util.*;
import jdk.internal.org.objectweb.asm.*;
import static jdk.internal.org.objectweb.asm.Opcodes.*;

/** Original Entity.baseTick, updateInWaterStateAndDoFluidPushing, fluid-on-eyes,
 * lavaHurt, fire/frozen/sharedflags code. Controlled low-level world effects and
 * ordered callbacks are oracle-only; Minecraft classes are never rewritten.
 */
public final class entity_base_oracle {
    static final Gson JSON=new Gson();static final PrintStream OUT=System.out;
    static Class<?> cls(String n)throws Exception{return Class.forName(n);}
    static Class<?>[] types(Class<?>...t){return t;}
    static Object call(Object o,String n,Class<?>[]t,Object...a)throws Exception{return survival_oracle.call(o,n,t,a);}
    static void set(Object o,String owner,String n,Object v)throws Exception{survival_oracle.set(o,owner,n,v);}
    static Object get(Object o,String owner,String n)throws Exception{Field f=cls(owner).getDeclaredField(n);f.setAccessible(true);return f.get(o);}
    static final survival_oracle ADAPTERS=new survival_oracle();static Object unsafe;static Method allocate;static Object world,entity,vehicle,waterTag,lavaTag,damageSources,fireDamage,lavaDamage;
    static Class<?> ENTITY,WORLD,BOAT,LEASH;static JsonObject q;static JsonArray events;
    static Object alloc(Class<?> t)throws Exception{return allocate.invoke(unsafe,t);}
    static float f(JsonElement v){return Float.intBitsToFloat(Integer.parseUnsignedInt(v.getAsString(),16));}
    static double d(JsonElement v){return Double.longBitsToDouble(Long.parseUnsignedLong(v.getAsString(),16));}
    static String fb(float v){return String.format("%08x",Float.floatToRawIntBits(v));}
    static String db(double v){return String.format("%016x",Double.doubleToRawLongBits(v));}
    static boolean b(String n){return q.has(n)&&q.get(n).getAsBoolean();}
    public static void event(String name){events.add(name);}
    public static void portal(){if(b("portal_active")&&b("server_level"))event("portal");}
    public static void stopRiding(Object actor)throws Exception{event("stop_riding");set(actor,"bsr","q",null);}
    public static boolean flow(Object actor,Object tag,double coefficient)throws Exception{
        boolean water=tag.equals(waterTag);event(water?"fluid_water":"fluid_lava");
        double expected=water?.014:(b("ultrawarm")?.007:.0023333333333333335);
        if(Double.doubleToRawLongBits(coefficient)!=Double.doubleToRawLongBits(expected))throw new IllegalStateException("fluidcoefficient");
        double depth=d(q.get(water?"water_depth":"lava_depth"));
        Object map=get(actor,"bsr","ak");call(map,"put",types(Object.class,double.class),tag,depth);
        return depth>0.;
    }
    public static Object eyeFluid(Object position)throws Exception{
        int state=q.get("eye_fluid").getAsInt();Object block=call(cls("dfy"),"a",types(int.class),state);
        return call(block,"u",types());
    }
    public static boolean hurt(Object source,float amount){event(source==lavaDamage?"hurt_lava":"hurt_fire");return b("hurt_result");}
    public static void sound(Object actor,Object sound,float volume,float pitch)throws Exception{
        if(!b("silent")){event("lava_sound");JsonObject last=new JsonObject();last.addProperty("volume",fb(volume));last.addProperty("pitch",fb(pitch));q.add("observed_sound",last);}
    }
    public static void below(Object actor)throws Exception{event("discard_below_world");set(actor,"bsr","aH",cls("bsr$c").getField("b").get(null));}
    public static void melt(int id,int data){if(id!=1009||data!=1)throw new IllegalStateException("meltevent");event("melt_frozen");}
    public static Object leash(){event("leash_empty");return null;}
    static void boolGetter(ClassWriter c,String name,String field){MethodVisitor m=survival_oracle.method(c,name,"()Z");m.visitLdcInsn(field);m.visitMethodInsn(INVOKESTATIC,"oracle/entity_base_oracle","flag","(Ljava/lang/String;)Z",false);m.visitInsn(IRETURN);survival_oracle.end(m);}
    public static boolean flag(String name){return b(name);}
    static void eventMethod(ClassWriter c,String name,String event){MethodVisitor m=survival_oracle.method(c,name,"()V");m.visitLdcInsn(event);m.visitMethodInsn(INVOKESTATIC,"oracle/entity_base_oracle","event","(Ljava/lang/String;)V",false);m.visitInsn(RETURN);survival_oracle.end(m);}
    static void init()throws Exception{
        call(cls("ab"),"a",types());call(cls("akt"),"a",types());
        Class<?> u=cls("sun.misc.Unsafe");Field uf=u.getDeclaredField("theUnsafe");uf.setAccessible(true);unsafe=uf.get(null);allocate=u.getMethod("allocateInstance",Class.class);
        waterTag=cls("awk").getField("a").get(null);lavaTag=cls("awk").getField("b").get(null);
        // Bootstrap alone has not loaded data-pack tags. Bind the exact two
        // original built-in fluid tag lists before exercising private eye code.
        Map<Object,List<Object>> fluidTags=new HashMap<>();
        for(String[] row:new String[][]{{"water","c","b"},{"lava","e","d"}}){
            try(InputStream in=cls("bsr").getResourceAsStream("/data/minecraft/tags/fluid/"+row[0]+".json")){
                if(in==null)throw new IllegalStateException("missingofficialfluidtag");
                JsonArray values=JsonParser.parseReader(new InputStreamReader(in)).getAsJsonObject().getAsJsonArray("values");
                Set<String> ids=new HashSet<>();for(JsonElement v:values)ids.add(v.getAsString());
                if(!ids.equals(Set.of("minecraft:"+row[0],"minecraft:flowing_"+row[0])))throw new IllegalStateException("unexpectedofficialfluidtag");
            }
            List<Object> holders=new ArrayList<>();for(int i=1;i<3;i++)holders.add(call(cls("epf").getField(row[i]).get(null),"k",types()));
            fluidTags.put(row[0].equals("water")?waterTag:lavaTag,holders);
        }
        call(cls("lt").getField("c").get(null),"a",types(Map.class),fluidTags);
        damageSources=alloc(cls("brl"));fireDamage=alloc(cls("brk"));lavaDamage=alloc(cls("brk"));set(damageSources,"brl","e",fireDamage);set(damageSources,"brl","f",lavaDamage);
        String name="oracle/BaseWorld";ClassWriter c=survival_oracle.begin(name,"dcw");
        MethodVisitor m=survival_oracle.method(c,"ag","()Lbnf;");m.visitFieldInsn(GETSTATIC,"bnc","a","Lbnc;");m.visitInsn(ARETURN);survival_oracle.end(m);
        m=survival_oracle.method(c,"I_","()I");m.visitLdcInsn("min_y");m.visitMethodInsn(INVOKESTATIC,"oracle/entity_base_oracle","number","(Ljava/lang/String;)I",false);m.visitInsn(IRETURN);survival_oracle.end(m);
        m=survival_oracle.method(c,"b_","(Ljd;)Lepe;");m.visitVarInsn(ALOAD,1);m.visitMethodInsn(INVOKESTATIC,"oracle/entity_base_oracle","eyeFluid","(Ljava/lang/Object;)Ljava/lang/Object;",false);m.visitTypeInsn(CHECKCAST,"epe");m.visitInsn(ARETURN);survival_oracle.end(m);
        m=survival_oracle.method(c,"a","(Lcmx;ILjd;I)V");m.visitVarInsn(ILOAD,2);m.visitVarInsn(ILOAD,4);m.visitMethodInsn(INVOKESTATIC,"oracle/entity_base_oracle","melt","(II)V",false);m.visitInsn(RETURN);survival_oracle.end(m);
        survival_oracle.field(c,"dimension","Ldwy;");survival_oracle.getter(c,name,"D_","()Ldwy;","dimension","Ldwy;",ARETURN);WORLD=survival_oracle.define(ADAPTERS,name,c);
        name="oracle/BaseEntity";c=survival_oracle.begin(name,"bsr");survival_oracle.field(c,"world","Ldcw;");survival_oracle.field(c,"sources","Lbrl;");survival_oracle.getter(c,name,"dO","()Ldcw;","world","Ldcw;",ARETURN);survival_oracle.getter(c,name,"dP","()Lbrl;","sources","Lbrl;",ARETURN);
        boolGetter(c,"be","fire_immune");boolGetter(c,"br","sprint_particles");boolGetter(c,"R_","spectator");
        m=survival_oracle.method(c,"originalSwimming","()V");m.visitVarInsn(ALOAD,0);m.visitMethodInsn(INVOKESPECIAL,"bsr","bl","()V",false);m.visitInsn(RETURN);survival_oracle.end(m);
        m=survival_oracle.method(c,"originalSprint","()Z");m.visitVarInsn(ALOAD,0);m.visitMethodInsn(INVOKESPECIAL,"bsr","br","()Z",false);m.visitInsn(IRETURN);survival_oracle.end(m);
        m=survival_oracle.method(c,"ad","()V");m.visitVarInsn(ALOAD,0);m.visitMethodInsn(INVOKESTATIC,"oracle/entity_base_oracle","stopRiding","(Ljava/lang/Object;)V",false);m.visitInsn(RETURN);survival_oracle.end(m);
        m=survival_oracle.method(c,"bP","()V");m.visitMethodInsn(INVOKESTATIC,"oracle/entity_base_oracle","portal","()V",false);m.visitInsn(RETURN);survival_oracle.end(m);
        eventMethod(c,"bs","sprint_particle");eventMethod(c,"bo","water_splash");eventMethod(c,"bl","swimming");
        m=survival_oracle.method(c,"a","(Lawu;D)Z");m.visitVarInsn(ALOAD,0);m.visitVarInsn(ALOAD,1);m.visitVarInsn(DLOAD,2);m.visitMethodInsn(INVOKESTATIC,"oracle/entity_base_oracle","flow","(Ljava/lang/Object;Ljava/lang/Object;D)Z",false);m.visitInsn(IRETURN);survival_oracle.end(m);
        m=survival_oracle.method(c,"a","(Lbrk;F)Z");m.visitVarInsn(ALOAD,1);m.visitVarInsn(FLOAD,2);m.visitMethodInsn(INVOKESTATIC,"oracle/entity_base_oracle","hurt","(Ljava/lang/Object;F)Z",false);m.visitInsn(IRETURN);survival_oracle.end(m);
        m=survival_oracle.method(c,"a","(Lavo;FF)V");m.visitVarInsn(ALOAD,0);m.visitVarInsn(ALOAD,1);m.visitVarInsn(FLOAD,2);m.visitVarInsn(FLOAD,3);m.visitMethodInsn(INVOKESTATIC,"oracle/entity_base_oracle","sound","(Ljava/lang/Object;Ljava/lang/Object;FF)V",false);m.visitInsn(RETURN);survival_oracle.end(m);
        m=survival_oracle.method(c,"aE","()V");m.visitVarInsn(ALOAD,0);m.visitMethodInsn(INVOKESTATIC,"oracle/entity_base_oracle","below","(Ljava/lang/Object;)V",false);m.visitInsn(RETURN);survival_oracle.end(m);ENTITY=survival_oracle.define(ADAPTERS,name,c);
        name="oracle/BaseBoat";c=survival_oracle.begin(name,"cov");boolGetter(c,"bk","boat_underwater");BOAT=survival_oracle.define(ADAPTERS,name,c);
        c=new ClassWriter(ClassWriter.COMPUTE_FRAMES|ClassWriter.COMPUTE_MAXS);name="oracle/BaseLeashEntity";c.visit(V21,ACC_PUBLIC,name,null,"oracle/BaseEntity",new String[]{"btk"});m=survival_oracle.method(c,"X_","()Lbtk$a;");m.visitMethodInsn(INVOKESTATIC,"oracle/entity_base_oracle","leash","()Ljava/lang/Object;",false);m.visitTypeInsn(CHECKCAST,"btk$a");m.visitInsn(ARETURN);survival_oracle.end(m);survival_oracle.noOp(c,"a","(Lbtk$a;)V");LEASH=survival_oracle.define(ADAPTERS,name,c);
        OUT.println("SOURCE:"+cls("bsr").getProtectionDomain().getCodeSource().getLocation());
    }
    public static int number(String n){return q.get(n).getAsInt();}
    static void data(Object actor,int flags,int frozen)throws Exception{
        Object rows=Array.newInstance(cls("aka$b"),8);Object flag=null,freeze=null;
        String[] fields={"ap","aP","aQ","aR","aS","aT","at","aU"};
        Object[] values={(byte)flags,300,Optional.empty(),false,b("silent"),false,cls("bua").getEnumConstants()[q.has("pose")?q.get("pose").getAsInt():0],frozen};
        for(int i=0;i<fields.length;i++){Field f=cls("bsr").getDeclaredField(fields[i]);f.setAccessible(true);Object key=f.get(null);if(i==0)flag=key;if(i==7)freeze=key;Array.set(rows,(int)call(key,"a",types()),cls("aka$b").getConstructor(cls("ajw"),Object.class).newInstance(key,values[i]));}
        Object holder=Proxy.newProxyInstance(cls("ajz").getClassLoader(),new Class<?>[]{cls("ajz")},(o,m,a)->null);Constructor<?> ctor=cls("aka").getDeclaredConstructor(cls("ajz"),rows.getClass());ctor.setAccessible(true);set(actor,"bsr","ao",ctor.newInstance(holder,rows));
        Object data=get(actor,"bsr","ao");int pre=q.has("pre_dirty")?q.get("pre_dirty").getAsInt():0;
        if((pre&1)!=0){call(data,"a",types(cls("ajw"),Object.class),flag,(byte)(flags^1));call(data,"a",types(cls("ajw"),Object.class),flag,(byte)flags);}
        if((pre&128)!=0){call(data,"a",types(cls("ajw"),Object.class),freeze,frozen^1);call(data,"a",types(cls("ajw"),Object.class),freeze,frozen);}
    }
    static JsonObject state()throws Exception{
        JsonObject out=new JsonObject();for(String[] row:new String[][]{{"boarding_cooldown","K"},{"fire_ticks","aJ"},{"tick_count","ai"},{"portal_cooldown","aX"}})out.addProperty(row[0],(int)get(entity,"bsr",row[1]));
        for(String[] row:new String[][]{{"walk_dist_o","Y"},{"walk_dist","Z"},{"yaw_o","O"},{"pitch_o","P"},{"fall_distance","ac"}})out.addProperty(row[0],fb((float)get(entity,"bsr",row[1])));
        for(String[] row:new String[][]{{"in_powder_snow","az"},{"was_in_powder_snow","aA"},{"first_tick","an"},{"touching_water","aj"},{"was_eye_in_water","al"}})out.addProperty(row[0],(boolean)get(entity,"bsr",row[1]));
        out.addProperty("vehicle_present",get(entity,"bsr","q")!=null);out.addProperty("removed",(boolean)call(entity,"dJ",types()));out.addProperty("cache_cleared",get(entity,"bsr","bj")==null);out.addProperty("frozen_ticks",(int)call(entity,"cn",types()));
        Field flagKey=cls("bsr").getDeclaredField("ap");flagKey.setAccessible(true);out.addProperty("flags",((Byte)call(get(entity,"bsr","ao"),"a",types(cls("ajw")),flagKey.get(null))).byteValue()&255);
        JsonArray eye=new JsonArray();Set<?> tags=(Set<?>)get(entity,"bsr","aK");if(tags.contains(waterTag))eye.add("water");if(tags.contains(lavaTag))eye.add("lava");out.add("eye_tags",eye);
        JsonArray depths=new JsonArray();for(Object tag:new Object[]{waterTag,lavaTag})depths.add(db((double)call(entity,"b",types(cls("awu")),tag)));out.add("fluid_depths",depths);
        int mask=0;Object dirty=call(get(entity,"bsr","ao"),"b",types());if(dirty!=null)for(Object value:(List<?>)dirty)mask|=1<<(int)call(value,"a",types());out.addProperty("dirty_mask",mask);
        return out;
    }
    static JsonObject run(JsonObject query)throws Exception{
        q=query;events=new JsonArray();world=alloc(WORLD);entity=alloc(b("leashable")?LEASH:ENTITY);
        WORLD.getField("dimension").set(world,dimensionValue(b("ultrawarm")));ENTITY.getField("world").set(entity,world);ENTITY.getField("sources").set(entity,damageSources);set(world,"dcw","B",b("client"));set(entity,"bsr","r",world);
        for(String[] row:new String[][]{{"boarding_cooldown","K"},{"fire_ticks","aJ"},{"tick_count","ai"},{"portal_cooldown","aX"}})set(entity,"bsr",row[1],q.get(row[0]).getAsInt());
        for(String[] row:new String[][]{{"walk_dist_o","Y"},{"walk_dist","Z"},{"yaw_o","O"},{"pitch_o","P"},{"fall_distance","ac"},{"yaw","aD"},{"pitch","aE"}})set(entity,"bsr",row[1],f(q.get(row[0])));
        for(String[] row:new String[][]{{"in_powder_snow","az"},{"was_in_powder_snow","aA"},{"first_tick","an"},{"touching_water","aj"},{"was_eye_in_water","al"},{"visual_fire","bi"}})set(entity,"bsr",row[1],b(row[0]));
        Object pos=cls("exc").getConstructor(double.class,double.class,double.class).newInstance(8.,d(q.get("y")),8.);set(entity,"bsr","s",pos);set(entity,"bsr","t",call(cls("jd"),"a",types(double.class,double.class,double.class),8.,d(q.get("y")),8.));set(entity,"bsr","be",f(q.get("eye_height")));set(entity,"bsr","bj",call(cls("dfy"),"a",types(int.class),1));
        Set<Object> tags=new HashSet<>();for(JsonElement tag:q.getAsJsonArray("eye_tags"))tags.add(tag.getAsString().equals("water")?waterTag:lavaTag);set(entity,"bsr","aK",tags);set(entity,"bsr","ak",cls("it.unimi.dsi.fastutil.objects.Object2DoubleOpenHashMap").getConstructor().newInstance());
        Object random=cls("dyz").getConstructor(long.class).newInstance(q.get("seed").getAsLong());set(entity,"bsr","ah",random);data(entity,q.get("flags").getAsInt(),q.get("frozen_ticks").getAsInt());
        if(b("removed"))set(entity,"bsr","aH",cls("bsr$c").getField("b").get(null));
        vehicle=null;if(b("vehicle_present")){vehicle=alloc(b("boat")?BOAT:ENTITY);if(b("vehicle_removed"))set(vehicle,"bsr","aH",cls("bsr$c").getField("b").get(null));if(b("boat"))set(vehicle,"bsr","aF",cls("ewx").getConstructor(double.class,double.class,double.class,double.class,double.class,double.class).newInstance(7.,d(q.get("boat_min_y")),7.,9.,d(q.get("boat_max_y")),9.));}set(entity,"bsr","q",vehicle);
        if(q.has("op")&&q.get("op").getAsString().equals("defaults")){
            Object map=get(entity,"bsr","ak");call(map,"put",types(Object.class,double.class),lavaTag,d(q.get("lava_depth")));
            JsonObject out=new JsonObject();out.addProperty("sprint",(boolean)call(entity,"originalSprint",types()));call(entity,"originalSwimming",types());JsonObject finalState=state();out.add("flags",finalState.get("flags"));out.add("dirty_mask",finalState.get("dirty_mask"));return out;
        }
        call(entity,"aw",types());JsonObject out=new JsonObject();out.add("state",state());out.add("events",events);out.addProperty("rng_next_int",(int)random.getClass().getMethod("f").invoke(random));if(q.has("observed_sound"))out.add("sound",q.get("observed_sound"));return out;
    }
    static Object dimensionValue(boolean ultrawarm)throws Exception{
        return cls("dwy").getConstructor(OptionalLong.class,boolean.class,boolean.class,boolean.class,boolean.class,double.class,boolean.class,boolean.class,int.class,int.class,int.class,cls("awu"),cls("akr"),float.class,cls("dwy$a")).newInstance(OptionalLong.empty(),true,false,ultrawarm,true,1.0,true,false,-64,384,384,null,null,0f,null);
    }
    public static void main(String[]args)throws Exception{init();BufferedReader in=new BufferedReader(new InputStreamReader(System.in));for(String line;(line=in.readLine())!=null;)try{OUT.println("ORACLE:"+JSON.toJson(run(JsonParser.parseString(line).getAsJsonObject())));}catch(Throwable e){OUT.println("QUERY:"+line);e.printStackTrace(OUT);throw e;}}
}
