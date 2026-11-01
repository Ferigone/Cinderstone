package oracle;
import java.io.*;
import java.lang.reflect.*;
import java.util.*;

/** Invokes the shipped ServerLevel.advanceWeatherCycle on a minimally wired
 * world. The dynamic proxy only supplies level-data fields; it contains no
 * timer/transition implementation. Empty real PlayerList handles broadcasts. */
class WeatherOracle {
    static final PrintStream OUTPUT=System.out, ERROR=System.err;
    static Class<?> WORLD, SERVER_WORLD, RANDOM, DATA, RULES, HOLDER, CAPTURE;
    static final List<String> EVENTS=new ArrayList<>();
    static Object world, random, rules, unsafe;
    static int clear, thunder, rain;
    static boolean thundering, raining, skylight, cycle;
    static Object allocate(Class<?> c)throws Exception{return unsafe.getClass().getMethod("allocateInstance",Class.class).invoke(unsafe,c);}
    static void put(Class<?> c,Object o,String name,Object v)throws Exception{Field f=c.getDeclaredField(name);f.setAccessible(true);f.set(o,v);}
    static float level(String name)throws Exception{Field f=WORLD.getDeclaredField(name);f.setAccessible(true);return f.getFloat(world);}
    static int bits(String name)throws Exception{return Float.floatToIntBits(level(name));}
    public static void capture(String route,Object packet) {
        try {Object reason=packet.getClass().getMethod("b").invoke(packet);Field id=reason.getClass().getDeclaredField("b");id.setAccessible(true);float value=(float)packet.getClass().getMethod("e").invoke(packet);EVENTS.add(route+","+id.getInt(reason)+","+Integer.toUnsignedString(Float.floatToIntBits(value)));}
        catch(Exception error){throw new RuntimeException(error);}
    }
    static Object data(Object proxy,Method method,Object[] args)throws Exception {
        String name=method.getName();
        if(args==null||args.length==0){switch(name){
            case "f":return clear; case "h":return thunder;case "j":return rain;
            case "g":return thundering;case "i":return raining;case "o":return rules;
            case "toString":return "WeatherOracleLevelData";
        }} else if(args.length==1){
            if(args[0]instanceof Integer n){switch(name){case "a":clear=n;return null;case "b":thunder=n;return null;case "c":rain=n;return null;}}
            if(args[0]instanceof Boolean b){switch(name){case "a":thundering=b;return null;case "b":raining=b;return null;}}
        }
        throw new UnsupportedOperationException(method.toString());
    }
    static void setup(String[]f)throws Exception {
        clear=Integer.parseInt(f[1]);thunder=Integer.parseInt(f[2]);rain=Integer.parseInt(f[3]);thundering=Boolean.parseBoolean(f[4]);raining=Boolean.parseBoolean(f[5]);
        skylight=Boolean.parseBoolean(f[11]);cycle=Boolean.parseBoolean(f[12]);
        world=allocate(SERVER_WORLD);rules=RULES.getConstructor().newInstance();
        Object key=RULES.getField("v").get(null);Object rule=RULES.getMethod("a",Class.forName("dcs$e")).invoke(rules,key);
        put(Class.forName("dcs$a"),rule,"b",cycle);
        Object data=Proxy.newProxyInstance(DATA.getClassLoader(),new Class<?>[]{DATA},WeatherOracle::data);
        put(SERVER_WORLD,world,"K",data);put(WORLD,world,"A",data);
        random=RANDOM.getConstructor(long.class).newInstance(Long.parseLong(f[10]));put(WORLD,world,"z",random);
        Class<?> dimension=Class.forName("dwy");Object type=dimension.getConstructors()[0].newInstance(OptionalLong.empty(),skylight,false,false,true,1.0,true,false,-64,384,384,null,null,0.0f,null);
        Object holder=HOLDER.getMethod("a",Object.class).invoke(null,type);put(WORLD,world,"D",holder);put(WORLD,world,"H",WORLD.getField("h").get(null));
        for(int i=0;i<4;i++)put(WORLD,world,new String[]{"x","y","v","w"}[i],Float.intBitsToFloat((int)Long.parseLong(f[6+i])));
        Object server=allocate(Class.forName("apn")),players=allocate(CAPTURE);
        put(Class.forName("aur"),players,"l",List.of());put(Class.forName("net.minecraft.server.MinecraftServer"),server,"Q",players);put(SERVER_WORLD,world,"J",server);
    }
    static String state()throws Exception {
        return "WS|"+clear+","+thunder+","+rain+","+thundering+","+raining+","+Integer.toUnsignedString(bits("x"))+","+Integer.toUnsignedString(bits("y"))+","+Integer.toUnsignedString(bits("v"))+","+Integer.toUnsignedString(bits("w"))+","+WORLD.getMethod("ad").invoke(world)+","+WORLD.getMethod("ac").invoke(world)+","+RANDOM.getMethod("f").invoke(random);
    }
    static String run(String line)throws Exception {
        String[]f=line.split("\\t",-1);
        switch(f[0]){
            case "weather.new":setup(f);return "WN";
            case "weather.tick":{Method m=SERVER_WORLD.getDeclaredMethod("au");m.setAccessible(true);m.invoke(world);String events=String.join(";",EVENTS);EVENTS.clear();return state()+"|"+events;}
            case "weather.get":return state();
            case "weather.reset":SERVER_WORLD.getMethod("g").invoke(world);return "WR";
            case "weather.prepare":{Method m=WORLD.getDeclaredMethod("X");m.setAccessible(true);m.invoke(world);return "WP";}
            case "weather.set":SERVER_WORLD.getMethod("a",int.class,int.class,boolean.class,boolean.class).invoke(world,Integer.parseInt(f[1]),Integer.parseInt(f[2]),Boolean.parseBoolean(f[3]),Boolean.parseBoolean(f[4]));return "WA";
            default:throw new IllegalArgumentException(line);
        }
    }
    public static void main(String[]args)throws Exception {
        Class.forName("ab").getMethod("a").invoke(null);Class.forName("akt").getMethod("a").invoke(null);
        Class<?> uc=Class.forName("sun.misc.Unsafe");Field u=uc.getDeclaredField("theUnsafe");u.setAccessible(true);unsafe=u.get(null);
        CAPTURE=WeatherPlayerList.createType();
        WORLD=Class.forName("dcw");SERVER_WORLD=Class.forName("aqu");RANDOM=Class.forName("dyz");DATA=Class.forName("erk");RULES=Class.forName("dcs");HOLDER=Class.forName("jm");
        ERROR.println("WEATHER_ORACLE_CLASS=aqu,dcw,dyz;SOURCE="+SERVER_WORLD.getProtectionDomain().getCodeSource().getLocation());
        BufferedReader input=new BufferedReader(new InputStreamReader(System.in));for(String line;(line=input.readLine())!=null;)OUTPUT.println(run(line));
    }
}
