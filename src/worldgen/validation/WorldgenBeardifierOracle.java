import com.google.gson.*;
import java.io.*;
import java.nio.file.*;
import java.util.*;

/** Ordered original Beardifier values, distinct from StructureManager selection. */
public final class WorldgenBeardifierOracle {
 public static void main(String[] args)throws Exception {
  if(args.length!=1)throw new IllegalArgumentException("external output directory required");
  WorldgenExistingTargetOracle.output=Path.of(args[0]).toAbsolutePath().normalize();
  Files.createDirectories(WorldgenExistingTargetOracle.output);
  WorldgenExistingTargetOracle.setup();
  BufferedReader input=new BufferedReader(new InputStreamReader(System.in));
  for(String line;(line=input.readLine())!=null;) {
   JsonObject query=JsonParser.parseString(line).getAsJsonObject();
   try {
    Object result;
    if(query.has("mode")&&query.get("mode").getAsString().equals("beardifier_kernel")) {
     Map<String,Object> row=new LinkedHashMap<>();row.put("query",query);row.put("oracle_inner_sha256",WorldgenExistingTargetOracle.INNER);
     row.putAll(WorldgenExistingTargetOracle.beardifierObservations(query,WorldgenExistingTargetOracle.beardifier(query)));result=row;
    } else result=WorldgenExistingTargetOracle.run(query);
    WorldgenExistingTargetOracle.OUT.println(WorldgenExistingTargetOracle.JSON.toJson(result));
   }catch(java.lang.reflect.InvocationTargetException error){error.getCause().printStackTrace();System.exit(1);}
  }
 }
}
