package oracle;
import java.io.*;
import java.lang.reflect.*;
import java.util.*;
import com.google.gson.*;

/** Reflection invokes the actual pinned RNG and noise classes, not Java ports. */
class NoiseOracle {
    static final PrintStream OUTPUT=System.out,ERROR=System.err;
    static final Map<String,Object> RANDOMS=new HashMap<>(),FACTORIES=new HashMap<>(),NOISES=new HashMap<>();
    static Class<?> RANDOM,LEGACY,XORO,SUPPORT,SEED,IMPROVED,SIMPLEX,PERLIN,NORMAL,PARAMETERS,DOUBLE_LIST,PERLIN_SIMPLEX,WORLDGEN,BLENDED,CONTEXT,POINT;
    static JsonObject query;
    static String text(String name){return query.get(name).getAsString();}
    static int integer(String name){return query.get(name).getAsInt();}
    static long signed(String name){return query.get(name).getAsLong();}
    static boolean bool(String name){return query.has(name)&&query.get(name).getAsBoolean();}
    static double floating(String name){return Double.longBitsToDouble(Long.parseUnsignedLong(text(name)));}
    static String bits(double value){return Long.toUnsignedString(Double.doubleToRawLongBits(value));}
    static Object invoke(Object target,String method,Class<?>[]types,Object...args)throws Exception{return target.getClass().getMethod(method,types).invoke(target,args);}
    static Object field(Object value,String name)throws Exception{Field f=value.getClass().getDeclaredField(name);f.setAccessible(true);return f.get(value);}
    static double[] doubles(String name){JsonArray input=query.getAsJsonArray(name);double[] result=new double[input.size()];for(int i=0;i<result.length;i++)result[i]=Double.longBitsToDouble(Long.parseUnsignedLong(input.get(i).getAsString()));return result;}
    static String string(){JsonArray units=query.getAsJsonArray("units");char[] chars=new char[units.size()];for(int i=0;i<chars.length;i++)chars[i]=(char)units.get(i).getAsInt();return new String(chars);}
    static Object random(){return RANDOMS.get(text("source"));}
    static Object seed(Object seed)throws Exception{return List.of(Long.toUnsignedString((long)SEED.getMethod("b").invoke(seed)),Long.toUnsignedString((long)SEED.getMethod("c").invoke(seed)));}
    static Object improved(Object noise)throws Exception {
        if(noise==null)return null;
        byte[] permutation=(byte[])field(noise,"e");StringBuilder hex=new StringBuilder();for(byte value:permutation)hex.append(String.format("%02x",value&255));
        return Map.of("offsets",List.of(bits(IMPROVED.getField("a").getDouble(noise)),bits(IMPROVED.getField("b").getDouble(noise)),bits(IMPROVED.getField("c").getDouble(noise))),"permutation",hex.toString());
    }
    static Object simplex(Object noise)throws Exception {
        int[] permutation=(int[])field(noise,"h");StringBuilder hex=new StringBuilder();for(int i=0;i<256;i++)hex.append(String.format("%02x",permutation[i]));
        return Map.of("offsets",List.of(bits(SIMPLEX.getField("b").getDouble(noise)),bits(SIMPLEX.getField("c").getDouble(noise)),bits(SIMPLEX.getField("d").getDouble(noise))),"permutation",hex.toString());
    }
    static Object run()throws Exception {
        switch(text("op")){
            case "rng_new":{
                Object value;
                if(text("kind").equals("legacy"))value=LEGACY.getConstructor(long.class).newInstance(signed("seed"));
                else if(query.has("lo"))value=XORO.getConstructor(long.class,long.class).newInstance(signed("lo"),signed("hi"));
                else value=XORO.getConstructor(long.class).newInstance(signed("seed"));
                RANDOMS.put(text("id"),value);return null;
            }
            case "rng_seed":invoke(random(),"b",new Class<?>[]{long.class},signed("seed"));return null;
            case "rng_consume":invoke(random(),"b",new Class<?>[]{int.class},integer("count"));return null;
            case "rng_fork":RANDOMS.put(text("id"),invoke(random(),"d",new Class<?>[]{}));return null;
            case "rng_factory":FACTORIES.put(text("id"),invoke(random(),"e",new Class<?>[]{}));return null;
            case "rng_sample":{
                Object value=random();switch(text("method")){
                    case "i32":return invoke(value,"f",new Class<?>[]{}).toString();
                    case "i64":return invoke(value,"g",new Class<?>[]{}).toString();
                    case "bool":return invoke(value,"h",new Class<?>[]{});
                    case "f32":return Integer.toUnsignedString(Float.floatToRawIntBits((float)invoke(value,"i",new Class<?>[]{})));
                    case "f64":return bits((double)invoke(value,"j",new Class<?>[]{}));
                    case "bounded":return invoke(value,"a",new Class<?>[]{int.class},integer("bound")).toString();
                    default:throw new IllegalArgumentException(text("method"));
                }
            }
            case "rng_state":{
                Object value=random();if(value.getClass()==LEGACY)return Long.toUnsignedString(((java.util.concurrent.atomic.AtomicLong)field(value,"h")).get());
                Object engine=field(value,"e");return List.of(Long.toUnsignedString((long)field(engine,"b")),Long.toUnsignedString((long)field(engine,"c")));
            }
            case "factory_new":{
                Object factory=text("kind").equals("legacy")?Class.forName("dyz$a").getConstructor(long.class).newInstance(signed("seed")):Class.forName("dzz$a").getConstructor(long.class,long.class).newInstance(signed("lo"),signed("hi"));
                FACTORIES.put(text("id"),factory);return null;
            }
            case "factory_at":RANDOMS.put(text("id"),invoke(FACTORIES.get(text("factory")),"a",new Class<?>[]{int.class,int.class,int.class},integer("x"),integer("y"),integer("z")));return null;
            case "factory_hash":RANDOMS.put(text("id"),invoke(FACTORIES.get(text("factory")),"a",new Class<?>[]{String.class},string()));return null;
            case "factory_seed":RANDOMS.put(text("id"),invoke(FACTORIES.get(text("factory")),"a",new Class<?>[]{long.class},signed("seed")));return null;
            case "mix":return Long.toUnsignedString((long)SUPPORT.getMethod("a",long.class).invoke(null,signed("seed")));
            case "upgrade":return seed(SUPPORT.getMethod(bool("mixed")?"c":"b",long.class).invoke(null,signed("seed")));
            case "hash":return seed(SUPPORT.getMethod("a",String.class).invoke(null,string()));
            case "coord":return Long.toString((long)Class.forName("ayo").getMethod("b",int.class,int.class,int.class).invoke(null,integer("x"),integer("y"),integer("z")));
            case "improved_new":{Object value=IMPROVED.getConstructor(RANDOM).newInstance(random());NOISES.put(text("id"),value);return improved(value);}
            case "simplex_new":{Object value=SIMPLEX.getConstructor(RANDOM).newInstance(random());NOISES.put(text("id"),value);return simplex(value);}
            case "perlin_new":{
                double[] amplitudes=doubles("amplitudes");Object list=Class.forName("it.unimi.dsi.fastutil.doubles.DoubleArrayList").getConstructor(double[].class).newInstance((Object)amplitudes);
                Object value=PERLIN.getMethod(bool("legacy")?"a":"b",RANDOM,int.class,DOUBLE_LIST).invoke(null,random(),integer("first"),list);NOISES.put(text("id"),value);
                List<Object> levels=new ArrayList<>();Object[] levelArray=(Object[])field(value,"b");for(Object level:levelArray)levels.add(improved(level));
                return Map.of("levels",levels,"max",bits((double)field(value,"g")));
            }
            case "normal_new":{
                double[] amplitudes=doubles("amplitudes");Object list=Class.forName("it.unimi.dsi.fastutil.doubles.DoubleArrayList").getConstructor(double[].class).newInstance((Object)amplitudes);
                Object parameters=PARAMETERS.getConstructor(int.class,DOUBLE_LIST).newInstance(integer("first"),list);
                Object value=NORMAL.getMethod(bool("legacy")?"a":"b",RANDOM,PARAMETERS).invoke(null,random(),parameters);NOISES.put(text("id"),value);return bits((double)NORMAL.getMethod("a").invoke(value));
            }
            case "improved":return bits((double)IMPROVED.getMethod("a",double.class,double.class,double.class,double.class,double.class).invoke(NOISES.get(text("id")),floating("x"),floating("y"),floating("z"),floating("scale"),floating("ymax")));
            case "derivative":{
                double[] derivative=doubles("initial");double sample=(double)IMPROVED.getMethod("a",double.class,double.class,double.class,double[].class).invoke(NOISES.get(text("id")),floating("x"),floating("y"),floating("z"),derivative);
                return List.of(bits(sample),bits(derivative[0]),bits(derivative[1]),bits(derivative[2]));
            }
            case "simplex2":return bits((double)SIMPLEX.getMethod("a",double.class,double.class).invoke(NOISES.get(text("id")),floating("x"),floating("y")));
            case "simplex3":return bits((double)SIMPLEX.getMethod("a",double.class,double.class,double.class).invoke(NOISES.get(text("id")),floating("x"),floating("y"),floating("z")));
            case "perlin":return bits((double)PERLIN.getMethod("a",double.class,double.class,double.class,double.class,double.class,boolean.class).invoke(NOISES.get(text("id")),floating("x"),floating("y"),floating("z"),floating("scale"),floating("ymax"),bool("origin")));
            case "normal":return bits((double)NORMAL.getMethod("a",double.class,double.class,double.class).invoke(NOISES.get(text("id")),floating("x"),floating("y"),floating("z")));
            case "blended_new":{
                Object value=BLENDED.getConstructor(RANDOM,double.class,double.class,double.class,double.class,double.class).newInstance(random(),floating("xz_scale"),floating("y_scale"),floating("xz_factor"),floating("y_factor"),floating("smear"));NOISES.put(text("id"),value);return List.of(bits((double)BLENDED.getMethod("a").invoke(value)),bits((double)BLENDED.getMethod("b").invoke(value)));
            }
            case "blended":{
                Object point=POINT.getConstructor(int.class,int.class,int.class).newInstance(integer("x"),integer("y"),integer("z"));return bits((double)BLENDED.getMethod("a",CONTEXT).invoke(NOISES.get(text("id")),point));
            }
            case "perlinsimplex_new":{
                List<Integer> octaves=new ArrayList<>();for(JsonElement octave:query.getAsJsonArray("octaves"))octaves.add(octave.getAsInt());
                Object value=PERLIN_SIMPLEX.getConstructor(RANDOM,List.class).newInstance(random(),octaves);NOISES.put(text("id"),value);
                List<Object> levels=new ArrayList<>();for(Object level:(Object[])field(value,"a"))levels.add(level==null?null:simplex(level));return levels;
            }
            case "perlinsimplex":return bits((double)PERLIN_SIMPLEX.getMethod("a",double.class,double.class,boolean.class).invoke(NOISES.get(text("id")),floating("x"),floating("y"),bool("origin")));
            case "worldgen":{
                Object source=(text("kind").equals("legacy")?LEGACY:XORO).getConstructor(long.class).newInstance(signed("seed"));Object value=WORLDGEN.getConstructor(RANDOM).newInstance(source);Object mixed=null;
                switch(text("method")){
                    case "decoration":mixed=WORLDGEN.getMethod("a",long.class,int.class,int.class).invoke(value,signed("seed"),integer("x"),integer("z")).toString();break;
                    case "feature":WORLDGEN.getMethod("b",long.class,int.class,int.class).invoke(value,signed("seed"),integer("x"),integer("z"));break;
                    case "large":WORLDGEN.getMethod("c",long.class,int.class,int.class).invoke(value,signed("seed"),integer("x"),integer("z"));break;
                    case "salt":WORLDGEN.getMethod("a",long.class,int.class,int.class,int.class).invoke(value,signed("seed"),integer("x"),integer("z"),integer("salt"));break;
                    case "plain":break;default:throw new IllegalArgumentException(text("method"));
                }
                List<Object> samples=new ArrayList<>();for(int i=0;i<4;i++){samples.add(invoke(value,"f",new Class<?>[]{}).toString());samples.add(invoke(value,"g",new Class<?>[]{}).toString());samples.add(invoke(value,"h",new Class<?>[]{}));samples.add(Integer.toUnsignedString(Float.floatToRawIntBits((float)invoke(value,"i",new Class<?>[]{}))));samples.add(bits((double)invoke(value,"j",new Class<?>[]{})));samples.add(invoke(value,"a",new Class<?>[]{int.class},1073741825).toString());}
                Map<String,Object> result=new LinkedHashMap<>();result.put("seed",mixed);result.put("samples",samples);result.put("count",WORLDGEN.getMethod("l").invoke(value));return result;
            }
            case "slime":{
                Object value=WORLDGEN.getMethod("a",int.class,int.class,long.class,long.class).invoke(null,integer("x"),integer("z"),signed("seed"),signed("salt"));
                return List.of(invoke(value,"g",new Class<?>[]{}).toString(),invoke(value,"a",new Class<?>[]{int.class},10).toString());
            }
            case "wrap":return bits((double)PERLIN.getMethod("b",double.class).invoke(null,floating("value")));
            default:throw new IllegalArgumentException(text("op"));
        }
    }
    public static void main(String[]args)throws Exception {
        Class.forName("ab").getMethod("a").invoke(null);Class.forName("akt").getMethod("a").invoke(null);
        RANDOM=Class.forName("ayw");LEGACY=Class.forName("dyz");XORO=Class.forName("dzz");SUPPORT=Class.forName("dzn");SEED=Class.forName("dzn$a");IMPROVED=Class.forName("eof");SIMPLEX=Class.forName("eok");PERLIN=Class.forName("eoi");NORMAL=Class.forName("eoh");PARAMETERS=Class.forName("eoh$a");DOUBLE_LIST=Class.forName("it.unimi.dsi.fastutil.doubles.DoubleList");PERLIN_SIMPLEX=Class.forName("eoj");WORLDGEN=Class.forName("dzx");BLENDED=Class.forName("eoe");CONTEXT=Class.forName("dyr$b");POINT=Class.forName("dyr$e");
        for(Class<?> type:new Class<?>[]{LEGACY,XORO,SUPPORT,SEED,IMPROVED,SIMPLEX,PERLIN,NORMAL,PARAMETERS,PERLIN_SIMPLEX,WORLDGEN,BLENDED})if(!type.getProtectionDomain().getCodeSource().getLocation().equals(IMPROVED.getProtectionDomain().getCodeSource().getLocation()))throw new IllegalStateException("Wrong class origin: "+type);
        ERROR.println("NOISE_ORACLE_CLASSES=dyz,dzy,dzz,dzn,dzx,eof,eok,eoi,eoh,eoj,eoe;SOURCE="+IMPROVED.getProtectionDomain().getCodeSource().getLocation());
        Gson gson=new GsonBuilder().serializeNulls().create();BufferedReader input=new BufferedReader(new InputStreamReader(System.in));
        for(String line;(line=input.readLine())!=null;){query=JsonParser.parseString(line).getAsJsonObject();Object result;
            try{result=run();}catch(InvocationTargetException error){result=Map.of("error",error.getCause().getMessage());}
            OUTPUT.println(gson.toJson(result));
        }
    }
}
