package oracle;

import com.google.gson.*;
import java.io.*;
import java.lang.reflect.*;

/** Observe the original avu logger call boundary and forward to its real logger. */
public final class server_recipe_book_load_logs {
    static void install() throws Exception {
        Class<?> owner = server_recipe_book_oracle.C("avu");
        Field field = owner.getDeclaredField("d");
        field.setAccessible(true);
        Object original = field.get(null);
        Class<?> logger = server_recipe_book_oracle.C("org.slf4j.Logger");
        Object observer = Proxy.newProxyInstance(logger.getClassLoader(), new Class<?>[] {logger}, (proxy, method, args) -> {
            if (method.getName().equals("error") && args != null && args.length == 2 && args[0] instanceof String) {
                JsonObject event = server_recipe_book_oracle.state();
                event.addProperty("callback", "load_log");
                event.addProperty("message_template", (String)args[0]);
                String argument = String.valueOf(args[1]);
                event.addProperty("argument", argument);
                JsonArray units = new JsonArray();
                for (int i = 0; i < argument.length(); i++) units.add((int)argument.charAt(i));
                event.add("argument_utf16", units);
                server_recipe_book_oracle.events.add(event);
            }
            try {
                return method.invoke(original, args);
            } catch (InvocationTargetException error) {
                throw error.getCause();
            }
        });
        Object unsafe = server_recipe_book_oracle.unsafe();
        Class<?> type = server_recipe_book_oracle.C("sun.misc.Unsafe");
        Object base = type.getMethod("staticFieldBase", Field.class).invoke(unsafe, field);
        long offset = (long)type.getMethod("staticFieldOffset", Field.class).invoke(unsafe, field);
        type.getMethod("putObjectVolatile", Object.class, long.class, Object.class).invoke(unsafe, base, offset, observer);
    }

    public static void main(String[] args) throws Exception {
        server_recipe_book_oracle.init(args[0]);
        install();
        BufferedReader input = new BufferedReader(new InputStreamReader(System.in));
        for (String line; (line = input.readLine()) != null;) {
            JsonObject query = JsonParser.parseString(line).getAsJsonObject();
            server_recipe_book_oracle.OUT.println("ORACLE:" + server_recipe_book_oracle.JSON.toJson(server_recipe_book_oracle.run(query)));
        }
    }
}
