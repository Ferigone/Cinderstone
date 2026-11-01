package oracle;

import com.google.gson.*;
import java.lang.reflect.*;
import io.netty.buffer.*;
import java.util.Base64;

/** Enumerate the actual onDestroyed dispatch target of every signed-JAR item. */
public final class item_destroyed_oracle {
    static final java.io.PrintStream OUT=System.out;
    static Class<?> C(String name)throws Exception{return Class.forName(name);}
    static Object access;
    static String wire(Object stack)throws Exception {
        ByteBuf bytes=Unpooled.buffer();
        try {Object buffer=C("wk").getConstructor(ByteBuf.class,C("ka")).newInstance(bytes,access);C("yx").getMethod("encode",Object.class,Object.class).invoke(C("cuq").getField("h").get(null),buffer,stack);byte[] raw=new byte[bytes.readableBytes()];bytes.getBytes(bytes.readerIndex(),raw);return Base64.getEncoder().encodeToString(raw);}finally{bytes.release();}
    }
    public static void main(String[]args)throws Exception{
        item_hurt_oracle.init();
        access=C("ka").getMethod("a",C("jz")).invoke(null,C("lt").getField("aA").get(null));
        Object registry=C("lt").getField("g").get(null);JsonArray rows=new JsonArray();
        for(Object item:(Iterable<?>)registry){
            JsonObject row=new JsonObject();
            row.addProperty("item_id",(int)C("jz").getMethod("a",Object.class).invoke(registry,item));
            row.addProperty("item_name",C("jz").getMethod("b",Object.class).invoke(registry,item).toString());
            row.addProperty("item_class",item.getClass().getName());
            row.addProperty("declaring_class",item.getClass().getMethod("a",C("cjh")).getDeclaringClass().getName());
            Object world=item_hurt_oracle.alloc(item_hurt_oracle.WORLD);item_hurt_oracle.set(world,"dcw","B",false);
            Object stack=C("cuq").getConstructor(C("dcv"),int.class).newInstance(item,1);
            Object entity=C("cjh").getConstructor(C("dcw"),double.class,double.class,double.class,C("cuq"),double.class,double.class,double.class).newInstance(world,8.,64.,8.,stack,0.,0.,0.);
            Object entityRandom=C("dyz").getConstructor(long.class).newInstance(12345L),worldRandom=C("dyz").getConstructor(long.class).newInstance(54321L);
            item_hurt_oracle.set(entity,"bsr","ah",entityRandom);item_hurt_oracle.set(world,"dcw","z",worldRandom);
            item_hurt_oracle.item=entity;item_hurt_oracle.trace=new JsonArray();
            row.addProperty("default_stack_contains_container",(boolean)C("cuq").getMethod("b",C("kp")).invoke(stack,C("kq").getField("ab").get(null)));
            row.addProperty("stack_wire_before",wire(stack));
            item_hurt_oracle.call(stack,"a",new Class<?>[]{C("cjh")},entity);
            row.addProperty("stack_wire_after",wire(stack));
            row.addProperty("default_stack_on_destroyed_returned",true);
            row.addProperty("stack_count_after",(int)item_hurt_oracle.call(stack,"H",new Class<?>[]{}));
            row.addProperty("patch_size_after",(int)item_hurt_oracle.call(item_hurt_oracle.call(stack,"d",new Class<?>[]{}),"c",new Class<?>[]{}));
            row.addProperty("removed_after",(boolean)item_hurt_oracle.call(entity,"dJ",new Class<?>[]{}));
            row.addProperty("entity_rng_next_int",(int)C("dyz").getMethod("f").invoke(entityRandom));
            row.addProperty("world_rng_next_int",(int)C("dyz").getMethod("f").invoke(worldRandom));
            row.add("game_events",item_hurt_oracle.trace.deepCopy());
            rows.add(row);
        }
        OUT.println("SOURCE:"+C("cul").getProtectionDomain().getCodeSource().getLocation());
        JsonObject out=new JsonObject();out.addProperty("minecraft","1.21");out.addProperty("oracle_inner_sha256","8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01");out.add("items",rows);
        out.addProperty("expected_entity_rng_next_int",(int)C("dyz").getMethod("f").invoke(C("dyz").getConstructor(long.class).newInstance(12345L)));
        out.addProperty("expected_world_rng_next_int",(int)C("dyz").getMethod("f").invoke(C("dyz").getConstructor(long.class).newInstance(54321L)));
        OUT.println("ORACLE:"+new Gson().toJson(out));
    }
}
