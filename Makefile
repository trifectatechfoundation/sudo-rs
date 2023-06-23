PAM_SRC_DIR = src/pam

BINDGEN_CMD = bindgen --allowlist-function '^pam_.*$$' --allowlist-var '^PAM_.*$$' --opaque-type pam_handle_t --ctypes-prefix libc

.PHONY: all clean pam-sys pam-sys-diff

pam-sys-diff: $(PAM_SRC_DIR)/wrapper.h
	@$(BINDGEN_CMD) $< | diff --color=auto $(PAM_SRC_DIR)/sys.rs - || (echo run \'make -B pam-sys\' to apply these changes && false)
	@echo $(PAM_SRC_DIR)/sys.rs does not need to be re-generated

# use 'make pam-sys' to re-generate the sys.rs file
pam-sys: $(PAM_SRC_DIR)/sys.rs

$(PAM_SRC_DIR)/sys.rs: $(PAM_SRC_DIR)/wrapper.h
	$(BINDGEN_CMD) $< --output $@

clean:
	rm $(PAM_SRC_DIR)/sys.rs
