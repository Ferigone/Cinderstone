package oracle;

import java.io.*;
import java.lang.reflect.*;
import java.nio.file.*;
import net.jpountz.lz4.LZ4BlockInputStream;
import net.jpountz.lz4.LZ4BlockOutputStream;

/** Calls the shipped Java code/lz4-java dependency; does not reimplement it. */
public final class anvil_Oracle {
    public static void main(String[] args) throws Exception {
        switch (args[0]) {
            case "decode-lz4":
                try (InputStream input = new LZ4BlockInputStream(Files.newInputStream(Path.of(args[1])));
                     OutputStream output = Files.newOutputStream(Path.of(args[2]))) { input.transferTo(output); }
                return;
            case "encode-lz4":
                try (InputStream input = Files.newInputStream(Path.of(args[1]));
                     OutputStream output = new LZ4BlockOutputStream(Files.newOutputStream(Path.of(args[2])))) { input.transferTo(output); }
                return;
            case "read-region":
                Class<?> regionClass = Class.forName("dwp");
                Class<?> infoClass = Class.forName("dws");
                Class<?> chunkClass = Class.forName("dcd");
                Path path = Path.of(args[1]).toAbsolutePath();
                Object region = regionClass.getConstructor(infoClass, Path.class, Path.class, boolean.class)
                    .newInstance(null, path, path.getParent(), false);
                try {
                    Object position = chunkClass.getConstructor(int.class,int.class).newInstance(Integer.parseInt(args[2]),Integer.parseInt(args[3]));
                    try (DataInputStream input = (DataInputStream)regionClass.getMethod("a",chunkClass).invoke(region,position)) {
                        if (input == null) throw new IOException("Java did not find the Rust-written chunk");
                        writeNbt(input, Path.of(args[4]));
                    }
                } finally { regionClass.getMethod("close").invoke(region); }
                return;
            case "read-nbt":
                try (DataInputStream input = new DataInputStream(Files.newInputStream(Path.of(args[1])))) { writeNbt(input,Path.of(args[2])); }
                return;
            default: throw new IllegalArgumentException("unknown oracle operation");
        }
    }
    private static void writeNbt(DataInput input, Path outputPath) throws Exception {
        Class<?> io = Class.forName("uo");
        Method read = io.getMethod("a",DataInput.class);
        Object compound = read.invoke(null,input);
        Method write = io.getMethod("a",read.getReturnType(),DataOutput.class);
        try (DataOutputStream output = new DataOutputStream(Files.newOutputStream(outputPath))) { write.invoke(null,compound,output); }
    }
}
