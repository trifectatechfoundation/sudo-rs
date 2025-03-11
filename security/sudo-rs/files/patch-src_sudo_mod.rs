--- src/sudo/mod.rs.orig	2025-04-04 14:57:27 UTC
+++ src/sudo/mod.rs
@@ -51,8 +51,7 @@ pub(crate) fn candidate_sudoers_file() -> &'static Pat
     let file = if pb_rs.exists() {
         pb_rs
     } else if cfg!(target_os = "freebsd") {
-        // FIXME maybe make this configurable by the packager?
-        Path::new("/usr/local/etc/sudoers")
+        Path::new(env!("LOCALBASE"))
     } else {
         Path::new("/etc/sudoers")
     };
