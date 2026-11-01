package oracle;

import com.google.gson.*;
import java.lang.reflect.*;
import java.util.*;
import jdk.internal.org.objectweb.asm.*;
import static jdk.internal.org.objectweb.asm.Opcodes.*;

/** Actual ItemEntity.hurt/Entity.isInvulnerableTo/ItemStack.canBeHurtBy.
 * Only DamageSource tag/creative/source queries and world game-event delivery
 * are controlled callbacks. No Entity or ItemStack methods are overridden.
 */
public final class item_hurt_oracle {
    static final Gson JSON=new GsonBuilder().serializeNulls().create();static final java.io.PrintStream OUT=System.out;
    static Class<?> C(String n)throws Exception{return Class.forName(n);}static Class<?>[] T(Class<?>...t){return t;}
    static Object call(Object o,String n,Class<?>[]t,Object...v)throws Exception{return survival_oracle.call(o,n,t,v);}
    static void set(Object o,String c,String n,Object v)throws Exception{survival_oracle.set(o,c,n,v);}
    static Object get(Object o,String c,String n)throws Exception{Field f=C(c).getDeclaredField(n);f.setAccessible(true);return f.get(o);}
    static Object unsafe;static Method allocate;static Class<?> WORLD,SOURCE;static Object item;static JsonObject query;static JsonArray trace;
    static Object alloc(Class<?> c)throws Exception{return allocate.invoke(unsafe,c);}
    public static boolean tag(Object tag)throws Exception{
        for(String[] row:new String[][]{{"bypass","d"},{"fire","i"},{"explosion","l"},{"fall","m"}})if(tag.equals(C("awg").getField(row[1]).get(null))){trace.add("tag/"+row[0]);return query.get(row[0]).getAsBoolean();}
        throw new IllegalArgumentException("unexpecteddamagetag"+tag);
    }
    public static boolean creative(){trace.add("creative_source");return query.get("creative").getAsBoolean();}
    public static Object source(){trace.add("causing_entity");return null;}
    public static void event(Object holder)throws Exception{
        JsonObject e=new JsonObject();e.addProperty("event",(String)holder.getClass().getMethod("g").invoke(holder));e.addProperty("health",(int)get(item,"cjh","k"));e.addProperty("hurt_marked",(boolean)get(item,"bsr","U"));e.addProperty("removed",(boolean)call(item,"dJ",T()));trace.add(e);
    }
    static void init()throws Exception{
        call(C("ab"),"a",T());call(C("akt"),"a",T());Class<?> u=C("sun.misc.Unsafe");Field f=u.getDeclaredField("theUnsafe");f.setAccessible(true);unsafe=f.get(null);allocate=u.getMethod("allocateInstance",Class.class);
        String name="oracle/ItemHurtWorld";ClassWriter c=survival_oracle.begin(name,"dcw");MethodVisitor m=survival_oracle.method(c,"a","(Ljm;Lexc;Ldxz$a;)V");m.visitVarInsn(ALOAD,1);m.visitMethodInsn(INVOKESTATIC,"oracle/item_hurt_oracle","event","(Ljava/lang/Object;)V",false);m.visitInsn(RETURN);survival_oracle.end(m);WORLD=survival_oracle.define(new survival_oracle(),name,c);
        name="oracle/ControlledItemDamageSource";c=survival_oracle.begin(name,"brk");m=survival_oracle.method(c,"a","(Lawu;)Z");m.visitVarInsn(ALOAD,1);m.visitMethodInsn(INVOKESTATIC,"oracle/item_hurt_oracle","tag","(Ljava/lang/Object;)Z",false);m.visitInsn(IRETURN);survival_oracle.end(m);
        m=survival_oracle.method(c,"h","()Z");m.visitMethodInsn(INVOKESTATIC,"oracle/item_hurt_oracle","creative","()Z",false);m.visitInsn(IRETURN);survival_oracle.end(m);m=survival_oracle.method(c,"d","()Lbsr;");m.visitMethodInsn(INVOKESTATIC,"oracle/item_hurt_oracle","source","()Ljava/lang/Object;",false);m.visitTypeInsn(CHECKCAST,"bsr");m.visitInsn(ARETURN);survival_oracle.end(m);SOURCE=survival_oracle.define(new survival_oracle(),name,c);
        OUT.println("SOURCE:"+C("cjh").getProtectionDomain().getCodeSource().getLocation());
    }
    static JsonObject run(JsonObject q)throws Exception{
        query=q;trace=new JsonArray();Object world=alloc(WORLD);set(world,"dcw","B",q.get("client").getAsBoolean());Object registry=C("lt").getField("g").get(null),value=C("jz").getMethod("a",C("akr")).invoke(registry,C("akr").getMethod("a",String.class).invoke(null,q.get("item").getAsString()));Object stack=C("cuq").getConstructor(C("dcv"),int.class).newInstance(value,q.get("count").getAsInt());
        item=C("cjh").getConstructor(C("dcw"),double.class,double.class,double.class,C("cuq"),double.class,double.class,double.class).newInstance(world,8.,64.,8.,stack,0.,0.,0.);set(item,"cjh","k",q.get("health").getAsInt());set(item,"bsr","aY",q.get("invulnerable").getAsBoolean());set(item,"bsr","U",q.get("hurt_marked").getAsBoolean());if(q.get("removed").getAsBoolean())set(item,"bsr","aH",C("bsr$c").getField("b").get(null));Object source=alloc(SOURCE);
        float amount=Float.intBitsToFloat(Integer.parseUnsignedInt(q.get("amount_bits").getAsString(),16));boolean result=(boolean)call(item,"a",T(C("brk"),float.class),source,amount);
        JsonObject out=new JsonObject();out.addProperty("accepted",result);out.addProperty("health",(int)get(item,"cjh","k"));out.addProperty("hurt_marked",(boolean)get(item,"bsr","U"));out.addProperty("removed",(boolean)call(item,"dJ",T()));out.addProperty("fire_immune",(boolean)call(item,"be",T()));out.add("trace",trace);return out;
    }
    static JsonObject q(String item,int count,int health,float amount,int mask,boolean client,boolean invulnerable,boolean removed,boolean creative,boolean marked){JsonObject q=new JsonObject();q.addProperty("item",item);q.addProperty("count",count);q.addProperty("health",health);q.addProperty("amount_bits",String.format("%08x",Float.floatToRawIntBits(amount)));q.addProperty("bypass",(mask&1)!=0);q.addProperty("fire",(mask&2)!=0);q.addProperty("explosion",(mask&4)!=0);q.addProperty("fall",(mask&8)!=0);q.addProperty("client",client);q.addProperty("invulnerable",invulnerable);q.addProperty("removed",removed);q.addProperty("creative",creative);q.addProperty("hurt_marked",marked);return q;}
    static void add(JsonArray rows,JsonObject q)throws Exception{JsonObject row=new JsonObject();row.add("query",q);row.add("result",run(q));rows.add(row);}
    public static void main(String[]args)throws Exception{
        init();
        if(args.length>0&&args[0].equals("--replay")){
            java.io.BufferedReader input=new java.io.BufferedReader(new java.io.InputStreamReader(System.in));
            for(String line;(line=input.readLine())!=null;)OUT.println("ORACLE:"+JSON.toJson(run(JsonParser.parseString(line).getAsJsonObject())));
            return;
        }
        JsonArray rows=new JsonArray();
        for(String item:List.of("minecraft:stone","minecraft:nether_star","minecraft:netherite_sword"))for(int mask:new int[]{0,1,2,3,4,8})for(int bits=0;bits<32;bits++)add(rows,q(item,1,5,1f,mask,(bits&1)!=0,(bits&2)!=0,(bits&4)!=0,(bits&8)!=0,(bits&16)!=0));
        for(int health:new int[]{Integer.MIN_VALUE,-32768,-1,0,1,5,32767,Integer.MAX_VALUE})for(float amount:new float[]{Float.NEGATIVE_INFINITY,-3f,-.5f,-0f,0f,Float.MIN_VALUE,.01f,.1f,.99999994f,1f,1.1f,4f,4.01f,5f,Float.MAX_VALUE,Float.POSITIVE_INFINITY,Float.NaN})add(rows,q("minecraft:stone",1,health,amount,0,false,false,false,false,false));
        for(String item:List.of("minecraft:stone","minecraft:nether_star","minecraft:netherite_sword"))for(int mask:new int[]{0,2,4,8})add(rows,q(item,0,5,1f,mask,false,false,false,false,false));
        for(float amount:new float[]{1f,4f})for(String item:List.of("minecraft:stone","minecraft:netherite_sword"))add(rows,q(item,1,5,amount,2,false,false,false,false,false));
        JsonObject output=new JsonObject();output.addProperty("kind","original-class executable probes with controlled DamageSource predicates");output.addProperty("minecraft","1.21");output.addProperty("oracle_inner_sha256","8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01");output.addProperty("native_comparison",false);output.addProperty("queries",rows.size());output.add("cases",rows);OUT.println("ORACLE:"+JSON.toJson(output));
    }
}
