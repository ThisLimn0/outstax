import com.android.apksig.ApkVerifier;
import java.io.File;
import java.security.MessageDigest;
import java.util.HexFormat;
public class VerifyApk {
    public static void main(String[] args) throws Exception {
        for (String arg : args) {
            ApkVerifier.Result result=new ApkVerifier.Builder(new File(arg)).build().verify();
            System.out.println("FILE " + arg);
            System.out.println("VERIFIED " + result.isVerified());
            System.out.println("SCHEMES v1="+result.isVerifiedUsingV1Scheme()+" v2="+result.isVerifiedUsingV2Scheme()+" v3="+result.isVerifiedUsingV3Scheme());
            for (var cert:result.getSignerCertificates()) {
                System.out.println("CERT_SHA256 "+HexFormat.of().formatHex(MessageDigest.getInstance("SHA-256").digest(cert.getEncoded())));
                System.out.println("SUBJECT "+cert.getSubjectX500Principal().getName());
            }
            for(var e:result.getErrors())System.out.println("ERROR "+e);
        }
    }
}
