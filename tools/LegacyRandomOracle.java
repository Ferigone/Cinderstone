package oracle;

import java.io.BufferedReader;
import java.io.BufferedWriter;
import java.io.InputStreamReader;
import java.io.OutputStreamWriter;
import java.lang.reflect.Constructor;
import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Method;
import java.nio.charset.StandardCharsets;

/** Invoke the actual pinned Minecraft 1.21 dyz/dyn classes, not java.util.Random. */
public final class LegacyRandomOracle {
    public static void main(String[] args) throws Exception {
        Class<?> type = Class.forName("dyz");
        Constructor<?> constructor = type.getConstructor(long.class);
        Method setSeed = type.getMethod("b", long.class);
        Method next = type.getMethod("c", int.class);
        Method nextInt = type.getMethod("f");
        Method nextBounded = type.getMethod("a", int.class);
        Method nextLong = type.getMethod("g");
        Method nextBoolean = type.getMethod("h");
        Method nextFloat = type.getMethod("i");
        Method nextDouble = type.getMethod("j");
        Object random = constructor.newInstance(0L);
        System.err.println("ORACLE_CLASS=" + type.getName() + ";SOURCE="
            + type.getProtectionDomain().getCodeSource().getLocation());
        BufferedReader input = new BufferedReader(new InputStreamReader(System.in, StandardCharsets.UTF_8));
        BufferedWriter output = new BufferedWriter(new OutputStreamWriter(System.out, StandardCharsets.UTF_8));
        for (String line; (line = input.readLine()) != null;) {
            String[] fields = line.split("\\t", -1);
            if (fields.length > 2) throw new IllegalArgumentException("Too many fields");
            String op = fields[0];
            String arg = fields.length == 2 ? fields[1] : null;
            String value;
            switch (op) {
                case "new": {
                    long seed = Long.parseLong(arg);
                    random = constructor.newInstance(seed);
                    value = Long.toString(seed);
                    break;
                }
                case "seed": {
                    long seed = Long.parseLong(arg);
                    setSeed.invoke(random, seed);
                    value = Long.toString(seed);
                    break;
                }
                case "bits": value = next.invoke(random, Integer.parseInt(arg)).toString(); break;
                case "bounded": {
                    try { value = nextBounded.invoke(random, Integer.parseInt(arg)).toString(); }
                    catch (InvocationTargetException error) {
                        if (!(error.getCause() instanceof IllegalArgumentException)) throw error;
                        value = "error:bound_must_be_positive";
                    }
                    break;
                }
                case "i32": value = nextInt.invoke(random).toString(); break;
                case "i64": value = nextLong.invoke(random).toString(); break;
                case "bool": value = ((Boolean) nextBoolean.invoke(random)) ? "1" : "0"; break;
                case "f32": value = Integer.toUnsignedString(Float.floatToRawIntBits((Float) nextFloat.invoke(random))); break;
                case "f64": value = Long.toUnsignedString(Double.doubleToRawLongBits((Double) nextDouble.invoke(random))); break;
                default: throw new IllegalArgumentException("Unknown command: " + op);
            }
            output.write((op.equals("bits") || op.equals("bounded")) ? op + "(" + arg + ")" : op);
            output.write('\t');
            output.write(value);
            output.newLine();
        }
        output.flush();
    }
}
