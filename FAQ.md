# Frequently Asked Questions

## Who is behind sudo-rs?

Sudo-rs was originally started as a project by [ISRG](https://www.memorysafety.org), run by [Tweede golf](https://www.tweedegolf.com) and [Ferrous Systems](https://www.ferrous-systems.de). At this point in time it is owned and maintained by the non-profit [Trifecta Tech Foundation](https://trifectatech.org).

The sudo-rs team has seen a few changes over time, but the current composition is:

- Marc Schoolderman (core team)
- Björn Baron (core team)
- Ruben Nijveld
- Christian Poveda
- Jorge Aparicio

Marc Schoolderman has an academic background in formal verification of software correctness; Ruben Nijveld is a maintainer of the Rust implementation of the [NTP, NTS and PTP protocols](https://github.com/pendulum-project) and active in the related IETF working group; Christian Poveda is a contributor to important tools such as [bindgen](https://github.com/rust-lang/rust-bindgen), [Miri](https://github.com/rust-lang/miri) and others; Jorge Aparicio has contributed to widely used Rust crates such as [heapless](https://crates.io/crates/heapless) and [cast](https://crates.io/crates/cast) and numerous others; Björn Baron is an [active contributor](https://www.rust-lang.org/governance/teams/compiler) to the Rust compiler.

## I don't like the command name 'sudo-rs'?

We don't either!

"sudo-rs" is the name of the project, which aims to provide an implementation of the `sudo` command written in Rust. End-users shouldn't *need* to be concerned about what programming language their tools are written in (although there is nothing wrong with taking an interest, of course!). Ideally, your Linux distribution will allow you to easily switch between `sudo` implementations, just as easily as you can switch between `vi` or `awk` implementations.

But because for such a long time, `sudo` has been considered a core utility, Linux distributions are still catching up---the amount of work that they have to do to accomodate having multiple sudo options shouldn't be underestimated. And so for the initial versions, many have chosen to rename the command "sudo-rs" to avoid a packaging problem.

If you are annoyed by this, do let your voice be heard to the maintainers of the sudo-rs package for your distribution! And be patient---maintaining packages for Linux distributions is largely a volunteer job, and since sudo-rs is a new tool it's only logical that distributions take a conservative approach.

## What will I notice if I start using sudo-rs?

In most cases, not that much. Your password prompt will look differently.

In other cases, if you have crafted a very specialized configuration, you may notice that some features are missing. This can be by design (e.g. we do not want sudo-rs to send email, or initiate any form of network connection), or simply an oversight. This should not happen with default configurations, so if this happens to you, you should have good chance of diagnosing the change necessary.

If you think we are missing a feature that is not on our roadmap, or that we should prioritise higher, *do* file a feature request on our GitHub page!

## What is the advantage of rewriting sudo in Rust?

The reasons that were mentioned in the [blog post](https://tweedegolf.nl/en/blog/91/reimplementing-sudo-in-rust) announcing sudo-rs still hold true:

1. Obviously, better memory safety. In C a programmer needs to pay attention at every turn to check that memory is being used correctly. The Rust programming language helps the programmer avoid mistakes by tracking data allocation "at compile time". On top of that, it performs runtime checks to prevent the worst possible outcome in case mistakes do happen.

2. Rust can be used as a systems language, like C, but it also facilitates programming at a much higher level of abstraction. For example, parts of the business logic of sudo-rs are implemented using `enum` types, and evaluated by chaining Rust "iterators" together. And of course our entire code base leans into the ease-of-use offered by `Option` and `Result` types. To achieve the same thing in C, a programmer would need to explicitly implement the logic underpinning those concept themselves. (Which is what you will find that original sudo has done---and that added complexity is where bugs can thrive).

3. A rewrite is also a good time for a rethink. As in every realistic piece of software, there are many many code paths in original sudo [that are seldom exercised](https://www.stratascale.com/vulnerability-alert-CVE-2025-32463-sudo-chroot) in normal usage. Bugs can lurk there as well, undiscovered for years until someone takes a look. But, if some code paths are seldomly executed, why include them at all? This of course is the lesson that OpenBSD's `doas` teaches us.

## Why are you replacing a battle-tested utility?

Even though some people like to say that original sudo is "battle tested", that is only true for the most common usage scenarios. You can also say that COBOL is battle tested technology. And since sudo-rs is in fact a derived work of 'original' sudo, we also benefit from the "battle tested"-ness of the original. For example, we have [studied the past CVE's](https://github.com/trifectatechfoundation/sudo-rs/blob/main/docs/sudo-cve.md) so we don't fall prey to them.

What is correct to say is that the maintainer of sudo, Todd Miller, has been battle tested. He has had the job of maintaining sudo for many years now, either in his spare time or in time graciously donated by his employer. Many millions of people (including tech giants) benefit from this.

## If I do `grep unsafe` why do I find hundreds of occurrences?

Because they are necessary.

The `unsafe` keyword is part of Rust's memory safety design. The most important thing it allows is dereferencing "raw pointers", and calling other functions marked as "unsafe", such as those found in the C library. Because sudo-rs is a system utility, it needs to interface with the operating system and system libraries, which are written in C. Most of the `unsafe` code in sudo-rs lives at those seams. A prime example of this is the `setuid()` function itself---without which it would be really hard to write sudo.

Also note that about half of our `unsafe` blocks happens in unit test code---to test our "unsafe parts". For the other half, every usage of `unsafe` is accompanied by a `SAFETY` specification, every one of which has been vetted by at least two sudo-rs team members.

Finally, wherever it was possible, we use [Miri](https://github.com/rust-lang/miri) to test our `unsafe` blocks to be sure we didn't create any so-called "undefined behaviour".

We have seen some attempts at 'myth busting' Rust code by counting the number of times `unsafe` occurs. But that is mistaking the forest for the trees. Of course we understand the criticism: sudo-rs is a new program and needs to prove itself. But we are not spreading myths about sudo-rs having "memory safety-by-design" at its core.

At the very least, a few hundred lines of well-documented `unsafe` code is still less than hundreds of thousands of them.

## Why did you get rid of the GNU license?

We didn't.

sudo is not a GNU tool but a cross-platform software project maintained by Todd Miller. It existed long before the GNU project did. It is licensed under the OpenBSD license, which is functionally equivalent to the MIT license that one can choose for sudo-rs.

The reason Trifecta Tech Foundation keeps sudo-rs under the MIT+Apache 2.0 dual license is simply this: it is the most common in the Rust ecosystem, and it is exactly as permissive as original sudo towards end-users. In fact, requiring that external contributors also agree with distribution under Apache 2.0 actually makes sudo-rs a tiny bit more tightly licensed.

We understand the objections that some people may have when a piece of software that falls under the GPL gets re-implemented under a more permissive license. It also wouldn't make good engineering sense for sudo-rs to use a more permissive license than Todd Miller uses, because it would mean we wouldn't be able to consult his source code. 

Trifecta Tech projects that are "re-implementations" typically respect the original license: zlib-rs uses the Zlib license, bzip2-rs uses the bzip2 license, etc.

## What operating systems does sudo-rs support?

sudo-rs is fully supported for reasonably modern Linux systems, as well as on FreeBSD.

There are some small differences. For example, on FreeBSD, `NOEXEC:` is not supported (since it can't really be implemented with the same level of guarantees as on Linux). Our compliance testing framework comparing `sudo-rs` to `sudo` is also executed on both platforms.

In the future, we would also like to support MacOS and be able to say "of course it runs on NetBSD!", but right now we have prioritized other tasks. Patches are welcome!

## Why doesn't sudo-rs insult me when I mistype my password?

One of the sudo-rs developers has suffered at the hands of a [BOFH](https://en.wikipedia.org/wiki/Bastard_Operator_From_Hell) who thought it was funny to force the `sl` command (see https://github.com/mtoyoda/sl) on users. He has sadly lost his sense of humour as a result.

You *can* however get insulted by sudo-rs (and every other program that uses PAM!) by using https://github.com/cgoesche/pam-insults. The results will look like this:

```sh
$ sudo -s
[sudo: authenticate] Password: **************
[sudo] Did you forget your password or just your brain?
[sudo: authenticate] Password: **************
[sudo] Congratulations! You've just won the 'Most Consistent Incorrect Password Entry' award.
```
and so on.

https://github.com/cgoesche/pam-insults is under development and appears to aim at multi-lingual support, so why not help the author out?

## Comparisons with other tools

General remark: we try to honestly represent the advantages and disadvantages in this section, but of course we are hardly unbiased. At the same time, we are not trying to sell you anything, and respect any resources individual developers or companies invest in bringing more open source options to users.

### What about doas?

On OpenBSD, doas is great and sudo-rs has taken inspiration from it. But it was written specifically for OpenBSD.

On Linux, it is available as the OpenDoas port, which requires quite a bit of glue code (some of which is actually taken directly from Todd Miller's sudo!), and still uses over 5000 lines of C. It also doesn't come with an automated test suite. In the words of the maintainer of OpenDoas:

> There are fewer eyes on random `doas` ports, just because `sudo` had a vulnerability
> does not mean random doas ports are more secure if they are not reviewed.

OpenDoas also has one unresolved CVE related to TTY hijacking for 2 years (https://nvd.nist.gov/vuln/search/results?query=opendoas) for which a remedy isn't easy (https://github.com/Duncaen/OpenDoas/issues/106). This is an attack scenario that sudo-rs, like sudo, util-linux's su and systemd's run0 have remedies for (and have had to spent substantial effort in "getting things right"). It's also clear that *at the time of writing* OpenDoas is not that actively maintained (https://github.com/Duncaen/OpenDoas/pull/124).

That being said, we admire the minimalist approach exemplified by doas, and this is expressed by what we internally call our "Berlin Criteria" in our contributing guidelines.

If we zoom in on a line-for-line comparison, how does sudo-rs compare to OpenDoas' ~5000 lines of C code? sudo-rs is around 40.000 lines of Rust code. Of those, 25.000 lines are test code, which leaves around 15.000 lines of production code. Of those, less than 350 are "unsafe". If we compare both to original sudo, we find that it contains over 180.000 lines of C. So on this spectrum, it is much closer to doas than to original sudo.

On a more practical side, as with run0, switching to doas would require users to rewrite their sudoers configurations to doas configurations. That might be possible in many cases, but not all.

This is not say that *you* should not use OpenDoas. TTY hijacking attack might not be relevant for you (for example, because you disabled the feature that allows it in the Linux kernel), and you may need the tiny footprint or prefer the simpler configuration file. But OpenDoas (at least in its current form) isn't a solution for everybody.

### What about run0?

Run0 is a tool added to systemd in version 256 which serves a similar purpose to sudo/sudo-rs. Its main aim is to offer controlled privilege escalation without requiring the SUID flag on binaries, by merely functioning as a convenient interface to functionality that was already present in systemd. It is controlled by a security policy implemented through polkit.

Having had the experience of writing a SUID program, we can definitely say that we see advantages to that approach. Since systemd is a security-critical component anway, there is something to be said to let it handle privilege escalation as well.

However, there are still some trade-offs. 

- systemd is written in C. Which means it can't take advantage of Rust's memory safety features or higher-level abstractions for capturing the "business logic". And because `run0` itself is an untrusted program that is under full control of an attacker, its overall architecture is less simple than SUID programs such as `sudo` or `doas`.

- systemd circumvents that by letting polkit handle policy decisions. But polkit essentially uses configuration files that are small JavaScript programs. We think those are more complex, harder and/or more error-prone to write for a sysadmin than /etc/sudoers or `doas` configurations. And of course, a JavaScript interpreter itself is not a simple piece of software.

- sudo-rs can more easily be ported to other platforms -- see our port for FreeBSD.

## Are there actual memory safety vulnerabilities in the original sudo?

Serious vulnerabilities in sudo are listed by the developer of C-based sudo, Todd Miller, on https://www.sudo.ws/security/advisories/. The first page lists several memory safety vulnerabilities (anything that says “buffer overflow”, “heap overflow" or “double free”). One of the oldest ones we know of is from 2001, published in Phrack https://phrack.org/issues/57/8 under the whimsical name “Vudo”, which quite dramatically showed an attacker gaining full access on a system that it only had limited access to.

A good recent example is the “Baron Samedit” bug that was discovered by security firm Qualys in 2021, which like “Vudo" would cause an uncontrolled privilege escalation. There are many websites and YouTube videos that illustrate it. It is formally identified as CVE-2021-3156 and is described at https://www.sudo.ws/security/advisories/unescape_overflow/

Now, the fine point here of course is: "Baron Samedit” was discovered by security researchers who were working together with the developer of C-based sudo. If you want to know if any of these sudo vulnerabilities have been used to cause harm to systems, we need only look at CISA, that does include it (https://www.cisa.gov/news-events/cybersecurity-advisories/aa22-117a) in its list of “commonly exploited” vulnerabilities of 2021.

Also, consider this: the bug behind “Baron Samedit” was present in sudo between 2011 and 2021. That’s a long time. So it’s quite possible that someone already knew it existed before 2021, but simply didn’t tell anybody else.

Beyond sudo, a [memory safety vulnerability](https://nvd.nist.gov/vuln/detail/cve-2021-4034) was also discovered in `pkexec`, another sudo-like progam.

Note that in real-world attacks, sudo vulnerabilities would usually be combined with exploits in other software. For example, it may be possible to gain limited access on a machine by using an exploit in a webserver. If that machine then has a seriously vulnerable outdated sudo on it that allows an attacker to turn that limited access into full access, what may look like a minor bug in a webserver can turn into a nightmare. I.e. memory safety bugs in sudo have the potential to dramatically amplify the impact of bugs in other pieces of software.

## Are there fewer bugs in sudo-rs?

This is an unanswerable question. The real question of course is: what is the *probability* of discovering a bug in sudo-rs, compared to that in original sudo?

On the one hand: we are a newer project, so we are likely to have messed up at some point. So for sure, we expect that sudo-rs will have some bugs that original sudo doesn't have. We have even shipped sudo-rs versions that had known bugs, and we will probably continue to do so in the future. Being open about this is normal for open source/free software. 

On the other hand, we have found several bugs in original sudo while we were implementing sudo-rs, and through our compliance testing framework we can clearly see that original sudo also is still actively introducing and fixing bugs.

Most of the bugs we are talking about here as so small that no ordinary user will ever encounter them. Many of the more noticeable bugs have already been discovered by early adopters of sudo-rs, who have been sending in bug reports for over two years.

This is all talking about simple *bugs*. For vulnerabilities, we dare to give a bolder answer. Sudo-rs uses a memory safe-by-design approach with the aim of dramatically lowering the risk of a memory safety bug. For original sudo, this risk is only reduced compared to other C projects because it has been around for a long time. We expect the probability of a memory safety vulnerability to be discovered in sudo-rs to be dramatically small---especially because we know which parts of the code they could be hiding in. And to this day, none have been found.

For non-memory safety related vulnerabilities, we rely on our reduced feature set. Two recent CVE's in original sudo, [CVE-2025-32463](https://www.sudo.ws/security/advisories/chroot_bug/) and [ CVE-2025-32462](https://www.sudo.ws/security/advisories/host_any/) did not affect sudo-rs because of this reason. Secondly, because Rust allows describing the "business logic" in a more humanly readable way than C, it would also have been highly unlikely that we would have been suspectible to [CVE-2023-22809](https://www.sudo.ws/security/advisories/sudoedit_any/).

## Has sudo-rs been audited?

Twice. By [Radically Open Security](https://www.radicallyopensecurity.com). The first audit took place in August of 2023 and uncovered [a path traversal vulnerability](https://github.com/trifectatechfoundation/sudo-rs/security/advisories/GHSA-2r3c-m6v7-9354) that also affected original sudo. A second audit in 2025 found no new vulnerabilities.

Furthermore, an information leak vulnerability was discovered by cybersecurity enthusiast [Sonia Zorba](https://www.zonia3000.net).

## Is there a reason to not switch to sudo-rs?

Certainly. There are features that sudo-rs doesn't support (such as sending mail, storing the sudoers file in LDAP, matching commands using regular expressions). If you cannot upgrade your workflow to not need those features, you may need to stick with sudo. Also, original sudo is a highly portable program and runs a highly diverse set of operating systems. Sudo-rs is only available for Linux and FreeBSD.

There may also be socio-economic reasons. If you are operating in a highly conservative environment, that may also be a reason why you might prefer sudo: it has a lot of history behind it and is widely accepted. If you're the sysadmin that installed sudo on every workstation in your organization, you're unlikely to be blamed if a vulnerability is discovered.

The good news is, original sudo is still being maintained. All sudo-rs is does is give you more freedom to choose which implementation of sudo to use. Freedom is good.

## What is the "test framework" all about?

To ensure compatibility with original sudo, we have created an extensive set of integration tests where we construct a specified machine configuration inside a `docker` container, run a command with original sudo, run the same command with sudo-rs, and check whether the results are equivalent. This ensures that code paths are tested that are hard to test with only unit tests, and it also demonstrates that sudo-rs is a "drop-in replacement" for many cases.

Several surprising original sudo behaviours were discovered while developing this test framework. Most of these turned out to be bugs in original sudo that we reported to Todd Miller and were promptly fixed. This testing framework also acts as an extra set of regression tests for original sudo--we discovered this recently while transitioning from Debian Bookworm to Debian Trixie.

For details you can read the blog by sudo-rs engineer Jorge Aparicio: https://ferrous-systems.com/blog/testing-sudo-rs/

## How is the original sudo developer involved in your project?

Todd Miller is not part of the sudo-rs team, but he has collaborated with us and has frequently served as an advisor. For example, whenever we discovered behaviour
in original sudo and were not sure whether to copy this or not, he would [chime in](https://github.com/trifectatechfoundation/sudo-rs/issues/427#issuecomment-1589619556)
with useful advice.

We have collaborated on vulnerabilities that required mitigations from both of us, for example around our three advisories, as well as 
[CVE-2023-42465](https://www.packetlabs.net/posts/sudo-command-is-vulnerable-to-rowhammer/) and [CVE-2023-2002](https://www.cve.org/CVERecord?id=CVE-2023-2002)). He has also [fixed a bug](https://github.com/trifectatechfoundation/sudo-rs/pull/1017) in the FreeBSD port of sudo-rs.

## How did sudo-rs development affect original sudo?

During the development of our testing framework, we exercised some code paths in original sudo that were rarely used, and (not surprisingly), several bugs were discovered that way; most of which had a no or only a slight security impact. This also furthered the harmonization between sudo-rs and sudo on their common feature set.

Bugs fixed in sudo 1.9.14:

* https://github.com/sudo-project/sudo/commit/471028351650aa4477e59a1701608ffae5b1d4a2
* https://github.com/sudo-project/sudo/commit/8c1559e0e34fa83b061f148b63fc8e091a4b2517
* https://github.com/sudo-project/sudo/commit/64ab8cd23643feced561a1aabcc6be547e81ad58
* https://github.com/sudo-project/sudo/commit/78e65e14ea18278a904beddd54b964609b715762

Bugs fixed in sudo 1.9.15:

* https://github.com/sudo-project/sudo/commit/e7d4c05acea3f15fd8bcc4949acb7e06940284c1 
* https://github.com/sudo-project/sudo/commit/1c7a20d7447937cd2e29b61c9c013f5b1df76fd6
* https://github.com/sudo-project/sudo/commit/d1625f9c8325abe4f5c3706d4ac9442fcccc91ad
* https://github.com/sudo-project/sudo/commit/db704c22ec248c871907cfd966091a28332e1d0f
* https://github.com/sudo-project/sudo/commit/d486db46cf25f09b19aeb9109d58531f3a3d2d33
* https://github.com/sudo-project/sudo/commit/7363ad7b3230b7b03a83f68a0ea33b4144c78a79

Bugs fixed in sudo 1.9.17:

* https://github.com/sudo-project/sudo/commit/0be9f0f947139b32feaa5cd7b5d858069e0af48c
* https://github.com/sudo-project/sudo/commit/4dbb07c19bdeba34c93243adeb0114715afff473
* https://github.com/sudo-project/sudo/commit/b04386f63163d99eb67a78f9af8515b3af13c8b0
* https://github.com/sudo-project/sudo/commit/82ebb1eaa92368952ae92ee0819b573f24e304cd
* https://github.com/sudo-project/sudo/commit/28837b2af1d98c08f0cb75dd075fc290435775a1

Bugs fixed in sudo 1.9.18:

* https://github.com/sudo-project/sudo/commit/12724d1b73d6d7dd3197ceadefdd9e600fcda537
* https://github.com/sudo-project/sudo/commit/e2a2982153a39eb793adfc9f2a8524adfdae8c6f

Time permitting, we would also like to contribute our improvements to the Linux seccomp-based `NOEXEC` mechanism back to the sudo project.

## Do you participate in a bug bounty program?

We do not at the moment---also given the [experiences of other open source projects](https://arstechnica.com/gadgets/2025/05/open-source-project-curl-is-sick-of-users-submitting-ai-slop-vulnerabilities/).

If you discover a vulnerability in sudo-rs, follow the instructions in our [security policy](https://github.com/trifectatechfoundation/sudo-rs/blob/main/SECURITY.md). If we agree that it is a vulnerability we will publicly acknowledge this in our repository---you will get the public credit.

## Can I contribute to sudo-rs?

Yes! In fact, we also believe that the newer code base, written in a safer language, actually lends itself well for being more accepting of outside contributions. Multiple features/bugs in sudo-rs have already been implemented/fixes by external contributors.

We have a [contributors' guide](https://github.com/trifectatechfoundation/sudo-rs/blob/main/CONTRIBUTING.md) which lists some of the things to be mindful of. Happy hacking!
