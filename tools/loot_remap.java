import java.io.*;
import java.nio.file.*;
import java.util.*;
import java.util.jar.*;
import java.util.zip.*;
import jdk.internal.org.objectweb.asm.*;
import jdk.internal.org.objectweb.asm.commons.*;

/** Local-only helper: applies the official Mojang proguard mappings to the pinned server jar.
 *  Output is for reading/running in a scratch directory only; it is never committed. */
public class loot_remap {
  static Map<String,String> cls = new HashMap<>();           // obf internal -> deobf internal
  static Map<String,String> rcls = new HashMap<>();          // deobf internal -> obf internal
  static Map<String,String> fields = new HashMap<>();        // obfOwner.obfName -> deobfName
  static Map<String,String> methods = new HashMap<>();       // obfOwner.obfName+obfDesc -> deobfName
  static Map<String,List<String>> supers = new HashMap<>();  // obf internal -> obf supers

  static String toDesc(String type) {
    int dims = 0;
    while (type.endsWith("[]")) { dims++; type = type.substring(0, type.length() - 2); }
    String base;
    switch (type) {
      case "int": base = "I"; break; case "long": base = "J"; break; case "boolean": base = "Z"; break;
      case "byte": base = "B"; break; case "short": base = "S"; break; case "char": base = "C"; break;
      case "float": base = "F"; break; case "double": base = "D"; break; case "void": base = "V"; break;
      default: {
        String internal = type.replace('.', '/');
        String obf = rcls.get(internal);
        base = "L" + (obf != null ? obf : internal) + ";";
      }
    }
    return "[".repeat(dims) + base;
  }

  public static void main(String[] args) throws Exception {
    Path mapping = Path.of(args[0]); Path in = Path.of(args[1]); Path out = Path.of(args[2]);
    List<String> lines = Files.readAllLines(mapping);
    // pass 1: classes
    for (String l : lines) {
      if (l.startsWith("#") || l.startsWith(" ") || l.isEmpty()) continue;
      String[] p = l.substring(0, l.length() - 1).split(" -> ");
      String d = p[0].replace('.', '/'), o = p[1].replace('.', '/');
      cls.put(o, d); rcls.put(d, o);
    }
    // pass 2: members
    String curObf = null;
    for (String l : lines) {
      if (l.startsWith("#") || l.isEmpty()) continue;
      if (!l.startsWith(" ")) { curObf = l.substring(0, l.length() - 1).split(" -> ")[1].replace('.', '/'); continue; }
      String s = l.trim();
      String[] p = s.split(" -> ");
      String obfName = p[1];
      String left = p[0];
      int paren = left.indexOf('(');
      if (paren < 0) {
        String[] tn = left.split(" ");
        fields.put(curObf + "." + obfName, tn[1]);
      } else {
        // strip line numbers "a:b:"
        String sig = left;
        while (sig.matches("^\\d+:.*")) sig = sig.substring(sig.indexOf(':') + 1);
        String ret = sig.substring(0, sig.indexOf(' '));
        String name = sig.substring(sig.indexOf(' ') + 1, sig.indexOf('('));
        String params = sig.substring(sig.indexOf('(') + 1, sig.lastIndexOf(')'));
        StringBuilder d = new StringBuilder("(");
        if (!params.isEmpty()) for (String t : params.split(",")) d.append(toDesc(t.trim()));
        d.append(")").append(toDesc(ret));
        methods.put(curObf + "." + obfName + d, name);
      }
    }
    // read hierarchy
    try (JarFile jf = new JarFile(in.toFile())) {
      for (Enumeration<JarEntry> e = jf.entries(); e.hasMoreElements();) {
        JarEntry je = e.nextElement();
        if (!je.getName().endsWith(".class")) continue;
        ClassReader cr = new ClassReader(jf.getInputStream(je));
        List<String> s = new ArrayList<>();
        if (cr.getSuperName() != null) s.add(cr.getSuperName());
        s.addAll(Arrays.asList(cr.getInterfaces()));
        supers.put(cr.getClassName(), s);
      }
    }
    Remapper r = new Remapper() {
      @Override public String map(String internalName) { String m = cls.get(internalName); return m != null ? m : internalName; }
      String findField(String owner, String name) {
        String k = owner + "." + name; String v = fields.get(k); if (v != null) return v;
        for (String s : supers.getOrDefault(owner, List.of())) { v = findField(s, name); if (v != null) return v; }
        return null;
      }
      String findMethod(String owner, String name, String desc) {
        String k = owner + "." + name + desc; String v = methods.get(k); if (v != null) return v;
        for (String s : supers.getOrDefault(owner, List.of())) { v = findMethod(s, name, desc); if (v != null) return v; }
        return null;
      }
      @Override public String mapFieldName(String owner, String name, String desc) { String v = findField(owner, name); return v != null ? v : name; }
      @Override public String mapMethodName(String owner, String name, String desc) { String v = findMethod(owner, name, desc); return v != null ? v : name; }
    };
    try (JarFile jf = new JarFile(in.toFile()); JarOutputStream jo = new JarOutputStream(Files.newOutputStream(out))) {
      for (Enumeration<JarEntry> e = jf.entries(); e.hasMoreElements();) {
        JarEntry je = e.nextElement();
        if (je.isDirectory()) continue;
        byte[] data = jf.getInputStream(je).readAllBytes();
        String name = je.getName();
        if (name.startsWith("META-INF/") && (name.endsWith(".SF") || name.endsWith(".RSA") || name.endsWith(".DSA") || name.endsWith(".EC"))) continue;
        if (name.equals("META-INF/MANIFEST.MF")) data = "Manifest-Version: 1.0\n".getBytes();
        if (name.endsWith(".class")) {
          ClassReader cr = new ClassReader(data);
          ClassWriter cw = new ClassWriter(0);
          cr.accept(new ClassRemapper(cw, r), 0);
          data = cw.toByteArray();
          String internal = cr.getClassName();
          name = r.map(internal) + ".class";
        }
        jo.putNextEntry(new JarEntry(name)); jo.write(data); jo.closeEntry();
      }
    }
  }
}
