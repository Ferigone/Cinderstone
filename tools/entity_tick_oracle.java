package oracle;

import com.google.gson.*;
import java.io.*;
import java.lang.reflect.*;
import java.util.*;
import java.util.function.Consumer;

/** Calls the original1.21 EntityTickList. Entities are original ItemEntity
 * instances allocated without constructors; only their Entity.id field is set.
 * No Minecraft methods/classes are rewritten or replaced. */
public final class entity_tick_oracle {
    static final Gson G=new GsonBuilder().serializeNulls().create();
    static final PrintStream OUT=System.out;
    static Class<?> LIST,ENTITY,ITEM;
    static Field ID,ACTIVE,PASSIVE,ITERATED;
    static Method ALLOCATE,ADD,REMOVE,CONTAINS,EACH;
    static Object UNSAFE;
    static Throwable cause(Throwable e){
        while(e instanceof InvocationTargetException||e instanceof ExecutionFailure){
            Throwable next=e instanceof ExecutionFailure?((ExecutionFailure)e).original:e.getCause();
            if(next==null)break;e=next;
        }
        return e;
    }
    static String error(Throwable e){e=cause(e);return e.getClass().getSimpleName()+": "+e.getMessage();}
    static final class ProbeFailure extends RuntimeException {ProbeFailure(){super("controlled");}}
    static final class ExecutionFailure extends RuntimeException {
        final JsonObject record;final Throwable original;
        ExecutionFailure(JsonObject record,Throwable original){this.record=record;this.original=original;}
    }
    static final class Scene {
        final Object list;final Object[] maps;
        final Map<Integer,Object> entities=new HashMap<>();
        final IdentityHashMap<Object,Integer> labels=new IdentityHashMap<>();
        Scene(JsonObject q)throws Exception {
            list=LIST.getConstructor().newInstance();maps=new Object[]{ACTIVE.get(list),PASSIVE.get(list)};
            for(JsonElement entry:q.getAsJsonArray("entities")){
                JsonArray row=entry.getAsJsonArray();int label=row.get(0).getAsInt();
                Object entity=ALLOCATE.invoke(UNSAFE,ITEM);ID.setInt(entity,row.get(1).getAsInt());
                entities.put(label,entity);labels.put(entity,label);
            }
        }
        JsonArray entries(Object value)throws Exception {
            JsonArray out=new JsonArray();
            for(Map.Entry<?,?> e:((Map<?,?>)value).entrySet()){
                JsonArray row=new JsonArray();row.add((Integer)e.getKey());
                row.add(labels.get(e.getValue()));row.add(ID.getInt(e.getValue()));out.add(row);
            }
            return out;
        }
        JsonObject state()throws Exception {
            JsonObject s=new JsonObject();Object a=ACTIVE.get(list),b=PASSIVE.get(list),c=ITERATED.get(list);
            s.add("active",entries(a));s.add("passive",entries(b));
            s.addProperty("active_map",a==maps[0]?0:1);
            s.add("iterated_map",c==null?JsonNull.INSTANCE:new JsonPrimitive(c==maps[0]?0:1));
            return s;
        }
        JsonObject execute(JsonObject op){
            JsonObject out=new JsonObject();String kind=op.get("op").getAsString();out.addProperty("op",kind);
            Throwable failure=null;
            try {
                switch(kind){
                    case "add":ADD.invoke(list,entities.get(op.get("entity").getAsInt()));break;
                    case "remove":REMOVE.invoke(list,entities.get(op.get("entity").getAsInt()));break;
                    case "contains":out.addProperty("result",(boolean)CONTAINS.invoke(list,entities.get(op.get("entity").getAsInt())));break;
                    case "id":ID.setInt(entities.get(op.get("entity").getAsInt()),op.get("value").getAsInt());break;
                    case "throw":throw new ProbeFailure();
                    case "iterate":{
                        JsonArray visits=new JsonArray(),callbacks=new JsonArray();out.add("visits",visits);out.add("callbacks",callbacks);
                        int[] count={0};
                        EACH.invoke(list,(Consumer<Object>)entity->{
                            try {
                                int visit=count[0]++;JsonArray row=new JsonArray();row.add(labels.get(entity));row.add(ID.getInt(entity));visits.add(row);
                                if(!op.has("on_visit"))return;
                                for(JsonElement callback:op.getAsJsonArray("on_visit")){
                                    JsonObject program=callback.getAsJsonObject();if(program.get("at").getAsInt()!=visit)continue;
                                    JsonObject called=new JsonObject();called.addProperty("at",visit);JsonArray effects=new JsonArray();called.add("effects",effects);callbacks.add(called);
                                    for(JsonElement action:program.getAsJsonArray("actions"))try{effects.add(execute(action.getAsJsonObject()));}
                                    catch(ExecutionFailure e){effects.add(e.record);throw e;}
                                }
                            }catch(RuntimeException e){throw e;}catch(Exception e){throw new RuntimeException(e);}
                        });
                        break;
                    }
                    default:throw new IllegalArgumentException(kind);
                }
            }catch(Throwable e){failure=cause(e);}
            out.add("error",failure==null?JsonNull.INSTANCE:new JsonPrimitive(error(failure)));
            try{out.add("state",state());}catch(Exception e){throw new RuntimeException(e);}
            if(failure!=null&&op.has("propagate")&&op.get("propagate").getAsBoolean())throw new ExecutionFailure(out,failure);
            return out;
        }
        JsonArray run(JsonObject q){JsonArray out=new JsonArray();for(JsonElement op:q.getAsJsonArray("operations"))out.add(execute(op.getAsJsonObject()));return out;}
    }
    static Field field(Class<?> type,String name)throws Exception{Field f=type.getDeclaredField(name);f.setAccessible(true);return f;}
    static void init()throws Exception {
        Class.forName("ab").getMethod("a").invoke(null);Class.forName("akt").getMethod("a").invoke(null);
        LIST=Class.forName("dxm");ENTITY=Class.forName("bsr");ITEM=Class.forName("cjh");
        Class<?> unsafe=Class.forName("sun.misc.Unsafe");UNSAFE=field(unsafe,"theUnsafe").get(null);ALLOCATE=unsafe.getMethod("allocateInstance",Class.class);
        ID=field(ENTITY,"o");ACTIVE=field(LIST,"a");PASSIVE=field(LIST,"b");ITERATED=field(LIST,"c");
        ADD=LIST.getMethod("a",ENTITY);REMOVE=LIST.getMethod("b",ENTITY);CONTAINS=LIST.getMethod("c",ENTITY);EACH=LIST.getMethod("a",Consumer.class);
        OUT.println("SOURCE:"+LIST.getProtectionDomain().getCodeSource().getLocation());
    }
    public static void main(String[]args)throws Exception {
        init();BufferedReader reader=new BufferedReader(new InputStreamReader(System.in));
        for(String line;(line=reader.readLine())!=null;){JsonObject q=JsonParser.parseString(line).getAsJsonObject();OUT.println("ORACLE:"+G.toJson(new Scene(q).run(q)));}
    }
}
