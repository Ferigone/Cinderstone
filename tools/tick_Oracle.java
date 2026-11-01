package oracle;

import java.io.BufferedReader;
import java.io.InputStreamReader;
import java.lang.reflect.*;
import java.util.*;
import java.util.function.Function;
import java.util.function.LongPredicate;
import java.util.function.Supplier;
import java.util.function.BiConsumer;

/** Reflection into the actual signed vanilla 1.21 tick classes, not a Java rewrite. */
class TickOracle {
    static final java.io.PrintStream OUTPUT=System.out;
    static final Class<?> T, P, POS, Q, SAVED, TIME, TAG, L, CP, BOX, VEC;
    static final Map<String,String> symbols=new HashMap<>();
    static Object queue;
    static Object level;
    static Object randomLevel, rate;
    static Class<?> WORLD, RATE;
    static Field randomState;
    static final Set<Long> blocked=new HashSet<>();
    static final Map<Long,Object> chunkQueues=new HashMap<>();
    static {
        try {
            T=Class.forName("eyt");P=Class.forName("eyx");POS=Class.forName("jd");
            Q=Class.forName("eyo");SAVED=Class.forName("eys");TIME=Class.forName("afl");TAG=Class.forName("ub");
            L=Class.forName("eyq");CP=Class.forName("dcd");BOX=Class.forName("ejj");VEC=Class.forName("kh");
            queue=Q.getConstructor().newInstance();
            newLevel();
            System.err.println("ORACLE_CLASS=eyt,eyx,eyo,eys,afl;SOURCE="+T.getProtectionDomain().getCodeSource().getLocation());
        } catch (Exception e) { throw new ExceptionInInitializerError(e); }
    }
    static void newLevel()throws Exception {blocked.clear();chunkQueues.clear();Object profiler=Class.forName("bnc").getField("a").get(null);level=L.getConstructor(LongPredicate.class,Supplier.class).newInstance((LongPredicate)(key->!blocked.contains(key)),(Supplier<Object>)(()->profiler));}
    static Object chunk(String x,String z)throws Exception{return CP.getConstructor(int.class,int.class).newInstance(Integer.parseInt(x),Integer.parseInt(z));}
    static long chunkKey(String x,String z)throws Exception{return (long)CP.getMethod("c",int.class,int.class).invoke(null,Integer.parseInt(x),Integer.parseInt(z));}
    static Object bounds(String[]f,int offset)throws Exception{return BOX.getConstructor(int.class,int.class,int.class,int.class,int.class,int.class).newInstance(Integer.parseInt(f[offset]),Integer.parseInt(f[offset+1]),Integer.parseInt(f[offset+2]),Integer.parseInt(f[offset+3]),Integer.parseInt(f[offset+4]),Integer.parseInt(f[offset+5]));}
    static int levelCount()throws Exception{return (int)L.getMethod("a").invoke(level);}
    static Object pos(String x,String y,String z)throws Exception {
        return POS.getConstructor(int.class,int.class,int.class).newInstance(Integer.parseInt(x),Integer.parseInt(y),Integer.parseInt(z));
    }
    static Object priority(String v)throws Exception{return P.getMethod("a",int.class).invoke(null,Integer.parseInt(v));}
    static Object symbol(String v){return symbols.computeIfAbsent(v,k->new String(k));}
    static Object tick(String[] f)throws Exception {
        return T.getConstructor(Object.class,POS,long.class,P,long.class).newInstance(symbol(f[0]),pos(f[1],f[2],f[3]),Long.parseLong(f[4]),priority(f[5]),Long.parseLong(f[6]));
    }
    static Object saved(String[] f)throws Exception {
        return SAVED.getConstructor(Object.class,POS,int.class,P).newInstance(symbol(f[0]),pos(f[1],f[2],f[3]),Integer.parseInt(f[4]),priority(f[5]));
    }
    static Object filteredQueue(String x,String z,String values)throws Exception{
        Class<?> listClass=Class.forName("uh");Object list=listClass.getConstructor().newInstance();
        @SuppressWarnings("unchecked") List<Object> tags=(List<Object>)list;
        if(!values.isEmpty())for(String value:values.split(";")){Object s=saved(value.split(","));tags.add(SAVED.getMethod("a",Function.class).invoke(s,(Function<Object,String>)(Object::toString)));}
        return Q.getMethod("a",listClass,Function.class,CP).invoke(null,list,(Function<String,Optional<Object>>)(kind->Optional.of(symbol(kind))),chunk(x,z));
    }
    static int count()throws Exception{return (int)Q.getMethod("a").invoke(queue);}
    static String format(Object t)throws Exception {
        if(t==null)return "null";
        Object p=T.getMethod("b").invoke(t);
        return T.getMethod("a").invoke(t)+","+POS.getMethod("u").invoke(p)+","+POS.getMethod("v").invoke(p)+","+POS.getMethod("w").invoke(p)+","+T.getMethod("c").invoke(t)+","+P.getMethod("a").invoke(T.getMethod("d").invoke(t))+","+T.getMethod("e").invoke(t);
    }
    static String run(String line)throws Exception {
        String[] f=line.split("\t",-1);
        switch(f[0]) {
            case "new":queue=Q.getConstructor().newInstance();return "N";
            case "priority":return "P|"+P.getMethod("a").invoke(priority(f[1]));
            case "schedule":{int before=count();Q.getMethod("a",T).invoke(queue,tick(Arrays.copyOfRange(f,1,f.length)));return "S|"+(count()>before);}
            case "peek":return "T|"+format(Q.getMethod("b").invoke(queue));
            case "poll":return "T|"+format(Q.getMethod("c").invoke(queue));
            case "count":return "C|"+count();
            case "has":return "H|"+Q.getMethod("a",POS,Object.class).invoke(queue,pos(f[2],f[3],f[4]),symbol(f[1]));
            case "save":{
                Object list=Q.getMethod("a",long.class,Function.class).invoke(queue,Long.parseLong(f[1]),(Function<Object,String>)(Object::toString));
                List<String> parts=new ArrayList<>();
                for(Object tag:(List<?>)list)parts.add(TAG.getMethod("l",String.class).invoke(tag,"i")+","+TAG.getMethod("h",String.class).invoke(tag,"x")+","+TAG.getMethod("h",String.class).invoke(tag,"y")+","+TAG.getMethod("h",String.class).invoke(tag,"z")+","+TAG.getMethod("h",String.class).invoke(tag,"t")+","+TAG.getMethod("h",String.class).invoke(tag,"p"));
                return "V|"+String.join(";",parts);
            }
            case "load":{
                List<Object> ticks=new ArrayList<>();
                if(!f[1].isEmpty())for(String value:f[1].split(";"))ticks.add(saved(value.split(",")));
                queue=Q.getConstructor(List.class).newInstance(ticks);return "L";
            }
            case "load_chunk":queue=filteredQueue(f[1],f[2],f[3]);return "L";
            case "unpack":Q.getMethod("a",long.class).invoke(queue,Long.parseLong(f[1]));return "U";
            case "cmp":{
                @SuppressWarnings("unchecked") Comparator<Object> comparator=(Comparator<Object>)T.getField(f[1].equals("drain")?"a":"b").get(null);
                return "O|"+Integer.signum(comparator.compare(tick(f[2].split(",")),tick(f[3].split(","))));
            }
            case "time":{
                Object packet=TIME.getConstructor(long.class,long.class,boolean.class).newInstance(Long.parseLong(f[1]),Long.parseLong(f[2]),Boolean.parseBoolean(f[3]));
                long game=(long)TIME.getMethod("b").invoke(packet),day=(long)TIME.getMethod("e").invoke(packet);
                return "W|"+String.format("%016x%016x",game,day);
            }
            case "random.new":{
                if(randomLevel==null){Class.forName("ab").getMethod("a").invoke(null);Class.forName("akt").getMethod("a").invoke(null);Class<?> unsafeClass=Class.forName("sun.misc.Unsafe");Field u=unsafeClass.getDeclaredField("theUnsafe");u.setAccessible(true);Object unsafe=u.get(null);WORLD=Class.forName("dcw");randomLevel=unsafeClass.getMethod("allocateInstance",Class.class).invoke(unsafe,Class.forName("aqu"));randomState=WORLD.getDeclaredField("t");randomState.setAccessible(true);}
                randomState.setInt(randomLevel,Integer.parseInt(f[1]));return "RN";
            }
            case "random.pos":{
                Object p=WORLD.getMethod("a",int.class,int.class,int.class,int.class).invoke(randomLevel,Integer.parseInt(f[1]),Integer.parseInt(f[2]),Integer.parseInt(f[3]),Integer.parseInt(f[4]));
                return "RP|"+POS.getMethod("u").invoke(p)+","+POS.getMethod("v").invoke(p)+","+POS.getMethod("w").invoke(p)+","+randomState.getInt(randomLevel);
            }
            case "rate.new":RATE=Class.forName("brc");rate=RATE.getConstructor().newInstance();return "TN";
            case "rate.set":RATE.getMethod("a",float.class).invoke(rate,Float.intBitsToFloat((int)Long.parseLong(f[1])));return "TS";
            case "rate.freeze":RATE.getMethod("a",boolean.class).invoke(rate,Boolean.parseBoolean(f[1]));return "TF";
            case "rate.steps":RATE.getMethod("c",int.class).invoke(rate,Integer.parseInt(f[1]));return "TP";
            case "rate.tick":RATE.getMethod("m").invoke(rate);return "TT";
            case "rate.get":return "TG|"+Integer.toUnsignedString(Float.floatToIntBits((float)RATE.getMethod("f").invoke(rate)))+","+Integer.toUnsignedString(Float.floatToIntBits((float)RATE.getMethod("g").invoke(rate)))+","+RATE.getMethod("h").invoke(rate)+","+RATE.getMethod("i").invoke(rate)+","+RATE.getMethod("j").invoke(rate)+","+RATE.getMethod("k").invoke(rate)+","+RATE.getMethod("l").invoke(rate);
            case "level.new":newLevel();return "LN";
            case "level.add":{Object q=Q.getConstructor().newInstance();chunkQueues.put(chunkKey(f[1],f[2]),q);L.getMethod("a",CP,Q).invoke(level,chunk(f[1],f[2]),q);return "LA";}
            case "level.pending":{Object q=filteredQueue(f[1],f[2],f[3]);chunkQueues.put(chunkKey(f[1],f[2]),q);L.getMethod("a",CP,Q).invoke(level,chunk(f[1],f[2]),q);return "LP";}
            case "level.unpack":{Object q=chunkQueues.get(chunkKey(f[1],f[2]));if(q!=null)Q.getMethod("a",long.class).invoke(q,Long.parseLong(f[3]));return "LU";}
            case "level.remove":chunkQueues.remove(chunkKey(f[1],f[2]));L.getMethod("a",CP).invoke(level,chunk(f[1],f[2]));return "LR";
            case "level.block":{long key=chunkKey(f[1],f[2]);if(Boolean.parseBoolean(f[3]))blocked.add(key);else blocked.remove(key);return "LB";}
            case "level.schedule":{int count=levelCount();L.getMethod("a",T).invoke(level,tick(Arrays.copyOfRange(f,1,f.length)));return "LS|"+(levelCount()>count);}
            case "level.count":return "LC|"+levelCount();
            case "level.has":return "LH|"+L.getMethod("a",POS,Object.class).invoke(level,pos(f[2],f[3],f[4]),symbol(f[1]));
            case "level.order":{
                Field next=L.getDeclaredField("e");next.setAccessible(true);Object map=next.get(level);
                @SuppressWarnings("unchecked") Map<Long,Long> entries=(Map<Long,Long>)map;
                List<String> out=new ArrayList<>();for(Map.Entry<Long,Long> e:entries.entrySet()){long k=e.getKey();out.add((int)k+","+(int)(k>>>32)+","+e.getValue());}return "LO|"+String.join(";",out);
            }
            case "level.clear":L.getMethod("a",BOX).invoke(level,bounds(f,1));return "LX";
            case "level.copy":L.getMethod("a",BOX,VEC).invoke(level,bounds(f,1),VEC.getConstructor(int.class,int.class,int.class).newInstance(Integer.parseInt(f[7]),Integer.parseInt(f[8]),Integer.parseInt(f[9])));return "LY";
            case "level.tick":{
                long now=Long.parseLong(f[1]);List<String> out=new ArrayList<>();long[] order={10000};boolean[] first={true};
                BiConsumer<Object,Object> callback=(pos,kind)->{try{
                    boolean before=(boolean)L.getMethod("a",POS,Object.class).invoke(level,pos,kind),will=(boolean)L.getMethod("b",POS,Object.class).invoke(level,pos,kind);
                    boolean query=f[3].equals("query")||f[3].equals("clear")||f[3].equals("copy");Object watch=pos("0","0","0"),watchKind=symbol("watch");boolean watchBefore=query&&(boolean)L.getMethod("b",POS,Object.class).invoke(level,watch,watchKind);
                    if(f[3].equals("reschedule"))L.getMethod("a",T).invoke(level,T.getConstructor(Object.class,POS,long.class,long.class).newInstance(kind,pos,now,order[0]++));
                    if(first[0]&&(f[3].equals("clear")||f[3].equals("copy"))){Object box=BOX.getConstructor(int.class,int.class,int.class,int.class,int.class,int.class).newInstance(-16,-2,-16,15,2,15);if(f[3].equals("clear"))L.getMethod("a",BOX).invoke(level,box);else L.getMethod("a",BOX,VEC).invoke(level,box,VEC.getConstructor(int.class,int.class,int.class).newInstance(16,0,16));}first[0]=false;
                    boolean after=(boolean)L.getMethod("a",POS,Object.class).invoke(level,pos,kind),willAfter=(boolean)L.getMethod("b",POS,Object.class).invoke(level,pos,kind);
                    String entry=kind+","+POS.getMethod("u").invoke(pos)+","+POS.getMethod("v").invoke(pos)+","+POS.getMethod("w").invoke(pos)+":"+before+","+will+","+after+","+willAfter;if(query)entry+=","+watchBefore+","+L.getMethod("b",POS,Object.class).invoke(level,watch,watchKind);out.add(entry);
                }catch(Exception e){throw new RuntimeException(e);}};
                L.getMethod("a",long.class,int.class,BiConsumer.class).invoke(level,now,Integer.parseInt(f[2]),callback);return "LT|"+String.join(";",out);
            }
            default:throw new IllegalArgumentException(line);
        }
    }
    public static void main(String[] args)throws Exception {
        BufferedReader input=new BufferedReader(new InputStreamReader(System.in));
        for(String line;(line=input.readLine())!=null;)OUTPUT.println(run(line));
    }
}
