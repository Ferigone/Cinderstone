package oracle;
import com.google.gson.*;
import java.lang.reflect.*;
import java.nio.file.*;
import java.util.*;
/** Binds official JAR block tags to the original registry for isolated oracles. */
public class placement_tags {
    public static void bind(String path)throws Exception {
        JsonObject tags=JsonParser.parseString(Files.readString(Path.of(path))).getAsJsonObject().getAsJsonObject("tags");
        Object registry=Class.forName("lt").getField("e").get(null),key=Class.forName("lu").getField("f").get(null);
        Class<?> registryClass=Class.forName("jz");Method wrap=registryClass.getMethod("e",Object.class);Map<Object,List<Object>> values=new LinkedHashMap<>();
        for(var entry:tags.entrySet()){
            Object location=Class.forName("akr").getMethod("b",String.class).invoke(null,entry.getKey().replace("minecraft:",""));Object tag=Class.forName("awu").getMethod("a",Class.forName("akq"),Class.forName("akr")).invoke(null,key,location);List<Object> blocks=new ArrayList<>();
            for(JsonElement state:entry.getValue().getAsJsonArray()){Object block=Class.forName("dtc").getMethod("b").invoke(Class.forName("dfy").getMethod("a",int.class).invoke(null,state.getAsInt()));blocks.add(wrap.invoke(registry,block));}values.put(tag,blocks);
        }
        registryClass.getMethod("a",Map.class).invoke(registry,values);
    }
}
