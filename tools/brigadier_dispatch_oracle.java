package cinderstone.oracle;

// Executed Brigadier dispatcher oracle for WP-E3 stages 2-4 (docs/reference/commands-brigadier.md).
//
// Builds command trees from JSON descriptions with the UNCHANGED com.mojang.brigadier 1.2.9
// builders and CommandDispatcher (the pinned server's bundled library), installs the server's
// own BuiltInExceptionProvider (obfuscated `en`, net.minecraft.commands.BrigadierExceptions)
// and records, for each input, the parse result, every executed context and the execution
// outcome. Stage 4 adds completion suggestions at chosen cursors (inputs with "cursors"),
// custom suggestion providers ("sug" on argument nodes) and direct tests of
// Suggestions/SuggestionsBuilder and of the Java string semantics they use (entries with
// "direct" instead of "tree"). Inputs without "cursors" produce exactly the stage-2/3 output. Build and run it with tools/brigadier_dispatch_generate.py; the oracle directory
// must stay OUTSIDE git. The harness lives in its own package because the server jar is signed.
//
// Usage: java -cp <server-inner-1.21.jar:libraries/**/*.jar:classes>
//        cinderstone.oracle.brigadier_dispatch_oracle cases.json out.json
import com.google.gson.Gson;
import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import com.mojang.brigadier.Command;
import com.mojang.brigadier.CommandDispatcher;
import com.mojang.brigadier.ParseResults;
import com.mojang.brigadier.RedirectModifier;
import com.mojang.brigadier.StringReader;
import com.mojang.brigadier.arguments.ArgumentType;
import com.mojang.brigadier.arguments.BoolArgumentType;
import com.mojang.brigadier.arguments.DoubleArgumentType;
import com.mojang.brigadier.arguments.FloatArgumentType;
import com.mojang.brigadier.arguments.IntegerArgumentType;
import com.mojang.brigadier.arguments.LongArgumentType;
import com.mojang.brigadier.arguments.StringArgumentType;
import com.mojang.brigadier.builder.ArgumentBuilder;
import com.mojang.brigadier.builder.LiteralArgumentBuilder;
import com.mojang.brigadier.builder.RequiredArgumentBuilder;
import com.mojang.brigadier.context.CommandContext;
import com.mojang.brigadier.context.ParsedCommandNode;
import com.mojang.brigadier.exceptions.BuiltInExceptionProvider;
import com.mojang.brigadier.context.StringRange;
import com.mojang.brigadier.exceptions.CommandSyntaxException;
import com.mojang.brigadier.suggestion.IntegerSuggestion;
import com.mojang.brigadier.suggestion.Suggestion;
import com.mojang.brigadier.suggestion.SuggestionProvider;
import com.mojang.brigadier.suggestion.Suggestions;
import com.mojang.brigadier.suggestion.SuggestionsBuilder;
import com.mojang.brigadier.tree.CommandNode;
import java.lang.reflect.Method;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Collection;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.TreeSet;

@SuppressWarnings({"unchecked", "rawtypes"})
public final class brigadier_dispatch_oracle {
    private static final Gson GSON = new Gson();
    private static List<String> events = new ArrayList<>();
    private static List<String> argNames;

    public static void main(String[] args) throws Exception {
        BuiltInExceptionProvider provider =
            (BuiltInExceptionProvider) Class.forName("en").getDeclaredConstructor().newInstance();
        CommandSyntaxException.BUILT_IN_EXCEPTIONS = provider;
        JsonArray tests = JsonParser.parseString(Files.readString(Path.of(args[0]), StandardCharsets.UTF_8)).getAsJsonArray();
        JsonArray out = new JsonArray();
        for (JsonElement element : tests) {
            JsonObject entry = element.getAsJsonObject();
            out.add(entry.has("direct") ? runDirect(entry.getAsJsonArray("direct")) : runTest(entry));
        }
        Files.writeString(Path.of(args[1]), asciiOnly(GSON.toJson(out)), StandardCharsets.UTF_8);
    }

