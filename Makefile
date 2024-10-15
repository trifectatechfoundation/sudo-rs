PAM_SRC_DIR = src/pam

BINDGEN_CMD = bindgen --allowlist-function '^pam_.*$$' --allowlist-var '^PAM_.*$$' --opaque-type pam_handle_t --blocklist-function pam_vsyslog --blocklist-function pam_vprompt --blocklist-type '.*va_list.*' --ctypes-prefix libc --no-layout-tests --sort-semantically

.PHONY: all clean pam-sys pam-sys-diff

pam-sys-diff: $(PAM_SRC_DIR)/wrapper.h
	@$(BINDGEN_CMD) $< | diff --color=auto $(PAM_SRC_DIR)/sys.rs - || (echo run \'make -B pam-sys\' to apply these changes && false)
	@echo $(PAM_SRC_DIR)/sys.rs does not need to be re-generated

# use 'make pam-sys' to re-generate the sys.rs file
pam-sys: $(PAM_SRC_DIR)/sys.rs

$(PAM_SRC_DIR)/sys.rs: $(PAM_SRC_DIR)/wrapper.h
	$(BINDGEN_CMD) $< --output $@
	cargo minify --apply --allow-dirty
	sed -i 's/rust-bindgen \w*\.\w*\.\w*/\0, minified by cargo-minify/' $@

clean:
	rm $(PAM_SRC_DIR)/sys.rs
