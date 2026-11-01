package oracle;
import java.io.BufferedReader;
import java.io.InputStreamReader;
import java.lang.reflect.Method;

/** Calls the unchanged 1.21 BiomeManager.obfuscateSeed, not a Java rewrite. */
public class biome_seed_oracle {
    public static void main(String[] arguments) throws Exception {
        Method method=Class.forName("ddy").getMethod("a",long.class);
        try(BufferedReader reader=new BufferedReader(new InputStreamReader(System.in))) {
            for(String line;(line=reader.readLine())!=null;) {
                long seed=Long.parseLong(line);
                long result=(Long)method.invoke(null,seed);
                System.out.println(seed+","+result);
            }
        }
    }
}