    private static JsonObject runTest(JsonObject test) {
        CommandDispatcher<Integer> dispatcher = new CommandDispatcher<>();
        TreeSet<String> names = new TreeSet<>();
        for (JsonElement tree : test.getAsJsonArray("tree")) {
            collectNames(tree.getAsJsonObject(), names);
        }
        argNames = new ArrayList<>(names);
        dispatcher.setConsumer((context, success, result) -> events.add("done|src=" + context.getSource() + "|" + success + "|" + result));
        for (JsonElement tree : test.getAsJsonArray("tree")) {
            dispatcher.register((LiteralArgumentBuilder<Integer>) build(tree.getAsJsonObject(), dispatcher));
        }
        JsonArray results = new JsonArray();
        for (JsonElement element : test.getAsJsonArray("inputs")) {
            JsonObject in = element.getAsJsonObject();
            results.add(runInput(dispatcher, in.get("input").getAsString(), in.get("source").getAsInt(),
                in.has("cursors") ? in.getAsJsonArray("cursors") : null));
        }
        JsonObject out = new JsonObject();
        out.add("results", results);
        JsonArray usage = new JsonArray();
        collectUsage(dispatcher, dispatcher.getRoot(), new ArrayList<>(), usage);
        out.add("usage", usage);
        return out;
    }

    // getAllUsage / getSmartUsage for every node (root included) and several sources.
    private static void collectUsage(CommandDispatcher<Integer> dispatcher, CommandNode<Integer> node, List<String> path, JsonArray out) {
        for (int source : new int[] {0, 2, 4}) {
            JsonObject entry = new JsonObject();
            JsonArray names = new JsonArray();
            for (String name : path) {
                names.add(name);
            }
            entry.add("path", names);
            entry.addProperty("source", source);
            JsonArray restricted = new JsonArray();
            for (String line : dispatcher.getAllUsage(node, source, true)) {
                restricted.add(line);
            }
            entry.add("allRestricted", restricted);
            JsonArray unrestricted = new JsonArray();
            for (String line : dispatcher.getAllUsage(node, source, false)) {
                unrestricted.add(line);
            }
            entry.add("allUnrestricted", unrestricted);
            JsonArray smart = new JsonArray();
            for (Map.Entry<CommandNode<Integer>, String> item : dispatcher.getSmartUsage(node, source).entrySet()) {
                JsonArray pair = new JsonArray();
                pair.add(item.getKey().getName());
                pair.add(item.getValue());
                smart.add(pair);
            }
            entry.add("smart", smart);
            out.add(entry);
        }
        for (CommandNode<Integer> child : node.getChildren()) {
            List<String> next = new ArrayList<>(path);
            next.add(child.getName());
            collectUsage(dispatcher, child, next, out);
        }
    }

    private static void collectNames(JsonObject spec, TreeSet<String> names) {
        if (spec.get("t").getAsString().equals("arg")) {
            names.add(spec.get("name").getAsString());
        }
        if (spec.has("children")) {
            for (JsonElement child : spec.getAsJsonArray("children")) {
                collectNames(child.getAsJsonObject(), names);
            }
        }
    }

    private static ArgumentBuilder build(JsonObject spec, CommandDispatcher<Integer> dispatcher) {
        String name = spec.get("name").getAsString();
        ArgumentBuilder builder = spec.get("t").getAsString().equals("lit")
            ? LiteralArgumentBuilder.literal(name)
            : RequiredArgumentBuilder.argument(name, type(spec.getAsJsonObject("type")));
        if (spec.has("sug")) {
            ((RequiredArgumentBuilder) builder).suggests(provider(spec.getAsJsonObject("sug")));
        }
        if (spec.has("req")) {
            int level = spec.get("req").getAsInt();
            builder.requires(source -> (Integer) source >= level);
        }
        if (spec.has("cmd")) {
            JsonElement cmd = spec.get("cmd");
            builder.executes(command(cmd));
        }
        if (spec.has("children")) {
            for (JsonElement child : spec.getAsJsonArray("children")) {
                builder.then(build(child.getAsJsonObject(), dispatcher));
            }
        }
        if (spec.has("redirect")) {
            JsonObject r = spec.getAsJsonObject("redirect");
            CommandNode<Integer> target = r.get("to").getAsString().equals("root")
                ? dispatcher.getRoot()
                : dispatcher.getRoot().getChild(r.get("to").getAsString());
            String kind = r.get("kind").getAsString();
            if (kind.equals("plain")) {
                builder.redirect(target);
            } else if (kind.equals("single")) {
                builder.redirect(target, c -> {
                    events.add("single|" + digest((CommandContext<Integer>) c));
                    return ((CommandContext<Integer>) c).getSource() + 1;
                });
            } else {
                builder.fork(target, modifier(r.get("mod").getAsString()));
            }
        }
        return builder;
    }

