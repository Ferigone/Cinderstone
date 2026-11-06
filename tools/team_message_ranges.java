// SPDX-License-Identifier: GPL-2.0-only
// Execute the original TeamMsgCommand registration and Brigadier parse; never execute its callbacks.
package oracle;
import com.google.gson.*;
import com.mojang.brigadier.*;
import com.mojang.brigadier.context.*;
import java.io.*;

public class team_message_ranges {
 static final PrintStream OUT = System.out;
 public static void main(String[] args) throws Exception {
  Class.forName("ab").getMethod("a").invoke(null);
  Class.forName("akt").getMethod("a").invoke(null);
  CommandDispatcher<Object> dispatcher = new CommandDispatcher<>();
  Class.forName("aot").getMethod("a", CommandDispatcher.class).invoke(null, dispatcher);
  Gson gson = new GsonBuilder().serializeNulls().create();
  BufferedReader input = new BufferedReader(new InputStreamReader(System.in));
  for(String line; (line=input.readLine())!=null;) {
   String command = JsonParser.parseString(line).getAsString();
   ParseResults<Object> parsed = dispatcher.parse(command, new Object());
   CommandContextBuilder<Object> context = parsed.getContext();
   while(context.getChild()!=null) context=context.getChild();
   ParsedArgument<Object, ?> message = context.getArguments().get("message");
   JsonObject result = new JsonObject(); result.addProperty("command", command);
   boolean complete = !parsed.getReader().canRead() && context.getCommand()!=null && message!=null;
   result.addProperty("complete", complete);
   if(complete) {
    result.addProperty("start_utf16", message.getRange().getStart());
    result.addProperty("end_utf16", message.getRange().getEnd());
    result.addProperty("message", message.getRange().get(command));
   } else result.add("message", JsonNull.INSTANCE);
   OUT.println("ORACLE:"+gson.toJson(result));
  }
 }
}
