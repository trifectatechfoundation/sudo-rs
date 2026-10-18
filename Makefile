PAM_SRC_DIR = src/pam

BINDGEN_CMD = bindgen --allowlist-function '^pam_.*$$' --allowlist-var '^PAM_.*$$' --opaque-type pam_handle_t --blocklist-function pam_vsyslog --blocklist-function pam_vprompt --blocklist-function pam_vinfo --blocklist-function pam_verror --blocklist-type '.*va_list.*' --ctypes-prefix std::ffi --no-layout-tests --sort-semantically

PAM_VARIANT = $$(./util/get-pam-variant.bash)
MSGFMT ?= msgfmt
LOCALEDIR ?= /usr/share/locale

.PHONY: all clean install-mo pam-sys pam-sys-diff

pam-sys-diff:
	@$(BINDGEN_CMD) $(PAM_SRC_DIR)/wrapper.h | \
		sed 's/rust-bindgen [0-9]*\.[0-9]*\.[0-9]*/&, minified by cargo-minify/' | \
		diff --color=auto $(PAM_SRC_DIR)/sys_$(PAM_VARIANT).rs - \
		|| (echo run \'make -B pam-sys\' to apply these changes && false)
	@echo $(PAM_SRC_DIR)/sys_$(PAM_VARIANT).rs does not need to be re-generated

# use 'make pam-sys' to re-generate the sys.rs file for your local platform
pam-sys:
	$(BINDGEN_CMD) $(PAM_SRC_DIR)/wrapper.h --output $(PAM_SRC_DIR)/sys_$(PAM_VARIANT).rs
	cargo minify --apply --allow-dirty
	sed -i.bak 's/rust-bindgen [0-9]*\.[0-9]*\.[0-9]*/&, minified by cargo-minify/' $(PAM_SRC_DIR)/sys_$(PAM_VARIANT).rs
	rm $(PAM_SRC_DIR)/sys_$(PAM_VARIANT).rs.bak

install-mo:
	for file in po/*.po; do \
		lang="$${file##*/}"; \
		lang="$${lang%.po}"; \
		mkdir -p "$(LOCALEDIR)/$$lang/LC_MESSAGES"; \
		$(MSGFMT) --check -o "$(LOCALEDIR)/$$lang/LC_MESSAGES/sudo-rs.mo" "$$file"; \
	done

clean:
	rm $(PAM_SRC_DIR)/sys.rs