    private static Command<Integer> command(JsonElement cmd) {
        return context -> {
            events.add("run|" + digest(context));
            if (cmd.getAsJsonPrimitive().isString()) {
                throw CommandSyntaxException.BUILT_IN_EXCEPTIONS.dispatcherUnknownCommand().create();
            }
            return cmd.getAsInt() + context.getSource() % 7;
        };
    }

    private static RedirectModifier<Integer> modifier(String kind) {
        return context -> {
            events.add("mod|" + kind + "|" + digest(context));
            switch (kind) {
                case "id": return List.of(context.getSource());
                case "dup": return List.of(context.getSource(), context.getSource() + 100);
                case "none": return List.of();
                default: throw CommandSyntaxException.BUILT_IN_EXCEPTIONS.dispatcherUnknownArgument().create();
            }
        };
    }

    private static ArgumentType type(JsonObject t) {
        switch (t.get("k").getAsString()) {
            case "bool": return BoolArgumentType.bool();
            case "int": return IntegerArgumentType.integer(t.get("min").getAsInt(), t.get("max").getAsInt());
            case "long": return LongArgumentType.longArg(Long.parseLong(t.get("min").getAsString()), Long.parseLong(t.get("max").getAsString()));
            case "float": return FloatArgumentType.floatArg(Float.intBitsToFloat((int) Long.parseLong(t.get("min").getAsString(), 16)), Float.intBitsToFloat((int) Long.parseLong(t.get("max").getAsString(), 16)));
            case "double": return DoubleArgumentType.doubleArg(Double.longBitsToDouble(Long.parseUnsignedLong(t.get("min").getAsString(), 16)), Double.longBitsToDouble(Long.parseUnsignedLong(t.get("max").getAsString(), 16)));
            case "word": return StringArgumentType.word();
            case "string": return StringArgumentType.string();
            case "greedy": return StringArgumentType.greedyString();
            default: throw new IllegalArgumentException(t.get("k").getAsString());
        }
    }

    private static String valueText(Object value) {
        if (value instanceof Float) {
            return "f" + Integer.toHexString(Float.floatToRawIntBits((Float) value));
        }
        if (value instanceof Double) {
            return "d" + Long.toHexString(Double.doubleToRawLongBits((Double) value));
        }
        return value.getClass().getSimpleName().charAt(0) + String.valueOf(value);
    }

    // One line describing everything observable about a context, shared with the Rust test.
    private static String digest(CommandContext<Integer> c) {
        StringBuilder sb = new StringBuilder();
        sb.append("src=").append(c.getSource());
        sb.append(";in=").append(c.getInput());
        sb.append(";rng=").append(c.getRange().getStart()).append('-').append(c.getRange().getEnd());
        sb.append(";nodes=");
        for (ParsedCommandNode<Integer> node : c.getNodes()) {
            sb.append(node.getNode().getName()).append('@').append(node.getRange().getStart()).append('-').append(node.getRange().getEnd()).append(',');
        }
        sb.append(";args=");
        for (String name : argNames) {
            try {
                String text = valueText(c.getArgument(name, Object.class));
                sb.append(name).append('=').append(text).append(',');
            } catch (IllegalArgumentException e) {
                // not parsed in this context
            }
        }
        sb.append(";fork=").append(c.isForked());
        sb.append(";mod=").append(c.getRedirectModifier() != null);
        sb.append(";cmd=").append(c.getCommand() != null);
        sb.append(";child=").append(c.getChild() != null);
        return sb.toString();
    }

    private static JsonObject runInput(CommandDispatcher<Integer> dispatcher, String input, int source, JsonArray cursors) {
        boolean input_has_cursors = cursors != null;
        JsonObject out = new JsonObject();
        events = new ArrayList<>();
        ParseResults<Integer> parse = dispatcher.parse(new StringReader(input), source);
        JsonObject p = new JsonObject();
        p.addProperty("cursor", parse.getReader().getCursor());
        JsonArray errors = new JsonArray();
        for (Map.Entry<CommandNode<Integer>, CommandSyntaxException> entry : parse.getExceptions().entrySet()) {
            JsonObject e = describe(entry.getValue());
            e.addProperty("node", entry.getKey().getName());
            errors.add(e);
        }
        p.add("errors", errors);
        JsonArray chain = new JsonArray();
        CommandContext<Integer> context = parse.getContext().build(input);
        while (context != null) {
            chain.add(digest(context));
            context = context.getChild();
        }
        p.add("ctx", chain);
        out.add("parse", p);
        events = new ArrayList<>();
        try {
            out.addProperty("result", dispatcher.execute(parse));
        } catch (CommandSyntaxException e) {
            out.add("err", describe(e));
        }
        JsonArray log = new JsonArray();
        for (String event : events) {
            log.add(event);
        }
        out.add("events", log);
        if (input_has_cursors) {
            JsonArray suggestions = new JsonArray();
            for (JsonElement cursor : cursors) {
                suggestions.add(suggestAt(dispatcher, parse, cursor.getAsInt()));
            }
            out.add("suggest", suggestions);
        }
        return out;
    }

