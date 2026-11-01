package oracle;

import com.google.gson.*;
import java.io.*;
import java.lang.reflect.*;
import java.util.*;
import jdk.internal.org.objectweb.asm.*;
import static jdk.internal.org.objectweb.asm.Opcodes.*;

/** Original signed FoodData/CombatRules/LivingEntity methods with isolated injected contexts.
 * ASM creates adapters in oracle.*, leaving every Minecraft class byte-identical.
 * Health/damage callbacks and world context are controlled; this is not a running world oracle.
 */
public final class survival_oracle extends ClassLoader {
    static final Gson GSON=new Gson();static final PrintStream OUT=System.out;
    static Class<?> cls(String n)throws Exception{return Class.forName(n);}
    static Object unsafe;static Method allocate;static Class<?> PLAYER,LEVEL,TYPE,DAMAGE,TRACKER;
    static Object player,level,food,abilities,attributes,source,healthPlayer,healthAttributes;
    static Class<?> CPLAYER,CFOOD,CLEVEL;static final Map<String,Method> METHODS=new HashMap<>();
    static Class<?>[] types(Class<?>...c){return c;}
    static Object call(Object receiver,String name,Class<?>[] types,Object...args)throws Exception {
        Class<?> c=receiver instanceof Class<?>?(Class<?>)receiver:receiver.getClass();String key=c.getName()+name+Arrays.toString(types);Method m=METHODS.get(key);
        if(m==null){for(Class<?> at=c;at!=null;at=at.getSuperclass()){try{m=at.getDeclaredMethod(name,types);break;}catch(NoSuchMethodException e){}}if(m==null)throw new NoSuchMethodException(key);m.setAccessible(true);METHODS.put(key,m);}return m.invoke(receiver instanceof Class<?>?null:receiver,args);
    }
    static Object original(String owner,Object receiver,String name,Class<?>[] types,Object...args)throws Exception {Method m=cls(owner).getDeclaredMethod(name,types);m.setAccessible(true);return m.invoke(receiver,args);}
    static void set(Object obj,String owner,String name,Object value)throws Exception {Field f=cls(owner).getDeclaredField(name);f.setAccessible(true);f.set(obj,value);}
    static Object alloc(Class<?> c)throws Exception{return allocate.invoke(unsafe,c);}
    static float f(JsonElement v){return Float.intBitsToFloat(Integer.parseUnsignedInt(v.getAsString(),16));}
    static double d(JsonElement v){return Double.longBitsToDouble(Long.parseUnsignedLong(v.getAsString(),16));}
    static String bits(float v){return String.format("%08x",Float.floatToIntBits(v));}
    static String dbits(double v){return String.format("%016x",Double.doubleToLongBits(v));}
    static void field(ClassWriter c,String name,String descriptor){c.visitField(ACC_PUBLIC,name,descriptor,null,null).visitEnd();}
    static MethodVisitor method(ClassWriter c,String name,String descriptor){MethodVisitor m=c.visitMethod(ACC_PUBLIC,name,descriptor,null,null);m.visitCode();return m;}
    static void end(MethodVisitor m){m.visitMaxs(0,0);m.visitEnd();}
    static void getter(ClassWriter c,String owner,String name,String descriptor,String field,String fieldType,int ret){MethodVisitor m=method(c,name,descriptor);m.visitVarInsn(ALOAD,0);m.visitFieldInsn(GETFIELD,owner,field,fieldType);m.visitInsn(ret);end(m);}
    static void noOp(ClassWriter c,String name,String descriptor){MethodVisitor m=method(c,name,descriptor);m.visitInsn(RETURN);end(m);}
    static void countCallback(ClassWriter c,String owner,String name,String descriptor,int arg,String count,String amount,boolean returnsBoolean){MethodVisitor m=method(c,name,descriptor);m.visitVarInsn(ALOAD,0);m.visitInsn(DUP);m.visitFieldInsn(GETFIELD,owner,count,"I");m.visitInsn(ICONST_1);m.visitInsn(IADD);m.visitFieldInsn(PUTFIELD,owner,count,"I");m.visitVarInsn(ALOAD,0);m.visitVarInsn(FLOAD,arg);m.visitFieldInsn(PUTFIELD,owner,amount,"F");if(returnsBoolean)m.visitInsn(ICONST_1);m.visitInsn(returnsBoolean?IRETURN:RETURN);end(m);}
    static ClassWriter begin(String name,String parent){ClassWriter c=new ClassWriter(ClassWriter.COMPUTE_FRAMES|ClassWriter.COMPUTE_MAXS);c.visit(V21,ACC_PUBLIC,name,null,parent,null);return c;}
    static Class<?> define(survival_oracle loader,String name,ClassWriter c){c.visitEnd();byte[] bytes=c.toByteArray();return loader.defineClass(name.replace('/','.'),bytes,0,bytes.length);}
    static void buildAdapters()throws Exception {
        survival_oracle loader=new survival_oracle();String w="oracle/SurvivalWorld";ClassWriter c=begin(w,"dcw");field(c,"difficulty","Lbqo;");field(c,"rules","Ldcs;");getter(c,w,"al","()Lbqo;","difficulty","Lbqo;",ARETURN);getter(c,w,"ab","()Ldcs;","rules","Ldcs;",ARETURN);LEVEL=define(loader,w,c);
        String t="oracle/SurvivalType";c=begin(t,"bsx");field(c,"immune","Z");getter(c,t,"a","(Lawu;)Z","immune","Z",IRETURN);TYPE=define(loader,t,c);
        String ds="oracle/SurvivalDamage";c=begin(ds,"brk");getterConst(c,"a","()F",FCONST_0,FRETURN);getterConst(c,"e","()Lcuq;",ACONST_NULL,ARETURN);getterConst(c,"d","()Lbsr;",ACONST_NULL,ARETURN);
        MethodVisitor m=method(c,"a","(Lawu;)Z");m.visitVarInsn(ALOAD,1);m.visitFieldInsn(GETSTATIC,"awg","h","Lawu;");Label other=new Label();m.visitJumpInsn(IF_ACMPNE,other);m.visitInsn(ICONST_1);m.visitInsn(IRETURN);m.visitLabel(other);m.visitInsn(ICONST_0);m.visitInsn(IRETURN);end(m);DAMAGE=define(loader,ds,c);
        String tr="oracle/SurvivalTracker";c=begin(tr,"brh");noOp(c,"a","(Lbrk;F)V");TRACKER=define(loader,tr,c);
        String p="oracle/SurvivalPlayer";c=begin(p,"cmx");
        for(String name:new String[]{"health","heal","damage","absorption"})field(c,name,"F");field(c,"heals","I");field(c,"hurts","I");field(c,"world","Ldcw;");field(c,"sources","Lbrl;");field(c,"tracker","Lbrh;");field(c,"effect","Lbrz;");
        getter(c,p,"dO","()Ldcw;","world","Ldcw;",ARETURN);getter(c,p,"ew","()F","health","F",FRETURN);getter(c,p,"fo","()F","absorption","F",FRETURN);getter(c,p,"dP","()Lbrl;","sources","Lbrl;",ARETURN);getter(c,p,"eL","()Lbrh;","tracker","Lbrh;",ARETURN);getter(c,p,"c","(Ljm;)Lbrz;","effect","Lbrz;",ARETURN);
        countCallback(c,p,"c","(F)V",1,"heals","heal",false);countCallback(c,p,"a","(Lbrk;F)Z",2,"hurts","damage",true);
        getterConst(c,"b","(Lbrk;)Z",ICONST_0,IRETURN);getterConst(c,"b","(Ljm;)Z",ICONST_1,IRETURN);
        noOp(c,"a","(Lakr;I)V");noOp(c,"a","(Ljm;)V");
        for(String name:new String[]{"d","e"}){m=method(c,name,"(Lbrk;F)F");m.visitVarInsn(FLOAD,2);m.visitInsn(FRETURN);end(m);}
        m=method(c,"D","(F)V");m.visitVarInsn(ALOAD,0);m.visitVarInsn(FLOAD,1);m.visitFieldInsn(PUTFIELD,p,"absorption","F");m.visitInsn(RETURN);end(m);
        m=method(c,"v","(F)V");m.visitVarInsn(ALOAD,0);m.visitVarInsn(FLOAD,1);m.visitInsn(FCONST_0);m.visitVarInsn(ALOAD,0);m.visitMethodInsn(INVOKEVIRTUAL,"btn","eN","()F",false);m.visitMethodInsn(INVOKESTATIC,"ayo","a","(FFF)F",false);m.visitFieldInsn(PUTFIELD,p,"health","F");m.visitInsn(RETURN);end(m);
        m=method(c,"resistance","(Lbrk;F)F");m.visitVarInsn(ALOAD,0);m.visitVarInsn(ALOAD,1);m.visitVarInsn(FLOAD,2);m.visitMethodInsn(INVOKESPECIAL,"btn","e","(Lbrk;F)F",false);m.visitInsn(FRETURN);end(m);
        PLAYER=define(loader,p,c);
    }
    static void getterConst(ClassWriter c,String name,String descriptor,int constant,int ret){MethodVisitor m=method(c,name,descriptor);m.visitInsn(constant);m.visitInsn(ret);end(m);}
    static Object attrs(Object target)throws Exception {Object builder=call(cls("cmx"),"fM",types());Object supplier=call(builder,"a",types());Object attrs=cls("but").getConstructor(cls("buv")).newInstance(supplier);set(target,"btn","bU",attrs);return attrs;}
    static void attr(Object map,String name,double value)throws Exception {Object holder=cls("buw").getField(name).get(null);Object instance=call(map,"a",types(cls("jm")),holder);call(instance,"a",types(double.class),value);}
    static void initialize()throws Exception {
        call(cls("ab"),"a",types());call(cls("akt"),"a",types());Class<?> u=cls("sun.misc.Unsafe");Field uf=u.getDeclaredField("theUnsafe");uf.setAccessible(true);unsafe=uf.get(null);allocate=u.getMethod("allocateInstance",Class.class);buildAdapters();
        CPLAYER=cls("cmx");CFOOD=cls("cpq");CLEVEL=cls("dcw");level=alloc(LEVEL);LEVEL.getField("rules").set(level,cls("dcs").getConstructor().newInstance());LEVEL.getField("difficulty").set(level,cls("bqo").getEnumConstants()[2]);
        player=alloc(PLAYER);PLAYER.getField("world").set(player,level);PLAYER.getField("sources").set(player,alloc(cls("brl")));PLAYER.getField("tracker").set(player,alloc(TRACKER));attributes=attrs(player);attr(attributes,"r",2048.0);abilities=cls("cmu").getConstructor().newInstance();set(player,"cmx","cA",abilities);food=CFOOD.getConstructor().newInstance();set(player,"cmx","ce",food);source=alloc(DAMAGE);Object type=alloc(TYPE);set(player,"bsr","n",type);
        healthPlayer=alloc(cls("aqv"));healthAttributes=attrs(healthPlayer);
        Object healthKey=cls("btn").getDeclaredField("bM");Field hk=(Field)healthKey;hk.setAccessible(true);healthKey=hk.get(null);Field ak=cls("cmx").getDeclaredField("e");ak.setAccessible(true);Object absorptionKey=ak.get(null);
        int count=Math.max((int)call(healthKey,"a",types()),(int)call(absorptionKey,"a",types()))+1;Object items=Array.newInstance(cls("aka$b"),count);
        for(Object key:new Object[]{healthKey,absorptionKey})Array.set(items,(int)call(key,"a",types()),cls("aka$b").getConstructor(cls("ajw"),Object.class).newInstance(key,0.0f));
        Object holder=Proxy.newProxyInstance(cls("ajz").getClassLoader(),new Class<?>[]{cls("ajz")},(obj,m,args)->null);Constructor<?> constructor=cls("aka").getDeclaredConstructor(cls("ajz"),items.getClass());constructor.setAccessible(true);set(healthPlayer,"bsr","ao",constructor.newInstance(holder,items));
        OUT.println("SOURCE:"+CFOOD.getProtectionDomain().getCodeSource().getLocation());
    }
    static void natural(boolean value)throws Exception {Object rules=LEVEL.getField("rules").get(level);Object key=cls("dcs").getField("k").get(null);Object rule=call(rules,"a",types(cls("dcs$e")),key);call(rule,"a",types(boolean.class,cls("net.minecraft.server.MinecraftServer")),value,null);}
    static void resetEvents()throws Exception {PLAYER.getField("heals").setInt(player,0);PLAYER.getField("hurts").setInt(player,0);PLAYER.getField("heal").setFloat(player,0.0f);PLAYER.getField("damage").setFloat(player,0.0f);}
    static JsonArray foodState()throws Exception {JsonArray r=new JsonArray();for(String n:new String[]{"a","b","c","d","e"}){Field field=CFOOD.getDeclaredField(n);field.setAccessible(true);if(field.getType()==float.class)r.add(bits(field.getFloat(food)));else r.add(field.getInt(food));}r.add(PLAYER.getField("heals").getInt(player));r.add(bits(PLAYER.getField("heal").getFloat(player)));r.add(PLAYER.getField("hurts").getInt(player));r.add(bits(PLAYER.getField("damage").getFloat(player)));return r;}
    static JsonElement run(JsonObject q)throws Exception {
        String op=q.get("op").getAsString();JsonArray result=new JsonArray();
        switch(op){
        case "food":{
            JsonArray init=q.getAsJsonArray("initial");for(int i=0;i<5;i++){Field field=CFOOD.getDeclaredField(""+(char)('a'+i));field.setAccessible(true);if(field.getType()==float.class)field.setFloat(food,f(init.get(i)));else field.setInt(food,init.get(i).getAsInt());}
            for(JsonElement step:q.getAsJsonArray("steps")){JsonArray a=step.getAsJsonArray();String action=a.get(0).getAsString();resetEvents();
                switch(action){
                case "eat":call(food,"a",types(int.class,float.class),a.get(1).getAsInt(),f(a.get(2)));break;
                case "props":Object props=cls("cpr").getConstructor(int.class,float.class,boolean.class,float.class,Optional.class,List.class).newInstance(a.get(1).getAsInt(),f(a.get(2)),false,1.6f,Optional.empty(),List.of());call(food,"a",types(cls("cpr")),props);break;
                case "exhaust":call(food,"a",types(float.class),f(a.get(1)));break;
                case "cause":cls("cmu").getField("a").setBoolean(abilities,a.get(2).getAsBoolean());set(level,"dcw","B",a.get(3).getAsBoolean());call(player,"E",types(float.class),f(a.get(1)));break;
                case "caneat":cls("cmu").getField("a").setBoolean(abilities,a.get(2).getAsBoolean());JsonArray eating=foodState();eating.add((boolean)call(player,"u",types(boolean.class),a.get(1).getAsBoolean()));result.add(eating);continue;
                case "tick":case "peace":{
                    LEVEL.getField("difficulty").set(level,call(cls("bqo"),"a",types(int.class),a.get(1).getAsInt()));natural(a.get(2).getAsBoolean());PLAYER.getField("health").setFloat(player,f(a.get(3)));attr(attributes,"s",f(a.get(4)));
                    if(action.equals("tick")){call(food,"a",types(CPLAYER),player);}
                    else {cls("bsr").getField("ai").setInt(player,a.get(5).getAsInt());try{original("cmx",player,"m_",types());throw new IllegalStateException("expected controlled inventory stop");}catch(InvocationTargetException ex){Throwable cause=ex.getCause();if(!(cause instanceof NullPointerException)||!cause.getMessage().contains("cmw.j()"))throw ex;}}
                    break;
                }
                default:throw new IllegalArgumentException(action);
                }
                result.add(foodState());
            }
            return result;
        }
        case "food_nbt":{
            resetEvents();JsonArray init=q.getAsJsonArray("initial");for(int i=0;i<5;i++){Field field=CFOOD.getDeclaredField(""+(char)('a'+i));field.setAccessible(true);if(field.getType()==float.class)field.setFloat(food,f(init.get(i)));else field.setInt(food,init.get(i).getAsInt());}
            Object tag=cls("ub").getConstructor().newInstance();
            for(JsonElement entry:q.getAsJsonArray("entries")){JsonArray e=entry.getAsJsonArray();String key=e.get(0).getAsString(),type=e.get(1).getAsString();JsonElement value=e.get(2);
                switch(type){case "byte":call(tag,"a",types(String.class,byte.class),key,(byte)value.getAsInt());break;case "short":call(tag,"a",types(String.class,short.class),key,(short)value.getAsInt());break;case "int":call(tag,"a",types(String.class,int.class),key,value.getAsInt());break;case "long":call(tag,"a",types(String.class,long.class),key,value.getAsLong());break;case "float":call(tag,"a",types(String.class,float.class),key,f(value));break;case "double":call(tag,"a",types(String.class,double.class),key,d(value));break;case "string":call(tag,"a",types(String.class,String.class),key,value.getAsString());break;default:throw new IllegalArgumentException(type);}
            }
            call(food,"a",types(cls("ub")),tag);JsonObject out=new JsonObject();out.add("state",foodState());Object saved=cls("ub").getConstructor().newInstance();call(food,"b",types(cls("ub")),saved);JsonArray values=new JsonArray();values.add((int)call(saved,"h",types(String.class),"foodLevel"));values.add((int)call(saved,"h",types(String.class),"foodTickTimer"));values.add(bits((float)call(saved,"j",types(String.class),"foodSaturationLevel")));values.add(bits((float)call(saved,"j",types(String.class),"foodExhaustionLevel")));out.add("saved",values);return out;
        }
        case "armor":return new JsonPrimitive(bits((float)call(cls("brg"),"a",types(cls("btn"),float.class,cls("brk"),float.class,float.class),null,f(q.get("damage")),source,f(q.get("armor")),f(q.get("toughness")))));
        case "magic":return new JsonPrimitive(bits((float)call(cls("brg"),"a",types(float.class,float.class),f(q.get("damage")),f(q.get("protection")))));
        case "resistance":{Object effect=cls("brz").getConstructor(cls("jm"),int.class,int.class).newInstance(cls("bsb").getField("k").get(null),100,q.get("amplifier").getAsInt());PLAYER.getField("effect").set(player,effect);return new JsonPrimitive(bits((float)call(player,"resistance",types(cls("brk"),float.class),source,f(q.get("damage")))));}
        case "fall_flight_gate":{cls("cmu").getField("c").setBoolean(abilities,true);boolean applied=(boolean)original("cmx",player,"a",types(float.class,float.class,cls("brk")),f(q.get("distance")),1.0f,source);if(applied)throw new IllegalStateException("may-fly fall was applied");return new JsonPrimitive(0);}
        case "fall":{attr(attributes,"x",d(q.get("safe")));attr(attributes,"k",d(q.get("attribute_multiplier")));Object type=alloc(TYPE);TYPE.getField("immune").setBoolean(type,q.get("immune").getAsBoolean());set(player,"bsr","n",type);return new JsonPrimitive((int)original("btn",player,"e",types(float.class,float.class),f(q.get("distance")),f(q.get("block_multiplier"))));}
        case "health":{attr(healthAttributes,"s",f(q.get("max")));attr(healthAttributes,"r",f(q.get("max_absorption")));call(healthPlayer,"v",types(float.class),f(q.get("health")));call(healthPlayer,"C",types(float.class),f(q.get("absorption")));for(JsonElement step:q.getAsJsonArray("steps")){JsonArray a=step.getAsJsonArray();String action=a.get(0).getAsString();call(healthPlayer,switch(action){case "heal"->"c";case "set"->"v";case "absorb"->"C";default->throw new IllegalArgumentException(action);},types(float.class),f(a.get(1)));JsonArray pair=new JsonArray();pair.add(bits((float)call(healthPlayer,"ew",types())));pair.add(bits((float)call(healthPlayer,"fo",types())));pair.add((boolean)call(healthPlayer,"gj",types()));result.add(pair);}return result;}
        case "subtract":{attr(attributes,"s",f(q.get("max")));attr(attributes,"r",f(q.get("max_absorption")));PLAYER.getField("health").setFloat(player,f(q.get("health")));PLAYER.getField("absorption").setFloat(player,f(q.get("absorption")));original("cmx",player,"f",types(cls("brk"),float.class),source,f(q.get("damage")));result.add(bits(PLAYER.getField("health").getFloat(player)));result.add(bits(PLAYER.getField("absorption").getFloat(player)));return result;}
        default:throw new IllegalArgumentException(op);
        }
    }
    public static void main(String[] args)throws Exception{initialize();BufferedReader input=new BufferedReader(new InputStreamReader(System.in));for(String line;(line=input.readLine())!=null;){try{OUT.println("ORACLE:"+GSON.toJson(run(JsonParser.parseString(line).getAsJsonObject())));}catch(Throwable e){e.printStackTrace(OUT);throw e;}}}
}
