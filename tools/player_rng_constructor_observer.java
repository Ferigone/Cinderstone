package oracle;
import java.lang.reflect.*;import java.nio.file.*;import java.util.concurrent.atomic.AtomicLong;

/** Bootstrap-visible observer: reads RNG identity/state without drawing. */
public final class player_rng_constructor_observer {
 static Field field(Class<?> type,String name)throws Exception {for(Class<?> c=type;c!=null;c=c.getSuperclass())try{Field f=c.getDeclaredField(name);f.setAccessible(true);return f;}catch(NoSuchFieldException ignored){}throw new NoSuchFieldException(name);}
 static Field entityField(Object entity,String name)throws Exception {for(Class<?> c=entity.getClass();c!=null;c=c.getSuperclass())if(c.getName().equals("bsr")){Field f=c.getDeclaredField(name);f.setAccessible(true);return f;}throw new NoSuchFieldException("bsr."+name);}
 static String escape(String s){return s.replace("\\","\\\\").replace("\"","\\\"").replace("\n","\\n").replace("\r","\\r");}
 public static synchronized void observe(Object entity,String stage){
  if(!entity.getClass().getName().equals("aqv"))return;
  String path=System.getProperty("mc121.player_rng_trace");if(path==null)throw new IllegalStateException("missing constructor trace path");
  String line;
  try{
   Object random=entityField(entity,"ah").get(entity);Object raw=field(random.getClass(),"h").get(random);long state=raw instanceof AtomicLong?((AtomicLong)raw).get():((Number)raw).longValue();Object uuid=entityField(entity,"ax").get(entity);
   line="{\"stage\":\""+stage+"\",\"entity_identity\":"+System.identityHashCode(entity)+",\"entity_id\":"+entityField(entity,"o").getInt(entity)+",\"random_identity\":"+System.identityHashCode(random)+",\"random_class\":\""+random.getClass().getName()+"\",\"state48\":\""+state+"\",\"uuid\":"+(uuid==null?"null":"\""+uuid+"\"")+"}";
  }catch(Throwable error){line="{\"stage\":\""+stage+"\",\"error\":\""+escape(error.toString())+"\"}";}
  try{Files.writeString(Path.of(path),line+"\n",StandardOpenOption.CREATE,StandardOpenOption.APPEND);}catch(Exception error){throw new IllegalStateException("constructor trace write failed",error);}
 }
}