    // ---- stage 4: completion suggestions -------------------------------------------------

    private static SuggestionProvider<Integer> provider(JsonObject sug) {
        String kind = sug.get("kind").getAsString();
        List<String> words = new ArrayList<>();
        if (sug.has("words")) {
            for (JsonElement word : sug.getAsJsonArray("words")) {
                words.add(word.getAsString());
            }
        }
        return (context, builder) -> {
            switch (kind) {
                case "words":
                    for (String word : words) {
                        builder.suggest(word);
                    }
                    break;
                case "filter":
                    // the usual SharedSuggestionProvider.suggest(Iterable, builder) shape
                    for (String word : words) {
                        if (word.toLowerCase(Locale.ROOT).startsWith(builder.getRemainingLowerCase())) {
                            builder.suggest(word);
                        }
                    }
                    break;
                case "ctx":
                    builder.suggest("in=" + context.getInput() + "|nodes=" + context.getNodes().size()
                        + "|rng=" + context.getRange().getStart() + "-" + context.getRange().getEnd()
                        + "|src=" + context.getSource() + "|start=" + builder.getStart()
                        + "|rem=" + builder.getRemaining());
                    break;
                case "throw":
                    throw CommandSyntaxException.BUILT_IN_EXCEPTIONS.dispatcherUnknownArgument().create();
                default:
                    throw new IllegalArgumentException(kind);
            }
            return builder.buildFuture();
        };
    }

    private static JsonObject suggestAt(CommandDispatcher<Integer> dispatcher, ParseResults<Integer> parse, int cursor) {
        JsonObject o = new JsonObject();
        o.addProperty("cursor", cursor);
        try {
            o.add("result", describe(dispatcher.getCompletionSuggestions(parse, cursor).join(), null));
        } catch (IllegalStateException | IndexOutOfBoundsException e) {
            o.addProperty("throws", e.getClass().getSimpleName());
        }
        return o;
    }

    private static JsonArray pair(int a, int b) {
        JsonArray array = new JsonArray();
        array.add(a);
        array.add(b);
        return array;
    }

    private static JsonObject describe(Suggestion suggestion, String command) {
        JsonObject o = new JsonObject();
        o.add("r", pair(suggestion.getRange().getStart(), suggestion.getRange().getEnd()));
        o.addProperty("t", suggestion.getText());
        if (suggestion instanceof IntegerSuggestion) {
            o.addProperty("i", ((IntegerSuggestion) suggestion).getValue());
        }
        if (command != null) {
            // hash code and apply() are recorded for the direct tests only
            o.addProperty("h", suggestion.hashCode());
            o.addProperty("a", suggestion.apply(command));
        }
        return o;
    }

    private static JsonObject describe(Suggestions suggestions, String command) {
        JsonObject o = new JsonObject();
        o.add("range", pair(suggestions.getRange().getStart(), suggestions.getRange().getEnd()));
        JsonArray list = new JsonArray();
        for (Suggestion suggestion : suggestions.getList()) {
            list.add(describe(suggestion, command));
        }
        o.add("list", list);
        return o;
    }

    private static Suggestion suggestion(JsonObject spec) {
        StringRange range = new StringRange(spec.getAsJsonArray("r").get(0).getAsInt(), spec.getAsJsonArray("r").get(1).getAsInt());
        return spec.has("i") ? new IntegerSuggestion(range, spec.get("i").getAsInt()) : new Suggestion(range, spec.get("t").getAsString());
    }

    private static List<Suggestion> suggestionList(JsonArray specs) {
        List<Suggestion> list = new ArrayList<>();
        for (JsonElement spec : specs) {
            list.add(suggestion(spec.getAsJsonObject()));
        }
        return list;
    }

