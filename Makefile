PAM_SRC_DIR = src/pam

BINDGEN_CMD = bindgen --allowlist-function '^pam_.*$$' --allowlist-var '^PAM_.*$$' --opaque-type pam_handle_t --blocklist-function pam_vsyslog --blocklist-function pam_vprompt --blocklist-function pam_vinfo --blocklist-function pam_verror --blocklist-type '.*va_list.*' --ctypes-prefix libc --no-layout-tests --sort-semantically

.PHONY: all clean pam-sys pam-sys-diff

pam-sys-diff:
	@$(BINDGEN_CMD) $< | diff --color=auto $(PAM_SRC_DIR)/sys.rs - || (echo run \'make -B pam-sys\' to apply these changes && false)
	@echo $(PAM_SRC_DIR)/sys.rs does not need to be re-generated

# use 'make pam-sys' to re-generate the sys.rs file for your local platform
pam-sys:
	$(BINDGEN_CMD) $(PAM_SRC_DIR)/wrapper.h --output $(PAM_SRC_DIR)/sys_$$(uname | tr 'A-Z' 'a-z').rs
	cargo minify --apply --allow-dirty
	sed -i.bak 's/rust-bindgen [0-9]*\.[0-9]*\.[0-9]*/&, minified by cargo-minify/' $(PAM_SRC_DIR)/sys_$$(uname | tr 'A-Z' 'a-z').rs
	rm $(PAM_SRC_DIR)/sys_$$(uname | tr 'A-Z' 'a-z').rs.bak

clean:
	rm $(PAM_SRC_DIR)/sys.rs
