package oracle;

import java.io.BufferedReader;
import java.io.InputStreamReader;
import java.lang.reflect.Method;

/** Executes the unchanged shipped VecDeltaCodec and Mth classes. */
public final class entity_quantization_oracle {
    public static void main(String[] args) throws Exception {
        Method coordinate = Class.forName("aig").getDeclaredMethod("a", double.class);
        coordinate.setAccessible(true);
        Method floor = Class.forName("ayo").getDeclaredMethod("d", float.class);
        floor.setAccessible(true);
        try (BufferedReader reader = new BufferedReader(new InputStreamReader(System.in))) {
            for (String line; (line = reader.readLine()) != null;) {
                String[] fields = line.split("\\t");
                if (fields[0].equals("coordinate")) {
                    double value = Double.longBitsToDouble(Long.parseUnsignedLong(fields[1], 16));
                    System.out.println(coordinate.invoke(null, value));
                } else {
                    float angle = Float.intBitsToFloat((int) Long.parseLong(fields[1], 16));
                    float scaled = angle * 256.0f / 360.0f;
                    int units = fields[0].equals("angle") ? (Integer) floor.invoke(null, scaled) : (int) scaled;
                    System.out.println((byte) units);
                }
            }
        }
    }
}
