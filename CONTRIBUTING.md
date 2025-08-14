# Contributing to sudo-rs

We welcome contributions to building a memory safe sudo / su implementation; this document lists
some ways in which you can help.

This project is about building a "drop-in replacement" for sudo and su. That does not mean we want
to copy *all* of the behavior of original sudo or other su implementations.

Whenever we add a feature, sudo becomes more complex, and unforeseen interactions due to complexity
can result in security issues. Also, sudo has some features for backwards compatibility only---it makes no
sense for us to re-implement a feature that by its nature won't be very well-used in practice. Other features
have a very specific use-case in mind (for example, matching command lines with [regular expressions](https://xkcd.com/1171/)),
which are very complex to use and would require the inclusion of a third-party library.

I.e. every time we add a feature, we have to weigh its benefits to the cost of adding the feature.
For this, the sudo-rs Core Team has adopted a few criteria for inclusions of features in sudo / su:

1. Security is more important than functionality.
2. Simplicity is preferred over complexity.
3. A feature to be added should actually *solve* a problem.
4. Features must support a common and reasonable use case.
5. Dependencies must be kept to an absolute minimum.

## Feature requests

The easiest way to contribute is to request a feature that we currently do not have; use
the issue tracker for this and explain the situation. Things that are currently possible
with original sudo and that pass the above-mentioned criteria are likely to be accepted.

## Testing sudo

You can install and run sudo on your personal system, or any other non-mission critical
machine. We recommend installing it in `/usr/local/bin` so you have original sudo as a backup.

Although sudo-rs is thoroughly tested for every change we make, a real-world test like this
can uncover subtle problems in technical parts, or uncover common sudo use cases that we
ignored so far.

## Small contributions

You can also go through our code --- if you see any small mistakes or have suggestions
please create an issue for them.  If it is really a minor issue, like a typo or formatting
issue, you can immediately create a pull request.

## Security auditing

One way you can help is by looking at the security of our code and proposing fixes in it.
More eyeballs spot more problems.

If you find a security problem that can be used to used to compromise a system,
do follow our [security policy] and report a vulnerability instead of using the
issue tracker.

[security policy]: https://github.com/trifectatechfoundation/sudo-rs/security/policy

## Working on a bigger issue

If you want to pick up a bigger issue in the issue tracker, please reach out to the
sudo-rs team first. The easiest way to do this is to comment on the issue. If you want
to work on something that is not on the issue tracker, do make an issue *before* you
begin to make sure your work will not be conflicting with ours.

# Expectations for contributors

## Respect free software/open source licenses

Since sudo-rs is licensed very permissively (Apache 2.0 or MIT), for every contribution
you make, you have to ensure that it is either your original work, or a derived work from
software that falls under a free software/open source license that allows its inclusion in
our repository. In the latter case your contribution must have clear attributions so we can
review whether we can include it in our project.

## Make your code easy to review

The sudo-rs team has a limited amount of time to review contributions. You can help us by
structuring your pull request in atomic commits, using the commentary field to explain what
you are doing, and making an effort yourself to pass our CI checks. In short, you are the first
reviewer of your contribution.

## Use of generative artificial intelligence

Contributions clearly showing heavy use of generative AI, so-called "vibe coding", will be
dismissed out of hand.

There's nothing *inherently wrong* with using tool assistance while coding, including tools based
on generative AI. But when using AI to generate whole subroutines (or more) only based on prompts,
it becomes very hard to guarantee the previous two points: first, it is hard to tell whose original
work the contribution is; secondly, you are less able to perform a 'first review' of any code you
didn't write yourself.
