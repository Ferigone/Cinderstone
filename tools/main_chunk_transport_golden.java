package oracle;
import com.google.gson.*;
import java.lang.reflect.*;
import java.util.*;
import java.util.function.Consumer;

/** New successful 45-chunk history. Existing prefix-mode vectors stay unchanged. */
public final class main_chunk_transport_golden {
 static final java.io.PrintStream OUT=System.out;
 static Class<?> C(String n)throws Exception{return Class.forName(n);}
 public static void main(String[]args)throws Exception{
  C("ab").getMethod("a").invoke(null);C("akt").getMethod("a").invoke(null);
  Class<?> u=C("sun.misc.Unsafe");Field f=u.getDeclaredField("theUnsafe");f.setAccessible(true);survival_oracle.unsafe=f.get(null);survival_oracle.allocate=u.getMethod("allocateInstance",Class.class);
  player_chunk_sender_oracle.adapters();
  Object center=C("dcd").getConstructor(int.class,int.class).newInstance(0,0);
  Object view=C("aqg").getMethod("a",C("dcd"),int.class).invoke(null,center,2);
  JsonArray positions=new JsonArray();
  C("aqg").getMethod("a",Consumer.class).invoke(view,(Consumer<Object>)(p->{try{JsonArray point=new JsonArray();point.add(C("dcd").getField("e").getInt(p));point.add(C("dcd").getField("f").getInt(p));positions.add(point);}catch(Exception e){throw new RuntimeException(e);}}));
  if(positions.size()!=45)throw new IllegalStateException("expected original45 view");
  JsonArray ready=new JsonArray();for(JsonElement p:positions){JsonArray row=new JsonArray();row.add(p.getAsJsonArray().get(0));row.add(p.getAsJsonArray().get(1));row.add(p.getAsJsonArray().get(0));row.add(p.getAsJsonArray().get(1));ready.add(row);}
  JsonArray steps=new JsonArray();JsonObject mark=new JsonObject();mark.addProperty("op","mark");mark.add("positions",positions);steps.add(mark);
  steps.add(send(ready));for(int i=0;i<6;i++)steps.add(send(ready));
  JsonObject ack=new JsonObject();ack.addProperty("op","ack");ack.addProperty("desired","41800000");steps.add(ack);
  for(int i=0;i<3;i++)steps.add(send(ready));
  JsonObject query=new JsonObject();query.addProperty("memory",false);query.add("steps",steps);
  JsonObject expected=player_chunk_sender_oracle.query(query);JsonArray batches=new JsonArray();JsonArray batch=null;
  for(JsonElement st:expected.getAsJsonArray("steps")){
   if(st.getAsJsonObject().get("failed_transport").getAsBoolean())throw new IllegalStateException("unexpected transport failure");
   for(JsonElement t:st.getAsJsonObject().getAsJsonArray("trace")){
    JsonArray trace=t.getAsJsonArray();switch(trace.get(0).getAsString()){
     case "begin":if(batch!=null)throw new IllegalStateException();batch=new JsonArray();break;
     case "chunk":JsonArray point=new JsonArray();point.add(trace.get(1));point.add(trace.get(2));batch.add(point);break;
     case "end":if(batch.size()!=trace.get(1).getAsInt())throw new IllegalStateException();batches.add(batch);batch=null;break;
    }
   }
  }
  for(String n:new String[]{"aqg","aqg$a","arq","dvi","adg","adf","adk","eot"})OUT.println("SOURCE:"+n+":"+C(n).getProtectionDomain().getCodeSource().getLocation());
  JsonObject output=new JsonObject();output.add("query",query);output.add("expected",expected);output.add("mark_order",positions);output.add("batches",batches);OUT.println("GOLDEN:"+new Gson().toJson(output));
 }
 static JsonObject send(JsonArray ready){JsonObject s=new JsonObject();s.addProperty("op","send");s.addProperty("fail","");s.add("center",new JsonArray());s.getAsJsonArray("center").add(0);s.getAsJsonArray("center").add(0);s.add("ready",ready);return s;}
}
