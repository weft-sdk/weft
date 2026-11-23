# ADR 0023: Dashboard login uses an auth library with GitHub, and the local dashboard has none

- Status: Accepted (2026-11-23)

**Context.** [Q4](../13-open-questions.md) asked which provider handles sign-in for Weft Cloud: WorkOS or Clerk. The [dashboard](../08-dashboard.md) comes in three forms: local (`weft dashboard`), self-hosted, and Cloud. The people who use it are developers, who nearly all have a GitHub account. A hosted provider costs money and ties the product to a vendor, and nothing needs it before Weft Cloud exists.

**Decision.**
- **Local dashboard:** no login. It listens on localhost only, as the [dashboard doc](../08-dashboard.md) already says, with an optional token.
- **Self-hosted and Cloud:** sign-in runs inside the dashboard server through an auth library (Auth.js or Better Auth; the choice is made when the dashboard is built), with **GitHub as the only provider at first**. The user records are stored in the dashboard's own database.
- **Later:** more providers (Google, email) are added to the same library. Company single sign-on (OIDC, SAML), roles and the audit log stay on the [paid tier](0013-open-core.md). If SAML is too much to build, a hosted provider (WorkOS) can be added then, for that feature alone.

**Alternatives considered.**
- *A hosted provider from the start (Clerk, Auth0, WorkOS).* Handles teams and SSO out of the box, but costs money per user from day one and makes a vendor part of the product's core.
- *Email and password only.* More to build and secure, and nobody asks for it.
- *No login for self-hosted.* Fine on a laptop, not on a server anyone can reach.

**Consequences.** Sign-in is free to run, and we own the user data, so we must secure it ourselves. Adding GitHub login takes little work. Weft Cloud's company SSO is a later piece of work. [Q4](../13-open-questions.md) is answered and is removed from the list.
