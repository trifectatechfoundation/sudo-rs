PAM_SRC_DIR = src/pam

BINDGEN_CMD = bindgen --merge-extern-blocks --allowlist-function '^pam_.*$$' --allowlist-var '^PAM_.*$$' --opaque-type pam_handle_t --ctypes-prefix libc

.PHONY: all clean pam-sys pam-sys-diff

pam-sys-diff: $(PAM_SRC_DIR)/wrapper.h
	@$(BINDGEN_CMD) $< -- --target=x86_64-pc-linux-gnu | diff --color=auto $(PAM_SRC_DIR)/sys/x86_64_linux.rs - || (echo run \'make -B pam-sys\' to apply these changes && false)
	@echo $(PAM_SRC_DIR)/sys.rs does not need to be re-generated

# use 'make pam-sys' to re-generate the sys.rs file
pam-sys: $(PAM_SRC_DIR)/sys/x86_64_linux.rs $(PAM_SRC_DIR)/sys/i386_linux.rs

$(PAM_SRC_DIR)/sys/x86_64_linux.rs: $(PAM_SRC_DIR)/wrapper.h
	$(BINDGEN_CMD) $< --output $@ -- --target=x86_64-pc-linux-gnu

$(PAM_SRC_DIR)/sys/i386_linux.rs: $(PAM_SRC_DIR)/wrapper.h
	$(BINDGEN_CMD) $< --output $@ -- --target=i386-pc-linux-gnu

clean:
	rm $(PAM_SRC_DIR)/sys/x86_64_linux.rs
