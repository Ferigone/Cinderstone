package oracle;

import com.google.gson.*;
import com.mojang.serialization.Dynamic;
import com.mojang.serialization.DynamicLike;
import com.mojang.serialization.DynamicOps;
import java.io.*;
import java.lang.reflect.*;
import java.util.*;

/** Invokes the original signed1.21 border methods. Only Util.timeSource is
 * supplied by the environment; no Minecraft method or class is rewritten. */
public final class border_oracle {
    static final Gson G=new GsonBuilder().serializeNulls().create();
    static final PrintStream OUT=System.out;
    static Class<?> BORDER,SETTINGS,COMPOUND;
    static Constructor<?> SETTINGS_CTOR;
    static DynamicOps<Object> OPS;
    static long nowMs;
    static Class<?> c(String n)throws Exception{return Class.forName(n);}
    static Object call(Object target,String method,Class<?>[] types,Object...args)throws Exception{
        Class<?> owner=target instanceof Class<?>?(Class<?>)target:target.getClass();
        Method m=owner.getMethod(method,types);m.setAccessible(true);
        return m.invoke(target instanceof Class<?>?null:target,args);
    }
    static Class<?>[] t(Class<?>...v){return v;}
    static double d(JsonElement e){return Double.longBitsToDouble(Long.parseUnsignedLong(e.getAsString(),16));}
    static String bits(double v){return String.format("%016x",Double.doubleToLongBits(v));}
    static Object settings(JsonArray v)throws Exception{
        return SETTINGS_CTOR.newInstance(d(v.get(0)),d(v.get(1)),d(v.get(2)),d(v.get(3)),v.get(4).getAsInt(),v.get(5).getAsInt(),d(v.get(6)),v.get(7).getAsLong(),d(v.get(8)));
    }
    static JsonArray settingsOut(Object settings)throws Exception{
        JsonArray a=new JsonArray();
        for(int i=0;i<9;i++){
            Object value=call(settings,String.valueOf((char)('a'+i)),t());
            if(i==4||i==5)a.add((Integer)value);
            else if(i==7)a.add((Long)value);
            else a.add(bits((Double)value));
        }
        return a;
    }
    static Object nbt(String base64)throws Exception{
        return call(c("uo"),"a",t(DataInput.class),new DataInputStream(new ByteArrayInputStream(Base64.getDecoder().decode(base64))));
    }
    static String nbtOut(Object value)throws Exception{
        ByteArrayOutputStream bytes=new ByteArrayOutputStream();
        call(c("uo"),"a",t(COMPOUND,DataOutput.class),value,new DataOutputStream(bytes));
        return Base64.getEncoder().encodeToString(bytes.toByteArray());
    }
    static JsonObject read(JsonObject q)throws Exception{
        Object tag=nbt(q.get("input").getAsString());
        Object defaults=q.has("official_defaults")&&q.get("official_defaults").getAsBoolean()?BORDER.getField("d").get(null):settings(q.getAsJsonArray("defaults"));
        Object result=call(SETTINGS,"a",t(DynamicLike.class,SETTINGS),new Dynamic<>(OPS,tag),defaults);
        JsonObject out=new JsonObject();out.add("settings",settingsOut(result));
        call(result,"a",t(COMPOUND),tag);out.addProperty("written",nbtOut(tag));
        return out;
    }
    static JsonObject snapshot(Object border)throws Exception{
        JsonObject s=new JsonObject();
        s.add("settings",settingsOut(call(border,"t",t())));
        JsonArray bounds=new JsonArray();
        double minX=(double)call(border,"e",t()),minZ=(double)call(border,"f",t()),maxX=(double)call(border,"g",t()),maxZ=(double)call(border,"h",t());
        for(double value:new double[]{minX,minZ,maxX,maxZ})bounds.add(bits(value));
        s.add("bounds",bounds);
        s.addProperty("size",bits((double)call(border,"i",t())));
        s.addProperty("speed",bits((double)call(border,"p",t())));
        s.addProperty("remaining",(long)call(border,"j",t()));
        s.addProperty("target",bits((double)call(border,"k",t())));
        s.addProperty("status",((Enum<?>)call(border,"d",t())).ordinal());
        JsonArray queries=new JsonArray();
        double centerX=(double)call(border,"a",t()),centerZ=(double)call(border,"b",t());
        for(double[] p:new double[][]{{minX,centerZ,0},{maxX,centerZ,0},{Math.nextDown(maxX),centerZ,0},{centerX,minZ,0},{centerX,maxZ,0},{maxX+.5,centerZ,1},{minX-.5,centerZ,1},{centerX,centerZ,0},{maxX,Math.nextDown(maxZ),-1}}){
            JsonObject row=new JsonObject();JsonArray point=new JsonArray();for(double v:p)point.add(bits(v));row.add("point",point);
            row.addProperty("within",(boolean)call(border,"a",t(double.class,double.class,double.class),p[0],p[1],p[2]));
            row.addProperty("distance",bits((double)call(border,"b",t(double.class,double.class),p[0],p[1])));queries.add(row);
        }
        s.add("queries",queries);return s;
    }
    static JsonArray world(JsonObject q)throws Exception{
        nowMs=q.get("start_ms").getAsLong();Object border=BORDER.getConstructor().newInstance();
        call(border,"a",t(int.class),q.get("absolute_max").getAsInt());
        call(border,"a",t(SETTINGS),settings(q.getAsJsonArray("settings")));
        JsonArray states=new JsonArray();
        for(JsonElement entry:q.getAsJsonArray("actions")){
            JsonObject a=entry.getAsJsonObject();nowMs=a.get("now_ms").getAsLong();
            switch(a.get("op").getAsString()){
                case "snapshot":break;
                case "tick":call(border,"s",t());break;
                case "center":call(border,"c",t(double.class,double.class),d(a.get("x")),d(a.get("z")));break;
                case "size":call(border,"a",t(double.class),d(a.get("size")));break;
                case "lerp":call(border,"a",t(double.class,double.class,long.class),d(a.get("from")),d(a.get("to")),a.get("duration").getAsLong());break;
                case "absolute":call(border,"a",t(int.class),a.get("value").getAsInt());break;
                default:throw new IllegalArgumentException(a.toString());
            }
            states.add(snapshot(border));
        }
        return states;
    }
    @SuppressWarnings("unchecked")
    static void init()throws Exception{
        call(c("ab"),"a",t());call(c("akt"),"a",t());
        BORDER=c("dut");SETTINGS=c("dut$c");COMPOUND=c("ub");
        SETTINGS_CTOR=SETTINGS.getDeclaredConstructor(double.class,double.class,double.class,double.class,int.class,int.class,double.class,long.class,double.class);SETTINGS_CTOR.setAccessible(true);
        OPS=(DynamicOps<Object>)c("up").getField("a").get(null);
        Class<?> source=c("azo$a");Object clock=Proxy.newProxyInstance(source.getClassLoader(),new Class<?>[]{source},(proxy,m,args)->{
            if(m.getName().equals("getAsLong"))return Math.multiplyExact(nowMs,1_000_000L);
            if(m.getName().equals("toString"))return "Injected monotonic border clock";
            throw new UnsupportedOperationException(m.toString());
        });
        c("ad").getField("c").set(null,clock);
        OUT.println("SOURCE:"+BORDER.getProtectionDomain().getCodeSource().getLocation());
    }
    public static void main(String[]args)throws Exception{
        init();BufferedReader in=new BufferedReader(new InputStreamReader(System.in));
        for(String line;(line=in.readLine())!=null;)try{
            JsonObject q=JsonParser.parseString(line).getAsJsonObject();
            OUT.println("ORACLE:"+G.toJson(q.get("op").getAsString().equals("read")?read(q):world(q)));
        }catch(Throwable e){OUT.println("QUERY:"+line);e.printStackTrace(OUT);throw e;}
    }
}