    // Direct tests of the suggestion classes and the Java string semantics they depend on.
    private static JsonObject runDirect(JsonArray ops) {
        JsonArray results = new JsonArray();
        for (JsonElement element : ops) {
            JsonObject op = element.getAsJsonObject();
            JsonObject result = new JsonObject();
            try {
                switch (op.get("op").getAsString()) {
                    case "create": {
                        String command = op.get("command").getAsString();
                        result.add("ok", describe(Suggestions.create(command, suggestionList(op.getAsJsonArray("list"))), command));
                        break;
                    }
                    case "merge": {
                        String command = op.get("command").getAsString();
                        List<Suggestions> groups = new ArrayList<>();
                        for (JsonElement group : op.getAsJsonArray("groups")) {
                            groups.add(Suggestions.create(command, suggestionList(group.getAsJsonArray())));
                        }
                        result.add("ok", describe(Suggestions.merge(command, groups), command));
                        break;
                    }
                    case "builder": {
                        String input = op.get("input").getAsString();
                        SuggestionsBuilder builder = new SuggestionsBuilder(input, op.get("start").getAsInt());
                        for (JsonElement stepElement : op.getAsJsonArray("steps")) {
                            JsonObject step = stepElement.getAsJsonObject();
                            if (step.has("s")) {
                                builder.suggest(step.get("s").getAsString());
                            } else if (step.has("i")) {
                                builder.suggest(step.get("i").getAsInt());
                            } else if (step.has("off")) {
                                builder = builder.createOffset(step.get("off").getAsInt());
                            } else {
                                builder = builder.restart();
                            }
                        }
                        JsonObject state = describe(builder.build(), input);
                        state.addProperty("start", builder.getStart());
                        state.addProperty("remaining", builder.getRemaining());
                        state.addProperty("remainingLower", builder.getRemainingLowerCase());
                        result.add("ok", state);
                        break;
                    }
                    case "string": {
                        String a = op.get("a").getAsString();
                        String b = op.get("b").getAsString();
                        JsonObject o = new JsonObject();
                        o.addProperty("cmp", a.compareToIgnoreCase(b));
                        o.addProperty("cmpCase", a.compareTo(b));
                        o.addProperty("hash", a.hashCode());
                        o.addProperty("lower", a.toLowerCase(Locale.ROOT));
                        result.add("ok", o);
                        break;
                    }
                    case "charCase": {
                        // Character.toUpperCase(int) / toLowerCase(int) for every code point that changes
                        JsonArray table = new JsonArray();
                        for (int cp = 0; cp <= 0x10FFFF; cp++) {
                            if (cp >= 0xD800 && cp <= 0xDFFF) {
                                continue;
                            }
                            if (Character.toUpperCase(cp) != cp || Character.toLowerCase(cp) != cp) {
                                JsonArray row = new JsonArray();
                                row.add(cp);
                                row.add(Character.toUpperCase(cp));
                                row.add(Character.toLowerCase(cp));
                                table.add(row);
                            }
                        }
                        result.add("ok", table);
                        break;
                    }
                    default:
                        throw new IllegalArgumentException(op.get("op").getAsString());
                }
            } catch (IllegalStateException | IndexOutOfBoundsException e) {
                result.addProperty("throws", e.getClass().getSimpleName());
            }
            results.add(result);
        }
        JsonObject out = new JsonObject();
        out.add("results", results);
        return out;
    }

    private static JsonObject describe(CommandSyntaxException e) {
        JsonObject o = new JsonObject();
        o.addProperty("english", e.getRawMessage().getString());
        o.addProperty("message", e.getMessage());
        o.addProperty("context", e.getContext());
        o.addProperty("errCursor", e.getCursor());
        try {
            Object contents = Class.forName("wz").getMethod("b").invoke(e.getRawMessage());
            Method getKey = Class.forName("yk").getMethod("b");
            Method getArgs = Class.forName("yk").getMethod("d");
            o.addProperty("key", (String) getKey.invoke(contents));
            JsonArray args = new JsonArray();
            for (Object value : (Object[]) getArgs.invoke(contents)) {
                JsonObject a = new JsonObject();
                a.addProperty("t", value.getClass().getSimpleName());
                a.addProperty("v", String.valueOf(value));
                args.add(a);
            }
            o.add("args", args);
        } catch (ReflectiveOperationException ex) {
            throw new IllegalStateException(ex);
        }
        return o;
    }

    private static String asciiOnly(String json) {
        StringBuilder builder = new StringBuilder(json.length());
        for (int i = 0; i < json.length(); i++) {
            char ch = json.charAt(i);
            if (ch > 126) {
                builder.append(String.format("\\u%04x", (int) ch));
            } else {
                builder.append(ch);
            }
        }
        return builder.toString();
    }
}
