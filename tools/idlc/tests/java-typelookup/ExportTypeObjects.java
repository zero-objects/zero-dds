import java.nio.file.Files;
import java.nio.file.Path;
import org.zerodds.cdr.HexBytes;

/** Exports existing canonical metadata, without introducing a fixture codec. */
public class ExportTypeObjects {
    public static void main(String[] args) throws Exception {
        Path directory = Path.of(args[0]);
        Files.createDirectories(directory);
        int count = 0;
        for (var entry : ZdTypeSupports.Support_service_Root.INSTANCE.typeObjects().entries()) {
            Files.write(directory.resolve(HexBytes.encode(entry.identifier()) + ".bin"), entry.object());
            count++;
        }
        if (count < 66) throw new IllegalStateException("Incomplete Root closure: " + count);
        System.out.println("EXPORTED_STANDARD_TYPEOBJECTS=" + count);
    }
}
