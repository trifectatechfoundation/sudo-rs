# sudo-rs

A safety oriented and memory safe implementation of sudo and su written in Rust.

## ⚠️ WARNING

**Sudo-rs is currently under active development and is not suited for any
production environment. Using sudo-rs is only recommended for development and
testing purposes, but you should expect any system that has sudo-rs installed to
break easily and to not be secure.**

## Quick start

Sudo-rs currently only supports Linux-based operating systems, although other
unix-based operating systems may work, they are currently not actively
supported or maintained.

Sudo-rs is written in Rust. We currently only support the latest stable compiler
toolchain. To get a recent compiler toolchain we recommend using [rustup]. To
build sudo-rs, install the dependencies (listed below with their names in
Debian repositories):

+ clang (clang)
+ libclang development libraries (libclang-dev)
+ PAM library (libpam0g-dev)

With dependencies installed, building sudo-rs is a simple matter of:

```
cargo build --release
```

This produces a binary `target/release/sudo`. However, this binary must have
the setuid flag set and must be owned by the root user in order to provide any
useful functionality. If you are unsure about how to set this up, then the
current version of sudo is not intended for you.

Sudo-rs needs the sudoers configuration file. The sudoers configuration file
will be loaded from `/etc/sudoers-rs` if that file exists, otherwise the
original `/etc/sudoers` location will be used. You must make sure that a valid
sudoers configuration exists at that location. For an explanation of the
sudoers syntax you can look at the
[original sudo man page](https://www.sudo.ws/docs/man/sudoers.man/). While most
syntax should be supported as is, most functionality will currently not be
implemented. Sudo-rs currently may not always warn about this, so your sudoers
file may have a different meaning compared to the original sudo implementation.

Sudo-rs always uses PAM for authentication at this time, your system must be
set up for PAM. Sudo-rs will use the `sudo` service configuration.

[rustup]: https://rustup.rs/

## Current work

Our current target is to build a drop-in replacement for most basic use cases of
sudo. For the sudoers config syntax this means that we aim to at least support
the default configuration files of some common Linux distributions (we currently
aim to support both the Fedora and Debian default sudoers configs). Our
implementation should eventually at least support all commonly used CLI flags
from the original sudo implementation (e.g., flags like `-u`, `-g` and `-s`).

Some parts of the original sudo are explicitly not in scope. Sudo has a large
and rich history and some of the features available in the original sudo
implementation are largely unused or only available for legacy platforms. In
order to determine which features make it we both consider whether the feature
is relevant for modern systems, and whether it will receive at very least
decent usage. Finally, of course, a feature should not compromise the safety of
the whole program.

The `su` program is a much simpler program and will only include basic
functionality. However, we think that the building blocks that make up our sudo
implementation will be suited to be used for creating a simple su
implementation.

## Contributions

While we are still working on getting the basic infrastructure and architecture
of sudo-rs up and running, accepting arbitrary contributions will be difficult.
If you have any small changes or suggestions please do leave those, but if you
want to work on larger parts of sudo-rs please ask first, or we may risk doing
work twice or not being able to include your work.

## Future work

While our initial target is a drop-in replacement for most basic use cases of
sudo, our work may evolve beyond that target. We are also looking into
alternative ways to configure sudo without the sudoers config file syntax and to
extract parts of our work in usable crates for other people.
