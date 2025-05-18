# sudo-rs

A safety oriented and memory safe implementation of sudo and su written in Rust.

## Status of this project

Sudo-rs is being developed further; features you might expect from original sudo
may still be unimplemented or not planned. If there is an important one you need,
please request it using the issue tracker. If you encounter any usability bugs,
also please report them on the [issue tracker](https://github.com/trifectatechfoundation/sudo-rs/issues).
Suspected vulnerabilities can be reported on our [security page](https://github.com/trifectatechfoundation/sudo-rs/security).

An [audit of sudo-rs version 0.2.0](docs/audit/audit-report-sudo-rs.pdf) has been performed in August 2023.
The findings from that audit are addressed in the current version.

Sudo-rs currently is targeted for FreeBSD and Linux-based operating systems only.

## Installing sudo-rs

The recommended way to start using `sudo-rs` is via the package manager of your Linux distribution.

### Debian/Ubuntu
If you are running Debian 13 (trixie) or later, or Ubuntu 24.04 (Noble Numbat) or later, you can use:
```sh
apt-get install sudo-rs
```
This will offer the functionality using the commands `su-rs` and `sudo-rs`. If you want to invoke sudo-rs
via the usual commands `sudo` and `su` instead, prepend `/usr/lib/cargo/bin` to your current `$PATH` variable.

### Fedora

If you are running Fedora 38 or later, you can use:
```sh
dnf install sudo-rs
```
This will offer the functionality using the commands `su-rs` and `sudo-rs`.

### Arch Linux

Arch Linux can be installed from the distribution repositories:
```sh
pacman -S sudo-rs
```
This will offer the functionality using the commands `su-rs` and `sudo-rs`.

### Installing our pre-compiled x86-64 binaries

You can also switch to sudo-rs manually by using our pre-compiled tarballs.
We currently only offer these for x86-64 systems.

We recommend installing sudo-rs and su-s in your `/usr/local` hierarchy so it can co-exist with
your existing sudo installation. You can achieve this using the commands:
```sh
sudo tar -C /usr/local -xvf sudo-0.2.6.tar.gz
```
and for su-rs:
```sh
sudo tar -C /usr/local -xvf su-0.2.6.tar.gz
```
This will install sudo-rs and su-rs in `/usr/local/bin` using the usual commands `sudo` and `su`; it
will also install our version of `visudo` in that location.

Of course, if you **don't** have Todd Miller's `sudo` installed, you also have to make sure that:

* You manually create a `/etc/sudoers` or `/etc/sudoers-rs` file, this could be as simple as:

      Defaults secure_path="/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin"

      %sudo ALL=(ALL:ALL) ALL

  `sudo-rs` will try to process `/etc/sudoers-rs` exists if it exists, otherwise it will use `/etc/sudoers`.
  For an explanation of the sudoers syntax you can look at the
  [sudoers man page](https://www.sudo.ws/docs/man/sudoers.man/).

* (Strongly recommended) You create `/etc/pam.d/sudo` and `/etc/pam.d/sudo-i` files that contain:

      session required pam_limits.so

      @include common-auth
      @include common-account
      @include common-session-noninteractive

  If you don't do this, either a "fallback" PAM policy will be used or `sudo-rs` will simply refuse to run
  since it cannot initialize PAM. On FreeBSD, you may want to put these files in `/usr/local/etc/pam.d` instead.

### Building from source

Sudo-rs is written in Rust. The minimum required Rust version is 1.70. If your
Linux distribution does not package that version (or a later one), you can always
install the most recent version through [rustup]. You also need the C development
files for PAM (`libpam0g-dev` on Debian, `pam-devel` on Fedora).

On Ubuntu or Debian-based systems, use the following command to install the PAM development library:
```
sudo apt-get install libpam0g-dev
```

On Fedora, CentOS and other Red Hat-based systems, you can use the following command:
```
sudo yum install pam-devel
```

With dependencies installed, building sudo-rs is a simple matter of:
```
cargo build --release
```

This produces a binary `target/release/sudo`. However, this binary must have
the setuid flag set and must be owned by the root user in order to provide any
useful functionality. Consult your operating system manual for details.

Sudo-rs then also needs the configuration files; please follow the installation
suggestions in the previous section.

### Feature flags

#### --features pam-login
By default, sudo-rs will use the PAM service name `sudo`. On Debian and Fedora
systems, it is customary that the name `sudo-i` is used when the `-i / --login`
command line option is used. To get this behaviour, enable the `pam-login`
feature when building:
```
cargo build --release --features pam-login
```
This feature is enabled on our pre-supplied binaries.

#### --features apparmor
sudo-rs has support for selecting AppArmor profile on Linux distributions that
support AppArmor such as Debian and Ubuntu. To enable this feature, build sudo-rs
with apparmor support enabled:
```
cargo build --release --features apparmor
```

This feature is disabled on our pre-supplied binaries.

[rustup]: https://rustup.rs/

## Differences from original sudo

sudo-rs supports less functionality than sudo. Some of this is by design. In
most cases you will get a clear error if you try something that is not
supported (e.g. use a configuration flag or command line option that is not
implemented).

Exceptions to the above, with respect to your `/etc/sudoers` configuration:

* `use_pty` is enabled by default, but can be disabled.
* `env_reset` is ignored --- this is always enabled.
* `visiblepw` is ignored --- this is always disabled.
* `verifypw` is ignored --- this is always set to `all` (the default)
* the (NO)PASSWD tag on the "list" pseudocommand will determine whether a password
  is required for the `sudo -U --list` command, instead of `listpw`.
* `mail_badpass`, `always_set_home`, `always_query_group_plugin` and
  `match_group_by_gid` are not applicable to our implementation, but ignored for
  compatibility reasons.
* `timestamp_type` is always set at `tty`.

Some other notable restrictions to be aware of:

* Some functionality is not supported, such as preventing shell escapes using `INTERCEPT` and
  storing config in LDAP using `sudoers.ldap`, and `cvtsudoers`.
* Sudo-rs always uses PAM for authentication, so your system must be set up for PAM.
  Sudo-rs will use the `sudo` and `sudo-i` service configuration. This also means
  that resource limits, umasks, etc have to be configured via PAM and not through
  the sudoers file.
* sudo-rs will not include the sendmail support of original sudo.
* The sudoers file must be valid UTF-8.
* To prevent a common configuration mistake in the sudoers file, wildcards
  are not supported in *argument positions* for a command.
  E.g., `%sudoers ALL = /sbin/fsck*` will allow `sudo fsck` and `sudo fsck_exfat` as expected,
  but `%sudoers ALL = /bin/rm *.txt` will not allow an operator to run `sudo rm README.txt`,
  nor `sudo rm -rf /home .txt`, as with original sudo.

If you find a common use case for original sudo missing, please create a feature
request for it in our issue tracker.

## Aim of the project

Our current target is to build a drop-in replacement for all common use cases of
sudo. For the sudoers config syntax this means that we support the default
configuration files of common Linux distributions. Our implementation should support
all commonly used command line options from the original sudo implementation.

Some parts of the original sudo are explicitly not in scope. Sudo has a large
and rich history and some of the features available in the original sudo
implementation are largely unused or only available for legacy platforms. In
order to determine which features make it we both consider whether the feature
is relevant for modern systems, and whether it will receive at very least
decent usage. Finally, of course, a feature should not compromise the safety of
the whole program.

Our `su` implementation is made using the building blocks we created for our
sudo implementation.  It is a suitable replacement for the `su` distributed
by [util-linux].

[util-linux]: https://github.com/util-linux/util-linux

## Future work

While our initial target is a drop-in replacement for most basic use cases of
sudo, our work may evolve beyond that target. We are also looking into
alternative ways to configure sudo without the sudoers config file syntax and to
extract parts of our work in usable crates for other people.

## History

The initial development of sudo-rs was started and funded by the [Internet Security Research Group](https://www.abetterinternet.org/) as part of the [Prossimo project](https://www.memorysafety.org/)

## Acknowledgements

Sudo-rs is an independent implementation, but it incorporates documentation and Rust translations of code from [sudo](https://www.sudo.ws/), maintained by Todd C. Miller. We thank Todd and the other sudo contributors for their work.

An independent security audit of sudo-rs was made possible by the [NLNet Foundation](https://nlnet.nl/), who also [sponsored](https://nlnet.nl/project/sudo-rs/) work on increased compatibility with the original sudo and the FreeBSD port.

The sudo-rs project would not have existed without the support of its sponsors, a full overview is maintained at https://trifectatech.org/initiatives/privilege-boundary/
