VERSION_MINOR=$(xgettext --version | grep -m1 -o '[0-9]\+$')
VERSION_MAJOR=$(xgettext --version | grep -m1 -o ' [0-9]\+')
WORK_ROOT=$(cargo metadata --format-version 1 | jq -r .workspace_root)

SUDO_RS_VERSION="$(cargo metadata --format-version 1 | jq -r '.packages[] | select(.name=="sudo-rs") | .version')"

if [ "$VERSION_MAJOR" -eq 0 ] && [ "$VERSION_MINOR" -lt 24 ]; then
        echo "xgettext is too old and does not support Rust -- upgrade to 0.24 or higher"
        exit 1
fi

potfile="$WORK_ROOT/po/sudo-rs.pot"
test -e "$potfile" && EXISTING="--join-existing --omit-header"

cd "$WORK_ROOT" && find src -name "*.rs" -not -path "*/gettext/*" | xargs xgettext \
  --package-name="sudo-rs" \
  --package-version="$SUDO_RS_VERSION" \
  --msgid-bugs-address="https://github.com/trifectatechfoundation/sudo-rs/issues" \
  --language=Rust \
  --from-code="UTF-8" \
  --keyword='xlat!' \
  --keyword='xlat_write!:2' \
  --keyword='xlat_println!:1' \
  --keyword='user_error!:1' \
  --keyword='user_info!:1' \
  --keyword='user_warn!:1' \
  --add-comments='TRANSLATORS:' \
  ${EXISTING:-} \
  -o "$potfile"
