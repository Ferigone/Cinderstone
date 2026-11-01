import java.io.*;
import java.lang.reflect.*;
import java.util.*;
import java.util.function.Consumer;
import com.google.common.collect.ImmutableList;
import io.netty.buffer.*;
import com.google.gson.*;

/** Executes unchanged aqt.sendChanges; only the entity's input accessors are controlled. */
class entity_motion_oracle {
    static final Gson JSON = new Gson();
    static class ControlledEntity extends bsr {
        exc position,velocity;
        float yaw,pitch;
        boolean ground,headTracking;
        aka data;
        ControlledEntity() { super(null,null); }
        protected void a(aka.a b) {}
        protected void a(ub n) {}
        protected void b(ub n) {}
        public int an() { return 17; }
        public exc dn() { return position; }
        public exc dr() { return velocity; }
        public float dE() { return yaw; }
        public float dG() { return pitch; }
        public float ct() { return headTracking?yaw:0; }
        public boolean aF() { return ground; }
        public boolean bS() { return false; }
        public aka ar() { return data; }
    }
    static void set(Object o,Class<?> c,String name,Object value)throws Exception {Field f=c.getDeclaredField(name);f.setAccessible(true);f.set(o,value);}
    static final Field COORD;
    static { try { COORD=bsr.class.getDeclaredField("s");COORD.setAccessible(true); }catch(Exception e){throw new RuntimeException(e);} }
    static void input(ControlledEntity e,JsonObject p)throws Exception {
        e.position=new exc(p.get("x").getAsDouble(),p.get("y").getAsDouble(),p.get("z").getAsDouble());
        COORD.set(e,e.position);e.yaw=p.get("yaw").getAsFloat();e.pitch=p.get("pitch").getAsFloat();e.ground=p.get("on_ground").getAsBoolean();
    }
    static JsonObject encode(Object packet)throws Exception {
        String name=packet.getClass().getName();int id;String method;
        switch(name){case "ado$a":id=0x2e;method="b";break;case "ado$b":id=0x2f;method="b";break;case "ado$c":id=0x30;method="b";break;case "afw":id=0x70;method="a";break;case "aej":id=0x48;method="a";break;case "afb":id=0x5a;method="a";break;default:throw new IllegalStateException(name);}
        ByteBuf bytes=Unpooled.buffer();vw buffer=new vw(bytes);
        Method write=packet.getClass().getDeclaredMethod(method,vw.class);write.setAccessible(true);write.invoke(packet,buffer);
        byte[] data=new byte[bytes.readableBytes()];bytes.readBytes(data);bytes.release();
        JsonObject out=new JsonObject();out.addProperty("id",id);out.addProperty("payload",Base64.getEncoder().encodeToString(data));return out;
    }
    public static void main(String[] args)throws Exception {
        ab.a();akt.a();
        Field uf=sun.misc.Unsafe.class.getDeclaredField("theUnsafe");uf.setAccessible(true);sun.misc.Unsafe unsafe=(sun.misc.Unsafe)uf.get(null);
        ControlledEntity e=(ControlledEntity)unsafe.allocateInstance(ControlledEntity.class);
        set(e,bsr.class,"p",ImmutableList.of());e.velocity=new exc(0,0,0);
        Constructor<?> ctor=aka.class.getDeclaredConstructors()[0];ctor.setAccessible(true);
        Class<?> inner=Class.forName("aka$b");e.data=(aka)ctor.newInstance(e,Array.newInstance(inner,0));
        List<Object> packets=new ArrayList<>();aqt tracker=null;
        BufferedReader reader=new BufferedReader(new InputStreamReader(System.in));
        for(String line;(line=reader.readLine())!=null;){
            JsonObject query=JsonParser.parseString(line).getAsJsonObject();input(e,query.getAsJsonObject("position"));
            JsonObject out=new JsonObject();
            if(query.has("velocity")){JsonArray v=query.getAsJsonArray("velocity");e.velocity=new exc(v.get(0).getAsDouble(),v.get(1).getAsDouble(),v.get(2).getAsDouble());}
            packets.clear();
            if(query.get("op").getAsString().equals("new")) {e.headTracking=!query.has("track_head")||query.get("track_head").getAsBoolean();tracker=new aqt(null,e,query.has("interval")?query.get("interval").getAsInt():2,query.has("track_delta")&&query.get("track_delta").getAsBoolean(),(Consumer<zg<?>>)(packets::add));}
            else { e.av=(query.has("force")&&query.get("force").getAsBoolean())||(query.has("impulse")&&query.get("impulse").getAsBoolean());set(e.data,aka.class,"f",query.has("dirty")&&query.get("dirty").getAsBoolean());tracker.a(); }
            JsonArray encoded=new JsonArray();for(Object packet:packets)encoded.add(encode(packet));out.add("packets",encoded);
            exc base=tracker.b();JsonObject pos=new JsonObject();pos.addProperty("x",base.c);pos.addProperty("y",base.d);pos.addProperty("z",base.e);pos.addProperty("yaw",tracker.e());pos.addProperty("pitch",tracker.d());pos.addProperty("xbits",Long.toHexString(Double.doubleToRawLongBits(base.c)));pos.addProperty("ybits",Long.toHexString(Double.doubleToRawLongBits(base.d)));pos.addProperty("zbits",Long.toHexString(Double.doubleToRawLongBits(base.e)));out.add("spawn",pos);
            exc velocity=tracker.c();JsonArray velocityBits=new JsonArray();for(double v:new double[]{velocity.c,velocity.d,velocity.e})velocityBits.add(Long.toHexString(Double.doubleToRawLongBits(v)));out.add("velocity_bits",velocityBits);
            System.out.println("ORACLE:"+JSON.toJson(out));
        }
    }
}
