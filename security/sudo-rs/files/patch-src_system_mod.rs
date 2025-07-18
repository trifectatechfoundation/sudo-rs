--- src/system/mod.rs.orig	2025-07-01 09:04:15 UTC
+++ src/system/mod.rs
@@ -802,8 +802,8 @@ impl Process {
 
         let ki_start = ki_proc[0].ki_start;
         Ok(ProcessCreateTime::new(
-            ki_start.tv_sec,
-            ki_start.tv_usec * 1000,
+            (ki_start.tv_sec).into(),
+            (ki_start.tv_usec * 1000).into(),
         ))
     }
 }
