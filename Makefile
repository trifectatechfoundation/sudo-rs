PAM_SYS_PATH = sudo/lib/pam/sys.rs

BINDGEN_CMD = bindgen --allowlist-function '^pam_.*$$' --allowlist-var '^PAM_.*$$' --opaque-type pam_handle_t --ctypes-prefix libc

.PHONY: all clean pam-sys pam-sys-diff

all: pam-sys

pam-sys-diff: sudo/lib/pam/wrapper.h
	$(BINDGEN_CMD) $< | diff --color=auto $(PAM_SYS_PATH) -

pam-sys: $(PAM_SYS_PATH)

clean:
	rm $(PAM_SYS_PATH)

$(PAM_SYS_PATH): sudo/lib/pam/wrapper.h
	$(BINDGEN_CMD) $< --output $@
