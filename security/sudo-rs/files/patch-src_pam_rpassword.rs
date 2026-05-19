Based upon https://github.com/trifectatechfoundation/sudo-rs/commit/d43ff79df262568d8977771f03977d5965bf8474

---

--- src/pam/rpassword.rs.orig	2026-03-11 14:23:39 UTC
+++ src/pam/rpassword.rs
@@ -263,6 +263,7 @@ fn read_unbuffered(
             if read_byte == b'\t' && feedback.visible_len.take().is_some() {
                 feedback.clear();
                 let _ = feedback.sink.write(b"(no echo)");
+                continue;
             }
         }
 
