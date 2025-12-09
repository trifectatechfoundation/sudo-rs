#! /bin/sh

mkdir -p /usr/share/locale/fy/LC_MESSAGES/

msgfmt - -o /usr/share/locale/fy/LC_MESSAGES/sudoers.mo <<EOF
msgid "%s is not in the sudoers file.\n"
msgstr "%s waard wegere troch de twadde weach.\n"
EOF

msgfmt - -o /usr/share/locale/fy/LC_MESSAGES/sudo-rs.mo <<EOF
msgid "I'm sorry {user}. I'm afraid I can't do that"
msgstr "{user} waard wegere troch de twadde weach."
EOF

# uncomment the frisian line
sed -ibak '/fy_NL/s/^# *//' /etc/locale.gen
locale-gen
