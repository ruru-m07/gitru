# Gitru desktop privacy

Gitru works with repositories on your computer. Repository content is processed
locally unless you explicitly run a Git network operation such as fetch, pull,
push, or clone.

Optional anonymous usage analytics is disabled by default. You can enable or
disable it at any time with the analytics button at the bottom of the desktop
sidebar. When enabled, Gitru records only that the desktop app opened, periodic
presence while it is visible, and basic runtime metadata such as operating
system and screen size. It does not record repository paths, code, diffs,
remotes, branches, commit data, page URLs, or session recordings.

Avatar images are requested only from GitHub or Bitbucket for identities already
shown in the interface. External links open in the system browser only after an
HTTPS validation check.
